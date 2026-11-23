//! Background actor for filesystem access, external-change polling and session backups.
use super::{Command, Workspace, auto_save::AutoSave};
use crate::{
    account::Account, services, settings::StudioSettings, state::WorkspaceState,
    theme::LayoutPreferences,
};
use futures::channel::{
    mpsc::{UnboundedReceiver, UnboundedSender, unbounded},
    oneshot,
};
use std::{
    path::PathBuf,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

pub enum Event {
    Loaded(LayoutPreferences, StudioSettings, Account, WorkspaceState),
    AccountSaved(Account),
    Changed(WorkspaceState),
}
enum Request {
    Command(u64, Command),
    StoreCanvas(PathBuf, crate::visual::CanvasSettings),
    Layout(LayoutPreferences),
    Settings(StudioSettings),
    Account(Account),
    EditorBlur(PathBuf),
    WindowBlur,
    ModuleTargets(
        PathBuf,
        oneshot::Sender<Result<Vec<super::modules::ModuleTarget>, String>>,
    ),
    AffectedModules(
        PathBuf,
        bool,
        oneshot::Sender<Result<Vec<super::modules::Declaration>, String>>,
    ),
    AnalyzeView(
        omen::TextSnapshot,
        oneshot::Sender<Result<Vec<crate::visual::PreviewDeclaration>, String>>,
    ),
    ReadFile(PathBuf, oneshot::Sender<Result<String, String>>),
    SaveForRun(PathBuf, mpsc::SyncSender<Result<(), String>>),
    Flush(mpsc::SyncSender<Result<(), String>>),
}

struct Worker {
    sender: Option<mpsc::Sender<Request>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        // Drain queued requests and commit the final snapshot before a new service
        // can restore this profile. No worker is owned by its own request thread.
        drop(self.sender.take());
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            eprintln!("Studio workspace worker terminated unexpectedly");
        }
    }
}
#[derive(Clone)]
pub struct WorkspaceService {
    worker: Arc<Worker>,
}
impl WorkspaceService {
    pub fn start(directory: PathBuf) -> std::io::Result<(Self, UnboundedReceiver<Event>)> {
        let (sender, receiver) = mpsc::channel();
        let (events, updates) = unbounded();
        let thread = std::thread::Builder::new()
            .name("omen-studio-workspace".into())
            .spawn(move || run(directory, receiver, events))?;
        Ok((
            Self {
                worker: Arc::new(Worker {
                    sender: Some(sender),
                    thread: Some(thread),
                }),
            },
            updates,
        ))
    }
    fn enqueue(&self, request: Request) -> Result<(), String> {
        self.worker
            .sender
            .as_ref()
            .ok_or_else(|| "Workspace worker stopped".to_string())?
            .send(request)
            .map_err(|_| "Workspace worker stopped".into())
    }
    pub fn send(&self, sequence: u64, command: Command) -> Result<(), String> {
        self.enqueue(Request::Command(sequence, command))
    }
    pub fn store_canvas(
        &self,
        path: PathBuf,
        settings: crate::visual::CanvasSettings,
    ) -> Result<(), String> {
        self.enqueue(Request::StoreCanvas(path, settings))
    }
    /// Read bounded UTF-8 text on the filesystem worker, independently of any view.
    pub async fn read_file(&self, path: impl Into<PathBuf>) -> Result<String, String> {
        let (reply, answer) = oneshot::channel();
        self.enqueue(Request::ReadFile(path.into(), reply))?;
        answer
            .await
            .map_err(|_| "Workspace worker stopped".to_string())?
    }
    /// Analyze a file containing a single Design declaration.
    pub async fn analyze_view(
        &self,
        text: omen::TextSnapshot,
    ) -> Result<crate::visual::VisualTree, String> {
        let mut declarations = self.analyze_views(text).await?;
        if declarations.len() > 1 {
            return Err("Select a declaration in files containing multiple views.".into());
        }
        let declaration = declarations
            .pop()
            .ok_or("Design requires a top-level named view! declaration.")?;
        declaration.tree.map(Arc::unwrap_or_clone)
    }

