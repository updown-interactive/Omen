// Native text editing follows GPUI's input example at the pinned Zed revision.
// Upstream: https://github.com/zed-industries/zed/blob/faf8f0b6980205ca2659af174790a212ae3c416a/crates/gpui/examples/input.rs
// Copyright Zed Industries. Licensed under Apache-2.0; see LICENSE-APACHE.
use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Focusable, GlobalElementId, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ShapedLine,
    SharedString, TextRun, UTF16Selection, UnderlineStyle, Window, actions, div, fill, point,
    prelude::*, px, relative, size,
};
use std::ops::Range;
mod commands;
mod hover;
mod multicursor;
mod multiline;
mod popups;
mod scroll;
mod syntax;
use unicode_segmentation::UnicodeSegmentation;

// The library test binary can also link the normal adapter through the desktop UI
// feature. Distinct namespaces prevent duplicate inventory registrations.
#[cfg(not(test))]
actions!(
    omen_native_controls,
    [
        FocusNext,
        FocusPrevious,
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
    ]
);
#[cfg(test)]
actions!(
    omen_native_controls_test,
    [
        FocusNext,
        FocusPrevious,
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
    ]
);

pub(crate) struct TextInput {
    pub(crate) multiline: bool,
    last_lines: Vec<(usize, ShapedLine)>,
    last_line_style: Option<multiline::LineCacheKey>,
    first_visible_line: usize,
    editor_line_height: Pixels,
    scroll_y: Pixels,
    reveal_cursor: bool,
    undo: Vec<(omen_core::TextSnapshot, Range<usize>, Vec<Range<usize>>)>,
    redo: Vec<(omen_core::TextSnapshot, Range<usize>, Vec<Range<usize>>)>,
    snippet: Option<omen_core::EditorSnippet>,
    syntax_edits: Vec<syntax::SyntaxEdit>,
    pub(crate) focus_handle: FocusHandle,
    blur_subscription: Option<gpui::Subscription>,
    content: omen_core::TextSnapshot,
    pub(crate) code_document: Option<omen_core::EditorDocument>,
    code_initialized: bool,
    history_epoch: u64,
    pub(crate) autofocus_applied: bool,
    placeholder: SharedString,
    selected_range: Range<usize>,
    secondary_selections: Vec<Range<usize>>,
    cursor_chord: bool,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    context_position: Option<Point<Pixels>>,
    context_index: usize,
    scrollbar_drag: bool,
    hover_position: Option<Point<Pixels>>,
    hover_task: Option<gpui::Task<()>>,

    value: omen_core::Binding<String>,
    secure: bool,
    on_change: Option<omen_core::Action>,
    label: String,
    hint: Option<String>,
    pub(crate) rtl: bool,
    pub(crate) accent: gpui::Hsla,
    pub(crate) background: gpui::Hsla,
    pub(crate) max_content_width: Pixels,
    scroll: Pixels,
}

