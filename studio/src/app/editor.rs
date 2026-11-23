//! Foreground bindings only: snapshots and requests cross to background services.
use super::StudioEnvironment;
use crate::{
    editor::{
        hover,
        lsp::{self, protocol},
    },
    workspace::Command,
};
use futures::StreamExt;
use omen::{EditorDocument, EditorEvent, EditorState, Value};
use std::{path::PathBuf, sync::Arc};

impl StudioEnvironment {
    pub fn editor_document(&self, path: PathBuf) -> EditorDocument {
        let existing = self.editor_states.borrow().get(&path).cloned();
        let state = existing.unwrap_or_else(|| {
            let text = self
                .workspace
                .get()
                .documents
                .iter()
                .find(|document| document.path == path)
                .map(|document| document.content.clone())
                .unwrap_or_default();
            let state = Value::new(EditorState {
                text,
                letter_spacing: self
                    .settings
                    .read(|settings| settings.editor.letter_spacing),
                ..Default::default()
            });
            self.editor_states
                .borrow_mut()
                .insert(path.clone(), state.clone());
            if let (Some(language), Some(root)) = (&self.language, self.workspace.get().root) {
                language.document(root, path.clone(), state.get().text, 0);
            }
            state
        });
        let app = self.clone();
        EditorDocument::new(state, move |event| app.editor_event(&path, event))
    }
    fn editor_event(&self, path: &PathBuf, event: EditorEvent) {
        let Some(model) = self.editor_states.borrow().get(path).cloned() else {
            return;
        };
        let state = model.get();
        if matches!(event, EditorEvent::Edited) {
            let workspace = self.workspace.get();
            let Some(document) = workspace
                .documents
                .iter()
                .find(|document| document.path == *path)
            else {
                return;
            };
            self.code_actions.set(Vec::new());
            let baseline = document.baseline.clone();
            let disk_revision = document.disk_revision;
            self.workspace.update(|workspace| {
                if let Some(document) = workspace
                    .documents
                    .iter_mut()
                    .find(|document| document.path == *path)
                {
                    document.content = state.text.clone();
                }
            });
            self.send(Command::EditSnapshot {
                path: path.clone(),
                content: state.text.clone(),
                disk_revision,
                baseline,
            });
            if state.selection.is_empty() {
                let row = state.text.byte_to_line(state.selection.end);
                let line_prefix = state
                    .text
                    .slice(state.text.line_start(row)..state.selection.end);
                if line_prefix
                    .trim_end_matches(|ch: char| ch.is_ascii_alphanumeric() || ch == '_')
                    .ends_with('.')
                {
                    let items =
                        lsp::modifier_completions(&state.text.slice(..), state.selection.end);
                    if !items.is_empty() {
                        model.update(|state| {
                            state.completions = Arc::new(items);
                            state.completion_index = 0;
                        });
                    }
                }
            }
            if let (Some(language), Some(root)) = (&self.language, workspace.root) {
                language.document(root, path.clone(), state.text, state.selection.end);
            }
            return;
        }
        match event {
            EditorEvent::Blur => {
                self.editor_blur(path);
                return;
            }
            EditorEvent::Find => {
                self.find_visible.set(true);
                return;
            }
            EditorEvent::Rename => {
                self.rename_visible.set(true);
                return;
            }
            EditorEvent::ApplyAction(index) => {
                self.code_action(index);
                return;
            }
            EditorEvent::HoverAt(offset) => {
                self.hover_document(path, &model, &state.text, offset);
                return;
            }
            EditorEvent::Hover => {
                self.hover_document(path, &model, &state.text, state.selection.end);
                return;
            }
            _ => {}
        }
        let query = match event {
            EditorEvent::Complete => lsp::Query::Complete,
            EditorEvent::Definition => lsp::Query::Definition,
            EditorEvent::Format => lsp::Query::Format,
            EditorEvent::Actions => lsp::Query::Actions,
            _ => return,
        };
        self.language_query(path.clone(), query);
    }
    fn hover_document(
        &self,
        path: &std::path::Path,
        model: &Value<EditorState>,
        text: &omen::TextSnapshot,
        offset: usize,
    ) {
        let omen_help = hover::omen_help(text, offset);
        let local = omen_help.is_some();
        let help = omen_help.or_else(|| hover::rust_help(text, offset));
        model.update(|state| {
            state.hover_offset = Some(offset);
            state.hover = Some(help.unwrap_or_default());
        });
        if !local && let (Some(language), Some(root)) = (&self.language, self.workspace.get().root)
        {
            language.query(
                root,
                path.to_owned(),
                text.clone(),
                offset,
                lsp::Query::Hover,
            );
        }
    }
    pub fn language_query(&self, path: PathBuf, query: lsp::Query) {
        let document = self.editor_document(path.clone());
        let state = document.state.get();
        if matches!(query, lsp::Query::Hover) {
            self.hover_document(&path, &document.state, &state.text, state.selection.end);
            return;
        }
        document
            .state
            .update(|state| state.reveal = Some(state.selection.clone()));
        if let (Some(language), Some(root)) = (&self.language, self.workspace.get().root) {
            language.query(root, path, state.text, state.selection.end, query);
        } else {
            self.language_status
                .set("Rust language service is unavailable".into());
        }
    }
    pub fn request_language(&self, request: EditorEvent) {
        if let Some(path) = self.workspace.get().active_file {
            self.editor_event(&path, request);
        }
    }
    pub(super) fn synchronize_editor_states(&self, workspace: &crate::state::WorkspaceState) {
        if let Some(language) = &self.language {
            for path in self.editor_states.borrow().keys().filter(|path| {
                !workspace
                    .documents
                    .iter()
                    .any(|document| document.path == **path)
            }) {
                language.close(path.clone());
            }
        }
        self.visual_history.borrow_mut().retain(|path, _| {
            workspace
                .documents
                .iter()
                .any(|document| document.path == *path)
        });
        self.editor_states.borrow_mut().retain(|path, _| {
            workspace
                .documents
                .iter()
                .any(|document| document.path == *path)
        });
        for document in &workspace.documents {
            if let Some(state) = self.editor_states.borrow().get(&document.path).cloned()
                && !state.get().text.same_snapshot(&document.content)
            {
                state.update(|state| {
                    state.text = document.content.clone();
                    state.history_epoch = document.disk_revision;
                    state.selection_request = None;
                    state.selection.start = state.selection.start.min(state.text.len());
                    state.selection.end = state.selection.end.min(state.text.len());
                    state.diagnostics = Arc::default();
                    state.completions = Arc::default();
                });
                if let (Some(language), Some(root)) = (&self.language, workspace.root.clone()) {
                    language.document(
                        root,
                        document.path.clone(),
                        document.content.clone(),
                        state.get().selection.end,
                    );
                }
            }
        }
        if let Some((path, position)) = self.navigation.get()
            && let Some(document) = workspace
                .documents
                .iter()
                .find(|document| document.path == path)
        {
            let state = self
                .editor_states
                .borrow_mut()
                .entry(path.clone())
                .or_insert_with(|| {
                    Value::new(EditorState {
                        text: document.content.clone(),
                        letter_spacing: self
                            .settings
                            .read(|settings| settings.editor.letter_spacing),
                        ..Default::default()
                    })
                })
                .clone();
            let offset = protocol::offset(&document.content, &position);
            state.update(|state| {
                state.selection = offset..offset;
                state.reveal = Some(offset..offset);
            });
            self.navigation.set(None);
        }
    }
    pub(super) async fn receive_language_updates(&self) {
        let mut guard = self.language_updates.lock().await;
        let Some(events) = guard.as_mut() else {
            return;
        };
        while let Some(event) = events.next().await {
            match event {
                lsp::Event::Status(message) => self.language_status.set(message),
                lsp::Event::Tokens {
                    path,
                    identity,
                    spans,
                } => {
                    if let Some(state) = self.editor_states.borrow().get(&path).cloned()
                        && state.get().text.identity() == identity
                    {
                        state.update(|state| {
                            state.highlights = Arc::new(spans);
                            state.highlight_identity = Some(identity);
                        });
                    }
                }
                lsp::Event::Diagnostics {
                    path,
                    identity,
                    diagnostics,
                } => {
                    if let Some(state) = self.editor_states.borrow().get(&path).cloned()
                        && state.get().text.identity() == identity
                    {
                        state.update(|state| state.diagnostics = Arc::new(diagnostics));
                    }
                }
                lsp::Event::Completions {
                    path,
                    identity,
                    cursor,
                    items,
                } => {
                    if let Some(state) = self.editor_states.borrow().get(&path).cloned()
                        && state.get().text.identity() == identity
                        && state.get().selection.end == cursor
                    {
                        state.update(|state| {
                            state.completions = Arc::new(items);
                            state.completion_index = 0;
                        });
                    }
                }
                lsp::Event::Hover {
                    path,
                    identity,
                    text,
                    cursor,
                } => {
                    if !text.is_empty()
                        && let Some(state) = self.editor_states.borrow().get(&path).cloned()
                        && state.read(|state| {
                            state.text.identity() == identity && state.hover_offset == Some(cursor)
                        })
                    {
                        state.update(|state| state.hover = Some(text));
                    }
                }
                lsp::Event::Definition { path, position } => {
                    self.navigation.set(Some((path.clone(), position)));
                    self.send(Command::OpenFile(path));
                }
                lsp::Event::Edited {
                    path,
                    identity,
                    text,
                } => {
                    let model = self.editor_states.borrow().get(&path).cloned();
                    if let Some(model) = model
                        && model.get().text.identity() == identity
                    {
                        model.update(|state| {
                            state.text = text;
                            state.completions = Arc::default();
                        });
                        self.editor_event(&path, EditorEvent::Edited);
                    }
                }
                lsp::Event::WorkspaceEdit { edit, expected } => {
                    self.send(Command::ApplyLanguageEdit { edit, expected })
                }
                lsp::Event::Actions {
                    path,
                    identity,
                    items,
                } => {
                    if self
                        .editor_states
                        .borrow()
                        .get(&path)
                        .is_some_and(|state| state.get().text.identity() == identity)
                    {
                        if let Some(model) = self.editor_states.borrow().get(&path) {
                            model.update(|state| {
                                state.completion_index = 0;
                                state.actions = Arc::new(
                                    items
                                        .iter()
                                        .map(|item| {
                                            item["title"]
                                                .as_str()
                                                .unwrap_or("Apply action")
                                                .to_owned()
                                        })
                                        .collect(),
                                )
                            });
                        }
                        self.code_actions.set(items);
                    }
                }
            }
        }
    }
    pub fn accept_completion(&self, index: usize) {
        let Some(path) = self.workspace.get().active_file else {
            return;
        };
        let document = self.editor_document(path.clone());
        let state = document.state.get();
        let Some(item) = state.completions.get(index).cloned() else {
            return;
        };
        document.state.update(|state| {
            state.text.replace(item.range.clone(), &item.insert);
            let cursor = item.range.start + item.insert.len();
            state.selection = cursor..cursor;
            state.reveal = Some(cursor..cursor);
            state.completions = Arc::default();
        });
        self.editor_event(&path, EditorEvent::Edited);
    }
    pub async fn find_next(&self) {
        let needle = self.find_text.get();
        if needle.is_empty() {
            return;
        }
        let Some(path) = self.workspace.get().active_file else {
            return;
        };
        let document = self.editor_document(path);
        let state = document.state.get();
        let text = state.text.clone();
        let expected = state.text;
        let start = state.selection.end;
        let (tx, rx) = futures::channel::oneshot::channel();
        let query = needle.clone();
        let spawned = std::thread::Builder::new()
            .name("omen-find".into())
            .spawn(move || {
                let found = text
                    .slice(start..)
                    .find(&query)
                    .map(|offset| start + offset)
                    .or_else(|| text.slice(..start).find(&query));
                let _ = tx.send(found);
            });
        if spawned.is_err() {
            self.language_status
                .set("Could not start text search".into());
            return;
        }
        if let Ok(Some(start)) = rx.await
            && document.state.get().text.same_snapshot(&expected)
            && self.find_text.get() == needle
        {
            document.state.update(|state| {
                state.selection = start..start + needle.len();
                state.reveal = Some(state.selection.clone());
            });
        }
    }
    pub async fn replace_match(&self, all: bool) {
        let needle = self.find_text.get();
        if needle.is_empty() {
            return;
        }
        let replacement = self.replacement_text.get();
        let Some(path) = self.workspace.get().active_file else {
            return;
        };
        let document = self.editor_document(path.clone());
        let state = document.state.get();
        let expected = state.text.clone();
        if !all && state.text.slice(state.selection.clone()) != needle {
            self.find_next().await;
            return;
        }
        let (tx, rx) = futures::channel::oneshot::channel();
        let spawned = std::thread::Builder::new()
            .name("omen-replace".into())
            .spawn(move || {
                let mut text = state.text;
                if all {
                    let offsets = text
                        .to_string()
                        .match_indices(&needle)
                        .map(|(offset, _)| offset)
                        .collect::<Vec<_>>();
                    for offset in offsets.into_iter().rev() {
                        text.replace(offset..offset + needle.len(), &replacement);
                    }
                } else {
                    text.replace(state.selection, &replacement);
                }
                let _ = tx.send(text);
            });
        if spawned.is_err() {
            self.language_status.set("Could not start replace".into());
            return;
        }
        if let Ok(text) = rx.await
            && document.state.get().text.same_snapshot(&expected)
        {
            document.state.update(|state| {
                state.text = text;
                let cursor = state.selection.start.min(state.text.len());
                state.selection = cursor..cursor;
                state.reveal = Some(cursor..cursor);
            });
            self.editor_event(&path, EditorEvent::Edited);
        }
    }
    pub fn reveal_diagnostic(&self, index: usize) {
        let Some(path) = self.workspace.get().active_file else {
            return;
        };
        let document = self.editor_document(path);
        let state = document.state.get();
        if let Some(diagnostic) = state.diagnostics.get(index) {
            let range = diagnostic.range.clone();
            document.state.update(|state| {
                state.selection = range.clone();
                state.reveal = Some(range);
            });
        }
    }
    pub fn rename_symbol(&self) {
        let name = self.rename_text.get();
        if name.trim().is_empty() {
            return;
        }
        if let Some(path) = self.workspace.get().active_file {
            self.language_query(path, lsp::Query::Rename(name));
            self.rename_visible.set(false);
        }
    }
    pub fn code_action(&self, index: usize) {
        let Some(action) = self.code_actions.get().get(index).cloned() else {
            return;
        };
        if let Some(edit) = action.get("edit") {
            let expected = self
                .editor_states
                .borrow()
                .iter()
                .map(|(path, state)| (path.clone(), state.get().text))
                .collect();
            self.send(Command::ApplyLanguageEdit {
                edit: edit.clone(),
                expected,
            });
        } else if let Some(path) = self.workspace.get().active_file {
            self.language_query(path, lsp::Query::ResolveAction(action));
        }
        self.code_actions.set(Vec::new());
    }
}
