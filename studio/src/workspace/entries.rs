//! Explorer mutations. Called only by the workspace actor, never by a render task.
use super::*;
impl Workspace {
    pub(super) fn entry_path(&self, path: &Path) -> io::Result<PathBuf> {
        let candidate = if path.is_absolute() {
            path.to_owned()
        } else {
            self.state
                .root
                .as_ref()
                .ok_or_else(|| io::Error::other("Open a project first"))?
                .join(path)
        };
        if fs::symlink_metadata(&candidate)?.file_type().is_symlink() {
            return Err(io::Error::other(
                "Symbolic links cannot be changed by the explorer",
            ));
        }
        let path = self.project_path(&candidate)?;
        if self.state.root.as_ref() == Some(&path) {
            return Err(io::Error::other(
                "The project root cannot be renamed, moved or deleted",
            ));
        }
        Ok(path)
    }
    pub(super) fn check_saved(&self, path: &Path) -> io::Result<()> {
        if self
            .state
            .documents
            .iter()
            .any(|doc| doc.path.starts_with(path) && (doc.dirty() || doc.conflict))
        {
            return Err(io::Error::other(
                "Save modified documents and resolve conflicts before moving or deleting this item",
            ));
        }
        Ok(())
    }
    pub(super) fn remap_paths(&mut self, old: &Path, new: &Path) {
        let map = |path: &PathBuf| {
            path.strip_prefix(old)
                .map_or_else(|_| path.clone(), |suffix| new.join(suffix))
        };
        if let Some(root) = &self.state.root
            && let Some(canvases) = self.state.canvases.get_mut(root)
        {
            let mut moved = Vec::new();
            canvases.retain(|path, settings| {
                if let Ok(suffix) = root.join(path).strip_prefix(old) {
                    if let Ok(target) = new.join(suffix).strip_prefix(root) {
                        moved.push((target.to_owned(), settings.clone()));
                    }
                    false
                } else {
                    true
                }
            });
            canvases.extend(moved);
        }
        for document in &mut self.state.documents {
            document.path = map(&document.path);
        }
        self.state.active_file = self.state.active_file.as_ref().map(map);
        self.state.expanded = self.state.expanded.iter().map(map).collect();
    }
    pub(super) fn rename_entry(&mut self, path: &Path, name: &str) -> io::Result<()> {
        validate_entry_name(name)?;
        let path = self.entry_path(path)?;
        let target = path
            .parent()
            .ok_or_else(|| io::Error::other("Missing parent folder"))?
            .join(name);
        if target == path {
            return Ok(());
        }
        if fs::symlink_metadata(&target).is_ok() {
            return Err(io::Error::other("An item with that name already exists"));
        }
        fs::rename(&path, &target)?;
        self.remap_paths(&path, &target);
        self.refresh_tree()
    }
    pub(super) fn trash_entry(&mut self, path: &Path) -> io::Result<()> {
        self.trash_with(path, |path| {
            trash::delete(path)
                .map_err(|error| io::Error::other(format!("Cannot move item to Trash: {error}")))
        })
    }
    fn trash_with(
        &mut self,
        path: &Path,
        trash: impl FnOnce(&Path) -> io::Result<()>,
    ) -> io::Result<()> {
        let path = self.entry_path(path)?;
        self.check_saved(&path)?;
        trash(&path)?;
        if let Some(root) = &self.state.root
            && let Ok(relative) = path.strip_prefix(root)
            && let Some(canvases) = self.state.canvases.get_mut(root)
        {
            canvases.retain(|file, _| !file.starts_with(relative));
        }
        self.state
            .documents
            .retain(|doc| !doc.path.starts_with(&path));
        self.state
            .expanded
            .retain(|folder| !folder.starts_with(&path));
        if self
            .state
            .active_file
            .as_ref()
            .is_some_and(|file| file.starts_with(&path))
        {
            self.state.active_file = self.state.documents.last().map(|doc| doc.path.clone());
        }
        self.refresh_tree()
    }
    pub(super) fn paste_entry(
        &mut self,
        source: &Path,
        parent: &Path,
        cut: bool,
    ) -> io::Result<()> {
        let source = self.entry_path(source)?;
        let parent = self.project_path(parent)?;
        if !parent.is_dir() {
            return Err(io::Error::other("Paste destination must be a folder"));
        }
        if source.is_dir() && parent.starts_with(&source) {
            return Err(io::Error::other("A folder cannot be pasted inside itself"));
        }
        if cut {
            self.check_saved(&source)?;
        }
        let name = source
            .file_name()
            .ok_or_else(|| io::Error::other("Missing item name"))?;
        let mut target = parent.join(name);
        if cut && target == source {
            return Ok(());
        }
        if fs::symlink_metadata(&target).is_ok() {
            if cut {
                return Err(io::Error::other(
                    "Destination already exists; nothing was overwritten",
                ));
            }
            let stem = if source.is_dir() {
                name.to_string_lossy().into_owned()
            } else {
                source
                    .file_stem()
                    .unwrap_or(name)
                    .to_string_lossy()
                    .into_owned()
            };
            let extension = if source.is_dir() {
                String::new()
            } else {
                source
                    .extension()
                    .map_or(String::new(), |ext| format!(".{}", ext.to_string_lossy()))
            };
            let mut found = false;
            for index in 1..=1000 {
                let suffix = if index == 1 {
                    " copy".to_owned()
                } else {
                    format!(" copy {index}")
                };
                target = parent.join(format!("{stem}{suffix}{extension}"));
                if fs::symlink_metadata(&target).is_err() {
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(io::Error::other("Cannot find an available copy name"));
            }
        }
        if cut {
            fs::rename(&source, &target)?;
            self.remap_paths(&source, &target);
        } else {
            let mut count = 0;
            preflight(&source, &mut count)?;
            let mut created = false;
            if let Err(error) = copy_new(&source, &target, &mut created) {
                // copy_new creates the destination exclusively; only clean up
                // partial results belonging to this copy operation.
                if created {
                    if target.is_dir() {
                        let _ = fs::remove_dir_all(&target);
                    } else {
                        let _ = fs::remove_file(&target);
                    }
                }
                return Err(error);
            }
        }
        self.state.expanded.insert(parent);
        self.refresh_tree()
    }
}
fn preflight(path: &Path, count: &mut usize) -> io::Result<()> {
    *count += 1;
    if *count > MAX_DIRECTORY_ENTRIES {
        return Err(io::Error::other("Copy exceeds the 20,000-entry limit"));
    }
    let kind = fs::symlink_metadata(path)?.file_type();
    if kind.is_symlink() || !(kind.is_file() || kind.is_dir()) {
        return Err(io::Error::other(
            "Copying links or special filesystem entries is not supported",
        ));
    }
    if kind.is_dir() {
        for entry in fs::read_dir(path)? {
            preflight(&entry?.path(), count)?;
        }
    }
    Ok(())
}
fn copy_new(source: &Path, target: &Path, created: &mut bool) -> io::Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::other("Source changed to a symbolic link"));
    }
    if metadata.is_dir() {
        fs::create_dir(target)?;
        *created = true;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_new(&entry.path(), &target.join(entry.file_name()), &mut false)?;
        }
    } else if metadata.is_file() {
        let mut input = fs::File::open(source)?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)?;
        *created = true;
        io::copy(&mut input, &mut output)?;
    } else {
        return Err(io::Error::other(
            "Source changed to a special filesystem entry",
        ));
    }
    fs::set_permissions(target, metadata.permissions())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trash_checks_unsaved_buffers_and_cleans_tabs_only_after_success() {
        let root = std::env::temp_dir().join(format!("omen-trash-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("project")).unwrap();
        let file = root.join("project/file.rs");
        fs::write(&file, "hello").unwrap();
        let mut workspace = Workspace::new(WorkspaceState::default());
        workspace.open(&root.join("project")).unwrap();
        workspace.open_file(&file).unwrap();
        workspace.state.documents[0].content = "unsaved".into();
        assert!(
            workspace
                .trash_with(&file, |_| panic!("dirty document must be protected"))
                .is_err()
        );
        workspace.state.documents[0].content = workspace.state.documents[0].baseline.clone();
        assert!(
            workspace
                .trash_with(&file, |_| Err(io::Error::other("Trash unavailable")))
                .is_err()
        );
        assert_eq!(workspace.state.documents.len(), 1);
        assert!(file.exists());
        workspace
            .trash_with(&file, |path| fs::rename(path, root.join("mock-trash")))
            .unwrap();
        assert!(!file.exists());
        assert!(workspace.state.documents.is_empty());
        assert!(workspace.state.active_file.is_none());
        fs::remove_dir_all(root).unwrap();
    }
}
