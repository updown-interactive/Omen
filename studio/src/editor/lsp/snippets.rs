//! Cached project view signatures for Omen snippets. Runs on the LSP worker.
use omen::{EditorCompletion, TextSnapshot};
use omen_macro_core::editor::{self, DslComponent};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
#[derive(Default)]
pub(super) struct Catalog {
    root: Option<PathBuf>,
    refreshed: Option<Instant>,
    views: Vec<DslComponent>,
}
impl Catalog {
    pub(super) fn complete(
        &mut self,
        root: &Path,
        text: &TextSnapshot,
        cursor: usize,
    ) -> Vec<EditorCompletion> {
        let source = text.to_string();
        let modifiers = modifier_completions(&source, cursor);
        if !modifiers.is_empty() {
            return modifiers;
        }
        let declarations = editor::declaration_completions(&source, cursor);
        if !declarations.is_empty() {
            return declarations.into_iter().map(completion).collect();
        }
        if !editor::is_completion_context(&source, cursor) {
            return Vec::new();
        }
        editor::completions_with_components(&source, cursor, self.views(root))
            .into_iter()
            .map(completion)
            .collect()
    }

    pub(super) fn views(&mut self, root: &Path) -> &[DslComponent] {
        if self.root.as_deref() != Some(root)
            || self
                .refreshed
                .is_none_or(|time| time.elapsed() >= Duration::from_secs(2))
        {
            self.views = project_components(root);
            self.root = Some(root.to_owned());
            self.refreshed = Some(Instant::now());
        }
        &self.views
    }
}
pub(crate) fn modifier_completions(source: &str, cursor: usize) -> Vec<EditorCompletion> {
    editor::modifier_completions(source, cursor)
        .into_iter()
        .map(completion)
        .collect()
}

fn completion(item: editor::DslCompletion) -> EditorCompletion {
    EditorCompletion {
        label: item.name,
        insert: item.insert,
        range: item.range,
        placeholders: item.placeholders,
        additional_edits: item
            .additional_edits
            .into_iter()
            .map(|edit| omen::EditorCompletionEdit {
                range: edit.range,
                insert: edit.insert,
            })
            .collect(),
    }
}
fn project_components(root: &Path) -> Vec<DslComponent> {
    let mut pending = vec![root.to_owned()];
    // Shared component crates often live beside the app (including Omen's
    // omen-components crate). Read declared local dependencies without running Cargo.
    if let Ok(source) = fs::read_to_string(root.join("Cargo.toml"))
        && let Ok(manifest) = toml::from_str::<toml::Value>(&source)
        && let Some(dependencies) = manifest.get("dependencies").and_then(toml::Value::as_table)
    {
        for dependency in dependencies.values() {
            if let Some(path) = dependency.get("path").and_then(toml::Value::as_str) {
                let directory = root.join(path).join("src");
                if let Ok(directory) = directory.canonicalize() {
                    pending.push(directory);
                }
            }
        }
    }
    let mut visited = std::collections::BTreeSet::new();
    let mut views = Vec::new();
    let mut files = 0;
    let mut bytes = 0;
    let mut entries_seen = 0;
    while let Some(directory) = pending.pop() {
        if !visited.insert(directory.clone()) {
            continue;
        }
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            entries_seen += 1;
            if entries_seen > 20_000 {
                return views;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                if !["target", ".git", "node_modules", "build", "dist", "vendor"]
                    .iter()
                    .any(|name| entry.file_name() == *name)
                {
                    pending.push(entry.path());
                }
            } else if entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "rs")
            {
                files += 1;
                if files > 256 || bytes > 4 * 1024 * 1024 {
                    return views;
                }
                if entry
                    .metadata()
                    .is_ok_and(|metadata| metadata.len() > 512 * 1024)
                {
                    continue;
                }
                if let Ok(source) = fs::read_to_string(entry.path()) {
                    bytes += source.len();
                    views.extend(editor::components(&source));
                }
            }
        }
    }
    views
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovers_project_view_properties_without_indexing_build_outputs() {
        let root =
            std::env::temp_dir().join(format!("omen-snippet-catalog-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("target")).unwrap();
        fs::write(
            root.join("src/card.rs"),
            "view! { pub Profile(title: String, enabled: bool) { Text(title) } }",
        )
        .unwrap();
        fs::write(
            root.join("target/generated.rs"),
            "view! { Generated { Text(\"skip\") } }",
        )
        .unwrap();
        let mut catalog = Catalog::default();
        let source = "view! { Home { Pro";
        let items = catalog.complete(&root, &source.into(), source.len());
        let item = items.iter().find(|item| item.label == "Profile").unwrap();
        assert_eq!(item.insert, "Profile(title: \"value\", enabled: false)");
        assert_eq!(item.placeholders.len(), 3);
        assert!(!catalog.views.iter().any(|view| view.name == "Generated"));
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod dependency_tests {
    use super::*;
    #[test]
    fn shared_local_component_crates_supply_named_property_snippets() {
        let root =
            std::env::temp_dir().join(format!("omen-shared-completions-{}", std::process::id()));
        fs::create_dir_all(root.join("app")).unwrap();
        fs::create_dir_all(root.join("shared/src")).unwrap();
        fs::write(
            root.join("app/Cargo.toml"),
            "[dependencies]\nshared={path=\"../shared\"}\n",
        )
        .unwrap();
        fs::write(
            root.join("shared/src/lib.rs"),
            "view! { pub SharedHeader(title: String) { Text(title) } }",
        )
        .unwrap();
        let mut catalog = Catalog::default();
        let source = "view! { Home { Shared";
        let items = catalog.complete(&root.join("app"), &source.into(), source.len());
        assert_eq!(
            items
                .iter()
                .find(|item| item.label == "SharedHeader")
                .unwrap()
                .insert,
            "SharedHeader(title: \"value\")"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
