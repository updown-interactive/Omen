use super::{
    file_kind::{FileNames, StudioFileKind},
    modules::{self, ModuleTarget},
    *,
};
use std::io::Write;
impl Workspace {
    fn file_header(&self, root: &Path) -> file_header::FileHeader {
        file_header::FileHeader::new(&file_header::project_name(root), &self.account.name)
    }

    pub(super) fn create_project(
        &mut self,
        name: &str,
        parent: &Path,
        sdk: &Path,
        manifest: omen_cli::project::manifest::Manifest,
    ) -> io::Result<()> {
        let header = file_header::FileHeader::new(&manifest.app.name, &self.account.name);
        omen_cli::project::template::create_configured_with(
            name,
            parent,
            Some(sdk),
            manifest,
            |path, source| header.add(path, source),
        )
        .map_err(|error| io::Error::other(format!("{error:#}")))?;
        self.open(&parent.join(name))
    }

    pub(super) fn create_plain_file(&self, path: &Path) -> io::Result<()> {
        let root = self
            .state
            .root
            .as_deref()
            .ok_or_else(|| io::Error::other("Open a project first"))?;
        let contents = self.file_header(root).add(path, "");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        if let Err(error) = file
            .write_all(contents.as_bytes())
            .and_then(|()| file.sync_all())
        {
            drop(file);
            let _ = fs::remove_file(path);
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn create_typed(
        &mut self,
        parent: &Path,
        kind: StudioFileKind,
        name: &str,
        registration: Option<ModuleTarget>,
    ) -> io::Result<()> {
        self.create_typed_with(parent, kind, name, registration, services::atomic_write)
    }
    fn create_typed_with(
        &mut self,
        parent: &Path,
        kind: StudioFileKind,
        name: &str,
        registration: Option<ModuleTarget>,
        commit: impl FnOnce(&Path, &[u8]) -> io::Result<()>,
    ) -> io::Result<()> {
        let selected = self.project_path(parent)?;
        let names = FileNames::new(name, kind)?;
        if kind == StudioFileKind::Module && registration.is_none() {
            return Err(io::Error::other(
                "Choose a resolved Rust parent module before creating a module",
            ));
        }
        let directory = if let Some(target) = &registration {
            if !self.module_targets(&selected)?.contains(target) {
                return Err(io::Error::other(
                    "Parent module changed; reopen the creation dialog",
                ));
            }
            target.directory.clone()
        } else if selected.is_file() {
            selected
                .parent()
                .ok_or_else(|| io::Error::other("Missing parent folder"))?
                .to_owned()
        } else {
            selected
        };
        let root = self
            .state
            .root
            .clone()
            .ok_or_else(|| io::Error::other("Open a project first"))?;
        if !directory.starts_with(&root) {
            return Err(io::Error::other("Creation path is outside the workspace"));
        }
        let destination = directory.join(&names.file_name);
        let entry = if kind == StudioFileKind::Module {
            directory.join(&names.module_name)
        } else {
            destination.clone()
        };
        if fs::symlink_metadata(&entry).is_ok()
            || kind == StudioFileKind::Module
                && directory.join(format!("{}.rs", names.module_name)).exists()
            || registration.is_some() && directory.join(&names.module_name).join("mod.rs").exists()
        {
            return Err(io::Error::other(
                "A file, folder or Rust module with that name already exists",
            ));
        }
        if kind != StudioFileKind::Folder && self.state.documents.len() >= MAX_DOCUMENTS {
            return Err(io::Error::other(
                "Close a document before creating another file",
            ));
        }
        let parent_edit = if let Some(target) = &registration {
            self.ensure_parent_saved(&target.file)?;
            let source = self.read_file(&target.file)?;
            let formatted = modules::insertion(&source, target, &names.module_name)?;
            if fs::metadata(&target.file)?.permissions().readonly() {
                return Err(io::Error::other("The parent module is read-only"));
            }
            Some((target.file.clone(), source, formatted))
        } else {
            None
        };
        let mut created = Vec::<PathBuf>::new();
        let result = (|| {
            ensure_directories(&directory, &root, &mut created)?;
            if kind == StudioFileKind::Module || kind == StudioFileKind::Folder {
                fs::create_dir(&entry)?;
                created.push(entry.clone());
            }
            if kind != StudioFileKind::Folder {
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&destination)?;
                created.push(destination.clone());
                let contents = self
                    .file_header(&root)
                    .add(&destination, &names.render(kind));
                file.write_all(contents.as_bytes())?;
                file.sync_all()?;
            }
            if let Some((path, source, formatted)) = &parent_edit {
                if fs::read_to_string(path)? != *source {
                    return Err(io::Error::other(
                        "Parent module changed on disk; creation was rolled back",
                    ));
                }
                commit(path, formatted.as_bytes())?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            let mut failures = Vec::new();
            for path in created.iter().rev() {
                let result = if path.is_dir() {
                    fs::remove_dir(path)
                } else {
                    fs::remove_file(path)
                };
                if let Err(error) = result {
                    failures.push(format!("{}: {error}", path.display()));
                }
            }
            if !failures.is_empty() {
                return Err(io::Error::other(format!(
                    "{error}; rollback failed: {}",
                    failures.join(", ")
                )));
            }
            return Err(error);
        }
        if let Some((path, _, formatted)) = parent_edit {
            self.update_parent_document(&path, &formatted);
        }
        for ancestor in directory
            .ancestors()
            .take_while(|path| path.starts_with(&root))
        {
            self.state.expanded.insert(ancestor.to_owned());
        }
        if kind == StudioFileKind::Module {
            self.state.expanded.insert(entry);
        }
        // The filesystem transaction is now committed. A later tree/read failure
        // must not undo a valid Rust module registration.
        self.refresh_tree()?;
        if kind != StudioFileKind::Folder {
            self.open_file(&destination)?;
        }
        Ok(())
    }
    pub(super) fn ensure_parent_saved(&self, path: &Path) -> io::Result<()> {
        if self
            .state
            .documents
            .iter()
            .any(|doc| doc.path == path && (doc.dirty() || doc.conflict))
        {
            return Err(io::Error::other(
                "Save the parent module and resolve external changes before updating its declarations",
            ));
        }
        Ok(())
    }
    pub(super) fn update_parent_document(&mut self, path: &Path, content: &str) {
        if let Some(doc) = self.state.documents.iter_mut().find(|doc| doc.path == path) {
            doc.content = content.into();
            doc.baseline = doc.content.clone();
            doc.disk_revision = doc.disk_revision.wrapping_add(1);
        }
    }
}
fn ensure_directories(path: &Path, root: &Path, created: &mut Vec<PathBuf>) -> io::Result<()> {
    if path.exists() {
        if fs::symlink_metadata(path)?.file_type().is_symlink()
            || !path.canonicalize()?.starts_with(root)
        {
            return Err(io::Error::other(
                "The module destination must be a real directory inside the project",
            ));
        }
        if !path.is_dir() {
            return Err(io::Error::other("Destination parent is not a directory"));
        }
        return Ok(());
    }
    let parent = path
        .parent()
        .filter(|parent| parent.starts_with(root))
        .ok_or_else(|| io::Error::other("Invalid destination parent"))?;
    ensure_directories(parent, root, created)?;
    fs::create_dir(path)?;
    created.push(path.to_owned());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn creation_rolls_back_file_and_module_directory_when_parent_write_fails() {
        let root =
            std::env::temp_dir().join(format!("omen-module-rollback-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname=\"rollback_test\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
        )
        .unwrap();
        fs::write(root.join("src/lib.rs"), "//! original\n").unwrap();
        let mut workspace = Workspace::new(WorkspaceState::default());
        workspace.open(&root).unwrap();
        let parent = root.join("src").canonicalize().unwrap();
        let target = workspace.module_targets(&parent).unwrap().remove(0);
        for kind in [StudioFileKind::View, StudioFileKind::Module] {
            let result = workspace.create_typed_with(
                &parent,
                kind,
                "Dashboard",
                Some(target.clone()),
                |_, _| Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            );
            assert!(result.is_err());
            assert!(!parent.join("dashboard_view.rs").exists());
            assert!(!parent.join("dashboard").exists());
            assert_eq!(
                fs::read_to_string(parent.join("lib.rs")).unwrap(),
                "//! original\n"
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}
