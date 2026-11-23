//! Application menus hosted in the native window title bar.
use super::{appearance::SettingsButton, menu, run::RunControls};
use crate::state::{StudioMenu, ToolbarState};
use crate::strings::StudioStrings;
use crate::{app::StudioEnvironment, workspace::Command};
use omen::{Binding, FilePicker, view};

view! {
    pub StudioTopbar {
        @Environment app: StudioEnvironment
        @Localize strings = StudioStrings()
        let colors = app.colors();
        let settings = app.settings;
        @State menus = ToolbarState()
        let file_visible = menu_visibility($menus.active_menu, StudioMenu::File);
        let window_visible = menu_visibility($menus.active_menu, StudioMenu::Window);
        let file_close = file_visible.clone();
        let window_close = window_visible.clone();
        Layout {
            Text(strings.omen_studio)
                .fontSize(13)
                .weight(.semibold)
            Button(strings.file) { menus.toggle(StudioMenu::File); }
            .padding(6)
            .cornerRadius(4)
            .background(if menus.active_menu == StudioMenu::File { colors.surface_container } else { colors.surface_container_high })
            .popover(file_visible, move || menu::panel(view! { FileMenu(visible: file_close.clone()) }))
            Button(strings.window) { menus.toggle(StudioMenu::Window); }
            .padding(6)
            .cornerRadius(4)
            .background(if menus.active_menu == StudioMenu::Window { colors.surface_container } else { colors.surface_container_high })
            .popover(window_visible, move || menu::panel(view! { WindowMenu(visible: window_close.clone()) }))
            Layout {}
            .fill()
            if (app.workspace).root.is_some() {
                RunControls(visible: menu_visibility($menus.active_menu, StudioMenu::Run))
            }
            SettingsButton()
        }
        .mode(.horizontal)
        .spacing(10)
        .padding(8)
        .frame(.infinity, .infinity)
        .theme(settings.theme())
        .background(colors.surface_container_high)
        .color(colors.on_surface)
    }
}

view! {
    FileMenu(visible: Binding<bool>) {
        @Environment app: StudioEnvironment
        @Localize strings = StudioStrings()
        let colors = app.colors();
        let settings = app.settings;
        let create_menu = visible.clone();
        let save_menu = visible.clone();
        let save_all_menu = visible.clone();
        let palette_menu=visible.clone();
        let close_menu=visible.clone();
        let picker = app.folder_picker(false);
        let dismiss = visible.clone();
        let picker = FilePicker::folders(move |result| { dismiss.set(false); picker.complete(result); });
        Layout {
            Button(strings.new_project) { create_menu.set(false); app.create(); }
            .frame(.infinity, 32)
            .alignment(.leading)
            .padding(6)
            .cornerRadius(4)
            .hoverBackground(colors.selection)
            Button(strings.open_project) {}
            .filePicker(picker)
            .frame(.infinity, 32)
            .alignment(.leading)
            .padding(6)
            .cornerRadius(4)
            .hoverBackground(colors.selection)
            if (app.workspace).root.is_some() {
                Button(strings.close_project) {close_menu.set(false);app.close_project();}
                .frame(.infinity,32)
                .alignment(.leading)
                .padding(6)
                .cornerRadius(4)
                .hoverBackground(colors.selection)
            }
            Button(strings.command_palette) {palette_menu.set(false);app.open_file_chooser();}
            .frame(.infinity,32)
            .alignment(.leading)
            .padding(6)
            .cornerRadius(4)
            .hoverBackground(colors.selection)
            Divider()
            Button(strings.save) { save_menu.set(false); app.save_active(); }
            .frame(.infinity, 32)
            .alignment(.leading)
            .padding(6)
            .cornerRadius(4)
            .hoverBackground(colors.selection)
            Button(strings.save_all) { save_all_menu.set(false); app.send(Command::SaveAll); }
            .frame(.infinity, 32)
            .alignment(.leading)
            .padding(6)
            .cornerRadius(4)
            .hoverBackground(colors.selection)
        }
        .frame(210, if (app.workspace).root.is_some() {220} else {185})
        .padding(4)
        .spacing(2)
        .fontSize(13)
        .theme(settings.theme())
        .background(colors.surface_container_high)
        .color(colors.on_surface)
    }
}

view! {
    WindowMenu(visible: Binding<bool>) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let settings = app.settings;
        let layout = app.layout;
        let explorer_menu = visible.clone();
        let preview_menu = visible.clone();
        let inspector_focus_menu = visible.clone();
        let editor_focus_menu = visible.clone();
        Layout {
            Button(if layout.sidebar_visible { "Hide Sidebar" } else { "Show Sidebar" }) {
                explorer_menu.set(false);
                app.update_layout(|layout| layout.sidebar_visible = !layout.sidebar_visible);
            }
            .frame(.infinity, 32)
            .alignment(.leading)
            .padding(6)
            .cornerRadius(4)
            .hoverBackground(colors.selection)
            .accessibilityLabel("Toggle sidebar")
            Button(if layout.preview_visible { "Hide Inspector" } else { "Show Inspector" }) {
                preview_menu.set(false);
                app.update_layout(|layout| layout.preview_visible = !layout.preview_visible);
            }
            .frame(.infinity, 32)
            .alignment(.leading)
            .padding(6)
            .cornerRadius(4)
            .hoverBackground(colors.selection)
            .accessibilityLabel("Toggle inspector")
            Divider()
            if layout.preview_visible {
                Button(if cfg!(target_os="macos") {"Focus Inspector    ⌘I"} else {"Focus Inspector    Ctrl+I"}) {inspector_focus_menu.set(false);app.focus_inspector();}
                .frame(.infinity,32)
                .alignment(.leading)
                .padding(6)
                .cornerRadius(4)
                .hoverBackground(colors.selection)
            }
            Button(if cfg!(target_os="macos") {"Focus Editor    ⌘1"} else {"Focus Editor    Ctrl+1"}) {editor_focus_menu.set(false);app.focus_editor();}
            .frame(.infinity,32)
            .alignment(.leading)
            .padding(6)
            .cornerRadius(4)
            .hoverBackground(colors.selection)
        }
        .frame(240, 150)
        .padding(4)
        .spacing(2)
        .fontSize(13)
        .theme(settings.theme())
        .background(colors.surface_container_high)
        .color(colors.on_surface)
    }
}

fn menu_visibility(menu: impl Into<Binding<StudioMenu>>, kind: StudioMenu) -> Binding<bool> {
    let menu = menu.into();
    let read = menu.clone();
    Binding::new(
        move || read.get() == kind,
        move |visible| {
            if visible {
                menu.set(kind);
            } else if menu.get() == kind {
                menu.set(StudioMenu::Closed);
            }
        },
    )
}
