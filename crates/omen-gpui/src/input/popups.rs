//! Deferred, window-clamped editor overlays. They never consume editor layout space.
use super::commands::Command;
use super::*;

const MENU: &[(&str, &str, Command)] = &[
    ("Go to Definition", "F12", Command::Definition),
    ("Rename Symbol", "F2", Command::Rename),
    ("Quick Fix / Refactor…", "⌘/Ctrl .", Command::Actions),
    ("Format Document", "⇧⌥F", Command::Format),
    ("Trigger Suggest", "Ctrl Space", Command::Complete),
    ("Undo", "⌘/Ctrl Z", Command::Undo),
    ("Redo", "⇧⌘Z / Ctrl Y", Command::Redo),
    ("Cut", "⌘/Ctrl X", Command::Cut),
    ("Copy", "⌘/Ctrl C", Command::Copy),
    ("Paste", "⌘/Ctrl V", Command::Paste),
    ("Select All", "⌘/Ctrl A", Command::SelectAll),
    ("Find / Replace…", "⌘/Ctrl F", Command::Find),
    ("Toggle Line Comment", "⌘/Ctrl /", Command::Comment),
];
impl TextInput {
    pub(super) fn dismiss_popups(&mut self) {
        self.context_position = None;
        self.hover_position = None;
        self.hover_task = None;
        if let Some(document) = &self.code_document {
            let state = document.state.get();
            if !state.completions.is_empty()
                || state.hover.is_some()
                || !state.actions.is_empty()
                || state.hover_offset.is_some()
            {
                document.state.update(|state| {
                    state.completions = Default::default();
                    state.actions = Default::default();
                    state.hover = None;
                    state.hover_offset = None;
                });
            }
        }
    }
    pub(super) fn open_context_menu(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.code_document.is_none() {
            return;
        }
        self.dismiss_popups();
        self.is_selecting = false;
        self.focus_handle.focus(window, cx);
        let offset = self.index_for_mouse_position(event.position);
        if self.selected_range.is_empty()
            || offset < self.selected_range.start
            || offset >= self.selected_range.end
        {
            self.move_to(offset, window, cx);
        }
        self.context_position = Some(event.position);
        self.context_index = 0;
        cx.stop_propagation();
        cx.notify();
    }
    pub(super) fn open_context_menu_at_caret(&mut self, cx: &mut Context<Self>) {
        if self.code_document.is_none() {
            return;
        }
        self.dismiss_popups();
        self.context_position = Some(self.caret_popup_position());
        self.context_index = 0;
        cx.notify();
    }
    pub(super) fn pointer_hover(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(document) = &self.code_document else {
            return;
        };
        if self.context_position.is_some() || !document.state.get().completions.is_empty() {
            return;
        }
        let offset = self.index_for_mouse_position(event.position);
        if document.state.get().hover_offset == Some(offset) {
            return;
        }
        self.hover_position = Some(event.position + point(px(12.), px(20.)));
        document.state.update(|state| {
            state.hover_offset = Some(offset);
            state.hover = None;
        });
        let document = document.clone();
        self.hover_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(450))
                .await;
            let _ = this.update(cx, |_, _| {
                if document.state.get().hover_offset == Some(offset) {
                    document.emit(omen_core::EditorEvent::HoverAt(offset));
                }
            });
        }));
    }
    pub(super) fn popup_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.context_position.is_none() {
            let Some(document) = &self.code_document else {
                return false;
            };
            let state = document.state.get();
            if state.actions.is_empty() {
                return false;
            }
            match event.keystroke.key.as_str() {
                "escape" => document
                    .state
                    .update(|state| state.actions = Default::default()),
                "up" => document.state.update(|state| {
                    let count = state.actions.len();
                    state.completion_index = (state.completion_index + count - 1) % count;
                }),
                "down" => document.state.update(|state| {
                    state.completion_index = (state.completion_index + 1) % state.actions.len();
                }),
                "enter" => {
                    document.emit(omen_core::EditorEvent::ApplyAction(state.completion_index));
                    document
                        .state
                        .update(|state| state.actions = Default::default());
                }
                _ => return false,
            }
            cx.notify();
            return true;
        }
        match event.keystroke.key.as_str() {
            "escape" => self.context_position = None,
            "up" => self.context_index = (self.context_index + MENU.len() - 1) % MENU.len(),
            "down" => self.context_index = (self.context_index + 1) % MENU.len(),
            "enter" => self.run_command(MENU[self.context_index].2, window, cx),
            _ => {
                self.context_position = None;
                return false;
            }
        }
        cx.notify();
        true
    }
    fn caret_popup_position(&self) -> Point<Pixels> {
        let Some(bounds) = self.last_bounds else {
            return point(px(0.), px(0.));
        };
        let offset = self.cursor_offset();
        let row = self.content.byte_to_line(offset);
        let start = self.content.line_start(row);
        let x = self
            .last_lines
            .iter()
            .find(|(at, _)| *at == start)
            .map_or(px(0.), |(_, line)| line.x_for_index(offset - start));
        point(
            bounds.left() + self.gutter_width() + x - self.scroll,
            bounds.top() + self.editor_line_height * (row + 1) as f32 - self.scroll_y,
        )
    }
    pub(super) fn editor_popups(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let Some(document) = &self.code_document else {
            return Vec::new();
        };
        let state = document.state.get();
        let dark = window.text_style().color.l > 0.5;
        let background = gpui::rgb(if dark { 0x252526 } else { 0xffffff });
        let foreground = gpui::rgb(if dark { 0xdddddd } else { 0x242424 });
        let border = gpui::rgb(if dark { 0x454545 } else { 0xd1d1d1 });
        let selected = self.accent.opacity(if dark { 0.4 } else { 0.18 });
        let panel = |id| {
            div()
                .id(id)
                .flex()
                .flex_col()
                .w(px(320.))
                .py(px(4.))
                .rounded(px(6.))
                .border_1()
                .border_color(border)
                .bg(background)
                .text_color(foreground)
                .text_size(px(12.))
                .font_family("system-ui")
                .cursor(CursorStyle::Arrow)
                .shadow_lg()
                .block_mouse_except_scroll()
                .on_mouse_down_out(cx.listener(|input, _: &MouseDownEvent, _, cx| {
                    input.dismiss_popups();
                    cx.notify();
                }))
        };
        let mut overlay = panel("editor-popup");
        let position;
        if let Some(anchor) = self.context_position {
            position = anchor;
            overlay = overlay.role(gpui::Role::Menu);
            for (index, (label, shortcut, command)) in MENU.iter().enumerate() {
                let command = *command;
                overlay = overlay.child(
                    div()
                        .id(("editor-menu-item", index))
                        .h(px(27.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .justify_between()
                        .role(gpui::Role::MenuItem)
                        .when(index == self.context_index, |row| row.bg(selected))
                        .when(matches!(index, 5 | 7 | 11), |row| {
                            row.border_t_1().border_color(border)
                        })
                        .hover(move |row| row.bg(selected))
                        .child(*label)
                        .child(div().text_color(foreground.opacity(0.55)).child(
                            if cfg!(target_os = "macos") {
                                shortcut
                                    .replace("⌘/Ctrl ", "⌘")
                                    .replace("⇧⌘Z / Ctrl Y", "⇧⌘Z")
                            } else {
                                shortcut
                                    .replace("⌘/Ctrl ", "Ctrl+")
                                    .replace("⇧⌘Z / Ctrl Y", "Ctrl+Y")
                                    .replace("⇧⌥F", "Shift+Alt+F")
                            },
                        ))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |input, _, window, cx| {
                            input.run_command(command, window, cx)
                        })),
                );
            }
        } else if !state.completions.is_empty() {
            position = self.caret_popup_position();
            overlay = overlay.role(gpui::Role::ListBox);
            let first = state.completion_index.saturating_sub(7);
            for (index, item) in state.completions.iter().enumerate().skip(first).take(8) {
                let item = item.clone();
                overlay = overlay.child(
                    div()
                        .id(("editor-suggestion", index))
                        .h(px(27.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .role(gpui::Role::ListBoxOption)
                        .when(index == state.completion_index, |row| row.bg(selected))
                        .hover(move |row| row.bg(selected))
                        .overflow_hidden()
                        .child(item.label.clone())
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |input, _, window, cx| {
                            input.accept_completion(&item, window, cx);
                            input.focus_handle.focus(window, cx);
                        })),
                );
            }
        } else if !state.actions.is_empty() {
            position = self.caret_popup_position();
            overlay = overlay.role(gpui::Role::Menu);
            for (index, title) in state
                .actions
                .iter()
                .enumerate()
                .skip(state.completion_index.saturating_sub(11))
                .take(12)
            {
                overlay = overlay.child(
                    div()
                        .id(("editor-code-action", index))
                        .when(index == state.completion_index, |row| row.bg(selected))
                        .px(px(10.))
                        .py(px(6.))
                        .hover(move |row| row.bg(selected))
                        .child(title.clone())
                        .on_click(cx.listener(move |input, _, _, cx| {
                            if let Some(document) = &input.code_document {
                                document.emit(omen_core::EditorEvent::ApplyAction(index));
                                document
                                    .state
                                    .update(|state| state.actions = Default::default());
                            }
                            cx.notify();
                        })),
                );
            }
        } else if let Some(hover) = state.hover.as_deref() {
            let diagnostics: Vec<_> = state
                .hover_offset
                .into_iter()
                .flat_map(|offset| {
                    state
                        .diagnostics
                        .iter()
                        .filter(move |item| item.range.contains(&offset))
                })
                .collect();
            if hover.trim().is_empty() && diagnostics.is_empty() {
                return Vec::new();
            }
            position = self
                .hover_position
                .unwrap_or_else(|| self.caret_popup_position());
            let viewport = window.viewport_size();
            overlay = panel("editor-documentation")
                .w(px(490.).min((viewport.width - px(16.)).max(px(1.))))
                .max_h(px(340.).min((viewport.height - px(16.)).max(px(1.))))
                .overflow_y_scroll()
                .py(px(0.))
                .rounded(px(9.))
                .child(hover::content(hover, &diagnostics, dark, self.accent));
        } else {
            return Vec::new();
        }
        vec![
            gpui::deferred(
                gpui::anchored()
                    .position(position)
                    .snap_to_window_with_margin(px(8.))
                    .child(overlay),
            )
            .with_priority(30)
            .into_any_element(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[gpui::test]
    fn context_menu_preserves_selection_and_popups_do_not_change_layout(
        cx: &mut gpui::TestAppContext,
    ) {
        let state = omen_core::Value::new(omen_core::EditorState {
            text: "let value = 42;\n".into(),
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
        let bounds = vcx.update(|window, cx| {
            input.update(cx, |input, cx| {
                let bounds = input.last_bounds.unwrap();
                input.move_to(4, window, cx);
                input.select_to(9, window, cx);
                let line = &input.last_lines[0].1;
                let position = point(
                    bounds.left() + input.gutter_width() + line.x_for_index(6),
                    bounds.top() + px(2.),
                );
                input.open_context_menu(
                    &MouseDownEvent {
                        position,
                        button: MouseButton::Right,
                        ..Default::default()
                    },
                    window,
                    cx,
                );
                assert_eq!(input.selected_range, 4..9);
                assert!(input.context_position.is_some());
                bounds
            })
        });
        vcx.run_until_parked();
        vcx.update(|_, cx| assert_eq!(input.read(cx).last_bounds, Some(bounds)));
        vcx.simulate_keystrokes("escape");
        vcx.run_until_parked();
        vcx.update(|_, cx| assert!(input.read(cx).context_position.is_none()));
        state.update(|state| {
            state.completions = std::sync::Arc::new(vec![omen_core::EditorCompletion {
                label: "value".into(),
                insert: "value".into(),
                placeholders: Vec::new(),
                additional_edits: Vec::new(),
                range: 4..9,
            }])
        });
        vcx.update(|_, cx| input.update(cx, |_, cx| cx.notify()));
        vcx.run_until_parked();
        vcx.update(|_, cx| assert_eq!(input.read(cx).last_bounds, Some(bounds)));
        state.update(|state| {
            state.completions = Default::default();
            state.hover = Some(
                "## Example\n\n```rust\n// Keep the indent\n    let text = \"hello\\nworld\";\n```"
                    .into(),
            );
        });
        vcx.update(|_, cx| input.update(cx, |_, cx| cx.notify()));
        vcx.run_until_parked();
        vcx.update(|_, cx| assert_eq!(input.read(cx).last_bounds, Some(bounds)));
    }
}
