//! UI-independent workspace engine. All production calls run on a dedicated worker.
use crate::{
    services,
    state::{Document, TreeRow, WorkspaceState},
};
use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

mod creation;
mod entries;
mod file_header;
pub mod file_kind;
mod module_mutations;
pub mod modules;

pub const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_DOCUMENTS: usize = 32;
const MAX_DIRECTORY_ENTRIES: usize = 20_000;

#[derive(Clone, Debug)]
pub enum Command {
    OpenProject(PathBuf),
    CloseProject,
    Create {
        name: String,
        parent: PathBuf,
        sdk: PathBuf,
    },
    CreateConfigured {
        name: String,
        parent: PathBuf,
        sdk: PathBuf,
        source: String,
    },
    NewEntry {
        parent: PathBuf,
        name: String,
        directory: bool,
    },
    CreateTyped {
        parent: PathBuf,
        kind: file_kind::StudioFileKind,
        name: String,
        registration: Option<modules::ModuleTarget>,
    },
    RenameModuleEntry {
        path: PathBuf,
        name: String,
    },
    TrashModuleEntry(PathBuf),
    RenameEntry {
        path: PathBuf,
        name: String,
    },
    TrashEntry(PathBuf),
    PasteEntry {
        source: PathBuf,
        parent: PathBuf,
        cut: bool,
    },
    SetCanvas {
        path: PathBuf,
        settings: crate::visual::CanvasSettings,
    },
    ToggleDirectory(PathBuf),
    OpenFile(PathBuf),
    Edit {
        path: PathBuf,
        content: String,
    },
    /// A UI edit based on a particular displayed disk snapshot.
    EditFromSnapshot {
        path: PathBuf,
        content: String,
        disk_revision: u64,
        baseline: String,
    },
    EditSnapshot {
        path: PathBuf,
        content: omen::TextSnapshot,
        disk_revision: u64,
        baseline: omen::TextSnapshot,
    },
    ApplyLanguageEdit {
        edit: serde_json::Value,
        expected: std::collections::HashMap<PathBuf, omen::TextSnapshot>,
    },
    Select(PathBuf),
    Save(PathBuf),
    SaveAll,
    Reload(PathBuf),
    Close(PathBuf),
    Refresh,
}

pub struct Workspace {
    pub state: WorkspaceState,
    account: crate::account::Account,
}
impl Workspace {
    pub fn new(mut state: WorkspaceState) -> Self {
        for document in &mut state.documents {
            if document.content == document.baseline {
                document.baseline = document.content.clone();
            }
        }
        for project in state.canvases.values_mut() {
            for canvas in project.values_mut() {
                canvas.sanitize();
            }
        }
        Self {
            state,
            account: Default::default(),
        }
    }

    pub(super) fn store_canvas(
        &mut self,
        path: &Path,
        mut settings: crate::visual::CanvasSettings,
    ) -> io::Result<()> {
        let root = self
            .state
            .root
            .as_ref()
            .ok_or_else(|| io::Error::other("Open a project first"))?;
        let relative = path.strip_prefix(root).map_err(io::Error::other)?;
        if !self
            .state
            .documents
            .iter()
            .any(|document| document.path == path)
        {
            return Err(io::Error::other("Open the view before changing its canvas"));
        }
        settings.sanitize();
        self.state
            .canvases
            .entry(root.clone())
            .or_default()
            .insert(relative.to_owned(), settings);
        Ok(())
    }

