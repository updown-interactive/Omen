//! Atomic edits across normalized UTF-8 selections on the persistent text buffer.
use super::*;
impl TextInput {
    pub(super) fn multi_motion(
        &mut self,
        key: &str,
        shift: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.secondary_selections.is_empty() {
            return false;
        }
        self.multicursor_key(
            &gpui::KeyDownEvent {
                keystroke: gpui::Keystroke {
                    key: key.into(),
                    key_char: None,
                    modifiers: gpui::Modifiers {
                        shift,
                        ..Default::default()
                    },
                },
                is_held: false,
                prefer_character_input: false,
            },
            window,
            cx,
        )
    }
    pub(super) fn paste_cursors(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ranges = self.cursor_ranges();
        let pieces: Vec<_> = text.split('\n').map(str::to_string).collect();
        let mut values = vec![text.to_string(); ranges.len()];
        if pieces.len() == ranges.len() {
            let mut order: Vec<_> = ranges.iter().enumerate().collect();
            order.sort_by_key(|(_, range)| range.start);
            for ((index, _), piece) in order.into_iter().zip(pieces) {
                values[index] = piece;
            }
        }
        self.replace_ranges(ranges, &values, window, cx);
    }
    fn cursor_ranges(&self) -> Vec<Range<usize>> {
        std::iter::once(self.selected_range.clone())
            .chain(self.secondary_selections.iter().cloned())
            .collect()
    }
    pub(super) fn sync_selections(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.reveal_cursor = true;
        self.selection_reversed = false;
        if let Some(document) = &self.code_document {
            document.state.update(|state| {
                state.selection = self.selected_range.clone();
                state.secondary_selections = self.secondary_selections.clone();
                state.completions = Default::default();
                state.hover = None;
            });
            document.emit(omen_core::EditorEvent::Selection);
        }
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
    }
    pub(super) fn add_cursor(
        &mut self,
        range: Range<usize>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .cursor_ranges()
            .iter()
            .any(|old| *old == range || old.start < range.end && range.start < old.end)
        {
            return;
        }
        self.secondary_selections.push(self.selected_range.clone());
        self.selected_range = range;
        self.snippet = None;
        self.sync_selections(window, cx);
    }
    fn select_occurrence(&mut self, all: bool, window: &Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let word = self.word_range(self.cursor_offset());
            if word.is_empty() {
                return;
            }
            self.selected_range = word;
            self.sync_selections(window, cx);
            if !all {
                return;
            }
        }
        let needle = self.content.slice(self.selected_range.clone()).into_owned();
        if needle.is_empty() {
            return;
        }
        let source = self.content.to_string();
        let words = needle.chars().all(word_char);
        let existing = self.cursor_ranges();
        let mut matches: Vec<_> = source
            .match_indices(&needle)
            .filter_map(|(start, _)| {
                let end = start + needle.len();
                let boundary = !words
                    || (!source[..start].chars().next_back().is_some_and(word_char)
                        && !source[end..].chars().next().is_some_and(word_char));
                (boundary
                    && !existing
                        .iter()
                        .any(|range| range.start < end && start < range.end))
                .then_some(start..end)
            })
            .collect();
        if all {
            for range in matches {
                self.secondary_selections.push(range);
            }
        } else if !matches.is_empty() {
            let end = self.selected_range.end;
            matches.sort_by_key(|range| (range.start < end, range.start));
            self.secondary_selections.push(self.selected_range.clone());
            self.selected_range = matches.remove(0);
        }
        self.snippet = None;
        self.sync_selections(window, cx);
    }
    pub(super) fn replace_cursors(
        &mut self,
        values: &[String],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_ranges(self.cursor_ranges(), values, window, cx);
    }
    fn replace_ranges(
        &mut self,
        ranges: Vec<Range<usize>>,
        values: &[String],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut edits: Vec<_> = ranges
            .into_iter()
            .enumerate()
            .map(|(i, range)| {
                (
                    range,
                    i == 0,
                    values
                        .get(i)
                        .cloned()
                        .unwrap_or_default()
                        .replace("\r\n", "\n")
                        .replace('\r', "\n"),
                )
            })
            .collect();
        edits.sort_by_key(|(range, _, _)| (range.start, range.end));
        let mut merged: Vec<(Range<usize>, bool, String)> = Vec::new();
        for (range, primary, text) in edits {
            if let Some((old, active, value)) = merged.last_mut()
                && (range == *old || range.start < old.end)
            {
                old.end = old.end.max(range.end);
                *active |= primary;
                if primary {
                    *value = text;
                }
            } else {
                merged.push((range, primary, text));
            }
        }
        self.checkpoint();
        let mut delta = 0isize;
        let mut secondary = Vec::new();
        for (range, primary, text) in &merged {
            let caret = range.start.saturating_add_signed(delta) + text.len();
            if *primary {
                self.selected_range = caret..caret;
            } else {
                secondary.push(caret..caret);
            }
            delta += text.len() as isize - range.len() as isize;
        }
        for (range, _, text) in merged.into_iter().rev() {
            self.replace_content(range, &text);
        }
        self.secondary_selections = secondary;
        self.selection_reversed = false;
        self.marked_range = None;
        self.snippet = None;
        self.changed();
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }
    pub(super) fn delete_cursors(
        &mut self,
        forward: bool,
        word: bool,
        line: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ranges: Vec<_> = self
            .cursor_ranges()
            .into_iter()
            .map(|range| {
                if !range.is_empty() {
                    return range;
                }
                let at = range.start;
                let edge = if line {
                    if forward {
                        self.line_end(at)
                    } else {
                        self.line_start(at)
                    }
                } else if word {
                    self.word_boundary(at, forward)
                } else if forward {
                    self.next_boundary(at)
                } else {
                    self.previous_boundary(at)
                };
                at.min(edge)..at.max(edge)
            })
            .collect();
        let values = vec![String::new(); ranges.len()];
        self.replace_ranges(ranges, &values, window, cx);
    }
    fn clipboard_ranges(&self) -> Vec<Range<usize>> {
        let mut ranges: Vec<_> = self
            .cursor_ranges()
            .into_iter()
            .map(|range| {
                if range.is_empty() {
                    let row = self.content.byte_to_line(range.start);
                    self.content.line_start(row)..self.content.line_start(row + 1)
                } else {
                    range
                }
            })
            .collect();
        ranges.sort_by_key(|range| (range.start, range.end));
        ranges.dedup();
        ranges
    }
    pub(super) fn multi_clipboard(&self) -> String {
        self.clipboard_ranges()
            .iter()
            .map(|range| {
                self.content
                    .slice(range.clone())
                    .trim_end_matches('\n')
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub(super) fn cut_cursors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ranges = self
            .cursor_ranges()
            .into_iter()
            .map(|range| {
                if range.is_empty() {
                    let row = self.content.byte_to_line(range.start);
                    self.content.line_start(row)..self.content.line_start(row + 1)
                } else {
                    range
                }
            })
            .collect::<Vec<_>>();
        self.replace_ranges(
            ranges,
            &vec![String::new(); self.secondary_selections.len() + 1],
            window,
            cx,
        );
    }
    fn indent_cursors(&mut self, outdent: bool, window: &mut Window, cx: &mut Context<Self>) {
        let selections = self.cursor_ranges();
        let mut rows = std::collections::BTreeSet::new();
        for range in &selections {
            let first = self.content.byte_to_line(range.start);
            let mut last = self.content.byte_to_line(range.end);
            if !range.is_empty() && range.end == self.content.line_start(last) {
                last = last.saturating_sub(1);
            }
            rows.extend(first..=last);
        }
        let edits: Vec<_> = rows
            .into_iter()
            .filter_map(|row| {
                let start = self.content.line_start(row);
                if !outdent {
                    return Some((start..start, "    "));
                }
                let line = self.content.line(row);
                let width = if line.starts_with('\t') {
                    1
                } else {
                    line.bytes()
                        .take(4)
                        .take_while(|byte| *byte == b' ')
                        .count()
                };
                (width > 0).then_some((start..start + width, ""))
            })
            .collect();
        if edits.is_empty() {
            return;
        }
        self.checkpoint();
        for (range, text) in edits.iter().rev() {
            self.replace_content(range.clone(), text);
        }
        let remap = |offset: usize| {
            let mut delta = 0isize;
            for (range, text) in &edits {
                if offset < range.start {
                    break;
                }
                if offset <= range.end {
                    return range.start.saturating_add_signed(delta) + text.len();
                }
                delta += text.len() as isize - range.len() as isize;
            }
            offset.saturating_add_signed(delta)
        };
        let shifted: Vec<_> = selections
            .iter()
            .map(|range| remap(range.start)..remap(range.end))
            .collect();
        self.selected_range = shifted[0].clone();
        self.secondary_selections = shifted[1..].to_vec();
        self.changed();
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }
    fn vertical_cursors(&mut self, down: bool, window: &Window, cx: &mut Context<Self>) {
        let at = self.cursor_offset();
        let row = self.content.byte_to_line(at);
        if (!down && row == 0) || (down && row + 1 >= self.content.line_count()) {
            return;
        }
        let column = self
            .content
            .slice(self.content.line_start(row)..at)
            .graphemes(true)
            .count();
        let target = if down { row + 1 } else { row - 1 };
        let start = self.content.line_start(target);
        let line = self.content.line(target);
        let offset = line
            .trim_end_matches(['\r', '\n'])
            .grapheme_indices(true)
            .nth(column)
            .map_or_else(|| self.line_end(start), |(index, _)| start + index);
        self.add_cursor(offset..offset, window, cx);
    }
    pub(super) fn multicursor_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let key = event.keystroke.key.as_str();
        let m = event.keystroke.modifiers;
        let primary = m.secondary();
        let chord = self.cursor_chord;
        self.cursor_chord = false;
        if key == "k" && primary && !m.shift {
            self.cursor_chord = true;
            return true;
        }
        if chord && key == "d" && primary {
            let old = self.selected_range.clone();
            self.select_occurrence(false, window, cx);
            self.secondary_selections.retain(|range| *range != old);
            self.sync_selections(window, cx);
            return true;
        }
        if key == "d" && primary && !m.shift && !m.alt {
            self.select_occurrence(false, window, cx);
            return true;
        }
        if key == "l" && primary && m.shift && !m.alt {
            self.select_occurrence(true, window, cx);
            return true;
        }
        if key == "f2" && (m.control || primary) {
            self.select_occurrence(true, window, cx);
            return true;
        }
        let column = if cfg!(target_os = "macos") {
            m.alt && m.platform
        } else {
            m.alt && m.control
        };
        if matches!(key, "up" | "down") && column {
            self.vertical_cursors(key == "down", window, cx);
            return true;
        }
        if key == "u" && primary && !m.shift {
            if let Some(previous) = self.secondary_selections.pop() {
                self.selected_range = previous;
                self.sync_selections(window, cx);
            }
            return true;
        }
        if key == "i" && m.alt && m.shift && !primary {
            let start = self.content.byte_to_line(self.selected_range.start);
            let end = self.content.byte_to_line(self.selected_range.end);
            self.secondary_selections.clear();
            self.selected_range = self.line_end(self.content.line_start(start))
                ..self.line_end(self.content.line_start(start));
            for row in start + 1..=end {
                let at = self.line_end(self.content.line_start(row));
                self.secondary_selections.push(at..at);
            }
            self.sync_selections(window, cx);
            return true;
        }
        let word = if cfg!(target_os = "macos") {
            m.alt
        } else {
            m.control
        };
        let line = cfg!(target_os = "macos") && m.platform;
        if matches!(key, "backspace" | "delete") && (word || line) {
            self.delete_cursors(key == "delete", word, line, window, cx);
            return true;
        }
        if key == "k" && primary && m.shift {
            let ranges: Vec<_> = self
                .cursor_ranges()
                .iter()
                .map(|range| {
                    let row = self.content.byte_to_line(range.start);
                    let mut last = self.content.byte_to_line(range.end);
                    if !range.is_empty() && range.end == self.content.line_start(last) {
                        last = last.saturating_sub(1);
                    }
                    let mut start = self.content.line_start(row);
                    let end = self.content.line_start(last + 1);
                    if end == self.content.len()
                        && start > 0
                        && !self.content.line(last).ends_with('\n')
                    {
                        start -= 1;
                    }
                    start..end
                })
                .collect();
            self.replace_ranges(
                ranges,
                &vec![String::new(); self.secondary_selections.len() + 1],
                window,
                cx,
            );
            return true;
        }
        if self.secondary_selections.is_empty() {
            return false;
        }
        if key == "escape" {
            self.secondary_selections.clear();
            self.sync_selections(window, cx);
            return true;
        }
        if key == "enter" && !primary {
            let values = self
                .cursor_ranges()
                .iter()
                .map(|range| {
                    let before = self
                        .content
                        .slice(self.line_start(range.start)..range.start);
                    let indent: String = before
                        .chars()
                        .take_while(|c| *c == ' ' || *c == '\t')
                        .collect();
                    format!(
                        "\n{indent}{}",
                        if before.trim_end().ends_with(['{', '[', '(']) {
                            "    "
                        } else {
                            ""
                        }
                    )
                })
                .collect::<Vec<_>>();
            self.replace_cursors(&values, window, cx);
            return true;
        }
        if key == "tab" {
            if m.shift || self.cursor_ranges().iter().any(|range| !range.is_empty()) {
                self.indent_cursors(m.shift, window, cx);
            } else {
                let values = vec!["    ".to_string(); self.secondary_selections.len() + 1];
                self.replace_cursors(&values, window, cx);
            }
            return true;
        }

        if matches!(key, "left" | "right" | "up" | "down" | "home" | "end") {
            let reversed = self.selection_reversed;
            let ranges = self
                .cursor_ranges()
                .into_iter()
                .map(|range| {
                    let at = if reversed { range.start } else { range.end };
                    let forward = matches!(key, "right" | "down" | "end");
                    let edge = if matches!(key, "home" | "end")
                        || (line && matches!(key, "left" | "right"))
                    {
                        if forward {
                            self.line_end(at)
                        } else {
                            self.line_start(at)
                        }
                    } else if word && matches!(key, "left" | "right") {
                        self.word_boundary(at, forward)
                    } else if matches!(key, "up" | "down") {
                        let row = self.content.byte_to_line(at);
                        let target = if forward {
                            (row + 1).min(self.content.line_count() - 1)
                        } else {
                            row.saturating_sub(1)
                        };
                        let col = self
                            .content
                            .slice(self.content.line_start(row)..at)
                            .graphemes(true)
                            .count();
                        let start = self.content.line_start(target);
                        self.content
                            .line(target)
                            .trim_end_matches(['\r', '\n'])
                            .grapheme_indices(true)
                            .nth(col)
                            .map_or_else(|| self.line_end(start), |(i, _)| start + i)
                    } else if !m.shift && !range.is_empty() {
                        if forward { range.end } else { range.start }
                    } else if forward {
                        self.next_boundary(at)
                    } else {
                        self.previous_boundary(at)
                    };
                    if m.shift {
                        let anchor = if reversed { range.end } else { range.start };
                        anchor.min(edge)..anchor.max(edge)
                    } else {
                        edge..edge
                    }
                })
                .collect::<Vec<_>>();
            self.selected_range = ranges[0].clone();
            self.secondary_selections = ranges[1..].to_vec();
            self.sync_selections(window, cx);
            self.selection_reversed = m.shift && matches!(key, "left" | "up" | "home");
            return true;
        }
        false
    }
}
fn word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}
