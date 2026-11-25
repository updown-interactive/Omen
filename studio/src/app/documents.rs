//! Document selection, persistence and edit bindings.
use super::*;

impl StudioEnvironment {
    pub fn close_project(&self) {
        self.send(Command::CloseProject);
    }
    pub fn close_active_document(&self) {
        if let Some(path) = self.workspace.get().active_file {
            self.send(Command::Close(path));
        }
    }
    pub fn cycle_document(&self, forward: bool) {
        let state = self.workspace.get();
        let count = state.documents.len();
        if count <= 1 {
            return;
        }
        let current = state
            .active_file
            .as_ref()
            .and_then(|active| state.documents.iter().position(|doc| &doc.path == active))
            .unwrap_or(0);
        let next = if forward {
            (current + 1) % count
        } else {
            (current + count - 1) % count
        };
        self.send(Command::Select(state.documents[next].path.clone()));
    }
    pub fn save_active(&self) {
        if let Some(path) = self.workspace.get().active_file {
            let conflict = self
                .workspace
                .get()
                .documents
                .iter()
                .find(|d| d.path == path)
                .is_some_and(|d| d.conflict);
            if conflict {
                self.reload_visible.set(true);
            } else {
                self.send(Command::Save(path));
            }
        }
    }
    pub(crate) fn editor_blur(&self, path: &std::path::Path) {
        if self.settings.read(|settings| {
            settings.editor.auto_save == crate::settings::AutoSaveMode::OnFocusChange
        }) && let Some(service) = &self.service
        {
            let _ = service.editor_blur(path.to_owned());
        }
    }
    pub(crate) fn window_blur(&self) {
        if self.settings.read(|settings| {
            settings.editor.auto_save == crate::settings::AutoSaveMode::OnWindowChange
        }) && let Some(service) = &self.service
        {
            let _ = service.window_blur();
        }
    }
    pub fn content(&self, path: PathBuf) -> Binding<String> {
        let read = self.clone();
        let write = self.clone();
        let read_path = path.clone();
        Binding::new(
            move || {
                read.workspace
                    .get()
                    .documents
                    .iter()
                    .find(|document| document.path == read_path)
                    .map(|document| document.content.to_string())
                    .unwrap_or_default()
            },
            move |content| {
                let snapshot = write
                    .workspace
                    .get()
                    .documents
                    .iter()
                    .find(|document| document.path == path)
                    .map(|document| (document.disk_revision, document.baseline.to_string()));
                let Some((disk_revision, baseline)) = snapshot else {
                    return;
                };
                write.workspace.update(|state| {
                    if let Some(document) = state
                        .documents
                        .iter_mut()
                        .find(|document| document.path == path)
                    {
                        document.content = content.clone().into();
                    }
                });
                write.send(Command::EditFromSnapshot {
                    path: path.clone(),
                    content,
                    disk_revision,
                    baseline,
                });
            },
        )
    }
}
