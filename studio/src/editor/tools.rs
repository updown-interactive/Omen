use crate::app::StudioEnvironment;
use crate::strings::StudioStrings;
use omen::view;

view! {
    pub FindBar {
        @Environment app: StudioEnvironment
        @Localize strings = StudioStrings()
        let colors = app.colors();
        let query = app.find_text;
        let match_count = app.find_match_count;
        let match_index = app.find_match_index;
        let case_sensitive = app.find_case_sensitive;
        let whole_word = app.find_whole_word;
        let count_label = if query.is_empty() {
            String::new()
        } else if match_count == 0 {
            "No results".into()
        } else if let Some(index) = match_index {
            format!("{index} of {match_count}")
        } else {
            format!("{match_count} matches")
        };
        Layout {
            Layout {
                TextField("Find text")
                    .autofocus(true)
                    .focusRequest($app.find_focus)
                    .value($app.find_text)
                    .onChange(move || app.refresh_find_matches())
                    .frame(170,32)
                    .background(colors.surface)
                    .cornerRadius(4)
            }
            .shortcut("enter", move || app.find_step_sync(true))
            .shortcut("shift-enter", move || app.find_step_sync(false))
            Layout {
                TextField("Replace with")
                    .focusRequest($app.replace_focus)
                    .value($app.replacement_text)
                    .frame(150,32)
                    .background(colors.surface)
                    .cornerRadius(4)
            }
            .shortcut("enter", move || app.replace_match_sync(false))
            .shortcut(if cfg!(target_os="macos") {"cmd-enter"} else {"ctrl-enter"}, move || app.replace_match_sync(true))
            Button("Aa") {
                app.find_case_sensitive = !case_sensitive;
                app.refresh_find_matches();
            }
                .frame(28, 28).cornerRadius(4)
                .background(if case_sensitive { colors.selection } else { colors.surface })
                .color(if case_sensitive { colors.on_selection } else { colors.on_surface_variant })
                .accessibilityLabel("Match case")
            Button("\\b") {
                app.find_whole_word = !whole_word;
                app.refresh_find_matches();
            }
                .frame(28, 28).cornerRadius(4)
                .background(if whole_word { colors.selection } else { colors.surface })
                .color(if whole_word { colors.on_selection } else { colors.on_surface_variant })
                .accessibilityLabel("Match whole word")
            if !count_label.is_empty() {
                Text(count_label).fontSize(11).color(colors.on_surface_variant)
            }
            Button(strings.previous) async { app.find_previous().await; }
            Button(strings.next) async { app.find_next().await; }
            Button(strings.replace) async { app.replace_match(false).await; }
            Button(strings.replace_all) async { app.replace_match(true).await; }
            Button(strings.close_find) {
                app.close_find();
            }
        }
        .mode(.horizontal)
        .spacing(6)
        .padding(6)
        .fontSize(12)
        .frame(.infinity,42)
        .scrollableHorizontal()
        .focusScope(true)
        .background(colors.surface_container_high)
        .color(colors.on_surface)
        .shortcut("f3", move || app.find_step_sync(true))
        .shortcut("shift-f3", move || app.find_step_sync(false))
        .shortcut("escape", move || {
            app.close_find();
        })
    }
}
view! {
    pub RenameDialog {
        @Environment app: StudioEnvironment
        @Localize strings = StudioStrings()
        let colors = app.colors();
        let settings = app.settings;
        Layout {
            Text(strings.rename_symbol)
                .font(.headline)
            TextField("New symbol name")
                .autofocus(true)
                .focusRequest($app.rename_focus)
                .value($app.rename_text)
                .frame(.infinity,38)
                .background(colors.surface)
                .cornerRadius(6)
            Layout {
                Button(strings.cancel) {
                    app.rename_visible = false;
                    app.focus_editor();
                }
                Button(strings.rename_label) { app.rename_symbol(); }
            }
            .mode(.horizontal)
            .spacing(16)
        }
        .padding(24)
        .spacing(16)
        .frame(420,200)
        .focusScope(true)
        .theme(settings.theme())
        .background(colors.surface_container_high)
        .color(colors.on_surface)
        .shortcut("enter", move || app.rename_symbol())
        .shortcut("escape", move || {
            app.rename_visible = false;
            app.focus_editor();
        })
    }
}
view! {
    pub GoToLineDialog {
        @Environment app: StudioEnvironment
        @Localize strings = StudioStrings()
        let colors = app.colors();
        let settings = app.settings;
        Layout {
            Text(strings.go_to_line)
                .font(.headline)
            TextField("Line or line:column (e.g. 42 or 42:8)")
                .autofocus(true)
                .focusRequest($app.goto_focus)
                .value($app.goto_text)
                .frame(.infinity,38)
                .background(colors.surface)
                .cornerRadius(6)
            Layout {
                Button(strings.cancel) {
                    app.goto_visible = false;
                    app.focus_editor();
                }
                Button(strings.go) { app.go_to_line(); }
            }
            .mode(.horizontal)
            .spacing(16)
        }
        .padding(24)
        .spacing(16)
        .frame(420,200)
        .focusScope(true)
        .theme(settings.theme())
        .background(colors.surface_container_high)
        .color(colors.on_surface)
        .shortcut("enter", move || app.go_to_line())
        .shortcut("escape", move || {
            app.goto_visible = false;
            app.focus_editor();
        })
    }
}
