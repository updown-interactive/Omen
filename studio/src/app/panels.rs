//! Activity-bar routing and asynchronous sidebar tools.
use super::*;
use crate::{
    services::project_tools,
    state::{GitChange, SearchMatch, SearchResults, SidebarPane, SourceControlState},
};

// Dropping a pane's task must not leave its refresh button permanently busy.
struct RequestGuard {
    busy: Value<bool>,
    generation: Rc<Cell<u64>>,
    id: u64,
}
impl RequestGuard {
    fn new(busy: Value<bool>, generation: Rc<Cell<u64>>) -> Self {
        let id = generation.get().wrapping_add(1);
        generation.set(id);
        busy.set(true);
        Self {
            busy,
            generation,
            id,
        }
    }
    fn current(&self) -> bool {
        self.generation.get() == self.id
    }
}
impl Drop for RequestGuard {
    fn drop(&mut self) {
        if self.current() {
            self.busy.set(false);
        }
    }
}
impl StudioEnvironment {
    pub(super) fn reset_project_tools(&self) {
        self.search_generation
            .set(self.search_generation.get().wrapping_add(1));
        self.git_generation
            .set(self.git_generation.get().wrapping_add(1));
        self.search_busy.set(false);
        self.source_control_busy.set(false);
        self.search_index.set(0);
        self.search_results.set(SearchResults::default());
        self.source_control.set(SourceControlState::default());
    }
    pub fn trigger_project_search(&self) {
        self.search_nonce.update(|n| *n += 1);
    }
    pub fn search_move(&self, delta: isize) {
        let count = self.search_results.get().matches.len();
        if count == 0 {
            return;
        }
        self.search_index.update(|index| {
            let next = (*index as isize + delta).rem_euclid(count as isize);
            *index = next as usize;
        });
    }
    pub fn activate_search_result(&self) {
        let results = self.search_results.get();
        if let Some(item) = results.matches.get(self.search_index.get()).cloned() {
            self.open_search_result(item);
        }
    }
    pub fn select_sidebar(&self, pane: SidebarPane) {
        self.update_layout(|layout| {
            layout.active_sidebar = pane;
            layout.sidebar_visible = true;
        });
    }
    pub async fn search_project(&self) {
        if self.search_busy.get() {
            return;
        }
        let workspace = self.workspace.get();
        let Some(root) = workspace.root else {
            self.search_results.set(SearchResults {
                message: "Open a project to search its files.".into(),
                ..Default::default()
            });
            return;
        };
        let query = self.search_query.get();
        let sensitive = self.search_case_sensitive.get();
        let documents = workspace
            .documents
            .into_iter()
            .map(|doc| (doc.path, doc.content))
            .collect();
        let request = RequestGuard::new(self.search_busy.clone(), self.search_generation.clone());
        let selected_root = root.clone();
        let selected_query = query.clone();
        let result = project_tools::background(move || {
            project_tools::search(root, query, sensitive, documents)
        })
        .await;
        if !request.current()
            || self.workspace.get().root.as_ref() != Some(&selected_root)
            || self.search_query.get() != selected_query
            || self.search_case_sensitive.get() != sensitive
        {
            return;
        }
        self.search_index.set(0);
        self.search_results
            .set(result.unwrap_or_else(|message| SearchResults {
                root: Some(selected_root),
                query: selected_query,
                case_sensitive: sensitive,
                message,
                ..Default::default()
            }));
    }
    pub fn open_search_result(&self, result: SearchMatch) {
        self.navigation.set(Some((
            result.path.clone(),
            serde_json::json!({"line":result.line,"character":result.character}),
        )));
        self.send(Command::OpenFile(result.path));
    }
    pub fn source_control_task(&self) -> impl std::future::Future<Output = ()> + 'static + use<> {
        let app = self.clone();
        async move {
            app.refresh_source_control().await;
        }
    }
    pub async fn refresh_source_control(&self) {
        if self.source_control_busy.get() {
            return;
        }
        self.run_git(None).await;
    }
    pub async fn stage_change(&self, change: GitChange, stage: bool) {
        if self.source_control_busy.get() {
            return;
        }
        // Rows retain a stable action handle. Resolve fresh rename metadata
        // before changing the index rather than acting on an old row snapshot.
        let current = self
            .source_control
            .get()
            .changes
            .into_iter()
            .find(|current| current.path == change.path);
        if let Some(current) = current {
            self.run_git(Some((current, stage))).await;
        } else {
            self.refresh_source_control().await;
        }
    }
    async fn run_git(&self, action: Option<(GitChange, bool)>) {
        let Some(root) = self.workspace.get().root else {
            self.source_control.set(SourceControlState {
                message: "Open a project to view source control.".into(),
                ..Default::default()
            });
            return;
        };
        let request = RequestGuard::new(
            self.source_control_busy.clone(),
            self.git_generation.clone(),
        );
        let selected = root.clone();
        let result = project_tools::background(move || match action {
            Some((change, stage)) => project_tools::stage(root, change, stage),
            None => project_tools::status(root),
        })
        .await;
        if !request.current() || self.workspace.get().root.as_ref() != Some(&selected) {
            return;
        }
        self.source_control
            .set(result.unwrap_or_else(|message| SourceControlState {
                root: Some(selected),
                message,
                ..Default::default()
            }));
    }
}
