//! Auto Save runs beside the workspace so edits and disk writes stay ordered.
use super::Workspace;
use crate::settings::{AutoSaveMode, EditorSettings};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

struct PendingSave {
    revision: u64,
    due: Option<Instant>,
}

pub(super) struct AutoSave {
    mode: AutoSaveMode,
    delay: Duration,
    pending: HashMap<PathBuf, PendingSave>,
}

impl AutoSave {
    pub(super) fn new(settings: &EditorSettings) -> Self {
        Self {
            mode: settings.auto_save,
            delay: Duration::from_millis(u64::from(settings.auto_save_delay_ms)),
            pending: HashMap::new(),
        }
    }

    pub(super) fn configure(&mut self, settings: &EditorSettings, workspace: &Workspace) {
        let delay = Duration::from_millis(u64::from(settings.auto_save_delay_ms));
        if self.mode != settings.auto_save || self.delay != delay {
            self.mode = settings.auto_save;
            self.delay = delay;
            self.pending.clear();
            self.observe(workspace);
        }
    }

    pub(super) fn observe(&mut self, workspace: &Workspace) {
        if self.mode != AutoSaveMode::AfterDelay {
            return;
        }
        let now = Instant::now();
        self.pending.retain(|path, _| {
            workspace
                .state
                .documents
                .iter()
                .any(|document| document.path == *path && document.dirty() && !document.conflict)
        });
        for document in workspace
            .state
            .documents
            .iter()
            .filter(|document| document.dirty() && !document.conflict)
        {
            let revision = document.content.identity();
            let save = self
                .pending
                .entry(document.path.clone())
                .or_insert(PendingSave {
                    revision,
                    due: Some(now + self.delay),
                });
            if save.revision != revision {
                *save = PendingSave {
                    revision,
                    due: Some(now + self.delay),
                };
            }
        }
    }

    pub(super) fn save_due(&mut self, workspace: &mut Workspace) -> bool {
        let now = Instant::now();
        let paths = self.pending.iter_mut().filter_map(|(path, save)| {
            if save.due.is_some_and(|deadline| deadline <= now) {
                save.due = None;
                Some(path.as_path())
            } else {
                None
            }
        });
        Self::save(workspace, paths)
    }

    pub(super) fn focus_lost(&self, workspace: &mut Workspace, path: &Path) -> bool {
        self.mode == AutoSaveMode::OnFocusChange && Self::save(workspace, [path])
    }

    pub(super) fn window_lost(&self, workspace: &mut Workspace) -> bool {
        if self.mode != AutoSaveMode::OnWindowChange {
            return false;
        }
        let paths: Vec<_> = workspace
            .state
            .documents
            .iter()
            .filter(|document| document.dirty() && !document.conflict)
            .map(|document| document.path.clone())
            .collect();
        Self::save(workspace, paths.iter().map(PathBuf::as_path))
    }

    fn save<'a>(workspace: &mut Workspace, paths: impl IntoIterator<Item = &'a Path>) -> bool {
        let mut attempted = false;
        for path in paths {
            if !workspace
                .state
                .documents
                .iter()
                .any(|document| document.path == path && document.dirty() && !document.conflict)
            {
                continue;
            }
            attempted = true;
            if let Err(error) = workspace.save_file(path) {
                workspace.state.message =
                    format!("Auto Save failed for {}. {error}", path.display());
            }
        }
        attempted
    }
}
