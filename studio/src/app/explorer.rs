//! Explorer actions, file creation and Rust module management.
use super::*;

impl StudioEnvironment {
    pub fn begin_new_entry(&self, directory: bool) {
        if self.new_entry.get().pending.is_some() {
            return;
        }
        let Some(parent) = self.workspace.get().root else {
            self.workspace.update(|state| {
                state.message = "Open a project before creating files or folders".into()
            });
            return;
        };
        self.new_entry.set(EntryDraft {
            parent: Some(parent),
            directory,
            ..Default::default()
        });
        self.new_entry_visible.set(true);
    }
    pub fn open_file_chooser(&self) {
        self.context_visible.set(false);
        self.palette_query.set(String::new());
        self.palette_index.set(0);
        self.palette_visible.set(true);
    }
    pub fn palette_move(&self, forward: bool) {
        let query = self.palette_query.get().to_lowercase();
        let count = crate::workspace::file_kind::StudioFileKind::CREATABLE
            .into_iter()
            .filter(|kind| kind.matches(&query))
            .count();
        if count == 0 {
            return;
        }
        self.palette_index.update(|index| {
            *index = if forward {
                (*index + 1) % count
            } else {
                (*index + count - 1) % count
            }
        });
    }
    pub fn open_quick_open(&self) {
        self.context_visible.set(false);
        self.palette_visible.set(false);
        self.quick_open_query.set(String::new());
        self.quick_open_index.set(0);
        let open_docs: Vec<PathBuf> = self
            .workspace
            .get()
            .documents
            .into_iter()
            .map(|doc| doc.path)
            .collect();
        self.quick_open_files.set(open_docs);
        self.quick_open_visible.set(true);
        self.quick_open_focus.set(true);
    }
    pub async fn load_quick_open_files(&self) {
        let Some(root) = self.workspace.get().root else {
            return;
        };
        if let Ok(files) =
            crate::services::project_tools::background(move || {
                crate::services::project_tools::list_files(root)
            })
            .await
        {
            self.quick_open_files.set(files);
        }
    }
    fn filtered_quick_open_files(&self) -> Vec<PathBuf> {
        let query = self.quick_open_query.get().trim().to_lowercase();
        let root = self.workspace.get().root;
        self.quick_open_files
            .get()
            .into_iter()
            .filter(|path| {
                if query.is_empty() {
                    return true;
                }
                let display = root
                    .as_ref()
                    .and_then(|root| path.strip_prefix(root).ok())
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_lowercase();
                display.contains(&query)
            })
            .take(100)
            .collect()
    }
    pub fn quick_open_move(&self, forward: bool) {
        let count = self.filtered_quick_open_files().len();
        if count == 0 {
            return;
        }
        self.quick_open_index.update(|index| {
            *index = if forward {
                (*index + 1) % count
            } else {
                (*index + count - 1) % count
            };
        });
    }
    pub fn activate_quick_open(&self) {
        if let Some(path) = self
            .filtered_quick_open_files()
            .get(self.quick_open_index.get())
            .cloned()
        {
            self.quick_open_visible.set(false);
            self.quick_open_focus.set(false);
            self.send(Command::OpenFile(path));
        }
    }
    pub fn activate_palette(&self) {
        let query = self.palette_query.get().to_lowercase();
        if let Some(kind) = crate::workspace::file_kind::StudioFileKind::CREATABLE
            .into_iter()
            .filter(|kind| kind.matches(&query))
            .nth(self.palette_index.get())
        {
            self.begin_typed_entry(kind);
        }
    }
    pub fn begin_typed_entry(&self, kind: crate::workspace::file_kind::StudioFileKind) {
        let state = self.workspace.get();
        let parent = self
            .explorer_selection
            .get()
            .or(state.active_file)
            .or(state.root);
        self.context_visible.set(false);
        self.palette_visible.set(false);
        self.new_entry.set(EntryDraft {
            parent,
            kind: Some(kind),
            directory: kind == crate::workspace::file_kind::StudioFileKind::Folder,
            register: kind != crate::workspace::file_kind::StudioFileKind::Folder,
            resolving: true,
            ..Default::default()
        });
        self.new_entry_visible.set(true);
    }
    pub fn entry_dialog_visibility(&self) -> Binding<bool> {
        let read = self.clone();
        let write = self.clone();
        Binding::new(
            move || read.new_entry_visible.get(),
            move |visible| {
                if visible || write.new_entry.get().pending.is_none() {
                    write.new_entry_visible.set(visible);
                }
            },
        )
    }
    pub fn delete_dialog_visibility(&self) -> Binding<bool> {
        let read = self.clone();
        let write = self.clone();
        Binding::new(
            move || read.delete_visible.get(),
            move |visible| {
                if visible || write.delete_pending.get().is_none() {
                    write.delete_visible.set(visible);
                }
            },
        )
    }
    pub fn entry_registration(&self) -> Binding<bool> {
        let read = self.clone();
        let write = self.clone();
        Binding::new(
            move || read.new_entry.get().register,
            move |value| write.new_entry.update(|draft| draft.register = value),
        )
    }
    pub fn entry_module_updates(&self) -> Binding<bool> {
        let read = self.clone();
        let write = self.clone();
        Binding::new(
            move || read.new_entry.get().update_modules,
            move |value| write.new_entry.update(|draft| draft.update_modules = value),
        )
    }
    pub async fn inspect_entry(self) {
        let draft = self.new_entry.get();
        let Some(service) = &self.service else {
            return;
        };
        if let Some(source) = draft.source.clone() {
            let result = service.affected_modules(source.clone(), true).await;
            if self.new_entry.get().source != Some(source) {
                return;
            }
            self.new_entry.update(|draft| {
                draft.resolving = false;
                match result {
                    Ok(items) => {
                        draft.update_modules = !items.is_empty();
                        draft.module_note = items
                            .iter()
                            .map(|item| format!("mod {} in {}", item.name, item.file.display()))
                            .collect::<Vec<_>>()
                            .join("\n");
                    }
                    Err(error) => draft.module_note = format!("Module inspection: {error}"),
                }
            });
        } else if let (Some(kind), Some(parent)) = (draft.kind, draft.parent.clone()) {
            if kind == crate::workspace::file_kind::StudioFileKind::Folder {
                self.new_entry.update(|draft| draft.resolving = false);
                return;
            }
            let result = service.module_targets(parent.clone()).await;
            if self.new_entry.get().parent != Some(parent)
                || self.new_entry.get().kind != Some(kind)
            {
                return;
            }
            self.new_entry.update(|draft| {
                draft.resolving = false;
                match result {
                    Ok(targets) => {
                        draft.target = if targets.len() == 1 { Some(0) } else { None };
                        draft.targets = targets;
                    }
                    Err(error) => draft.registration_error = error,
                }
            });
        }
    }
    pub async fn inspect_delete(self) {
        let Some(row) = self.context.get() else {
            return;
        };
        let Some(service) = &self.service else {
            return;
        };
        let result = service.affected_modules(row.path.clone(), false).await;
        if self
            .context
            .get()
            .is_none_or(|current| current.path != row.path)
        {
            return;
        }
        self.delete_resolving.set(false);
        match result {
            Ok(items) => {
                self.delete_cleanup_available.set(!items.is_empty());
                self.delete_updates.set(!items.is_empty());
                self.delete_module_note.set(
                    items
                        .iter()
                        .map(|item| format!("mod {} in {}", item.name, item.file.display()))
                        .collect::<Vec<_>>()
                        .join("\n"),
                );
            }
            Err(error) => {
                self.delete_cleanup_available.set(false);
                self.delete_updates.set(false);
                self.delete_module_note.set(format!("Automatic module cleanup is unavailable. You can still move this item to Trash; update its module declarations manually.\n{error}"));
            }
        }
    }
    pub fn entry_name(&self) -> Binding<String> {
        let read = self.clone();
        let write = self.clone();
        Binding::new(
            move || read.new_entry.get().name,
            move |name| {
                write.new_entry.update(|draft| {
                    if draft.pending.is_none() {
                        draft.name = name;
                        draft.error.clear();
                    }
                })
            },
        )
    }
    pub fn cancel_new_entry(&self) {
        if self.new_entry.get().pending.is_none() {
            self.new_entry_visible.set(false);
        }
    }
    pub fn submit_new_entry(&self) {
        let draft = self.new_entry.get();
        if draft.pending.is_some() {
            return;
        }
        if draft.resolving {
            self.new_entry
                .update(|draft| draft.error = "Resolving Rust parent modules…".into());
            return;
        }
        if let Some(kind) = draft.kind {
            if let Err(error) = crate::workspace::file_kind::FileNames::new(&draft.name, kind) {
                self.new_entry
                    .update(|draft| draft.error = error.to_string());
                return;
            }
            let registration = if draft.register {
                draft
                    .target
                    .and_then(|index| draft.targets.get(index).cloned())
            } else {
                None
            };
            if (draft.register || kind == crate::workspace::file_kind::StudioFileKind::Module)
                && registration.is_none()
            {
                self.new_entry.update(|draft|draft.error="Choose a valid parent module, or disable registration for a plain source file".into());
                return;
            }
            if let Some(parent) = draft.parent {
                self.send(Command::CreateTyped {
                    parent,
                    kind,
                    name: draft.name,
                    registration,
                });
                let state = self.workspace.get();
                self.new_entry.update(|draft| {
                    if state.message == "Working…" {
                        draft.pending = Some(state.sequence)
                    } else {
                        draft.error = state.message
                    }
                });
            }
            return;
        }
        if let Err(error) = crate::workspace::validate_entry_name(&draft.name) {
            self.new_entry
                .update(|draft| draft.error = error.to_string());
            return;
        }
        let Some(parent) = draft.parent else {
            return;
        };
        if let Some(path) = draft.source {
            self.send(if draft.update_modules {
                Command::RenameModuleEntry {
                    path,
                    name: draft.name,
                }
            } else {
                Command::RenameEntry {
                    path,
                    name: draft.name,
                }
            });
        } else {
            self.send(Command::NewEntry {
                parent,
                name: draft.name,
                directory: draft.directory,
            });
        }
        let state = self.workspace.get();
        self.new_entry.update(|draft| {
            if state.message == "Working…" {
                draft.pending = Some(state.sequence);
            } else {
                draft.error = state.message;
            }
        });
    }
    pub fn show_context(&self, row: TreeRow) {
        self.explorer_selection.set(Some(row.path.clone()));
        self.context.set(Some(row));
        self.context_visible.set(true);
    }
    pub fn context_parent(&self) -> Option<PathBuf> {
        self.context.get().and_then(|row| {
            if row.directory {
                Some(row.path)
            } else {
                row.path.parent().map(std::path::Path::to_path_buf)
            }
        })
    }
    pub fn new_context_entry(&self, directory: bool) {
        let parent = self.context_parent();
        self.context_visible.set(false);
        self.new_entry.set(EntryDraft {
            parent,
            directory,
            ..Default::default()
        });
        self.new_entry_visible.set(true);
    }
    pub fn rename_context(&self) {
        if let Some(row) = self.context.get() {
            self.new_entry.set(EntryDraft {
                source: Some(row.path.clone()),
                resolving: true,
                parent: row.path.parent().map(std::path::Path::to_path_buf),
                name: row.name(),
                directory: row.directory,
                ..Default::default()
            });
            self.context_visible.set(false);
            self.new_entry_visible.set(true);
        }
    }
    pub fn copy_context_entry(&self, cut: bool) {
        if let Some(row) = self.context.get() {
            self.explorer_clipboard.set(Some((row.path, cut)));
        }
        self.context_visible.set(false);
    }
    pub fn paste_context_entry(&self) {
        if let (Some((source, cut)), Some(parent)) =
            (self.explorer_clipboard.get(), self.context_parent())
        {
            self.send(Command::PasteEntry {
                source,
                parent,
                cut,
            });
            if cut && self.workspace.get().message == "Working…" {
                self.paste_pending.set(Some(self.workspace.get().sequence));
            }
        }
        self.context_visible.set(false);
    }
    pub fn begin_delete_context(&self) {
        self.delete_error.set(String::new());
        self.delete_updates.set(false);
        self.delete_cleanup_available.set(false);
        self.delete_resolving.set(true);
        self.delete_module_note.set(String::new());
        self.context_visible.set(false);
        self.delete_visible.set(true);
    }
    pub fn cancel_delete_context(&self) {
        if self.delete_pending.get().is_none() {
            self.delete_visible.set(false);
        }
    }
    pub fn delete_context(&self) {
        if self.delete_resolving.get() {
            self.delete_error
                .set("Resolving Rust module declarations…".into());
            return;
        }
        if self.delete_pending.get().is_some() {
            return;
        }
        if let Some(row) = self.context.get() {
            self.send(self.deletion_command(row.path));
            let state = self.workspace.get();
            if state.message == "Working…" {
                self.delete_pending.set(Some(state.sequence));
            } else {
                self.delete_error.set(state.message);
            }
        }
    }
    fn deletion_command(&self, path: PathBuf) -> Command {
        if self.delete_updates.get() && self.delete_cleanup_available.get() {
            Command::TrashModuleEntry(path)
        } else {
            Command::TrashEntry(path)
        }
    }
    pub fn context_relative_path(&self) -> String {
        self.context.get().map_or(String::new(), |row| {
            self.workspace
                .get()
                .root
                .and_then(|root| {
                    row.path
                        .strip_prefix(root)
                        .ok()
                        .map(|path| path.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| row.path.display().to_string())
        })
    }
    pub fn activate_context(&self) {
        if let Some(row) = self.context.get() {
            self.send(if row.directory {
                Command::ToggleDirectory(row.path)
            } else {
                Command::OpenFile(row.path)
            });
        }
        self.context_visible.set(false);
    }
}

#[cfg(test)]
mod deletion_tests {
    use super::*;
    #[test]
    fn invalid_parent_disables_cleanup_but_keeps_plain_trash_available() {
        let root =
            std::env::temp_dir().join(format!("omen-delete-fallback-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let project = root.join("project");
        std::fs::create_dir_all(project.join("src")).unwrap();
        std::fs::write(
            project.join("Cargo.toml"),
            "[package]\nname=\"fallback\"\nversion=\"0.1.0\"\n",
        )
        .unwrap();
        let unfinished = "mod example;\nfn main() { let text = \"unfinished";
        std::fs::write(project.join("src/main.rs"), unfinished).unwrap();
        let file = project.join("src/example.rs");
        std::fs::write(&file, "// example\n").unwrap();
        let app = StudioEnvironment::at(root.join("profile"));
        app.send(Command::OpenProject(project));
        app.flush();
        app.context.set(Some(TreeRow {
            path: file.clone(),
            depth: 0,
            directory: false,
            expanded: false,
        }));
        app.begin_delete_context();
        futures::executor::block_on(app.clone().inspect_delete());
        assert!(!app.delete_resolving.get());
        assert!(!app.delete_cleanup_available.get());
        assert!(!app.delete_updates.get());
        assert!(app.delete_module_note.get().contains("manually"));
        // A stale or programmatic checkbox value must not force unsafe cleanup.
        app.delete_updates.set(true);
        assert!(
            matches!(app.deletion_command(file.clone()), Command::TrashEntry(path) if path == file)
        );
        assert!(file.exists());
        drop(app);
        std::fs::remove_dir_all(root).unwrap();
    }
}
