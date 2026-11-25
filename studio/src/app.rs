//! Shared reactive environment and asynchronous workspace event delivery.
mod editor;
mod inspector;
mod library;
use crate::{
    services,
    settings::{AppearanceMode, StudioSettings},
    state::{EntryDraft, TreeRow, WorkspaceState},
    theme::{LayoutPreferences, StudioColors, ThemePreset},
    workspace::{
        Command,
        service::{Event, WorkspaceService},
    },
};
use futures::{StreamExt, channel::mpsc::UnboundedReceiver};
use omen::{Binding, Value, ViewExt, environment};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    path::PathBuf,
    rc::Rc,
};

pub(crate) mod agent;
mod documents;
mod explorer;
mod panels;
mod preferences;
mod projects;
pub(crate) mod runner;
mod session;
mod visual;

environment! {
    #[derive(Clone)]
    pub StudioEnvironment {
        pub(self) agent_client: services::agent::Client = services::agent::Client::default();
        pub(self) agent_request: Rc<Cell<u64>> = Rc::default();
        pub(self) agent_task: Rc<RefCell<Option<services::agent::Task>>> = Rc::default();
        @Reactive agent_chat: agent::Chat = agent::Chat::default();
        @Reactive agent_prompt: String = String::new();
        pub(self) runner: services::runner::Runner = services::runner::Runner::default();
        pub(self) run_session: Rc<RefCell<Option<runner::ActiveRun>>> = Rc::default();
        pub(self) device_scan: Rc<Cell<u64>> = Rc::default();
        pub(self) run_sequence: Rc<Cell<u64>> = Rc::default();
        @Reactive run_menu: runner::RunMenu = runner::RunMenu::default();
        @Reactive console_visible: bool = false;
        @Reactive console_output: Rc<str> = Rc::from("");
        @Reactive run_status: runner::RunStatus = runner::RunStatus::default();
        @Reactive workspace: WorkspaceState = WorkspaceState::default();
        pub(self) canvas_motion: Value<Option<(PathBuf, crate::visual::CanvasSettings)>> = Value::new(None);
        pub(self) preview_marker: Rc<RefCell<Option<visual::PreviewMarker>>> = Rc::default();
        @Reactive visual_result: Option<crate::visual::PreviewResult> = None;
        @Reactive visual_message: String = String::new();
        pub(self) visual_history: Rc<RefCell<HashMap<PathBuf, visual::History>>> = Rc::default();
        @Reactive inspector: crate::state::InspectorState = crate::state::InspectorState::default();
        @Reactive inspector_focus: bool = false;
        @Reactive inspector_query: String = String::new();
        pub(self) inspector_service: services::inspector::InspectorService = services::inspector::InspectorService::new();
        @Reactive search_query: String = String::new();
        @Reactive search_case_sensitive: bool = false;
        @Reactive search_results: crate::state::SearchResults = crate::state::SearchResults::default();
        @Reactive search_busy: bool = false;
        @Reactive search_index: usize = 0;
        @Reactive search_nonce: u64 = 0;
        @Reactive source_control: crate::state::SourceControlState = crate::state::SourceControlState::default();
        @Reactive source_control_busy: bool = false;
        pub(self) search_generation: Rc<Cell<u64>> = Rc::new(Cell::new(0));
        pub(self) git_generation: Rc<Cell<u64>> = Rc::new(Cell::new(0));
        editor_states: Rc<RefCell<HashMap<PathBuf, Value<omen::EditorState>>>> = Rc::default();
        @Reactive language_status: String = "Rust support idle".into();
        @Reactive library_visible: bool = false;
        @Reactive library_query: String = String::new();
        @Reactive library_index: usize = 0;
        @Reactive library_items: Vec<crate::editor::library::LibraryItem> = Vec::new();
        @Reactive library_message: String = String::new();
        pub(self) library_target: Rc<RefCell<Option<(PathBuf, u64, usize)>>> = Rc::default();
        @Reactive find_visible: bool = false;
        @Reactive find_focus: bool = false;
        @Reactive replace_focus: bool = false;
        @Reactive find_text: String = String::new();
        @Reactive replacement_text: String = String::new();
        @Reactive find_case_sensitive: bool = false;
        @Reactive find_whole_word: bool = false;
        @Reactive find_match_count: usize = 0;
        @Reactive find_match_index: Option<usize> = None;
        @Reactive rename_visible: bool = false;
        @Reactive rename_focus: bool = false;
        @Reactive rename_text: String = String::new();
        @Reactive goto_visible: bool = false;
        @Reactive goto_focus: bool = false;
        @Reactive goto_text: String = String::new();
        @Reactive problems_visible: bool = false;
        @Reactive quick_open_visible: bool = false;
        @Reactive quick_open_focus: bool = false;
        @Reactive quick_open_query: String = String::new();
        @Reactive quick_open_index: usize = 0;
        @Reactive quick_open_files: Vec<PathBuf> = Vec::new();
        @Reactive code_actions: Vec<serde_json::Value> = Vec::new();
        pub(self) language: Option<crate::editor::lsp::LanguageService>;
        pub(self) language_updates: Rc<futures::lock::Mutex<Option<UnboundedReceiver<crate::editor::lsp::Event>>>>;
        @Reactive pub(self) navigation: Option<(PathBuf, serde_json::Value)> = None;
        @Reactive layout: LayoutPreferences = LayoutPreferences::default();
        @Reactive settings: StudioSettings = StudioSettings::default();
        @Reactive account: crate::account::Account = crate::account::Account::default();
        @Reactive ready: bool = false;
        @Reactive editor_visible: bool = false;
        @Reactive open_path: String = String::new();
        @Reactive project_name: String = "my-app".into();
        @Reactive project_parent: String = String::new();
        @Reactive new_entry: EntryDraft = EntryDraft::default();
        @Reactive new_entry_visible: bool = false;
        @Reactive explorer_selection: Option<PathBuf> = None;
        @Reactive context: Option<TreeRow> = None;
        @Reactive context_visible: bool = false;
        @Reactive explorer_clipboard: Option<(PathBuf, bool)> = None;
        @Reactive delete_visible: bool = false;
        @Reactive delete_error: String = String::new();
        @Reactive delete_updates: bool = false;
        @Reactive delete_cleanup_available: bool = false;
        @Reactive delete_resolving: bool = false;
        @Reactive delete_module_note: String = String::new();
        @Reactive palette_visible: bool = false;
        @Reactive palette_query: String = String::new();
        @Reactive palette_index: usize = 0;
        pub(self) delete_pending: Rc<Cell<Option<u64>>> = Rc::new(Cell::new(None));
        pub(self) paste_pending: Rc<Cell<Option<u64>>> = Rc::new(Cell::new(None));
        @Reactive reload_visible: bool = false;
        @Reactive creation_visible: bool = false;
        @Reactive manifest_source: String = String::new();
        @Reactive creation_error: String = String::new();
        pub(self) creating: Rc<Cell<Option<u64>>> = Rc::new(Cell::new(None));
        pub(self) opening: Rc<Cell<Option<u64>>> = Rc::new(Cell::new(None));
        pub(self) service: Option<WorkspaceService>;
        pub(self) updates: Rc<futures::lock::Mutex<Option<UnboundedReceiver<Event>>>>;
        pub(self) sequence: Rc<Cell<u64>> = Rc::new(Cell::new(0));
    }
}

