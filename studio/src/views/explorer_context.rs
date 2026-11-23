//! Explorer context menu and destructive-action confirmation.
use crate::strings::StudioStrings;
use crate::{app::StudioEnvironment, workspace::Command};
use omen::view;

view! {
    pub(super) ContextMenu {
        @Environment app: StudioEnvironment
        @Localize strings = StudioStrings()
        let colors = app.colors();
        let selected = app.context;
        let is_root = selected.as_ref().is_some_and(|row|Some(&row.path)==(app.workspace).root.as_ref());
        Layout {
            if let Some(row) = selected {
                Text(row.name())
                    .font(.caption)
                    .weight(.semibold)
                    .color(colors.on_surface_variant)
                    .padding(6)
                if !is_root { Button(if row.directory { if row.expanded { "Collapse folder" } else { "Expand folder" } } else { "Open" }) { app.activate_context(); }
                    .frame(.infinity,28)
                    .alignment(.leading)
                    .padding(6)
                    .cornerRadius(4)
                    .hoverBackground(colors.selection) }
                Divider()
                Button(strings.create_new_menu) { app.open_file_chooser(); }
                .frame(.infinity,28)
                .alignment(.leading)
                .padding(6)
                .cornerRadius(4)
                .hoverBackground(colors.selection)
                Divider()
                if !is_root {
                    Button(strings.cut) { app.copy_context_entry(true); }
                    .frame(.infinity,28)
                    .alignment(.leading)
                    .padding(6)
                    .cornerRadius(4)
                    .hoverBackground(colors.selection)
                    Button(strings.copy) { app.copy_context_entry(false); }
                    .frame(.infinity,28)
                    .alignment(.leading)
                    .padding(6)
                    .cornerRadius(4)
                    .hoverBackground(colors.selection)
                }
                if (app.explorer_clipboard).is_some() { Button(strings.paste) { app.paste_context_entry(); }
                    .frame(.infinity,28)
                    .alignment(.leading)
                    .padding(6)
                    .cornerRadius(4)
                    .hoverBackground(colors.selection) }
                Button(strings.copy_path) { app.context_visible = false; }
                .desktopAction(omen::DesktopAction::CopyText(row.path.display().to_string()))
                .frame(.infinity,28)
                .alignment(.leading)
                .padding(6)
                .cornerRadius(4)
                .hoverBackground(colors.selection)
                Button(strings.copy_relative_path) { app.context_visible = false; }
                .desktopAction(omen::DesktopAction::CopyText(app.context_relative_path()))
                .frame(.infinity,28)
                .alignment(.leading)
                .padding(6)
                .cornerRadius(4)
                .hoverBackground(colors.selection)
                Button(if cfg!(target_os="macos") {"Reveal in Finder"} else if cfg!(target_os="windows") {"Reveal in File Explorer"} else {"Reveal in file manager"}) { app.context_visible = false; }
                .desktopAction(omen::DesktopAction::RevealPath(row.path.clone()))
                .frame(.infinity,28)
                .alignment(.leading)
                .padding(6)
                .cornerRadius(4)
                .hoverBackground(colors.selection)
                if !is_root {
                    Divider()
                    Button(strings.rename) { app.rename_context(); }
                    .frame(.infinity,28)
                    .alignment(.leading)
                    .padding(6)
                    .cornerRadius(4)
                    .hoverBackground(colors.selection)
                    Button(strings.move_to_trash) { app.begin_delete_context(); }
                    .frame(.infinity,28)
                    .alignment(.leading)
                    .padding(6)
                    .cornerRadius(4)
                    .hoverBackground(colors.selection)
                    .color(.red)
                }
                Divider()
                Button(strings.refresh_explorer) { app.send(Command::Refresh); app.context_visible = false; }
                .frame(.infinity,28)
                .alignment(.leading)
                .padding(6)
                .cornerRadius(4)
                .hoverBackground(colors.selection)
            }
        }
        .padding(4)
        .spacing(1)
        .fontSize(12)
        .minWidth(260)
        .background(colors.surface_container_high)
        .color(colors.on_surface)
    }
}
view! {
    pub(super) ConfirmTrash {
        @Environment app: StudioEnvironment
        @Localize strings = StudioStrings()
        let colors = app.colors();
        let settings = app.settings;
        Layout {
            Text(strings.move_to_trash_label)
                .font(.headline)
            if let Some(row)=app.context { Text(row.path.display().to_string())
                    .font(.caption) }
            Text(strings.files_and_folders_can_be_restored_from)
                .fontSize(12)
            if app.delete_resolving { Text(strings.resolving_rust_module_declarations)
                    .font(.caption) }
            if !(app.delete_module_note).is_empty() {
                Text(app.delete_module_note)
                    .font(.caption)
                    .color(colors.on_surface_variant)
            }
            if app.delete_cleanup_available {
                Checkbox("Remove Rust module declarations",$app.delete_updates)
                    .fontSize(12)
            }
            if !(app.delete_error).is_empty() { Text(app.delete_error)
                    .font(.caption)
                    .color(.red) }
            Layout {
                Button(strings.cancel) { app.cancel_delete_context(); }
                .frame(90,34)
                .background(colors.surface_container)
                .cornerRadius(6)
                Button(strings.move_to_trash_label_label) { app.delete_context(); }
                .frame(130,34)
                .background(.red)
                .color(.white)
                .cornerRadius(6)
            }
            .mode(.horizontal)
            .spacing(12)
        }
        .padding(24)
        .spacing(16)
        .frame(540,360)
        .scrollable()
        .theme(settings.theme())
        .background(colors.surface_container_high)
        .color(colors.on_surface)
        .task(move ||app.clone().inspect_delete())
    }
}
