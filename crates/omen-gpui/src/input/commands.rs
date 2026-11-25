//! Native editing commands shared by keyboard bindings and context menus.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum Command {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Find,
    Definition,
    Rename,
    Actions,
    Format,
    Complete,
    Comment,
    DuplicateLine,
    MoveUp,
    MoveDown,
    InsertLineBelow,
    InsertLineAbove,
    JumpToBracket,
}
impl TextInput {
    pub(super) fn run_command(
        &mut self,
        command: Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.context_position = None;
        self.focus_handle.focus(window, cx);
        match command {
            Command::Undo => self.history(false, window, cx),
            Command::Redo => self.history(true, window, cx),
            Command::Cut => self.cut(&Cut, window, cx),
            Command::Copy => self.copy(&Copy, window, cx),
            Command::Paste => self.paste(&Paste, window, cx),
            Command::SelectAll => self.select_all(&SelectAll, window, cx),
            Command::Comment => self.toggle_comment(window, cx),
            Command::DuplicateLine => self.duplicate_line(window, cx),
            Command::MoveUp => self.move_lines(false, window, cx),
            Command::MoveDown => self.move_lines(true, window, cx),
            Command::InsertLineBelow => self.insert_line(true, window, cx),
            Command::InsertLineAbove => self.insert_line(false, window, cx),
            Command::JumpToBracket => self.jump_to_bracket(window, cx),
            host => {
                if let Some(document) = &self.code_document {
                    document.emit(match host {
                        Command::Find => omen_core::EditorEvent::Find,
                        Command::Definition => omen_core::EditorEvent::Definition,
                        Command::Rename => omen_core::EditorEvent::Rename,
                        Command::Actions => omen_core::EditorEvent::Actions,
                        Command::Format => omen_core::EditorEvent::Format,
                        Command::Complete => omen_core::EditorEvent::Complete,
                        _ => return,
                    });
                }
            }
        }
        cx.notify();
    }
    pub(super) fn word_range(&self, offset: usize) -> Range<usize> {
        let row = self.content.byte_to_line(offset);
        let start = self.content.line_start(row);
        let line = self.content.line(row);
        let local = offset.saturating_sub(start).min(line.len());
        line.split_word_bound_indices()
            .find(|(index, word)| local >= *index && local < index + word.len())
            .map_or(offset..offset, |(index, word)| {
                start + index..start + index + word.len()
            })
    }
    pub(super) fn word_boundary(&self, offset: usize, forward: bool) -> usize {
        let row = self.content.byte_to_line(offset);
        let start = self.content.line_start(row);
        let line = self.content.line(row);
        let local = offset - start;
        if forward {
            line.split_word_bound_indices()
                .find(|(index, word)| index + word.len() > local && !word.trim().is_empty())
                .map_or_else(
                    || self.next_boundary(offset),
                    |(index, word)| start + index + word.len(),
                )
        } else {
            line.split_word_bound_indices()
                .rfind(|(index, word)| *index < local && !word.trim().is_empty())
                .map_or_else(
                    || self.previous_boundary(offset),
                    |(index, _)| start + index,
                )
        }
    }
    fn selected_rows(&self) -> (usize, usize) {
        let first = self.content.byte_to_line(self.selected_range.start);
        let mut last = self.content.byte_to_line(self.selected_range.end);
        if last > first && self.selected_range.end == self.content.line_start(last) {
            last -= 1;
        }
        (first, last)
    }
    fn toggle_comment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (first, last) = self.selected_rows();
        let uncomment =
            (first..=last).all(|row| self.content.line(row).trim_start().starts_with("//"));
        let start = self.content.line_start(first);
        let end = self.content.line_start(last + 1);
        let mut replacement = String::new();
        for row in first..=last {
            let text = self
                .content
                .slice(self.content.line_start(row)..self.content.line_start(row + 1));
            let indent = text.len() - text.trim_start_matches([' ', '\t']).len();
            replacement.push_str(&text[..indent]);
            let text = &text[indent..];
            if uncomment {
                replacement.push_str(
                    text.strip_prefix("// ")
                        .or_else(|| text.strip_prefix("//"))
                        .unwrap_or(text),
                );
            } else {
                replacement.push_str("// ");
                replacement.push_str(text);
            }
        }
        self.replace_text_in_range(
            Some(self.range_to_utf16(&(start..end))),
            &replacement,
            window,
            cx,
        );
        self.move_to(start, window, cx);
        self.select_to(start + replacement.len(), window, cx);
    }
    fn duplicate_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (first, last) = self.selected_rows();
        let start = self.content.line_start(first);
        let end = self.content.line_start(last + 1);
        let mut text = self.content.slice(start..end).into_owned();
        if !text.ends_with('\n') {
            text.insert(0, '\n');
        }
        self.replace_text_in_range(Some(self.range_to_utf16(&(end..end))), &text, window, cx);
    }
    fn move_lines(&mut self, down: bool, window: &mut Window, cx: &mut Context<Self>) {
        let (first, last) = self.selected_rows();
        if (!down && first == 0) || (down && last + 1 >= self.content.line_count()) {
            return;
        }
        let a = if down { first } else { first - 1 };
        let b = if down { last + 1 } else { last };
        let start = self.content.line_start(a);
        let end = self.content.line_start(b + 1);
        let before = self.content.slice(start..end).into_owned();
        let mut lines: Vec<&str> = before
            .strip_suffix('\n')
            .unwrap_or(&before)
            .split('\n')
            .collect();
        if down {
            lines.rotate_right(1);
        } else {
            lines.rotate_left(1);
        }
        let mut text = lines.join("\n");
        if before.ends_with('\n') {
            text.push('\n');
        }
        self.replace_text_in_range(Some(self.range_to_utf16(&(start..end))), &text, window, cx);
        let new_first = if down { first + 1 } else { first - 1 };
        let new_last = new_first + last - first;
        self.move_to(self.content.line_start(new_first), window, cx);
        self.select_to(self.content.line_start(new_last + 1), window, cx);
    }
    fn insert_line(&mut self, below: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.secondary_selections.clear();
        let row = self.content.byte_to_line(self.cursor_offset());
        let line_start = self.content.line_start(row);
        let line = self.content.line(row);
        let indent: String = line
            .chars()
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .collect();
        if below {
            let end = self.line_end(line_start);
            let extra = if line.trim_end().ends_with(['{', '[', '(']) {
                "    "
            } else {
                ""
            };
            let inserted = format!("\n{indent}{extra}");
            self.replace_text_in_range(Some(self.range_to_utf16(&(end..end))), &inserted, window, cx);
            self.move_to(end + inserted.len(), window, cx);
        } else {
            let inserted = format!("{indent}\n");
            self.replace_text_in_range(
                Some(self.range_to_utf16(&(line_start..line_start))),
                &inserted,
                window,
                cx,
            );
            self.move_to(line_start + indent.len(), window, cx);
        }
    }
    fn jump_to_bracket(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let offset = self.cursor_offset();
        let Some((open, close)) = self.content.bracket_pair(offset) else {
            return;
        };
        let target = if offset == open || offset == open + 1 {
            close
        } else {
            open
        };
        self.move_to(target, window, cx);
    }
    pub(super) fn pair_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let modifiers = event.keystroke.modifiers;
        if !self.secondary_selections.is_empty()
            || modifiers.secondary()
            || modifiers.control
            || modifiers.alt
        {
            return false;
        }
        let key = event
            .keystroke
            .key_char
            .as_deref()
            .unwrap_or(event.keystroke.key.as_str());
        let offset = self.cursor_offset();
        if matches!(key, ")" | "]" | "}" | "\"")
            && self.selected_range.is_empty()
            && self
                .content
                .slice(offset..(offset + key.len()).min(self.content.len()))
                .as_ref()
                == key
        {
            self.move_to(offset + key.len(), window, cx);
            return true;
        }
        let close = match key {
            "(" => ")",
            "[" => "]",
            "{" => "}",
            "\"" => "\"",
            _ => return false,
        };
        if key == "\"" && self.selected_range.is_empty() {
            let prev = self
                .content
                .slice(offset.saturating_sub(4)..offset)
                .chars()
                .next_back();
            if prev.is_some_and(|ch| ch == '\\' || ch.is_alphanumeric() || ch == '_') {
                return false;
            }
        }
        let next = self
            .content
            .slice(self.selected_range.end..(self.selected_range.end + 4).min(self.content.len()))
            .chars()
            .next();
        if self.selected_range.is_empty()
            && next.is_some_and(|ch| !ch.is_whitespace() && !")]}.,;:".contains(ch))
        {
            return false;
        }
        let selected = self.selected_range.clone();
        let text = format!("{key}{}{close}", self.content.slice(selected.clone()));
        self.replace_text_in_range(None, &text, window, cx);
        self.move_to(selected.start + key.len(), window, cx);
        if !selected.is_empty() {
            self.select_to(selected.end + key.len(), window, cx);
        }
        true
    }
    pub(super) fn command_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        let command = match key {
            "f" if modifiers.secondary() => Some(Command::Find),
            "f2" => Some(Command::Rename),
            "." if modifiers.secondary() => Some(Command::Actions),
            "/" if modifiers.secondary() => Some(Command::Comment),
            "enter" if modifiers.secondary() && !modifiers.alt => Some(if modifiers.shift {
                Command::InsertLineAbove
            } else {
                Command::InsertLineBelow
            }),
            "\\" if modifiers.secondary() && modifiers.shift && !modifiers.alt => {
                Some(Command::JumpToBracket)
            }
            "up" | "down" if modifiers.alt && modifiers.shift => Some(Command::DuplicateLine),
            "up" if modifiers.alt => Some(Command::MoveUp),
            "down" if modifiers.alt => Some(Command::MoveDown),
            _ => None,
        };
        if let Some(command) = command {
            self.run_command(command, window, cx);
            return true;
        }
        let word_modifier = if cfg!(target_os = "macos") {
            modifiers.alt
        } else {
            modifiers.control
        };
        if (key == "left" || key == "right") && word_modifier {
            let offset = self.word_boundary(self.cursor_offset(), key == "right");
            if modifiers.shift {
                self.select_to(offset, window, cx);
            } else {
                self.move_to(offset, window, cx);
            }
            return true;
        }
        if matches!(key, "home" | "end" | "pageup" | "pagedown")
            || (cfg!(target_os = "macos")
                && modifiers.platform
                && matches!(key, "left" | "right" | "up" | "down"))
        {
            let offset = self.cursor_offset();
            let row = self.content.byte_to_line(offset);
            let target = match key {
                "home" | "end" if modifiers.control => {
                    if key == "home" {
                        0
                    } else {
                        self.content.len()
                    }
                }
                "up" => 0,
                "down" => self.content.len(),
                "end" | "right" => self.line_end(offset),
                "pageup" | "pagedown" => {
                    let rows = self.last_bounds.map_or(20, |bounds| {
                        (f32::from(bounds.size.height) / f32::from(self.editor_line_height).max(1.))
                            as usize
                    });
                    self.content.line_start(if key == "pageup" {
                        row.saturating_sub(rows)
                    } else {
                        (row + rows).min(self.content.line_count() - 1)
                    })
                }
                _ => {
                    let start = self.content.line_start(row);
                    let indent = self
                        .content
                        .line(row)
                        .chars()
                        .take_while(|ch| *ch == ' ' || *ch == '\t')
                        .map(char::len_utf8)
                        .sum::<usize>();
                    if offset == start + indent {
                        start
                    } else {
                        start + indent
                    }
                }
            };
            if modifiers.shift {
                self.select_to(target, window, cx);
            } else {
                self.move_to(target, window, cx);
            }
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[gpui::test]
    fn comments_and_line_moves_are_single_undoable_commands(cx: &mut gpui::TestAppContext) {
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let entity = cx.new(|cx| {
                let mut input = TextInput::new(
                    "Code".into(),
                    omen_core::TextValue::new("    let café = 1;\n\nnext\n".into()),
                    false,
                    None,
                    None,
                    cx,
                );
                input.multiline = true;
                input
            });
            entity.update(cx, |input, cx| {
                input.move_to(0, window, cx);
                input.run_command(Command::Comment, window, cx);
                assert!(input.content.to_string().starts_with("    // let café"));
                input.run_command(Command::Undo, window, cx);
                assert_eq!(input.content.to_string(), "    let café = 1;\n\nnext\n");
                input.run_command(Command::MoveDown, window, cx);
                assert_eq!(input.content.to_string(), "\n    let café = 1;\nnext\n");
                input.run_command(Command::Undo, window, cx);
                assert_eq!(input.content.to_string(), "    let café = 1;\n\nnext\n");
                let offset = "    let ca".len();
                assert_eq!(
                    input.content.slice(input.word_range(offset)).as_ref(),
                    "café"
                );
            });
        });
    }
    #[gpui::test]
    fn bracket_pairs_indent_delete_and_wrap_unicode_without_losing_undo(
        cx: &mut gpui::TestAppContext,
    ) {
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let entity = cx.new(|cx| {
                let mut input = TextInput::new(
                    "Code".into(),
                    omen_core::TextValue::new(String::new()),
                    false,
                    None,
                    None,
                    cx,
                );
                input.multiline = true;
                input.code_document = Some(omen_core::EditorDocument::new(
                    omen_core::Value::new(Default::default()),
                    |_| {},
                ));
                input
            });
            entity.update(cx, |input, cx| {
                let key = |key: &str| gpui::KeyDownEvent {
                    keystroke: gpui::Keystroke {
                        key: key.into(),
                        key_char: Some(key.into()),
                        ..Default::default()
                    },
                    is_held: false,
                    prefer_character_input: false,
                };
                assert!(input.pair_key(&key("{"), window, cx));
                assert_eq!(input.content.to_string(), "{}");
                assert_eq!(input.selected_range, 1..1);
                input.editor_key(&key("enter"), window, cx);
                assert_eq!(input.content.to_string(), "{\n    \n}");
                assert_eq!(input.selected_range, 6..6);
                input.run_command(Command::Undo, window, cx);
                assert_eq!(input.content.to_string(), "{}");
                input.backspace(&Backspace, window, cx);
                assert!(input.content.is_empty());
                input.replace_text_in_range(None, "café", window, cx);
                input.move_to(0, window, cx);
                input.select_to(5, window, cx);
                assert!(input.pair_key(&key("("), window, cx));
                assert_eq!(input.content.to_string(), "(café)");
                assert_eq!(input.selected_range, 1..6);
            });
        });
    }
    #[gpui::test]
    fn quote_pairs_insert_line_and_jump_to_bracket(cx: &mut gpui::TestAppContext) {
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let entity = cx.new(|cx| {
                let mut input = TextInput::new(
                    "Code".into(),
                    omen_core::TextValue::new("fn main() {\n    let s = ;\n}".into()),
                    false,
                    None,
                    None,
                    cx,
                );
                input.multiline = true;
                input.code_document = Some(omen_core::EditorDocument::new(
                    omen_core::Value::new(Default::default()),
                    |_| {},
                ));
                input
            });
            entity.update(cx, |input, cx| {
                let key = |key: &str| gpui::KeyDownEvent {
                    keystroke: gpui::Keystroke {
                        key: key.into(),
                        key_char: Some(key.into()),
                        ..Default::default()
                    },
                    is_held: false,
                    prefer_character_input: false,
                };
                // Auto-pair quote before `;` and skip over closing quote
                let eq = input.content.to_string().find(";").unwrap();
                input.move_to(eq, window, cx);
                assert!(input.pair_key(&key("\""), window, cx));
                assert!(input.content.to_string().contains("\"\";"));
                assert!(input.pair_key(&key("\""), window, cx));
                assert_eq!(input.cursor_offset(), eq + 2);
                input.move_to(eq + 1, window, cx);
                input.backspace(&Backspace, window, cx);
                assert!(!input.content.to_string().contains("\"\""));

                // Jump to matching bracket
                let open_brace = input.content.to_string().find('{').unwrap();
                let close_brace = input.content.to_string().rfind('}').unwrap();
                input.move_to(open_brace, window, cx);
                input.run_command(Command::JumpToBracket, window, cx);
                assert_eq!(input.cursor_offset(), close_brace);
                input.run_command(Command::JumpToBracket, window, cx);
                assert_eq!(input.cursor_offset(), open_brace);

                // Insert line above and below
                input.move_to(open_brace + 2, window, cx);
                input.run_command(Command::InsertLineAbove, window, cx);
                assert!(input.content.to_string().starts_with("fn main() {\n    \n"));
                input.run_command(Command::InsertLineBelow, window, cx);
                assert_eq!(input.content.line_count(), 5);
            });
        });
    }
}
