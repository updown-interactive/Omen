//! Project/document state; layout preferences live in the theme module.
mod inspector;
pub use inspector::InspectorState;
mod panels;
pub use panels::{GitChange, SearchMatch, SearchResults, SidebarPane, SourceControlState};
mod ui;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::PathBuf};
pub use ui::{StudioMenu, ToolbarState};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(from = "StoredDocument")]
pub struct Document {
    pub path: PathBuf,
    pub content: omen::TextSnapshot,
    pub baseline: omen::TextSnapshot,
    /// Changes only when disk contents are reloaded, not on a normal save.
    #[serde(default)]
    pub disk_revision: u64,
    #[serde(default)]
    pub conflict: bool,
}
#[derive(Deserialize)]
struct StoredDocument {
    path: PathBuf,
    content: omen::TextSnapshot,
    baseline: omen::TextSnapshot,
    #[serde(default)]
    disk_revision: u64,
    #[serde(default)]
    conflict: bool,
}
impl From<StoredDocument> for Document {
    fn from(saved: StoredDocument) -> Self {
        let baseline = if saved.content == saved.baseline {
            saved.content.clone()
        } else {
            saved.baseline
        };
        Self {
            path: saved.path,
            content: saved.content,
            baseline,
            disk_revision: saved.disk_revision,
            conflict: saved.conflict,
        }
    }
}
impl Document {
    pub fn dirty(&self) -> bool {
        !self.content.same_snapshot(&self.baseline)
    }
    pub fn tab_title(&self) -> String {
        format!(
            "{}{}{}",
            self.path.file_name().unwrap_or_default().to_string_lossy(),
            if self.dirty() { " •" } else { "" },
            if self.conflict { " !" } else { "" }
        )
    }
}

#[derive(Clone, Debug)]
pub struct TreeRow {
    pub path: PathBuf,
    pub depth: usize,
    pub directory: bool,
    pub expanded: bool,
}
impl TreeRow {
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }
    pub fn icon(&self) -> &'static str {
        if self.directory {
            return if self.expanded {
                "folder-open"
            } else {
                "folder"
            };
        }
        let kind = crate::workspace::file_kind::StudioFileKind::classify(&self.path, false);
        if kind != crate::workspace::file_kind::StudioFileKind::Other {
            return kind.icon();
        }
        match self
            .path
            .extension()
            .and_then(|extension| extension.to_str())
        {
            Some("rs") => "file-rust",
            Some("toml" | "json" | "yaml" | "yml" | "lock") => "file-config",
            Some("png" | "jpg" | "jpeg" | "svg" | "webp" | "gif") => "file-image",
            Some("ts" | "tsx" | "js" | "jsx" | "dart" | "go" | "py" | "sh" | "html" | "css") => {
                "file-code"
            }
            _ => "file",
        }
    }
}

omen::state! {
    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(default)]
    pub WorkspaceState {
        pub root: Option<PathBuf> = Default::default();
        pub documents: Vec<Document> = Default::default();
        pub active_file: Option<PathBuf> = Default::default();
        pub expanded: BTreeSet<PathBuf> = Default::default();
        pub recent: Vec<PathBuf> = Default::default();
        pub canvases: crate::visual::ProjectCanvases = Default::default();
        #[serde(skip)]
        pub rows: Vec<TreeRow> = Default::default();
        #[serde(skip)]
        pub message: String = Default::default();
        #[serde(skip)]
        pub sequence: u64 = Default::default();
    }
}

impl WorkspaceState {
    pub fn active_document(&self) -> Option<&Document> {
        self.documents
            .iter()
            .find(|document| Some(&document.path) == self.active_file.as_ref())
    }
    pub fn canvas(&self, path: &std::path::Path) -> Option<&crate::visual::CanvasSettings> {
        let root = self.root.as_ref()?;
        self.canvases.get(root)?.get(path.strip_prefix(root).ok()?)
    }
    pub fn unsaved_count(&self) -> usize {
        self.documents
            .iter()
            .filter(|document| document.dirty())
            .count()
    }
}

omen::state! {
    /// Transient explorer creation form, kept outside persisted project state.
    #[derive(Clone, Debug)]
    pub EntryDraft {
        pub kind: Option<crate::workspace::file_kind::StudioFileKind> = Default::default();
        pub register: bool = Default::default();
        pub targets: Vec<crate::workspace::modules::ModuleTarget> = Default::default();
        pub target: Option<usize> = Default::default();
        pub resolving: bool = Default::default();
        pub registration_error: String = Default::default();
        pub update_modules: bool = Default::default();
        pub module_note: String = Default::default();
        pub source: Option<PathBuf> = Default::default();
        pub parent: Option<PathBuf> = Default::default();
        pub directory: bool = Default::default();
        pub name: String = Default::default();
        pub error: String = Default::default();
        pub pending: Option<u64> = Default::default();
    }
}