impl TextInput {
    pub(crate) fn display_snapshot(&self) -> String {
        let value = self.value.get();
        if self.secure {
            "•".repeat(value.chars().count())
        } else if value.is_empty() {
            self.label.clone()
        } else {
            value
        }
    }
    pub(crate) fn configure(
        &mut self,
        label: String,
        value: omen_core::Binding<String>,
        on_change: Option<omen_core::Action>,
        hint: Option<String>,
    ) {
        self.placeholder = label.clone().into();
        self.label = label;
        self.value = value;
        self.on_change = on_change;
        self.hint = hint;
        self.max_content_width = px(0.);
    }
    pub(crate) fn new(
        label: String,
        value: impl Into<omen_core::Binding<String>>,
        secure: bool,
        on_change: Option<omen_core::Action>,
        hint: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        let value = value.into();
        let content = omen_core::TextSnapshot::from(value.get());
        let end = content.len();
        Self {
            multiline: false,
            last_lines: Vec::new(),
            last_line_style: None,
            first_visible_line: 0,
            editor_line_height: px(20.),
            scroll_y: px(0.),
            reveal_cursor: true,
            undo: Vec::new(),
            redo: Vec::new(),
            snippet: None,
            syntax_edits: Vec::new(),
            focus_handle: cx.focus_handle().tab_stop(true).tab_index(0),
            blur_subscription: None,
            placeholder: label.clone().into(),
            label,
            content,
            code_document: None,
            code_initialized: false,
            history_epoch: 0,
            autofocus_applied: false,
            selected_range: end..end,
            secondary_selections: Vec::new(),
            cursor_chord: false,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
            context_position: None,
            context_index: 0,
            scrollbar_drag: false,
            hover_position: None,
            hover_task: None,

            value,
            secure,
            on_change,
            hint,
            rtl: false,
            accent: gpui::rgb(0x2563eb).into(),
            background: gpui::hsla(0., 0., 0.1, 1.),
            max_content_width: px(0.),
            scroll: px(0.),
        }
    }
    fn display_offset(&self, offset: usize) -> usize {
        if self.secure {
            self.content.slice(..offset).chars().count() * "•".len()
        } else {
            offset
        }
    }
    fn content_offset(&self, offset: usize) -> usize {
        if self.secure {
            self.content
                .slice(..)
                .char_indices()
                .nth(offset / "•".len())
                .map_or(self.content.len(), |(index, _)| index)
        } else {
            offset
        }
    }
    fn replace_content(&mut self, range: Range<usize>, text: &str) {
        if self.code_document.is_some() {
            self.syntax_edits.push(syntax::SyntaxEdit {
                touching: !text.chars().all(char::is_whitespace)
                    || (!range.is_empty()
                        && text.is_empty()
                        && self
                            .content
                            .slice(range.start.saturating_sub(4)..range.start)
                            .chars()
                            .next_back()
                            .is_some_and(syntax::identifier_char)
                        && self
                            .content
                            .slice(range.end..range.end.saturating_add(4))
                            .chars()
                            .next()
                            .is_some_and(syntax::identifier_char)),
                range: range.clone(),
                inserted: text.len(),
            });
        }
        self.content.replace(range, text);
    }
    fn changed(&mut self) {
        self.reveal_cursor = true;
        if let Some(document) = &self.code_document {
            document.state.update(|state| {
                if !self.syntax_edits.is_empty()
                    && !state.highlights.is_empty()
                    && state
                        .highlight_identity
                        .is_none_or(|identity| identity == state.text.identity())
                {
                    syntax::rebase_highlights(
                        std::sync::Arc::make_mut(&mut state.highlights),
                        &self.syntax_edits,
                    );
                    state.highlight_identity = Some(self.content.identity());
                } else {
                    state.highlights = Default::default();
                    state.highlight_identity = None;
                }
                state.text = self.content.clone();
                state.selection = self.selected_range.clone();
                state.secondary_selections = self.secondary_selections.clone();
                state.completions = Default::default();
                state.diagnostics = Default::default();
                state.hover = None;
                state.hover_offset = None;
                state.actions = Default::default();
            });
            document.emit(omen_core::EditorEvent::Edited);
        } else {
            self.value.set(self.content.to_string());
        }
        self.syntax_edits.clear();
        if let Some(action) = &self.on_change {
            action.invoke();
        }
    }

