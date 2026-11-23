//! Visible-line shaping for the native plain-text editor.
use super::*;

pub(super) type LineCacheKey = (gpui::Font, Pixels, gpui::Hsla, usize, usize, usize, Pixels);
pub(super) fn style_key(input: &TextInput, window: &Window) -> LineCacheKey {
    let style = window.text_style();
    let (syntax, diagnostics) = input
        .code_document
        .as_ref()
        .map(|document| {
            let state = document.state.get();
            (
                if state
                    .highlight_identity
                    .is_none_or(|identity| identity == input.content.identity())
                {
                    std::sync::Arc::as_ptr(&state.highlights) as usize
                } else {
                    0
                },
                std::sync::Arc::as_ptr(&state.diagnostics) as usize,
            )
        })
        .unwrap_or_default();
    (
        style.font(),
        style.font_size.to_pixels(window.rem_size()),
        style.color,
        syntax,
        diagnostics,
        input.cursor_offset(),
        input.letter_spacing(),
    )
}

impl TextInput {
    fn letter_spacing(&self) -> Pixels {
        px(self
            .code_document
            .as_ref()
            .map_or(0., |document| document.state.get().letter_spacing)
            .clamp(0., 4.))
    }
    pub(super) fn gutter_width(&self) -> Pixels {
        if self.code_document.is_some() {
            px(52.)
        } else {
            px(0.)
        }
    }
    pub(super) fn checkpoint(&mut self) {
        if !self.multiline || self.marked_range.is_some() {
            return;
        }
        self.undo.push((
            self.content.clone(),
            self.selected_range.clone(),
            self.secondary_selections.clone(),
        ));
        while self.undo.len() > 256
            || (self.code_document.is_none()
                && self
                    .undo
                    .iter()
                    .map(|(text, _, _)| text.len())
                    .sum::<usize>()
                    > 8 * 1024 * 1024)
        {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    pub(super) fn history(&mut self, redo: bool, window: &mut Window, cx: &mut Context<Self>) {
        let item = if redo {
            self.redo.pop()
        } else {
            self.undo.pop()
        };
        let Some((content, selection, secondary)) = item else {
            return;
        };
        let old = (
            self.content.clone(),
            self.selected_range.clone(),
            self.secondary_selections.clone(),
        );
        if redo {
            self.undo.push(old);
        } else {
            self.redo.push(old);
        }
        self.snippet = None;
        self.content = content;
        self.selected_range = selection;
        self.secondary_selections = secondary;
        self.selection_reversed = false;
        self.marked_range = None;
        self.changed();
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }
    pub(super) fn accept_completion(
        &mut self,
        item: &omen_core::EditorCompletion,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.snippet = None;
        let mut start = item.range.start;
        if item.additional_edits.is_empty() {
            self.replace_text_in_range(
                Some(self.range_to_utf16(&item.range)),
                &item.insert,
                window,
                cx,
            );
        } else {
            let mut edits: Vec<_> = item
                .additional_edits
                .iter()
                .map(|edit| (edit.range.clone(), edit.insert.as_str()))
                .collect();
            edits.push((item.range.clone(), item.insert.as_str()));
            edits.sort_by_key(|(range, _)| (range.start, range.end));
            if edits
                .iter()
                .any(|(range, _)| range.start > range.end || range.end > self.content.len())
                || edits.windows(2).any(|pair| pair[0].0.end > pair[1].0.start)
            {
                return;
            }
            for edit in &item.additional_edits {
                if edit.range.end <= item.range.start {
                    start = start - (edit.range.end - edit.range.start) + edit.insert.len();
                }
            }
            self.checkpoint();
            for (range, text) in edits.into_iter().rev() {
                self.replace_content(range, text);
            }
            self.selected_range = start + item.insert.len()..start + item.insert.len();
            self.selection_reversed = false;
            self.marked_range = None;
            self.changed();
            window.notify_text_input_changed(&self.focus_handle);
        }
        let stops: Vec<_> = item
            .placeholders
            .iter()
            .filter(|range| {
                range.start <= range.end
                    && range.end <= item.insert.len()
                    && item.insert.is_char_boundary(range.start)
                    && item.insert.is_char_boundary(range.end)
            })
            .map(|range| start + range.start..start + range.end)
            .collect();
        if let Some(first) = stops.first().cloned() {
            if stops.len() > 1 || !first.is_empty() {
                self.snippet = Some(omen_core::EditorSnippet { stops, active: 0 });
            }
            self.selected_range = first;
            self.selection_reversed = false;
            if let Some(document) = &self.code_document {
                document
                    .state
                    .update(|state| state.selection = self.selected_range.clone());
            }
            window.notify_text_selection_changed(&self.focus_handle);
            cx.notify();
        }
    }
    pub(super) fn line_start(&self, offset: usize) -> usize {
        self.content.line_start(self.content.byte_to_line(offset))
    }
    pub(super) fn line_end(&self, offset: usize) -> usize {
        let row = self.content.byte_to_line(offset);
        self.content.line_start(row) + self.content.line(row).len()
    }
    pub(super) fn indent(&mut self, outdent: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.marked_range.is_some() {
            return;
        }
        if !outdent
            && self
                .code_document
                .as_ref()
                .is_some_and(|document| !document.state.get().completions.is_empty())
        {
            let state = self.code_document.as_ref().unwrap().state.get();
            let item =
                state.completions[state.completion_index.min(state.completions.len() - 1)].clone();
            self.accept_completion(&item, window, cx);
            return;
        }
        if self.multi_motion("tab", outdent, window, cx) {
            return;
        }
        if self.snippet.is_some() {
            let selection = self
                .snippet
                .as_mut()
                .and_then(|snippet| snippet.advance(&self.selected_range, outdent));
            if let Some(selection) = selection {
                let finished = selection.is_empty()
                    && self
                        .snippet
                        .as_ref()
                        .is_some_and(|snippet| snippet.active + 1 == snippet.stops.len());
                self.move_to(selection.start, window, cx);
                self.selected_range = selection;
                if let Some(document) = &self.code_document {
                    document
                        .state
                        .update(|state| state.selection = self.selected_range.clone());
                }
                if finished {
                    self.snippet = None;
                }
                window.notify_text_selection_changed(&self.focus_handle);
                cx.notify();
                return;
            }
            self.snippet = None;
        }
        if !outdent && self.selected_range.is_empty() {
            self.replace_text_in_range(None, "    ", window, cx);
            return;
        }
        self.checkpoint();
        let first = self.content.byte_to_line(self.selected_range.start);
        let mut last = self.content.byte_to_line(self.selected_range.end);
        if !self.selected_range.is_empty()
            && self.selected_range.end == self.content.line_start(last)
        {
            last = last.saturating_sub(1);
        }
        for row in (first..=last).rev() {
            let offset = self.content.line_start(row);
            if outdent {
                let line = self.content.line(row);
                let count = if line.starts_with('\t') {
                    1
                } else {
                    line.bytes()
                        .take(4)
                        .take_while(|byte| *byte == b' ')
                        .count()
                };
                self.replace_content(offset..offset + count, "");
                if self.selected_range.start >= offset {
                    self.selected_range.start =
                        self.selected_range.start.saturating_sub(count).max(offset);
                }
                if self.selected_range.end >= offset {
                    self.selected_range.end =
                        self.selected_range.end.saturating_sub(count).max(offset);
                }
            } else {
                self.replace_content(offset..offset, "    ");
                if self.selected_range.start >= offset {
                    self.selected_range.start += 4;
                }
                if self.selected_range.end >= offset {
                    self.selected_range.end += 4;
                }
            }
        }
        self.changed();
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }
    pub(super) fn editor_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.multiline || self.marked_range.is_some() {
            return;
        }
        if self.code_document.is_some() && self.multicursor_key(event, window, cx) {
            cx.stop_propagation();
            return;
        }
        if self.popup_key(event, window, cx) {
            cx.stop_propagation();
            return;
        }
        if self.code_document.is_some() && self.command_key(event, window, cx) {
            cx.stop_propagation();
            return;
        }
        if event.keystroke.key == "escape" {
            self.secondary_selections.clear();
            self.sync_selections(window, cx);
            self.snippet = None;
            self.dismiss_popups();
            cx.notify();
        }
        if let Some(document) = &self.code_document {
            let state = document.state.get();
            if !state.completions.is_empty() {
                match event.keystroke.key.as_str() {
                    "escape" => {
                        document
                            .state
                            .update(|state| state.completions = Default::default());
                        cx.stop_propagation();
                        return;
                    }
                    "up" | "down" => {
                        let up = event.keystroke.key == "up";
                        document.state.update(|state| {
                            state.completion_index = if up {
                                state.completion_index.saturating_sub(1)
                            } else {
                                (state.completion_index + 1).min(state.completions.len() - 1)
                            }
                        });
                        cx.stop_propagation();
                        return;
                    }
                    "enter" => {
                        let item = state.completions
                            [state.completion_index.min(state.completions.len() - 1)]
                        .clone();
                        self.accept_completion(&item, window, cx);
                        cx.stop_propagation();
                        return;
                    }
                    _ => {}
                }
            }
        }
        if let Some(document) = &self.code_document {
            let request = match event.keystroke.key.as_str() {
                "space" if event.keystroke.modifiers.control => {
                    Some(omen_core::EditorEvent::Complete)
                }
                "f12" => Some(omen_core::EditorEvent::Definition),
                "i" if event.keystroke.modifiers.secondary()
                    && !event.keystroke.modifiers.alt
                    && event.keystroke.modifiers.shift =>
                {
                    Some(omen_core::EditorEvent::Hover)
                }
                "f" if event.keystroke.modifiers.alt && event.keystroke.modifiers.shift => {
                    Some(omen_core::EditorEvent::Format)
                }
                _ => None,
            };
            if let Some(request) = request {
                if matches!(request, omen_core::EditorEvent::Hover) {
                    self.hover_position = None;
                }
                document.emit(request);
                cx.stop_propagation();
                return;
            }
        }
        if self.code_document.is_some() && self.pair_key(event, window, cx) {
            cx.stop_propagation();
            return;
        }
        if event.keystroke.modifiers.secondary() {
            match event.keystroke.key.as_str() {
                "z" => self.history(event.keystroke.modifiers.shift, window, cx),
                "y" => self.history(true, window, cx),
                _ => return,
            }
            cx.stop_propagation();
            return;
        }
        let offset = self.cursor_offset();
        match event.keystroke.key.as_str() {
            "enter" => {
                let start = self.line_start(offset);
                let before = self.content.slice(start..offset);
                let indent: String = before
                    .chars()
                    .take_while(|character| *character == ' ' || *character == '\t')
                    .collect();
                let extra = if self.code_document.is_some()
                    && before.trim_end().ends_with(['{', '[', '('])
                {
                    "    "
                } else {
                    ""
                };
                let newline = if self.code_document.is_some() {
                    format!("\n{indent}{extra}")
                } else {
                    "\n".into()
                };
                let closing = self
                    .content
                    .slice(offset..(offset + 1).min(self.content.len()));
                let expand_pair = !extra.is_empty() && matches!(closing.as_ref(), "}" | "]" | ")");
                let cursor = self.selected_range.start + newline.len();
                let newline = if expand_pair {
                    format!("{newline}\n{indent}")
                } else {
                    newline
                };
                self.replace_text_in_range(None, &newline, window, cx);
                if expand_pair {
                    self.move_to(cursor, window, cx);
                }
            }
            "up" | "down" => {
                let start = self.line_start(offset);
                let column = self.content.slice(start..offset).graphemes(true).count();
                let target = if event.keystroke.key == "up" {
                    if start == 0 {
                        0
                    } else {
                        self.line_start(start - 1)
                    }
                } else {
                    (self.line_end(offset) + 1).min(self.content.len())
                };
                let end = self.line_end(target);
                let target = self
                    .content
                    .slice(target..end)
                    .grapheme_indices(true)
                    .nth(column)
                    .map_or(end, |(index, _)| target + index);
                if event.keystroke.modifiers.shift {
                    self.select_to(target, window, cx);
                } else {
                    self.move_to(target, window, cx);
                }
            }
            _ => return,
        }
        cx.stop_propagation();
    }
    pub(super) fn editor_index(&self, position: Point<Pixels>) -> usize {
        let Some(bounds) = self.last_bounds else {
            return 0;
        };
        let line_height: f32 = self.editor_line_height.into();
        let y: f32 = (position.y - bounds.top() + self.scroll_y).into();
        let row = (y.max(0.) / line_height.max(1.)).floor() as usize;
        let row = row.min(self.content.line_count().saturating_sub(1));
        let start = self.content.line_start(row);
        self.last_lines
            .iter()
            .find(|(offset, _)| *offset == start)
            .map_or(start, |(_, line)| {
                start
                    + line.closest_index_for_x(
                        position.x - bounds.left() + self.scroll - self.gutter_width(),
                    )
            })
    }
    pub(super) fn editor_bounds(
        &self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
    ) -> Vec<Bounds<Pixels>> {
        self.last_lines
            .iter()
            .enumerate()
            .filter_map(|(index, (start, line))| {
                let end = start + line.text.len();
                if range.start > end || range.end < *start {
                    return None;
                }
                let y = bounds.top()
                    + self.editor_line_height * (self.first_visible_line + index) as f32
                    - self.scroll_y;
                let left = bounds.left()
                    + self.gutter_width()
                    + line.x_for_index(range.start.saturating_sub(*start).min(line.text.len()))
                    - self.scroll;
                let right = bounds.left()
                    + self.gutter_width()
                    + line.x_for_index(range.end.saturating_sub(*start).min(line.text.len()))
                    - self.scroll;
                Some(Bounds::new(
                    point(left, y),
                    size((right - left).max(px(2.)), self.editor_line_height),
                ))
            })
            .collect()
    }
}

pub(super) fn prepaint(
    input: &TextInput,
    bounds: Bounds<Pixels>,
    window: &mut Window,
) -> PrepaintState {
    let bounds = Bounds::new(
        bounds.origin + point(input.gutter_width(), px(0.)),
        size(
            (bounds.size.width - input.gutter_width()).max(px(1.)),
            bounds.size.height,
        ),
    );
    let style = window.text_style();
    let line_height = window.line_height();
    let text = &input.content;
    let mut selections: Vec<_> = input.secondary_selections.clone();
    if !input.selected_range.is_empty() {
        selections.push(input.selected_range.clone());
    }
    selections.sort_by_key(|range| range.start);
    let cursor = input.cursor_offset();
    let brackets = if input.code_document.is_some() {
        input.content.bracket_pair(cursor)
    } else {
        None
    };
    let cursor_row = text.byte_to_line(cursor);
    let line_count = text.line_count();
    let mut scroll_y = input.scroll_y;
    if input.reveal_cursor {
        let y = line_height * cursor_row as f32;
        if y < scroll_y {
            scroll_y = y;
        }
        if y + line_height > scroll_y + bounds.size.height {
            scroll_y = y + line_height - bounds.size.height;
        }
    }
    scroll_y = scroll_y
        .max(px(0.))
        .min((line_height * line_count as f32 - bounds.size.height).max(px(0.)));
    let first = (f32::from(scroll_y) / f32::from(line_height).max(1.)).floor() as usize;
    let count =
        (f32::from(bounds.size.height) / f32::from(line_height).max(1.)).ceil() as usize + 1;
    let mut lines = Vec::new();
    let mut selection = Vec::new();
    let mut caret = None;
    let mut scroll = input.scroll;
    for row in first..(first + count).min(line_count) {
        let start = text.line_start(row);
        let text = text.line(row);
        let run = TextRun {
            len: text.len(),
            font: style.font(),
            color: style.color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let mut runs = super::syntax::runs(input, &text, start, run.clone(), brackets);
        if let Some(marked) = &input.marked_range {
            let a = marked.start.saturating_sub(start).min(text.len());
            let b = marked.end.saturating_sub(start).min(text.len());
            if b > a {
                runs = vec![
                    TextRun {
                        len: a,
                        ..run.clone()
                    },
                    TextRun {
                        len: b - a,
                        underline: Some(UnderlineStyle {
                            color: Some(run.color),
                            thickness: px(1.),
                            wavy: false,
                        }),
                        ..run.clone()
                    },
                    TextRun {
                        len: text.len() - b,
                        ..run
                    },
                ];
                runs.retain(|run| run.len > 0);
            }
        }
        let cached = if input.marked_range.is_none()
            && input.last_line_style.as_ref() == Some(&style_key(input, window))
        {
            input
                .last_lines
                .iter()
                .find(|(offset, line)| *offset == start && line.text.as_str() == text.as_ref())
                .map(|(_, line)| line.clone())
        } else {
            None
        };
        let line = cached.unwrap_or_else(|| {
            let line = window.text_system().shape_line(
                SharedString::from(text.to_string()),
                style.font_size.to_pixels(window.rem_size()),
                &runs,
                None,
            );
            with_letter_spacing(line, input.letter_spacing())
        });
        let y = bounds.top() + line_height * row as f32 - scroll_y;
        if row == cursor_row && input.reveal_cursor {
            let x = line.x_for_index(cursor.saturating_sub(start).min(text.len()));
            if x < scroll {
                scroll = x;
            }
            scroll = scroll.max(x - bounds.size.width + px(2.)).max(px(0.));
        }
        if row == cursor_row && input.selected_range.is_empty() {
            let x = line.x_for_index(cursor.saturating_sub(start).min(text.len()));
            caret = Some(fill(
                Bounds::new(
                    point(bounds.left() + x - scroll, y),
                    size(px(2.), line_height),
                ),
                input.accent,
            ));
        }
        for range in selections[selections.partition_point(|range| range.end < start)..]
            .iter()
            .take_while(|range| range.start <= start + text.len())
        {
            if range.is_empty() {
                if input.focus_handle.is_focused(window)
                    && input.content.byte_to_line(range.start) == row
                {
                    let x = line.x_for_index(range.start.saturating_sub(start).min(text.len()));
                    selection.push(fill(
                        Bounds::new(
                            point(bounds.left() + x - scroll, y),
                            size(px(2.), line_height),
                        ),
                        input.accent,
                    ));
                }
                continue;
            }
            let a = range.start.saturating_sub(start).min(text.len());
            let b = range.end.saturating_sub(start).min(text.len());
            selection.push(fill(
                Bounds::new(
                    point(bounds.left() + line.x_for_index(a) - scroll, y),
                    size(
                        (line.x_for_index(b) - line.x_for_index(a)).max(px(2.)),
                        line_height,
                    ),
                ),
                input.accent.opacity(0.25),
            ));
        }
        lines.push((start, line));
    }
    let visible_max = lines.iter().map(|(_, l)| l.width).max().unwrap_or(px(0.));
    let max_content_width = input.max_content_width.max(visible_max);
    let max_scroll = (max_content_width + px(48.) - bounds.size.width).max(px(0.));
    let scroll = scroll.min(max_scroll).max(px(0.));
    PrepaintState {
        line: None,
        cursor: caret,
        selection: None,
        scroll,
        lines: Some(lines),
        selections: selection,
        scroll_y,
        first_visible_line: first,
    }
}

/// Adjust a private copy of the shaped geometry, so painting, caret hit testing,
/// selections and IME bounds all use the same advances. Keep combining marks
/// and emoji sequences within their original grapheme cluster.
fn with_letter_spacing(mut line: ShapedLine, spacing: Pixels) -> ShapedLine {
    if spacing <= px(0.) || line.text.is_empty() {
        return line;
    }
    let starts: Vec<usize> = line
        .text
        .grapheme_indices(true)
        .map(|(index, _)| index)
        .collect();
    let mut layout = gpui::LineLayout {
        font_size: line.font_size,
        width: line.width + spacing * starts.len().saturating_sub(1) as f32,
        ascent: line.ascent,
        descent: line.descent,
        runs: line.runs.clone(),
        len: line.len,
    };
    for run in &mut layout.runs {
        for glyph in &mut run.glyphs {
            let cluster = starts
                .partition_point(|start| *start <= glyph.index)
                .saturating_sub(1);
            glyph.position.x += spacing * cluster as f32;
        }
    }
    *line = std::sync::Arc::new(layout);
    line
}

#[cfg(test)]
mod spacing_tests {
    use super::*;
    #[gpui::test]
    fn letter_spacing_updates_caret_and_hit_geometry_without_splitting_graphemes(
        cx: &mut gpui::TestAppContext,
    ) {
        let state = omen_core::Value::new(omen_core::EditorState {
            text: "a\u{301}b".into(),
            ..Default::default()
        });
        let (input, vcx) = cx.add_window_view(|_, cx| {
            let mut input = TextInput::new(
                "Code".into(),
                omen_core::TextValue::new(String::new()),
                false,
                None,
                None,
                cx,
            );
            input.multiline = true;
            input.code_document = Some(omen_core::EditorDocument::new(state.clone(), |_| {}));
            input
        });
        vcx.run_until_parked();
        let (width, caret) = vcx.update(|_, cx| {
            let line = &input.read(cx).last_lines[0].1;
            (line.width, line.x_for_index(3))
        });
        state.update(|state| state.letter_spacing = 0.5);
        vcx.update(|_, cx| input.update(cx, |_, cx| cx.notify()));
        vcx.run_until_parked();
        vcx.update(|_, cx| {
            let line = &input.read(cx).last_lines[0].1;
            assert!((line.width - width - px(0.5)).abs() < px(0.01));
            assert!((line.x_for_index(3) - caret - px(0.5)).abs() < px(0.01));
            assert_eq!(line.closest_index_for_x(line.x_for_index(3)), 3);
        });
    }
}
