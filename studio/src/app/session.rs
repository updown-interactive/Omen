//! Workspace event delivery and application lifecycle.
use super::*;

impl StudioEnvironment {
    pub fn receive(&self) -> impl std::future::Future<Output = ()> + 'static + use<> {
        let app = self.clone();
        async move {
            futures::join!(
                app.receive_updates(),
                app.receive_language_updates(),
                app.receive_run_updates(),
                app.receive_agent_updates()
            );
        }
    }
    async fn receive_updates(&self) {
        // A mounted task is cancelled while its window is hidden. Keep the stream
        // in a cancellation-safe mutex so the foreground task resumes queued events.
        let mut guard = self.updates.lock().await;
        let Some(updates) = guard.as_mut() else {
            return;
        };
        while let Some(event) = updates.next().await {
            let state = match event {
                Event::Loaded(layout, settings, account, state) => {
                    self.account.set(account);
                    self.settings.set(settings);
                    self.layout.set(layout);
                    if let Some(root) = &state.root {
                        self.open_path.set(root.to_string_lossy().into());
                    }
                    self.editor_visible.set(state.root.is_some());
                    self.ready.set(true);
                    state
                }
                Event::Changed(state) => state,
                Event::AccountSaved(account) => {
                    self.account.set(account);
                    continue;
                }
            };
            let mut renamed_selection = None;
            if self.new_entry.get().pending == Some(state.sequence) {
                self.new_entry.update(|draft| {
                    draft.pending = None;
                    draft.error = if state.message == "Ready" {
                        String::new()
                    } else {
                        state.message.clone()
                    };
                });
                if state.message == "Ready" {
                    let draft = self.new_entry.get();
                    if draft.source.is_some() {
                        renamed_selection = draft.parent.map(|parent| parent.join(draft.name));
                    }
                    self.new_entry_visible.set(false);
                }
            }
            if self.delete_pending.get() == Some(state.sequence) {
                self.delete_pending.set(None);
                if state.message == "Ready" {
                    self.delete_visible.set(false);
                } else {
                    self.delete_error.set(state.message.clone());
                }
            }
            if self.paste_pending.get() == Some(state.sequence) {
                self.paste_pending.set(None);
                if state.message == "Ready" {
                    self.explorer_clipboard.set(None);
                }
            }
            if self.opening.get() == Some(state.sequence) {
                self.opening.set(None);
                if state.message == "Ready" && state.root.is_some() {
                    self.editor_visible.set(true);
                }
            }
            if self.creating.get() == Some(state.sequence) {
                self.creating.set(None);
                if state.message == "Ready" {
                    self.creation_visible.set(false);
                    if let Some(root) = &state.root {
                        self.open_path.set(root.to_string_lossy().into());
                    }
                } else {
                    self.creation_error.set(state.message.clone());
                }
            }
            if state.sequence >= self.workspace.get().sequence {
                let previous = self.workspace.get();
                if state.root != previous.root {
                    self.reset_project_tools();
                    self.reset_run_project();
                    self.new_agent_chat();
                }
                if state.root != previous.root || state.active_file != previous.active_file {
                    self.canvas_motion.set(None);
                    self.visual_result.set(None);
                    self.visual_message.set(String::new());
                    self.explorer_selection.set(state.active_file.clone());
                }
                if previous.root.is_some() && state.root.is_none() {
                    self.editor_visible.set(false);
                    self.open_path.set(String::new());
                    self.context.set(None);
                    self.context_visible.set(false);
                    self.explorer_clipboard.set(None);
                    self.palette_visible.set(false);
                    self.new_entry_visible.set(false);
                    self.delete_visible.set(false);
                    self.find_visible.set(false);
                    self.rename_visible.set(false);
                }
                self.synchronize_editor_states(&state);
                self.workspace.set(state);
                if let Some(path) = renamed_selection {
                    self.explorer_selection.set(Some(path));
                }
            }
        }
    }
    pub fn send(&self, command: Command) {
        let sequence = self.sequence.get() + 1;
        self.sequence.set(sequence);
        if let Command::Select(path) = &command {
            self.explorer_selection.set(Some(path.clone()));
        }
        if matches!(
            &command,
            Command::OpenProject(_) | Command::Create { .. } | Command::CreateConfigured { .. }
        ) {
            self.opening.set(Some(sequence));
        }
        let is_edit = matches!(&command, Command::EditSnapshot { .. });
        self.workspace.update(|state| {
            state.sequence = sequence;
            if !is_edit {
                state.message = "Working…".into();
            }
        });
        let result = self
            .service
            .as_ref()
            .ok_or_else(|| "Workspace worker is unavailable".to_string())
            .and_then(|service| service.send(sequence, command));
        if let Err(message) = result {
            self.workspace.update(|state| state.message = message);
        }
    }
    pub fn window_close(&self) {
        self.stop_run();
        self.stop_agent();
        self.flush();
    }
    pub fn flush(&self) {
        if let Some(service) = &self.service
            && let Err(error) = service.flush()
        {
            eprintln!("Studio backup failed: {error}");
        }
    }
    pub fn open(&self) {
        self.send(Command::OpenProject(PathBuf::from(self.open_path.get())));
    }
}