    pub(crate) async fn analyze_views(
        &self,
        text: omen::TextSnapshot,
    ) -> Result<Vec<crate::visual::PreviewDeclaration>, String> {
        let (reply, answer) = oneshot::channel();
        self.enqueue(Request::AnalyzeView(text, reply))?;
        answer
            .await
            .map_err(|_| "Workspace worker stopped".to_string())?
    }
    pub async fn module_targets(
        &self,
        path: PathBuf,
    ) -> Result<Vec<super::modules::ModuleTarget>, String> {
        let (reply, answer) = oneshot::channel();
        self.enqueue(Request::ModuleTargets(path, reply))?;
        answer
            .await
            .map_err(|_| "Workspace worker stopped".to_owned())?
    }
    pub async fn affected_modules(
        &self,
        path: PathBuf,
        rename: bool,
    ) -> Result<Vec<super::modules::Declaration>, String> {
        let (reply, answer) = oneshot::channel();
        self.enqueue(Request::AffectedModules(path, rename, reply))?;
        answer
            .await
            .map_err(|_| "Workspace worker stopped".to_owned())?
    }
    pub fn layout(&self, layout: LayoutPreferences) -> Result<(), String> {
        self.enqueue(Request::Layout(layout))
    }
    pub fn settings(&self, settings: StudioSettings) -> Result<(), String> {
        self.enqueue(Request::Settings(settings))
    }
    pub fn account(&self, account: Account) -> Result<(), String> {
        self.enqueue(Request::Account(account))
    }
    pub fn editor_blur(&self, path: PathBuf) -> Result<(), String> {
        self.enqueue(Request::EditorBlur(path))
    }
    pub fn window_blur(&self) -> Result<(), String> {
        self.enqueue(Request::WindowBlur)
    }
    pub(crate) fn save_for_run(&self, root: PathBuf) -> Result<(), String> {
        let (reply, answer) = mpsc::sync_channel(1);
        self.enqueue(Request::SaveForRun(root, reply))?;
        answer
            .recv()
            .map_err(|_| "Workspace worker stopped".to_string())?
    }

    /// Flush queued commands and backups on window teardown, never during rendering.
    pub fn flush(&self) -> Result<(), String> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.enqueue(Request::Flush(sender))?;
        receiver
            .recv_timeout(Duration::from_secs(10))
            .map_err(|error| error.to_string())?
    }
}