    pub fn apply(&mut self, command: Command) -> io::Result<()> {
        match command {
            Command::OpenProject(path) => self.open(&path)?,
            Command::CloseProject => {
                if self
                    .state
                    .documents
                    .iter()
                    .any(|document| document.dirty() || document.conflict)
                {
                    return Err(io::Error::other(
                        "Save modified documents and resolve conflicts before closing the project",
                    ));
                }
                self.state.root = None;
                self.state.documents.clear();
                self.state.active_file = None;
                self.state.expanded.clear();
                self.state.rows.clear();
            }
            Command::Create { name, parent, sdk } => {
                if self.state.unsaved_count() > 0 {
                    return Err(io::Error::other(
                        "Save all modified documents before creating another project",
                    ));
                }
                let manifest = omen_cli::project::template::default_manifest(&name, None);
                self.create_project(&name, &parent, &sdk, manifest)?;
            }
            Command::CreateConfigured {
                name,
                parent,
                sdk,
                source,
            } => {
                if self.state.unsaved_count() > 0 {
                    return Err(io::Error::other(
                        "Save all modified documents before creating another project",
                    ));
                }
                let manifest = toml::from_str(&source)
                    .map_err(|error| io::Error::other(format!("Invalid Omen.toml: {error}")))?;
                self.create_project(&name, &parent, &sdk, manifest)?;
            }
            Command::NewEntry {
                parent,
                name,
                directory,
            } => {
                validate_entry_name(&name)?;
                let parent = self.project_path(&parent)?;
                if !parent.is_dir() {
                    return Err(io::Error::other("Choose a folder in the project"));
                }
                if !directory && self.state.documents.len() >= MAX_DOCUMENTS {
                    return Err(io::Error::other(
                        "Close a document before creating another file",
                    ));
                }
                let path = parent.join(name);
                let result = if directory {
                    fs::create_dir(&path)
                } else {
                    self.create_plain_file(&path)
                };
                result.map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        if error.kind() == io::ErrorKind::AlreadyExists {
                            format!("{} already exists. Choose another name.", path.display())
                        } else {
                            format!("Cannot create {}: {error}", path.display())
                        },
                    )
                })?;
                self.state.expanded.insert(parent);
                self.refresh_tree()?;
                if !directory {
                    self.open_file(&path)?;
                }
            }
            Command::CreateTyped {
                parent,
                kind,
                name,
                registration,
            } => self.create_typed(&parent, kind, &name, registration)?,
            Command::RenameModuleEntry { path, name } => self.rename_module_entry(&path, &name)?,
            Command::TrashModuleEntry(path) => self.trash_module_entry(&path)?,
            Command::RenameEntry { path, name } => self.rename_entry(&path, &name)?,
            Command::TrashEntry(path) => self.trash_entry(&path)?,
            Command::PasteEntry {
                source,
                parent,
                cut,
            } => self.paste_entry(&source, &parent, cut)?,
            Command::SetCanvas { path, settings } => self.store_canvas(&path, settings)?,
            Command::ToggleDirectory(path) => {
                let path = self.project_path(&path)?;
                if !path.is_dir() {
                    return Err(io::Error::other("Not a directory"));
                }
                let previous = self.state.expanded.clone();
                if !self.state.expanded.remove(&path) {
                    self.state.expanded.insert(path);
                }
                if let Err(error) = self.refresh_tree() {
                    self.state.expanded = previous;
                    return Err(error);
                }
            }
            Command::OpenFile(path) => self.open_file(&path)?,
            Command::Edit { path, content } => {
                if content.len() as u64 > MAX_FILE_BYTES {
                    return Err(io::Error::other("Document exceeds the 16 MiB editor limit"));
                }
                let document = self.document_mut(&path)?;
                document.content = if document.baseline == content {
                    document.baseline.clone()
                } else {
                    content.into()
                };
            }
            Command::EditFromSnapshot {
                path,
                content,
                disk_revision,
                baseline,
            } => {
                if content.len() as u64 > MAX_FILE_BYTES || content.contains('\0') {
                    return Err(io::Error::other(
                        "Document must be UTF-8 text without NUL bytes, at most 16 MiB",
                    ));
                }
                let document = self.document_mut(&path)?;
                document.content = if document.baseline == content {
                    document.baseline.clone()
                } else {
                    content.into()
                };
                if disk_revision != document.disk_revision {
                    // Keep the disk baseline the editor actually displayed. Otherwise
                    // a watcher reload could silently authorize overwriting external edits.
                    document.baseline = baseline.into();
                    document.conflict = true;
                    return Err(io::Error::other(
                        "File changed while editing; your buffer was retained. Review changes before reloading",
                    ));
                }
            }
            Command::EditSnapshot {
                path,
                content,
                disk_revision,
                baseline,
            } => {
                if content.len() as u64 > MAX_FILE_BYTES {
                    return Err(io::Error::other("Document exceeds the editor size limit"));
                }
                let document = self.document_mut(&path)?;
                document.content = content;
                if disk_revision != document.disk_revision {
                    document.baseline = baseline;
                    document.conflict = true;
                }
            }
            Command::ApplyLanguageEdit { edit, expected } => {
                let mut changes = Vec::new();
                if let Some(files) = edit["changes"].as_object() {
                    for (uri, edits) in files {
                        let path = crate::editor::lsp::protocol::path(uri)
                            .ok_or_else(|| io::Error::other("Language edit is not a local file"))?;
                        changes.push((path, edits.clone()));
                    }
                }
                if let Some(files) = edit["documentChanges"].as_array() {
                    for file in files {
                        if file.get("kind").is_some() {
                            return Err(io::Error::other(
                                "File creation/rename actions are not supported yet",
                            ));
                        }
                        let path = crate::editor::lsp::protocol::path(
                            file["textDocument"]["uri"].as_str().unwrap_or_default(),
                        )
                        .ok_or_else(|| io::Error::other("Invalid language edit URI"))?;
                        changes.push((path, file["edits"].clone()));
                    }
                }
                let mut staged = Self::new(self.state.clone());
                for (path, edits) in changes {
                    let path = staged.project_path(&path)?;
                    staged.open_file(&path)?;
                    let document = staged.document_mut(&path)?;
                    if expected
                        .get(&path)
                        .is_some_and(|expected| document.content != *expected)
                    {
                        return Err(io::Error::other(
                            "The document changed while the language action was running; request it again",
                        ));
                    }
                    document.content =
                        crate::editor::lsp::protocol::checked_edits(&document.content, &edits)?;
                }
                self.state = staged.state;
            }
            Command::Select(path) => {
                self.document_mut(&path)?;
                self.state.active_file = Some(path);
            }
            Command::Save(path) => self.save_file(&path)?,
            Command::SaveAll => {
                let paths = self
                    .state
                    .documents
                    .iter()
                    .filter(|document| document.dirty())
                    .map(|document| document.path.clone())
                    .collect::<Vec<_>>();
                for path in paths {
                    self.save_file(&path)?;
                }
            }
            Command::Reload(path) => {
                let disk = self.read_file(&path)?;
                let document = self.document_mut(&path)?;
                document.content = disk.into();
                document.baseline = document.content.clone();
                document.disk_revision = document.disk_revision.wrapping_add(1);
                document.conflict = false;
            }
            Command::Close(path) => {
                if self.document_mut(&path)?.dirty() {
                    return Err(io::Error::other(
                        "Save or explicitly reload this document before closing it",
                    ));
                }
                self.state
                    .documents
                    .retain(|document| document.path != path);
                if self.state.active_file.as_ref() == Some(&path) {
                    self.state.active_file = self
                        .state
                        .documents
                        .last()
                        .map(|document| document.path.clone());
                }
            }
            Command::Refresh => {
                self.poll_external()?;
                self.refresh_tree()?;
            }
        }
        self.state.message = "Ready".into();
        Ok(())
    }

    /// Remove unavailable recent folders and detach an unavailable restored root.
    /// Documents remain intact so unsaved recovery text is never discarded.
    pub fn recover_missing_paths(&mut self) -> bool {
        let before = self.state.recent.len();
        self.state.recent.retain(|path| path.is_dir());
        let missing_root = self.state.root.as_ref().is_some_and(|path| !path.is_dir());
        if missing_root {
            self.state.root = None;
            self.state.expanded.clear();
            self.state.rows.clear();
        }
        missing_root || before != self.state.recent.len()
    }

    pub fn open(&mut self, path: &Path) -> io::Result<()> {
        let requested = path.to_path_buf();
        let path = path.canonicalize().map_err(|error| {
            self.state.recent.retain(|recent| recent != &requested);
            io::Error::new(error.kind(), format!("Cannot open project {}. The folder may have been moved or deleted. Choose an existing project folder. ({error})", requested.display()))
        })?;
        if !path.is_dir() {
            return Err(io::Error::other("Select a project directory"));
        }
        if self.state.root.as_ref() == Some(&path) {
            return self.refresh_tree();
        }
        if self.state.unsaved_count() > 0 {
            return Err(io::Error::other(
                "Save modified documents before switching projects",
            ));
        }
        let mut candidate = Self::new(WorkspaceState {
            root: Some(path.clone()),
            recent: self.state.recent.clone(),
            sequence: self.state.sequence,
            ..Default::default()
        });
        candidate.state.expanded.insert(path.clone());
        candidate.refresh_tree()?;
        candidate.state.recent.retain(|recent| recent != &path);
        candidate.state.recent.insert(0, path);
        candidate.state.recent.truncate(12);
        candidate.state.canvases = std::mem::take(&mut self.state.canvases);
        self.state = candidate.state;
        Ok(())
    }

    fn project_path(&self, path: &Path) -> io::Result<PathBuf> {
        let root = self
            .state
            .root
            .as_ref()
            .ok_or_else(|| io::Error::other("Open a project first"))?;
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            root.join(path)
        }
        .canonicalize()?;
        if !path.starts_with(root) {
            return Err(io::Error::other("Path is outside the open project"));
        }
        Ok(path)
    }

    pub fn read_file(&self, path: &Path) -> io::Result<String> {
        let path = self.project_path(path)?;
        let file = fs::File::open(&path)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::other("Not a regular file"));
        }
        let mut bytes = Vec::new();
        file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(io::Error::other("File exceeds the 16 MiB editor limit"));
        }
        if bytes.contains(&0) {
            return Err(io::Error::other("Binary files cannot be edited"));
        }
        String::from_utf8(bytes).map_err(io::Error::other)
    }

    pub fn open_file(&mut self, path: &Path) -> io::Result<()> {
        let path = self.project_path(path)?;
        if !self
            .state
            .documents
            .iter()
            .any(|document| document.path == path)
        {
            if self.state.documents.len() >= MAX_DOCUMENTS {
                return Err(io::Error::other(
                    "Close a tab before opening more than 32 documents",
                ));
            }
            let content: omen::TextSnapshot = self.read_file(&path)?.into();
            self.state.documents.push(Document {
                path: path.clone(),
                baseline: content.clone(),
                disk_revision: 0,
                content,
                conflict: false,
            });
        }
        self.state.active_file = Some(path);
        Ok(())
    }

    fn document_mut(&mut self, path: &Path) -> io::Result<&mut Document> {
        self.state
            .documents
            .iter_mut()
            .find(|document| document.path == path)
            .ok_or_else(|| io::Error::other("Document is not open"))
    }

    pub fn save_file(&mut self, path: &Path) -> io::Result<()> {
        let disk = self.read_file(path)?;
        let document = self.document_mut(path)?;
        if disk != document.baseline {
            document.conflict = true;
            return Err(io::Error::other(
                "File changed on disk. Copy your edits before reloading; saving will not overwrite external changes",
            ));
        }
        services::atomic_write(path, document.content.to_string().as_bytes())?;
        document.baseline = document.content.clone();
        document.conflict = false;
        Ok(())
    }

    pub fn poll_external(&mut self) -> io::Result<bool> {
        let mut changed = false;
        for index in 0..self.state.documents.len() {
            let disk = self.read_file(&self.state.documents[index].path);
            let document = &mut self.state.documents[index];
            match disk {
                Ok(disk) if disk != document.baseline => {
                    if document.dirty() {
                        if !document.conflict {
                            document.conflict = true;
                            changed = true;
                        }
                    } else {
                        document.content = disk.into();
                        document.baseline = document.content.clone();
                        document.disk_revision = document.disk_revision.wrapping_add(1);
                        document.conflict = false;
                        changed = true;
                    }
                }
                Ok(_) if document.conflict => {
                    document.conflict = false;
                    changed = true;
                }
                Err(_) if !document.conflict => {
                    document.conflict = true;
                    changed = true;
                }
                _ => {}
            }
        }
        Ok(changed)
    }

    pub fn refresh_tree(&mut self) -> io::Result<()> {
        let Some(root) = self.state.root.clone() else {
            self.state.rows.clear();
            return Ok(());
        };
        let mut rows = Vec::new();
        self.read_directory(&root, 0, &mut rows).map_err(|error| io::Error::new(error.kind(), format!("Cannot read project tree at {}. Check that the folder exists and is readable. ({error})", root.display())))?;
        self.state.rows = rows;
        Ok(())
    }
    fn read_directory(&self, path: &Path, depth: usize, rows: &mut Vec<TreeRow>) -> io::Result<()> {
        if depth > 64 {
            return Err(io::Error::other("Directory nesting exceeds 64 levels"));
        }
        let mut entries = Vec::new();
        for (index, entry) in fs::read_dir(path)?.enumerate() {
            if index >= MAX_DIRECTORY_ENTRIES {
                return Err(io::Error::other("Directory exceeds 20,000 entries"));
            }
            let entry = entry?;
            let kind = entry.file_type()?;
            // Hide Finder metadata and skip symlinks, build and VCS internals.
            if kind.is_symlink()
                || ["target", ".git", "node_modules", ".DS_Store"]
                    .iter()
                    .any(|name| entry.file_name() == *name)
            {
                continue;
            }
            entries.push((entry.path(), kind.is_dir()));
            if entries.len() > MAX_DIRECTORY_ENTRIES {
                return Err(io::Error::other("Directory exceeds 20,000 entries"));
            }
        }
        entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        for (path, directory) in entries {
            let expanded = directory && self.state.expanded.contains(&path);
            rows.push(TreeRow {
                path: path.clone(),
                depth,
                directory,
                expanded,
            });
            if rows.len() > MAX_DIRECTORY_ENTRIES {
                return Err(io::Error::other(
                    "Expanded tree exceeds 20,000 rows; collapse a directory",
                ));
            }
            if expanded {
                self.read_directory(&path, depth + 1, rows)?;
            }
        }
        Ok(())
    }
}

mod auto_save;
pub mod service;

/// Accept one portable filename, never an absolute path or relative traversal.
pub fn validate_entry_name(name: &str) -> io::Result<()> {
    if name.trim().is_empty()
        || name == "."
        || name == ".."
        || name.len() > 255
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|ch| ch.is_control() || "/\\<>:\"|?*".contains(ch))
    {
        return Err(io::Error::other(
            "Enter a file or folder name without path separators or reserved characters",
        ));
    }
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ]
    .contains(&stem.as_str())
    {
        return Err(io::Error::other(
            "This name is reserved by Windows. Choose another name.",
        ));
    }
    Ok(())
}
