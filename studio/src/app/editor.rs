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
            if self.find_visible.get() {
                self.refresh_find_matches();
            }
            return;
        }
        match event {
            EditorEvent::Blur => {
                self.editor_blur(path);
                return;
            }
            EditorEvent::Find => {
                self.open_find();
                return;
            }
            EditorEvent::Rename => {
                self.open_rename();
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
    pub fn open_find(&self) {
        self.find_visible.set(true);
        self.replace_focus.set(false);
        self.find_focus.set(true);
        if let Some(path) = self.workspace.get().active_file {
            let model = self.editor_states.borrow().get(&path).cloned();
            if let Some(model) = model {
                let state = model.get();
                if !state.selection.is_empty() {
                    let selected = state.text.slice(state.selection.clone());
                    if !selected.is_empty() && !selected.contains('\n') && selected.len() <= 100 {
                        self.find_text.set(selected.to_string());
                    }
                }
            }
        }
        self.refresh_find_matches();
    }
    pub fn open_replace(&self) {
        self.open_find();
        self.find_focus.set(false);
        self.replace_focus.set(true);
    }
    pub fn close_find(&self) {
        self.find_visible.set(false);
        self.find_focus.set(false);
        self.replace_focus.set(false);
        if let Some(path) = self.workspace.get().active_file
            && let Some(model) = self.editor_states.borrow().get(&path).cloned()
        {
            model.update(|state| {
                state.search_matches = Arc::default();
                state.active_search_match = None;
            });
        }
        self.focus_editor();
    }
    pub fn refresh_find_matches(&self) {
        let needle = self.find_text.get();
        let case_sensitive = self.find_case_sensitive.get();
        let whole_word = self.find_whole_word.get();
        let Some(path) = self.workspace.get().active_file else {
            self.find_match_count.set(0);
            self.find_match_index.set(None);
            return;
        };
        let document = self.editor_document(path);
        let state = document.state.get();
        if needle.is_empty() {
            self.find_match_count.set(0);
            self.find_match_index.set(None);
            document.state.update(|state| {
                state.search_matches = Arc::default();
                state.active_search_match = None;
            });
            return;
        }
        let source = state.text.to_string();
        let matches = collect_find_matches(&source, &needle, case_sensitive, whole_word);
        let count = matches.len();
        let active_index = matches.iter().position(|range| *range == state.selection);
        let active_match = active_index.map(|idx| matches[idx].clone());
        self.find_match_count.set(count);
        self.find_match_index.set(active_index.map(|idx| idx + 1));
        document.state.update(|state| {
            state.search_matches = Arc::new(matches);
            state.active_search_match = active_match;
        });
    }
    pub fn find_step_sync(&self, forward: bool) {
        let needle = self.find_text.get();
        if needle.is_empty() {
            self.refresh_find_matches();
            return;
        }
        let case_sensitive = self.find_case_sensitive.get();
        let whole_word = self.find_whole_word.get();
        let Some(path) = self.workspace.get().active_file else {
            return;
        };
        let document = self.editor_document(path);
        let state = document.state.get();
        let source = state.text.to_string();
        let (matches, picked) =
            compute_find(&source, &needle, case_sensitive, whole_word, &state.selection, forward);
        self.apply_find_result(&document, matches, picked);
    }
    pub async fn find_next(&self) {
        self.find_step(true).await;
    }
    pub async fn find_previous(&self) {
        self.find_step(false).await;
    }
    async fn find_step(&self, forward: bool) {
        let needle = self.find_text.get();
        if needle.is_empty() {
            self.refresh_find_matches();
            return;
        }
        let case_sensitive = self.find_case_sensitive.get();
        let whole_word = self.find_whole_word.get();
        let Some(path) = self.workspace.get().active_file else {
            return;
        };
        let document = self.editor_document(path);
        let state = document.state.get();
        let text = state.text.clone();
        let expected = state.text;
        let selection = state.selection;
        let (tx, rx) = futures::channel::oneshot::channel();
        let query = needle.clone();
        let spawned = std::thread::Builder::new()
            .name("omen-find".into())
            .spawn(move || {
                let source = text.to_string();
                let result =
                    compute_find(&source, &query, case_sensitive, whole_word, &selection, forward);
                let _ = tx.send(result);
            });
        if spawned.is_err() {
            self.language_status
                .set("Could not start text search".into());
            return;
        }
        if let Ok((matches, picked)) = rx.await
            && document.state.get().text.same_snapshot(&expected)
            && self.find_text.get() == needle
            && self.find_case_sensitive.get() == case_sensitive
            && self.find_whole_word.get() == whole_word
        {
            self.apply_find_result(&document, matches, picked);
        }
    }
    fn apply_find_result(
        &self,
        document: &EditorDocument,
        matches: Vec<std::ops::Range<usize>>,
        picked: Option<usize>,
    ) {
        let count = matches.len();
        let active = picked.map(|idx| matches[idx].clone());
        self.find_match_count.set(count);
        self.find_match_index.set(picked.map(|idx| idx + 1));
        document.state.update(|state| {
            state.search_matches = Arc::new(matches);
            state.active_search_match = active.clone();
            if let Some(range) = active {
                state.selection = range.clone();
                state.selection_request = Some(range);
            }
        });
    }
    pub fn replace_match_sync(&self, all: bool) {
        let needle = self.find_text.get();
        if needle.is_empty() {
            return;
        }
        let case_sensitive = self.find_case_sensitive.get();
        let whole_word = self.find_whole_word.get();
        let replacement = self.replacement_text.get();
        let Some(path) = self.workspace.get().active_file else {
            return;
        };
        let document = self.editor_document(path.clone());
        let state = document.state.get();
        if !all && !selection_matches(&state.text, &state.selection, &needle, case_sensitive, whole_word)
        {
            self.find_step_sync(true);
            return;
        }
        let (text, cursor) = compute_replace(
            state.text,
            state.selection,
            &needle,
            &replacement,
            case_sensitive,
            whole_word,
            all,
        );
        self.apply_replace_result(&path, &document, text, cursor);
        if !all {
            self.find_step_sync(true);
        }
    }
    pub async fn replace_match(&self, all: bool) {
        let needle = self.find_text.get();
        if needle.is_empty() {
            return;
        }
        let case_sensitive = self.find_case_sensitive.get();
        let whole_word = self.find_whole_word.get();
        let replacement = self.replacement_text.get();
        let Some(path) = self.workspace.get().active_file else {
            return;
        };
        let document = self.editor_document(path.clone());
        let state = document.state.get();
        let expected = state.text.clone();
        if !all && !selection_matches(&state.text, &state.selection, &needle, case_sensitive, whole_word)
        {
            self.find_next().await;
            return;
        }
        let (tx, rx) = futures::channel::oneshot::channel();
        let spawned = std::thread::Builder::new()
            .name("omen-replace".into())
            .spawn(move || {
                let result = compute_replace(
                    state.text,
                    state.selection,
                    &needle,
                    &replacement,
                    case_sensitive,
                    whole_word,
                    all,
                );
                let _ = tx.send(result);
            });
        if spawned.is_err() {
            self.language_status.set("Could not start replace".into());
            return;
        }
        if let Ok((text, cursor)) = rx.await
            && document.state.get().text.same_snapshot(&expected)
        {
            self.apply_replace_result(&path, &document, text, cursor);
            if !all {
                self.find_next().await;
            }
        }
    }
    fn apply_replace_result(
        &self,
        path: &PathBuf,
        document: &EditorDocument,
        text: omen::TextSnapshot,
        cursor: usize,
    ) {
        document.state.update(|state| {
            state.text = text;
            state.selection = cursor..cursor;
            state.selection_request = Some(cursor..cursor);
        });
        self.editor_event(path, EditorEvent::Edited);
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
    pub fn next_diagnostic(&self, forward: bool) {
        let Some(path) = self.workspace.get().active_file else {
            return;
        };
        let document = self.editor_document(path);
        let state = document.state.get();
        if state.diagnostics.is_empty() {
            return;
        }
        let cursor = state.selection.end;
        let target = if forward {
            state
                .diagnostics
                .iter()
                .position(|d| d.range.start > cursor)
                .unwrap_or(0)
        } else {
            state
                .diagnostics
                .iter()
                .rposition(|d| d.range.end < state.selection.start)
                .unwrap_or(state.diagnostics.len() - 1)
        };
        self.reveal_diagnostic(target);
    }
    pub fn open_rename(&self) {
        if let Some(path) = self.workspace.get().active_file {
            let state = self.editor_document(path).state.get();
            if let Some(symbol) = symbol_at_cursor(&state.text, state.selection.end) {
                self.rename_text.set(symbol);
            }
        }
        self.rename_visible.set(true);
        self.rename_focus.set(true);
    }
    pub fn rename_symbol(&self) {
        let name = self.rename_text.get();
        if name.trim().is_empty() {
            return;
        }
        if let Some(path) = self.workspace.get().active_file {
            self.language_query(path, lsp::Query::Rename(name));
            self.rename_visible.set(false);
            self.rename_focus.set(false);
        }
    }
    pub fn open_go_to_line(&self) {
        self.goto_text.set(String::new());
        self.goto_visible.set(true);
        self.goto_focus.set(true);
    }
    pub fn go_to_line(&self) {
        let query = self.goto_text.get();
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return;
        }
        let mut parts = trimmed.split(':');
        let Ok(line) = parts.next().unwrap_or_default().trim().parse::<usize>() else {
            return;
        };
        let column = parts
            .next()
            .and_then(|part| part.trim().parse::<usize>().ok())
            .unwrap_or(1);
        let Some(path) = self.workspace.get().active_file else {
            return;
        };
        let document = self.editor_document(path);
        document.state.update(|state| {
            let row = line.saturating_sub(1).min(state.text.line_count().saturating_sub(1));
            let start = state.text.line_start(row);
            let max_end = start + state.text.line(row).len();
            let offset = state
                .text
                .utf16_to_byte(state.text.byte_to_utf16(start) + column.saturating_sub(1))
                .min(max_end);
            state.selection = offset..offset;
            state.reveal = Some(offset..offset);
        });
        self.goto_visible.set(false);
        self.goto_focus.set(false);
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

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn compute_find(
    source: &str,
    needle: &str,
    case_sensitive: bool,
    whole_word: bool,
    selection: &std::ops::Range<usize>,
    forward: bool,
) -> (Vec<std::ops::Range<usize>>, Option<usize>) {
    let matches = collect_find_matches(source, needle, case_sensitive, whole_word);
    let picked = if matches.is_empty() {
        None
    } else if forward {
        matches
            .iter()
            .position(|range| range.start >= selection.end)
            .or(Some(0))
    } else {
        matches
            .iter()
            .rposition(|range| range.end <= selection.start)
            .or_else(|| Some(matches.len() - 1))
    };
    (matches, picked)
}

fn selection_matches(
    text: &omen::TextSnapshot,
    selection: &std::ops::Range<usize>,
    needle: &str,
    case_sensitive: bool,
    whole_word: bool,
) -> bool {
    let selected = text.slice(selection.clone());
    let same = if case_sensitive {
        selected == needle
    } else {
        selected.to_lowercase() == needle.to_lowercase()
    };
    same && (!whole_word || is_word_boundary(&text.to_string(), selection.start, selection.end))
}

fn compute_replace(
    mut text: omen::TextSnapshot,
    selection: std::ops::Range<usize>,
    needle: &str,
    replacement: &str,
    case_sensitive: bool,
    whole_word: bool,
    all: bool,
) -> (omen::TextSnapshot, usize) {
    if all {
        let source = text.to_string();
        let matches = collect_find_matches(&source, needle, case_sensitive, whole_word);
        for range in matches.into_iter().rev() {
            text.replace(range, replacement);
        }
        let cursor = selection.start.min(text.len());
        (text, cursor)
    } else {
        let start = selection.start;
        text.replace(selection, replacement);
        let cursor = (start + replacement.len()).min(text.len());
        (text, cursor)
    }
}

fn is_word_boundary(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();
    !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
}

fn symbol_at_cursor(text: &omen::TextSnapshot, cursor: usize) -> Option<String> {
    let row = text.byte_to_line(cursor);
    let line_start = text.line_start(row);
    let line = text.line(row);
    let local = cursor.saturating_sub(line_start).min(line.len());
    let mut start = local;
    while start > 0 {
        let Some(ch) = line[..start].chars().next_back() else {
            break;
        };
        if !is_word_char(ch) {
            break;
        }
        start -= ch.len_utf8();
    }
    let mut end = local;
    while end < line.len() {
        let Some(ch) = line[end..].chars().next() else {
            break;
        };
        if !is_word_char(ch) {
            break;
        }
        end += ch.len_utf8();
    }
    (start < end).then(|| line[start..end].to_string())
}

fn collect_find_matches(
    source: &str,
    needle: &str,
    case_sensitive: bool,
    whole_word: bool,
) -> Vec<std::ops::Range<usize>> {
    if needle.is_empty() || source.is_empty() {
        return Vec::new();
    }
    let raw_ranges: Vec<std::ops::Range<usize>> = if case_sensitive {
        source
            .match_indices(needle)
            .map(|(start, _)| start..start + needle.len())
            .collect()
    } else if source.is_ascii() && needle.is_ascii() {
        let lower_source = source.to_ascii_lowercase();
        let lower_needle = needle.to_ascii_lowercase();
        lower_source
            .match_indices(&lower_needle)
            .map(|(start, _)| start..start + lower_needle.len())
            .collect()
    } else {
        let lower_needle = needle.to_lowercase();
        let mut lower_source = String::with_capacity(source.len());
        let mut byte_map = Vec::with_capacity(source.len() + 1);
        for (orig_start, ch) in source.char_indices() {
            for lower_ch in ch.to_lowercase() {
                let mut buf = [0u8; 4];
                let encoded = lower_ch.encode_utf8(&mut buf);
                for _ in 0..encoded.len() {
                    byte_map.push(orig_start);
                }
                lower_source.push(lower_ch);
            }
        }
        byte_map.push(source.len());
        lower_source
            .match_indices(&lower_needle)
            .filter_map(|(start, _)| {
                let orig_start = *byte_map.get(start)?;
                let orig_end = *byte_map.get(start + lower_needle.len())?;
                (orig_start < orig_end).then_some(orig_start..orig_end)
            })
            .collect()
    };
    raw_ranges
        .into_iter()
        .filter(|range| !whole_word || is_word_boundary(source, range.start, range.end))
        .collect()
}
