use super::StudioEnvironment;
use omen::{EditorEvent, EditorSnippet};
impl StudioEnvironment {
    pub fn open_component_library(&self) {
        let Some(path) = self.workspace.get().active_file else {
            return;
        };
        let state = self.editor_document(path.clone()).state.get();
        let cursor = state.selection.end;
        self.library_items.set(crate::editor::library::items(
            &state.text.to_string(),
            cursor,
        ));
        *self.library_target.borrow_mut() = Some((path, state.text.identity(), cursor));
        self.library_query.set(String::new());
        self.library_index.set(0);
        self.library_message.set(String::new());
        self.library_visible.set(true);
    }
    pub fn library_move(&self, offset: isize) {
        let query = self.library_query.get();
        let count = self
            .library_items
            .get()
            .iter()
            .filter(|item| item.matches(&query))
            .count();
        self.library_index.update(|index| {
            *index = index
                .saturating_add_signed(offset)
                .min(count.saturating_sub(1))
        });
    }
    pub fn insert_library_selection(&self) {
        let query = self.library_query.get();
        let Some(item) = self
            .library_items
            .get()
            .into_iter()
            .filter(|item| item.matches(&query))
            .nth(self.library_index.get())
        else {
            return;
        };
        let Some((path, identity, cursor)) = self.library_target.borrow().clone() else {
            return;
        };
        let document = self.editor_document(path.clone());
        let state = document.state.get();
        if self.workspace.get().active_file.as_ref() != Some(&path)
            || state.text.identity() != identity
        {
            self.library_message.set(
                "The document changed. Close and reopen the library at the desired cursor.".into(),
            );
            return;
        }
        let mut text = state.text;
        text.replace(cursor..cursor, &item.snippet.insert);
        let stops: Vec<_> = item
            .snippet
            .placeholders
            .iter()
            .map(|range| cursor + range.start..cursor + range.end)
            .collect();
        let caret = stops
            .first()
            .cloned()
            .unwrap_or(cursor + item.snippet.insert.len()..cursor + item.snippet.insert.len());
        document.state.update(|state| {
            state.text = text;
            state.selection = caret.clone();
            state.reveal = Some(caret);
            state.snippet_request = Some(EditorSnippet { stops, active: 0 });
            state.completions = Default::default();
        });
        self.library_visible.set(false);
        document.emit(EditorEvent::Edited);
    }
}
