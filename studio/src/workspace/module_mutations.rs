//! Declaration-aware explorer mutations with rollback of parent source edits.
use super::*;
use std::{collections::BTreeMap, ops::Range};
impl Workspace {
    fn declaration_edits(
        &self,
        path: &Path,
        new_name: Option<&str>,
    ) -> io::Result<Vec<(PathBuf, String, String)>> {
        let declarations = if new_name.is_some() {
            self.affected_rename_declarations(path)?
        } else {
            self.affected_declarations(path)?
        };
        let mut grouped = BTreeMap::<PathBuf, Vec<(Range<usize>, String)>>::new();
        for declaration in declarations {
            grouped
                .entry(declaration.file)
                .or_default()
                .push(if let Some(name) = new_name {
                    (declaration.name_range, name.into())
                } else {
                    (declaration.range, String::new())
                });
        }
        let mut edits = Vec::new();
        for (file, mut ranges) in grouped {
            self.ensure_parent_saved(&file)?;
            if fs::metadata(&file)?.permissions().readonly() {
                return Err(io::Error::other("Parent module is read-only"));
            }
            let source = self.read_file(&file)?;
            let mut updated = source.clone();
            ranges.sort_by_key(|(range, _)| range.start);
            ranges.dedup_by(|a, b| a.0 == b.0);
            for (range, value) in ranges.into_iter().rev() {
                updated.replace_range(range, &value);
            }
            syn::parse_file(&updated).map_err(io::Error::other)?;
            edits.push((file, source, updated));
        }
        Ok(edits)
    }
    fn commit_declarations(&self, edits: &[(PathBuf, String, String)]) -> io::Result<()> {
        let mut completed = Vec::new();
        for (path, original, updated) in edits {
            let result = (|| {
                if fs::read_to_string(path)? != *original {
                    return Err(io::Error::other("Parent module changed externally"));
                }
                services::atomic_write(path, updated.as_bytes())
            })();
            if let Err(error) = result {
                let mut rollback_errors = Vec::new();
                for (path, content) in completed.into_iter().rev() {
                    if let Err(rollback) = services::atomic_write(path, content) {
                        rollback_errors.push(rollback.to_string());
                    }
                }
                return Err(io::Error::other(format!(
                    "{error}{}",
                    if rollback_errors.is_empty() {
                        String::new()
                    } else {
                        format!("; rollback failed: {}", rollback_errors.join(", "))
                    }
                )));
            }
            completed.push((path, original.as_bytes()));
        }
        Ok(())
    }
    pub(super) fn rename_module_entry(&mut self, path: &Path, name: &str) -> io::Result<()> {
        validate_entry_name(name)?;
        let path = self.entry_path(path)?;
        if path.is_dir()
            && let Some(owner) = self.modern_owner(&path)?
        {
            return self.rename_module_entry(&owner, &format!("{name}.rs"));
        }

        if path.file_name().is_some_and(|name| name == "mod.rs") {
            return Err(io::Error::other(
                "Rename the module directory instead of mod.rs",
            ));
        }
        let name_path = Path::new(name);
        let module_name = if path.is_dir() {
            name
        } else {
            if name_path.extension().is_none_or(|ext| ext != "rs") {
                return Err(io::Error::other(
                    "A registered Rust module must keep its .rs extension",
                ));
            }
            name_path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| io::Error::other("Invalid module filename"))?
        };
        syn::parse_str::<syn::Ident>(module_name)
            .map_err(|_| io::Error::other("The new module name must be a valid Rust identifier"))?;
        let edits = self.declaration_edits(&path, Some(module_name))?;
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("Missing parent"))?;
        let target = parent.join(name);
        if target == path {
            return Ok(());
        }
        if fs::symlink_metadata(&target).is_ok() {
            return Err(io::Error::other("Destination already exists"));
        }
        let companion = if path.is_file() && !edits.is_empty() {
            path.file_stem()
                .map(|stem| parent.join(stem))
                .filter(|directory| directory.is_dir())
        } else {
            None
        };
        let next_companion = parent.join(module_name);
        if let Some(companion) = &companion {
            self.entry_path(companion)?;
            if fs::symlink_metadata(&next_companion).is_ok() {
                return Err(io::Error::other(
                    "The new module's submodule directory already exists",
                ));
            }
        }
        fs::rename(&path, &target)?;
        if let Some(companion) = &companion
            && let Err(error) = fs::rename(companion, &next_companion)
        {
            if let Err(rollback) = fs::rename(&target, &path) {
                return Err(io::Error::other(format!(
                    "{error}; rename rollback failed: {rollback}"
                )));
            }
            return Err(error);
        }
        if let Err(error) = self.commit_declarations(&edits) {
            let mut errors = Vec::new();
            if let Some(companion) = &companion
                && let Err(error) = fs::rename(&next_companion, companion)
            {
                errors.push(error.to_string());
            }
            if let Err(error) = fs::rename(&target, &path) {
                errors.push(error.to_string());
            }
            return Err(io::Error::other(format!(
                "{error}{}",
                if errors.is_empty() {
                    String::new()
                } else {
                    format!("; rollback failed: {}", errors.join(", "))
                }
            )));
        }
        self.remap_paths(&path, &target);
        if let Some(companion) = companion {
            self.remap_paths(&companion, &next_companion);
        }
        for (file, _, content) in edits {
            self.update_parent_document(&file, &content);
        }
        self.refresh_tree()
    }
    pub(super) fn trash_module_entry(&mut self, path: &Path) -> io::Result<()> {
        self.trash_module_with(path, |workspace, path| workspace.trash_entry(path))
    }
    fn trash_module_with(
        &mut self,
        path: &Path,
        trash: impl FnOnce(&mut Workspace, &Path) -> io::Result<()>,
    ) -> io::Result<()> {
        let path = self.entry_path(path)?;
        self.check_saved(&path)?;
        let edits = self.declaration_edits(&path, None)?;
        self.commit_declarations(&edits)?;
        if let Err(error) = trash(self, &path) {
            if !path.exists() {
                for (file, _, content) in &edits {
                    self.update_parent_document(file, content);
                }
                return Err(error);
            }
            let inverse: Vec<_> = edits
                .iter()
                .map(|(path, original, updated)| (path.clone(), updated.clone(), original.clone()))
                .collect();
            self.commit_declarations(&inverse).map_err(|rollback| {
                io::Error::other(format!("{error}; declaration rollback failed: {rollback}"))
            })?;
            return Err(error);
        }
        for (file, _, content) in edits {
            self.update_parent_document(&file, &content);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deleting_a_module_ignores_unfinished_root_and_target_source() {
        let root =
            std::env::temp_dir().join(format!("omen-delete-recovery-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src/views")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname=\"recovery\"\nversion=\"0.1.0\"\n",
        )
        .unwrap();
        let unfinished = "mod views;\nfn main() { let text = \"unfinished";
        fs::write(root.join("src/main.rs"), unfinished).unwrap();
        fs::write(root.join("src/views/mod.rs"), "// keep\npub mod new_env;\n").unwrap();
        let target = root.join("src/views/new_env.rs");
        fs::write(&target, "fn unfinished(").unwrap();
        let mut workspace = Workspace::new(WorkspaceState::default());
        workspace.open(&root).unwrap();
        workspace
            .trash_module_with(&target, |_, path| {
                fs::rename(path, root.join("mock-trash.rs"))
            })
            .unwrap();
        assert!(!target.exists());
        assert_eq!(
            fs::read_to_string(root.join("src/views/mod.rs")).unwrap(),
            "// keep\n\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("src/main.rs")).unwrap(),
            unfinished
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failed_module_deletion_restores_parent_declaration() {
        let root =
            std::env::temp_dir().join(format!("omen-declaration-rollback-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname=\"deletion_rollback\"\nversion=\"0.1.0\"\n",
        )
        .unwrap();
        fs::write(root.join("src/lib.rs"), "// keep\npub mod settings;\n").unwrap();
        fs::write(root.join("src/settings.rs"), "// settings\n").unwrap();
        let mut workspace = Workspace::new(WorkspaceState::default());
        workspace.open(&root).unwrap();
        workspace.open_file(&root.join("src/lib.rs")).unwrap();
        let result = workspace.trash_module_with(&root.join("src/settings.rs"), |_, _| {
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        });
        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(root.join("src/lib.rs")).unwrap(),
            "// keep\npub mod settings;\n"
        );
        assert!(root.join("src/settings.rs").exists());
        assert!(!workspace.state.documents[0].dirty());
        fs::remove_dir_all(root).unwrap();
    }
}
