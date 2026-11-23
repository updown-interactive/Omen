use crate::app::StudioEnvironment;
use crate::strings::StudioStrings;
use omen::view;

view! {
    pub FindBar {
        @Environment app: StudioEnvironment
        @Localize strings = StudioStrings()
        let colors=app.colors();
        Layout {
            TextField("Find text")
                .autofocus(true)
                .focusRequest($app.find_focus)
                .value($app.find_text)
                .frame(170,32)
                .background(colors.surface)
                .cornerRadius(4)
            TextField("Replace with")
                .value($app.replacement_text)
                .frame(170,32)
                .background(colors.surface)
                .cornerRadius(4)
            Button(strings.next) async { app.find_next().await; }
            Button(strings.replace) async { app.replace_match(false).await; }
            Button(strings.replace_all) async { app.replace_match(true).await; }
            Button(strings.close_find) {
                app.find_visible = false;
                app.focus_editor();
            }
        }
        .mode(.horizontal)
        .spacing(10)
        .padding(6)
        .fontSize(12)
        .frame(.infinity,42)
        .scrollableHorizontal()
        .background(colors.surface_container_high)
        .color(colors.on_surface)
        .shortcut("escape", move || {
            app.find_visible = false;
            app.focus_editor();
        })
    }
}
view! {
    pub RenameDialog {
        @Environment app: StudioEnvironment
        @Localize strings = StudioStrings()
        let colors=app.colors();
        let settings = app.settings;
        Layout {
            Text(strings.rename_symbol)
                .font(.headline)
            TextField("New symbol name")
                .value($app.rename_text)
                .frame(.infinity,38)
                .background(colors.surface)
                .cornerRadius(6)
            Layout {
                Button(strings.cancel) { app.rename_visible = false; }
                Button(strings.rename_label) { app.rename_symbol(); }
            }
            .mode(.horizontal)
            .spacing(16)
        }
        .padding(24)
        .spacing(16)
        .frame(420,200)
        .theme(settings.theme())
        .background(colors.surface_container_high)
        .color(colors.on_surface)
    }
}