    fn left(&mut self, _: &Left, window: &mut Window, cx: &mut Context<Self>) {
        if self.multi_motion("left", false, window, cx) {
            return;
        }
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), window, cx);
        } else {
            self.move_to(self.selected_range.start, window, cx)
        }
    }

    fn right(&mut self, _: &Right, window: &mut Window, cx: &mut Context<Self>) {
        if self.multi_motion("right", false, window, cx) {
            return;
        }
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), window, cx);
        } else {
            self.move_to(self.selected_range.end, window, cx)
        }
    }

    fn select_left(&mut self, _: &SelectLeft, window: &mut Window, cx: &mut Context<Self>) {
        if self.multi_motion("left", true, window, cx) {
            return;
        }
        self.select_to(self.previous_boundary(self.cursor_offset()), window, cx);
    }

    fn select_right(&mut self, _: &SelectRight, window: &mut Window, cx: &mut Context<Self>) {
        if self.multi_motion("right", true, window, cx) {
            return;
        }
        self.select_to(self.next_boundary(self.cursor_offset()), window, cx);
    }

    fn select_all(&mut self, _: &SelectAll, window: &mut Window, cx: &mut Context<Self>) {
        self.secondary_selections.clear();
        self.move_to(0, window, cx);
        self.select_to(self.content.len(), window, cx)
    }

    fn home(&mut self, _: &Home, window: &mut Window, cx: &mut Context<Self>) {
        if self.multi_motion("home", false, window, cx) {
            return;
        }
        self.move_to(
            if self.multiline {
                self.line_start(self.cursor_offset())
            } else {
                0
            },
            window,
            cx,
        );
    }

    fn end(&mut self, _: &End, window: &mut Window, cx: &mut Context<Self>) {
        if self.multi_motion("end", false, window, cx) {
            return;
        }
        self.move_to(
            if self.multiline {
                self.line_end(self.cursor_offset())
            } else {
                self.content.len()
            },
            window,
            cx,
        );
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if !self.secondary_selections.is_empty() {
            self.delete_cursors(false, false, false, window, cx);
            return;
        }
        if self.code_document.is_some() && self.selected_range.is_empty() {
            let offset = self.cursor_offset();
            let prev = self.previous_boundary(offset);
            let next = self.next_boundary(offset);
            let pair = format!(
                "{}{}",
                self.content.slice(prev..offset),
                self.content.slice(offset..next)
            );
            if matches!(pair.as_str(), "()" | "[]" | "{}") {
                self.replace_text_in_range(
                    Some(self.range_to_utf16(&(prev..next))),
                    "",
                    window,
                    cx,
                );
                return;
            }
        }
        if self.selected_range.is_empty() {
            let prev = self.previous_boundary(self.cursor_offset());
            if self.cursor_offset() == prev {
                window.play_system_bell();
                return;
            }
            self.select_to(prev, window, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if !self.secondary_selections.is_empty() {
            self.delete_cursors(true, false, false, window, cx);
            return;
        }
        if self.selected_range.is_empty() {
            let next = self.next_boundary(self.cursor_offset());
            if self.cursor_offset() == next {
                window.play_system_bell();
                return;
            }
            self.select_to(next, window, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        self.focus_handle.focus(window, cx);
        self.dismiss_popups();
        self.is_selecting = true;
        window.request_virtual_keyboard();

        let offset = self.index_for_mouse_position(event.position);
        if self.code_document.is_some() && event.modifiers.alt && !event.modifiers.shift {
            self.add_cursor(offset..offset, window, cx);
            self.is_selecting = false;
            return;
        }
        self.secondary_selections.clear();
        if event.click_count >= 3
            || (self.multiline
                && self
                    .last_bounds
                    .is_some_and(|bounds| event.position.x < bounds.left() + self.gutter_width()))
        {
            let row = self.content.byte_to_line(offset);
            self.move_to(self.content.line_start(row), window, cx);
            self.select_to(self.content.line_start(row + 1), window, cx);
        } else if event.click_count == 2 {
            let range = self.word_range(offset);
            self.move_to(range.start, window, cx);
            self.select_to(range.end, window, cx);
        } else if event.modifiers.shift {
            self.select_to(offset, window, cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), window, cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
        self.scrollbar_drag = false;
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.scrollbar_drag {
            if event.dragging() {
                self.scroll_to_pointer(event.position, cx);
            } else {
                self.scrollbar_drag = false;
            }
            return;
        }
        // A release can be missed when a reactive update replaces the hit target
        // or the pointer leaves the window. Trust the platform button state too.
        if !event.dragging() {
            self.is_selecting = false;
            self.pointer_hover(event, cx);
            return;
        }
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), window, cx);
        }
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            if !self.secondary_selections.is_empty() {
                self.paste_cursors(&text, window, cx);
                return;
            }
            self.replace_text_in_range(None, &text, window, cx);
        }
    }

    fn clipboard_range(&self) -> Range<usize> {
        if self.code_document.is_some() && self.selected_range.is_empty() {
            let row = self.content.byte_to_line(self.cursor_offset());
            self.content.line_start(row)..self.content.line_start(row + 1)
        } else {
            self.selected_range.clone()
        }
    }
    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.secondary_selections.is_empty() && !self.secure {
            cx.write_to_clipboard(ClipboardItem::new_string(self.multi_clipboard()));
            return;
        }
        let range = self.clipboard_range();
        if !self.secure && !range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content.slice(range).to_string(),
            ));
        }
    }
    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.secondary_selections.is_empty() && !self.secure {
            cx.write_to_clipboard(ClipboardItem::new_string(self.multi_clipboard()));
            self.cut_cursors(window, cx);
            return;
        }
        let range = self.clipboard_range();
        if !self.secure && !range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content.slice(range.clone()).to_string(),
            ));
            self.replace_text_in_range(Some(self.range_to_utf16(&range)), "", window, cx)
        }
    }

    fn move_to(&mut self, offset: usize, window: &Window, cx: &mut Context<Self>) {
        self.reveal_cursor = true;
        self.secondary_selections.clear();
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        if let Some(document) = &self.code_document {
            document.state.update(|state| {
                state.selection = self.selected_range.clone();
                state.secondary_selections = self.secondary_selections.clone();
                state.completions = Default::default();
                state.hover = None;
                state.hover_offset = None;
                state.actions = Default::default();
            });
            document.emit(omen_core::EditorEvent::Selection);
        }
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.multiline {
            return self.editor_index(position);
        }
        if self.content.is_empty() {
            return 0;
        }

        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        self.content_offset(line.closest_index_for_x(position.x - bounds.left() + self.scroll))
    }

    fn select_to(&mut self, offset: usize, window: &Window, cx: &mut Context<Self>) {
        self.reveal_cursor = true;
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        if let Some(document) = &self.code_document {
            document.state.update(|state| {
                state.selection = self.selected_range.clone();
                state.secondary_selections = self.secondary_selections.clone();
                state.completions = Default::default();
                state.hover = None;
                state.hover_offset = None;
                state.actions = Default::default();
            });
            document.emit(omen_core::EditorEvent::Selection);
        }
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify()
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        self.content.utf16_to_byte(offset)
    }
    fn offset_to_utf16(&self, offset: usize) -> usize {
        self.content.byte_to_utf16(offset)
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        if offset == 0 {
            return 0;
        }
        let line = self.content.byte_to_line(offset);
        let start = self.content.line_start(line);
        if offset == start {
            let previous = line.saturating_sub(1);
            return self.content.line_start(previous) + self.content.line(previous).len();
        }
        start
            + self
                .content
                .slice(start..offset)
                .grapheme_indices(true)
                .next_back()
                .map_or(0, |(index, _)| index)
    }
    fn next_boundary(&self, offset: usize) -> usize {
        if offset >= self.content.len() {
            return self.content.len();
        }
        let line = self.content.byte_to_line(offset);
        let end = self.content.line_start(line + 1);
        offset
            + self
                .content
                .slice(offset..end)
                .graphemes(true)
                .next()
                .map_or(0, str::len)
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(if self.secure {
            "•".repeat(self.content.slice(range).encode_utf16().count())
        } else {
            self.content.slice(range).to_string()
        })
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_range = self.range_from_utf16(&range_utf16);
        self.selection_reversed = false;
        if let Some(document) = &self.code_document {
            document.state.update(|state| {
                state.selection = self.selected_range.clone();
                state.secondary_selections = self.secondary_selections.clone();
                state.completions = Default::default();
                state.hover = None;
                state.hover_offset = None;
                state.actions = Default::default();
            });
            document.emit(omen_core::EditorEvent::Selection);
        }
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
    }

    fn text_length_utf16(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.content.len_utf16())
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.marked_range = None;
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.secondary_selections.is_empty()
            && range_utf16.is_none()
            && self.marked_range.is_none()
        {
            let values = vec![new_text.to_string(); self.secondary_selections.len() + 1];
            self.replace_cursors(&values, window, cx);
            return;
        }
        if range_utf16.is_some() {
            self.secondary_selections.clear();
        }
        self.checkpoint();
        let normalized = if self.multiline {
            new_text.replace("\r\n", "\n").replace('\r', "\n")
        } else {
            new_text.replace(['\n', '\r'], " ")
        };
        let new_text = normalized.as_str();
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        if self
            .snippet
            .as_mut()
            .is_some_and(|snippet| !snippet.edit(&range, new_text.len()))
        {
            self.snippet = None;
        }
        self.replace_content(range.clone(), new_text);
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.selection_reversed = false;
        self.marked_range.take();
        self.changed();
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.secondary_selections.clear();
        self.checkpoint();
        let normalized = if self.multiline {
            new_text.replace("\r\n", "\n").replace('\r', "\n")
        } else {
            new_text.replace(['\n', '\r'], " ")
        };
        let new_text = normalized.as_str();
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        if self
            .snippet
            .as_mut()
            .is_some_and(|snippet| !snippet.edit(&range, new_text.len()))
        {
            self.snippet = None;
        }
        self.replace_content(range.clone(), new_text);
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        let insertion_start_utf16 = self.offset_to_utf16(range.start);
        let inserted_length_utf16 = new_text.encode_utf16().count();
        self.selected_range = new_selected_range_utf16
            .map(|selected| {
                self.range_from_utf16(
                    &(insertion_start_utf16 + selected.start.min(inserted_length_utf16)
                        ..insertion_start_utf16 + selected.end.min(inserted_length_utf16)),
                )
            })
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        self.selection_reversed = false;

        self.changed();
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        if self.multiline {
            return self
                .editor_bounds(self.range_from_utf16(&range_utf16), bounds)
                .into_iter()
                .next();
        }
        let last_layout = self.last_layout.as_ref()?;
        if !self.secure && last_layout.text.as_str() != self.content.slice(..).as_ref() {
            return None;
        }
        let range = self.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout.x_for_index(self.display_offset(range.start))
                    - self.scroll,
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout.x_for_index(self.display_offset(range.end))
                    - self.scroll,
                bounds.bottom(),
            ),
        ))
    }

    fn selection_bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Bounds<Pixels>> {
        if self.multiline {
            return self.editor_bounds(self.range_from_utf16(&range_utf16), bounds);
        }
        self.bounds_for_range(range_utf16, bounds, window, cx)
            .into_iter()
            .collect()
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        if self.multiline {
            return Some(self.offset_to_utf16(self.editor_index(point)));
        }
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;

        if !self.secure && last_layout.text.as_str() != self.content.slice(..).as_ref() {
            return None;
        }
        let utf8_index = last_layout.closest_index_for_x(line_point.x + self.scroll);
        Some(self.offset_to_utf16(self.content_offset(utf8_index)))
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    lines: Option<Vec<(usize, ShapedLine)>>,
    selections: Vec<PaintQuad>,
    scroll_y: Pixels,
    first_visible_line: usize,
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
    scroll: Pixels,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = gpui::Style::default();
        style.size.width = relative(1.).into();
        style.size.height = if self.input.read(cx).multiline {
            relative(1.).into()
        } else {
            window.line_height().into()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        if input.multiline {
            return multiline::prepaint(input, bounds, window);
        }
        let content = SharedString::from(input.content.to_string());
        let selected_range = input.selected_range.clone();
        let cursor = input.cursor_offset();
        let style = window.text_style();

        let (display_text, text_color) = if content.is_empty() {
            (input.placeholder.clone(), style.color.opacity(0.55))
        } else {
            (
                if input.secure {
                    SharedString::from("•".repeat(content.chars().count()))
                } else {
                    content
                },
                style.color,
            )
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(marked_range) = input.marked_range.as_ref().filter(|_| !input.secure)
        {
            vec![
                TextRun {
                    len: marked_range.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked_range.end - marked_range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len() - marked_range.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        let cursor_pos = line.x_for_index(input.display_offset(cursor));
        let scroll = if input.rtl && line.width < bounds.size.width {
            line.width - bounds.size.width
        } else if cursor_pos < input.scroll {
            cursor_pos
        } else {
            input
                .scroll
                .max(cursor_pos - bounds.size.width + px(2.))
                .max(px(0.))
        };
        let (selection, cursor) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_pos - scroll, bounds.top()),
                        size(px(2.), bounds.bottom() - bounds.top()),
                    ),
                    input.accent,
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left()
                                + line.x_for_index(input.display_offset(selected_range.start))
                                - scroll,
                            bounds.top(),
                        ),
                        point(
                            bounds.left()
                                + line.x_for_index(input.display_offset(selected_range.end))
                                - scroll,
                            bounds.bottom(),
                        ),
                    ),
                    input.accent.opacity(0.25),
                )),
                None,
            )
        };
        PrepaintState {
            lines: None,
            selections: Vec::new(),
            scroll_y: px(0.),
            first_visible_line: 0,
            line: Some(line),
            cursor,
            selection,
            scroll,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        if let Some(lines) = prepaint.lines.take() {
            let input = self.input.read(cx);
            let gutter = input.gutter_width();
            let gutter_bg = input.background;
            let text_left = bounds.left() + gutter;
            let text_width = (bounds.size.width - gutter).max(px(1.));

            if input.code_document.is_some() && input.selected_range.is_empty() {
                let row = input.content.byte_to_line(input.cursor_offset());
                let y = bounds.top() + window.line_height() * row as f32 - prepaint.scroll_y;
                if y >= bounds.top() && y < bounds.bottom() {
                    window.paint_quad(fill(
                        Bounds::new(point(text_left, y), size(text_width, window.line_height())),
                        window.text_style().color.opacity(0.035),
                    ));
                }
            }
            for selection in prepaint.selections.drain(..) {
                window.paint_quad(selection);
            }
            for (index, (_, line)) in lines.iter().enumerate() {
                let y = window.line_height() * (prepaint.first_visible_line + index) as f32
                    - prepaint.scroll_y;
                let _ = line.paint(
                    bounds.origin + point(gutter - prepaint.scroll, y),
                    window.line_height(),
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            if focus_handle.is_focused(window)
                && let Some(cursor) = prepaint.cursor.take()
            {
                window.paint_quad(cursor);
            }
            if gutter > px(0.) {
                // Fixed line number gutter with solid opaque body to prevent scrolled text from showing through
                window.paint_quad(fill(
                    Bounds::new(bounds.origin, size(gutter, bounds.size.height)),
                    gutter_bg,
                ));
                // Subtle vertical divider between gutter and editor text
                window.paint_quad(fill(
                    Bounds::new(
                        point(bounds.left() + gutter - px(1.), bounds.top()),
                        size(px(1.), bounds.size.height),
                    ),
                    window.text_style().color.opacity(0.08),
                ));
                let style = window.text_style();
                let font_size = style.font_size.to_pixels(window.rem_size());
                for (index, _) in lines.iter().enumerate() {
                    let y = window.line_height() * (prepaint.first_visible_line + index) as f32
                        - prepaint.scroll_y;
                    let number: SharedString =
                        (prepaint.first_visible_line + index + 1).to_string().into();
                    let run = TextRun {
                        len: number.len(),
                        font: style.font(),
                        color: style.color.opacity(0.45),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let number = window
                        .text_system()
                        .shape_line(number, font_size, &[run], None);
                    let _ = number.paint(
                        bounds.origin + point((gutter - px(12.) - number.width).max(px(0.)), y),
                        window.line_height(),
                        gpui::TextAlign::Left,
                        None,
                        window,
                        cx,
                    );
                }
            }
            let line_height = window.line_height();
            let shape_style = multiline::style_key(self.input.read(cx), window);
            let visible_max = lines
                .iter()
                .map(|(_, line)| line.width)
                .max()
                .unwrap_or(px(0.));
            self.input.update(cx, |input, _| {
                input.max_content_width = input.max_content_width.max(visible_max);
                input.last_line_style = Some(shape_style);
                input.last_lines = lines;
                input.first_visible_line = prepaint.first_visible_line;
                input.editor_line_height = line_height;
                input.last_bounds = Some(bounds);
                input.scroll = prepaint.scroll;
                input.scroll_y = prepaint.scroll_y;
                input.reveal_cursor = false;
            });
            return;
        }
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection)
        }
        let Some(line) = prepaint.line.take() else {
            return;
        };
        let _ = line.paint(
            bounds.origin - point(prepaint.scroll, px(0.)),
            window.line_height(),
            gpui::TextAlign::Left,
            None,
            window,
            cx,
        );

        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
            input.scroll = prepaint.scroll;
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(document) = self.code_document.clone() {
            if self.blur_subscription.is_none() {
                self.blur_subscription =
                    Some(cx.on_blur(&self.focus_handle, window, |input, _, _| {
                        if let Some(document) = &input.code_document {
                            document.emit(omen_core::EditorEvent::Blur);
                        }
                    }));
            }
            let state = document.state.get();
            if state.history_epoch != self.history_epoch {
                self.snippet = None;
                self.undo.clear();
                self.redo.clear();
                self.code_initialized = false;
                self.history_epoch = state.history_epoch;
            }
            if !self.content.same_snapshot(&state.text) {
                if self.code_initialized {
                    self.checkpoint();
                }
                self.snippet = None;
                self.secondary_selections.clear();
                if !state.secondary_selections.is_empty() {
                    document
                        .state
                        .update(|state| state.secondary_selections.clear());
                }
                self.content = state.text;
                self.selected_range = state.selection.start.min(self.content.len())
                    ..state.selection.end.min(self.content.len());
                self.marked_range = None;
                self.reveal_cursor = true;
            }
            self.code_initialized = true;
            if let Some(snippet) = state.snippet_request {
                self.snippet = Some(snippet);
                document.state.update(|state| state.snippet_request = None);
            }
            if let Some(range) = state.selection_request {
                self.secondary_selections.clear();
                self.selected_range =
                    range.start.min(self.content.len())..range.end.min(self.content.len());
                self.reveal_cursor = true;
                document
                    .state
                    .update(|state| state.selection_request = None);
            }
            if let Some(range) = state.reveal {
                self.secondary_selections.clear();
                self.focus_handle.focus(window, cx);
                self.selected_range =
                    range.start.min(self.content.len())..range.end.min(self.content.len());
                self.reveal_cursor = true;
                document.state.update(|state| state.reveal = None);
            }
        } else {
            let shared = self.value.get();
            if self.content != shared {
                self.content = shared.into();
                self.selected_range = self.content.len()..self.content.len();
                self.marked_range = None;
                self.undo.clear();
                self.redo.clear();
                self.reveal_cursor = true;
            }
        }
        let input = cx.entity();
        let focus = self.focus_handle.clone();
        let accent = self.accent;
        div()
            .id("input")
            .on_a11y_action(gpui::AccessibleAction::Focus, move |_, window, cx| {
                focus.focus(window, cx)
            })
            .on_a11y_action(gpui::AccessibleAction::SetValue, move |data, window, cx| {
                if let Some(gpui::accesskit::ActionData::Value(text)) = data {
                    input.update(cx, |input, cx| {
                        let end = input.content.len_utf16();
                        input.replace_text_in_range(Some(0..end), text, window, cx);
                    });
                }
            })
            .w_full()
            .relative()
            .when(self.multiline, |element| element.h_full().min_h_0())
            .min_w(px(120.))
            .overflow_hidden()
            .key_context("OmenTextInput")
            .track_focus(&self.focus_handle)
            .tab_index(0)
            .role(if self.secure {
                gpui::Role::PasswordInput
            } else {
                gpui::Role::TextInput
            })
            .aria_label(self.label.clone())
            .aria_description(self.hint.clone().unwrap_or_default())
            .aria_placeholder(self.placeholder.clone())
            .aria_value(if self.secure {
                "•".repeat(self.content.len_utf16())
            } else {
                if self.code_document.is_some() {
                    self.content
                        .line(self.content.byte_to_line(self.cursor_offset()))
                        .into_owned()
                } else {
                    self.content.to_string()
                }
            })
            .cursor(CursorStyle::IBeam)
            .on_key_down(cx.listener(Self::editor_key))
            .when(self.multiline, |element| {
                element
                    .on_action(cx.listener(|input, _: &FocusNext, window, cx| {
                        input.indent(false, window, cx);
                        cx.stop_propagation();
                    }))
                    .on_action(cx.listener(|input, _: &FocusPrevious, window, cx| {
                        input.indent(true, window, cx);
                        cx.stop_propagation();
                    }))
            })
            .on_scroll(cx.listener(|input, event: &gpui::ScrollEvent, window, cx| {
                if !input.multiline {
                    return;
                }
                let delta = event.delta.pixel_delta(window.line_height());
                input.dismiss_popups();
                input.scroll_y = (input.scroll_y - delta.y).max(px(0.));
                let text_width = input
                    .last_bounds
                    .map(|b| (b.size.width - input.gutter_width()).max(px(1.)))
                    .unwrap_or(px(100.));
                let max_scroll = (input.max_content_width + px(48.) - text_width).max(px(0.));
                input.scroll = (input.scroll - delta.x).max(px(0.)).min(max_scroll);
                input.reveal_cursor = false;
                cx.notify();
                cx.stop_propagation();
            }))
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_click(|_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .when(self.code_document.is_none(), |element| {
                element.focus_visible(move |style| style.border_b_2().border_color(accent))
            })
            .on_mouse_down(MouseButton::Right, cx.listener(Self::open_context_menu))
            .child(TextElement { input: cx.entity() })
            .children(self.editor_popups(window, cx))
            .children(self.editor_scrollbar(cx))
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

struct BindingsInstalled;
impl gpui::Global for BindingsInstalled {}
pub(crate) fn initialize(cx: &mut App) {
    if cx.has_global::<BindingsInstalled>() {
        return;
    }
    cx.set_global(BindingsInstalled);
    cx.bind_keys([
        gpui::KeyBinding::new("tab", FocusNext, Some("Omen")),
        gpui::KeyBinding::new("shift-tab", FocusPrevious, Some("Omen")),
        gpui::KeyBinding::new("backspace", Backspace, Some("OmenTextInput")),
        gpui::KeyBinding::new("delete", Delete, Some("OmenTextInput")),
        gpui::KeyBinding::new("left", Left, Some("OmenTextInput")),
        gpui::KeyBinding::new("right", Right, Some("OmenTextInput")),
        gpui::KeyBinding::new("shift-left", SelectLeft, Some("OmenTextInput")),
        gpui::KeyBinding::new("shift-right", SelectRight, Some("OmenTextInput")),
        gpui::KeyBinding::new("home", Home, Some("OmenTextInput")),
        gpui::KeyBinding::new("end", End, Some("OmenTextInput")),
        gpui::KeyBinding::new("cmd-a", SelectAll, Some("OmenTextInput")),
        gpui::KeyBinding::new("ctrl-a", SelectAll, Some("OmenTextInput")),
        gpui::KeyBinding::new("cmd-c", Copy, Some("OmenTextInput")),
        gpui::KeyBinding::new("ctrl-c", Copy, Some("OmenTextInput")),
        gpui::KeyBinding::new("cmd-x", Cut, Some("OmenTextInput")),
        gpui::KeyBinding::new("ctrl-x", Cut, Some("OmenTextInput")),
        gpui::KeyBinding::new("cmd-v", Paste, Some("OmenTextInput")),
        gpui::KeyBinding::new("ctrl-v", Paste, Some("OmenTextInput")),
    ]);
}

pub(crate) fn keyboard_root(element: gpui::Stateful<gpui::Div>) -> gpui::Stateful<gpui::Div> {
    element
        .tab_group()
        .key_context("Omen")
        .on_action(|_: &FocusNext, window, cx| {
            window.focus_next(cx);
            cx.stop_propagation();
        })
        .on_action(|_: &FocusPrevious, window, cx| {
            window.focus_prev(cx);
            cx.stop_propagation();
        })
}

/// Trap sequential navigation within a nonmodal pane, retaining normal control key handling.
pub(crate) fn keyboard_scope(
    element: gpui::Stateful<gpui::Div>,
    scope: FocusHandle,
    arrows: bool,
) -> gpui::Stateful<gpui::Div> {
    let forward = scope.clone();
    let backward = scope.clone();
    let element = element
        .on_action(move |_: &FocusNext, window, cx| {
            move_scope_focus(&forward, false, window, cx);
            cx.stop_propagation();
        })
        .on_action(move |_: &FocusPrevious, window, cx| {
            move_scope_focus(&backward, true, window, cx);
            cx.stop_propagation();
        });
    if arrows {
        element.on_key_down(move |event, window, cx| {
            if event.keystroke.modifiers == gpui::Modifiers::default()
                && matches!(event.keystroke.key.as_str(), "up" | "down")
            {
                move_scope_focus(&scope, event.keystroke.key == "up", window, cx);
                cx.stop_propagation();
            }
        })
    } else {
        element
    }
}
fn move_scope_focus(scope: &FocusHandle, backward: bool, window: &mut Window, cx: &mut App) {
    if backward {
        window.focus_prev(cx);
    } else {
        window.focus_next(cx);
    }
    if scope.contains_focused(window, cx) {
        return;
    }
    scope.focus(window, cx);
    window.focus_next(cx);
    if backward {
        let first = window.focused(cx);
        let mut last = first.clone();
        for _ in 0..256 {
            window.focus_next(cx);
            let next = window.focused(cx);
            if !scope.contains_focused(window, cx) || next == first {
                break;
            }
            last = next;
        }
        if let Some(last) = last {
            last.focus(window, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use omen_core::TextValue;

    #[gpui::test]
    fn hovering_after_a_missed_release_does_not_extend_selection(cx: &mut TestAppContext) {
        let (input, vcx) = cx.add_window_view(|_, cx| {
            TextInput::new(
                "Code".into(),
                TextValue::new("let value = 42;".into()),
                false,
                None,
                None,
                cx,
            )
        });
        vcx.run_until_parked();
        vcx.update(|window, cx| {
            input.update(cx, |input, cx| {
                let bounds = input.last_bounds.expect("rendered text bounds");
                let start = bounds.origin;
                let end = point(bounds.right(), bounds.top());
                input.on_mouse_down(
                    &MouseDownEvent {
                        position: start,
                        button: MouseButton::Left,
                        ..Default::default()
                    },
                    window,
                    cx,
                );
                input.on_mouse_move(
                    &MouseMoveEvent {
                        position: end,
                        pressed_button: Some(MouseButton::Left),
                        ..Default::default()
                    },
                    window,
                    cx,
                );
                assert!(
                    !input.selected_range.is_empty(),
                    "a held button selects text"
                );
                let selected = input.selected_range.clone();
                // Deliberately omit MouseUp to reproduce a missed release.
                input.on_mouse_move(
                    &MouseMoveEvent {
                        position: start,
                        ..Default::default()
                    },
                    window,
                    cx,
                );
                assert_eq!(input.selected_range, selected);
                assert!(!input.is_selecting);
                input.on_mouse_move(
                    &MouseMoveEvent {
                        position: start,
                        pressed_button: Some(MouseButton::Left),
                        ..Default::default()
                    },
                    window,
                    cx,
                );
                assert_eq!(
                    input.selected_range, selected,
                    "a new drag must start in the editor"
                );
            });
        });
    }

    #[gpui::test]
    fn rope_code_input_preserves_ime_utf16_and_grapheme_boundaries(cx: &mut TestAppContext) {
        let model = omen_core::Value::new(omen_core::EditorState {
            text: "a👩‍💻z".into(),
            ..Default::default()
        });
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let input = cx.new(|cx| {
                let mut input = TextInput::new(
                    "Code".into(),
                    omen_core::TextValue::new(String::new()),
                    false,
                    None,
                    None,
                    cx,
                );
                input.multiline = true;
                input.content = model.get().text;
                input.code_document = Some(omen_core::EditorDocument::new(model.clone(), |_| {}));
                input
            });
            input.update(cx, |input, cx| {
                let original = input.content.clone();
                input.replace_and_mark_text_in_range(Some(1..6), "界", Some(1..1), window, cx);
                assert_eq!(model.get().text, "a界z");
                assert_eq!(input.marked_text_range(window, cx), Some(1..2));
                assert_eq!(original, "a👩‍💻z");
                input.unmark_text(window, cx);
                input.replace_text_in_range(Some(1..2), "👨‍👩‍👧‍👦", window, cx);
                input.move_to(input.content.len() - 1, window, cx);
                input.left(&Left, window, cx);
                assert_eq!(
                    input.selected_range,
                    1..1,
                    "left skips the whole family grapheme"
                );
            });
        });
    }
    #[gpui::test]
    fn large_code_document_shapes_only_visible_lines(cx: &mut TestAppContext) {
        let source = (0..100_000)
            .map(|row| format!("let item_{row} = {row};\n"))
            .collect::<String>();
        let model = omen_core::Value::new(omen_core::EditorState {
            text: source.into(),
            ..Default::default()
        });
        let (input, vcx) = cx.add_window_view(|_, cx| {
            let mut input = TextInput::new(
                "Large code".into(),
                omen_core::TextValue::new(String::new()),
                false,
                None,
                None,
                cx,
            );
            input.multiline = true;
            input.code_document = Some(omen_core::EditorDocument::new(model.clone(), |_| {}));
            input
        });
        vcx.run_until_parked();
        vcx.update(|_, cx| {
            assert!(input.read(cx).last_lines.len() < 100);
            input.update(cx, |input, cx| {
                input.scroll_y = px(1_600_000.);
                input.reveal_cursor = false;
                cx.notify();
            });
        });
        vcx.run_until_parked();
        vcx.update(|_, cx| {
            assert!(input.read(cx).first_visible_line > 50_000);
            assert!(input.read(cx).last_lines.len() < 100);
        });
    }

    #[gpui::test]
    fn ime_replacement_and_selection_use_utf16_boundaries(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let value = TextValue::new("a😀b".into());
        cx.update(|window, cx| {
            let input =
                cx.new(|cx| TextInput::new("Name".into(), value.clone(), false, None, None, cx));
            input.update(cx, |input, cx| {
                let mut actual = None;
                assert_eq!(
                    input
                        .text_for_range(1..3, &mut actual, window, cx)
                        .as_deref(),
                    Some("😀")
                );
                assert_eq!(actual, Some(1..3));
                input.replace_and_mark_text_in_range(Some(1..3), "界", Some(1..1), window, cx);
                assert_eq!(value.get(), "a界b");
                assert_eq!(input.marked_text_range(window, cx), Some(1..2));
                input.unmark_text(window, cx);
                input.replace_text_in_range(Some(1..2), "é", window, cx);
                assert_eq!(value.get(), "aéb");
                assert_eq!(
                    input.selected_text_range(false, window, cx).unwrap().range,
                    2..2
                );
            });
        });
    }

    #[gpui::test]
    fn secure_native_text_queries_return_only_masked_text(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let input = cx.new(|cx| {
                TextInput::new(
                    "Password".into(),
                    TextValue::new("a😀b".into()),
                    true,
                    None,
                    None,
                    cx,
                )
            });
            input.update(cx, |input, cx| {
                let mut actual = None;
                assert_eq!(
                    input
                        .text_for_range(0..4, &mut actual, window, cx)
                        .as_deref(),
                    Some("••••")
                );
                assert_eq!(actual, Some(0..4));
            });
        });
    }
}