impl StudioEnvironment {
    pub fn launch() -> Self {
        Self::at(services::data_directory())
    }
    pub fn at(directory: PathBuf) -> Self {
        let result = WorkspaceService::start(directory);
        let (service, updates, message) = match result {
            Ok((service, updates)) => (
                Some(service),
                Some(updates),
                "Restoring workspace…".to_string(),
            ),
            Err(error) => (None, None, format!("Workspace service failed: {error}")),
        };
        let (language, language_updates) = match crate::editor::lsp::LanguageService::start() {
            Ok((service, events)) => (Some(service), Some(events)),
            Err(_) => (None, None),
        };
        let app = Self::new(
            language,
            Rc::new(futures::lock::Mutex::new(language_updates)),
            service,
            Rc::new(futures::lock::Mutex::new(updates)),
        );
        app.workspace
            .update(|workspace| workspace.message = message);
        app
    }
    pub fn editor_view(&self) -> omen::AnyView {
        crate::views::Studio {}
            .environment(self.clone())
            .desktop_window(omen::DesktopWindow {
                title: "Omen Studio — Workspace".into(),
                width: 1280.,
                height: 800.,
                min_width: 1000.,
                min_height: 540.,
                toolbar: Some(omen::DesktopToolbar::new(|| crate::views::StudioTopbar {})),
            })
    }
}