fn run(directory: PathBuf, receiver: mpsc::Receiver<Request>, events: UnboundedSender<Event>) {
    let session = directory.join("workspace.json");
    let preferences = directory.join("layout.json");
    let mut errors = Vec::new();
    let mut layout =
        services::read_json::<LayoutPreferences>(&preferences).unwrap_or_else(|error| {
            errors.push(format!("Layout recovery: {error}"));
            LayoutPreferences::default()
        });
    layout.sanitize();
    let settings = StudioSettings::load(&directory).unwrap_or_else(|error| {
        errors.push(format!(
            "Settings recovery: {error}. Using default appearance; settings.json was preserved."
        ));
        StudioSettings::default()
    });
    let account = Account::load(&directory).unwrap_or_else(|error| {
        errors.push(format!(
            "Account recovery: {error}. account.json was preserved."
        ));
        Account::default()
    });
    let state = services::read_json::<WorkspaceState>(&session).unwrap_or_else(|error| {
        errors.push(format!("Workspace recovery: {error}"));
        WorkspaceState::default()
    });
    let mut workspace = Workspace::new(state);
    workspace.account = account.clone();
    let recovered_paths = workspace.recover_missing_paths();
    if let Err(error) = workspace.refresh_tree() {
        errors.push(error.to_string());
    }
    let _ = workspace.poll_external();
    workspace.state.message = if errors.is_empty() {
        "Ready".into()
    } else {
        errors.join("; ")
    };
    let mut auto_save = AutoSave::new(&settings.editor);
    auto_save.observe(&workspace);
    let _ = events.unbounded_send(Event::Loaded(
        layout.clone(),
        settings,
        account,
        workspace.state.clone(),
    ));
    let mut pending = recovered_paths;
    let mut changed_at = Instant::now();
    let mut saved_at = Instant::now();
    let mut checked_at = Instant::now();
    let mut preview = None;
    let mut next_request = None;
    loop {
        let mut saved_on_blur = false;
        match next_request
            .take()
            .map_or_else(|| receiver.recv_timeout(Duration::from_millis(50)), Ok)
        {
            Ok(Request::Command(sequence, command)) => {
                let previous = workspace.state.active_file.clone();
                workspace.state.sequence = sequence;
                if let Err(error) = workspace.apply(command) {
                    workspace.state.message = error.to_string();
                }
                if let Some(previous) = previous
                    && workspace.state.active_file.as_deref() != Some(previous.as_path())
                {
                    auto_save.focus_lost(&mut workspace, &previous);
                }
                auto_save.observe(&workspace);
                pending = true;
                changed_at = Instant::now();
                let _ = events.unbounded_send(Event::Changed(workspace.state.clone()));
            }
            Ok(Request::StoreCanvas(path, settings)) => {
                if workspace.store_canvas(&path, settings).is_ok() {
                    pending = true;
                    changed_at = Instant::now();
                }
            }
            Ok(Request::AnalyzeView(text, reply)) => {
                preview = Some((Instant::now(), text, reply));
            }
            Ok(Request::ModuleTargets(path, reply)) => {
                let _ = reply.send(
                    workspace
                        .module_targets(&path)
                        .map_err(|error| error.to_string()),
                );
            }
            Ok(Request::AffectedModules(path, rename, reply)) => {
                let _ = reply.send(
                    (if rename {
                        workspace.affected_rename_declarations(&path)
                    } else {
                        workspace.affected_declarations(&path)
                    })
                    .map_err(|error| error.to_string()),
                );
            }
            Ok(Request::ReadFile(path, reply)) => {
                let _ = reply.send(
                    workspace
                        .read_file(&path)
                        .map_err(|error| error.to_string()),
                );
            }
            Ok(Request::EditorBlur(path)) => {
                saved_on_blur = auto_save.focus_lost(&mut workspace, &path);
            }
            Ok(Request::WindowBlur) => {
                saved_on_blur = auto_save.window_lost(&mut workspace);
            }
            Ok(Request::Account(account)) => match account.save(&directory) {
                Ok(()) => {
                    workspace.account = account.clone();
                    let _ = events.unbounded_send(Event::AccountSaved(account));
                    if workspace.state.message.contains("Account recovery:")
                        || workspace
                            .state
                            .message
                            .starts_with("Could not save account.json:")
                    {
                        workspace.state.message = "Ready".into();
                        let _ = events.unbounded_send(Event::Changed(workspace.state.clone()));
                    }
                }
                Err(error) => {
                    workspace.state.message = format!("Could not save account.json: {error}");
                    let _ = events.unbounded_send(Event::Changed(workspace.state.clone()));
                }
            },
            Ok(Request::Settings(mut next)) => {
                next.sanitize();
                auto_save.configure(&next.editor, &workspace);
                if let Err(error) = next.save(&directory) {
                    workspace.state.message = format!("Could not save settings.json: {error}");
                    let _ = events.unbounded_send(Event::Changed(workspace.state.clone()));
                } else if workspace.state.message.starts_with("Settings recovery:")
                    || workspace
                        .state
                        .message
                        .starts_with("Could not save settings.json:")
                {
                    workspace.state.message = "Ready".into();
                    let _ = events.unbounded_send(Event::Changed(workspace.state.clone()));
                }
            }
            Ok(Request::Layout(next)) => {
                layout = next;
                layout.sanitize();
                pending = true;
                changed_at = Instant::now();
            }
            Ok(Request::SaveForRun(root, reply)) => {
                let result = if workspace.state.root.as_ref() == Some(&root) {
                    workspace
                        .apply(Command::SaveAll)
                        .map_err(|error| error.to_string())
                } else {
                    Err("The active project changed.".into())
                };
                auto_save.observe(&workspace);
                pending = true;
                changed_at = Instant::now();
                let _ = events.unbounded_send(Event::Changed(workspace.state.clone()));
                let _ = reply.send(result);
            }
            Ok(Request::Flush(reply)) => {
                let result = persist(&session, &preferences, &workspace.state, &layout);
                if result.is_ok() {
                    pending = false;
                }
                let _ = reply.send(result);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = persist(&session, &preferences, &workspace.state, &layout);
                return;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        // Apply queued edits before considering an idle deadline.
        next_request = receiver.try_recv().ok();
        if (next_request.is_none() && auto_save.save_due(&mut workspace)) || saved_on_blur {
            pending = true;
            changed_at = Instant::now();
            let _ = events.unbounded_send(Event::Changed(workspace.state.clone()));
        }
        if preview
            .as_ref()
            .is_some_and(|(requested, _, _)| requested.elapsed() >= Duration::from_millis(100))
            && let Some((_, text, reply)) = preview.take()
            && !reply.is_canceled()
        {
            let result = if text.len() > 512 * 1024 {
                Err("This file exceeds the Design preview size limit.".into())
            } else {
                crate::visual::VisualTree::for_preview(
                    &text.to_string(),
                    workspace.state.root.as_deref(),
                )
            };
            let _ = reply.send(result);
        }
        if pending
            && (changed_at.elapsed() >= Duration::from_millis(250)
                || saved_at.elapsed() >= Duration::from_secs(2))
        {
            match persist(&session, &preferences, &workspace.state, &layout) {
                Ok(()) => {
                    pending = false;
                    saved_at = Instant::now();
                }
                Err(error) => {
                    workspace.state.message = format!("Backup failed: {error}");
                    let _ = events.unbounded_send(Event::Changed(workspace.state.clone()));
                    changed_at = Instant::now();
                    saved_at = Instant::now();
                }
            }
        }
        if checked_at.elapsed() >= Duration::from_millis(750) {
            checked_at = Instant::now();
            let recovered_paths = workspace.recover_missing_paths();
            if recovered_paths {
                pending = true;
                changed_at = Instant::now();
            }
            let previous = workspace
                .state
                .rows
                .iter()
                .map(|row| (row.path.clone(), row.expanded))
                .collect::<Vec<_>>();
            let previous_message = workspace.state.message.clone();
            let documents_changed = workspace.poll_external().unwrap_or(false) || recovered_paths;
            match workspace.refresh_tree() {
                Err(error) => {
                    workspace.state.message = format!("Project tree unavailable: {error}")
                }
                Ok(())
                    if workspace
                        .state
                        .message
                        .starts_with("Project tree unavailable:") =>
                {
                    workspace.state.message = "Ready".into()
                }
                Ok(()) => {}
            }
            let tree_changed = !workspace
                .state
                .rows
                .iter()
                .map(|row| (row.path.clone(), row.expanded))
                .eq(previous);
            if documents_changed
                && !workspace
                    .state
                    .message
                    .starts_with("Project tree unavailable:")
            {
                workspace.state.message = "External file change detected".into();
            }
            if documents_changed || tree_changed || workspace.state.message != previous_message {
                pending = true;
                let _ = events.unbounded_send(Event::Changed(workspace.state.clone()));
            }
        }
    }
}
fn persist(
    session: &std::path::Path,
    preferences: &std::path::Path,
    state: &WorkspaceState,
    layout: &LayoutPreferences,
) -> Result<(), String> {
    services::write_json(session, state)
        .and_then(|()| services::write_json(preferences, layout))
        .map_err(|error| error.to_string())
}
