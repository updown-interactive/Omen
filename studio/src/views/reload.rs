//! Explicit confirmation before discarding an edited document.
use crate::strings::StudioStrings;
use crate::{app::StudioEnvironment, workspace::Command};
use omen::view;

view! {
    pub(super) ConfirmReload {
        @Environment app: StudioEnvironment
        @Localize strings = StudioStrings()
        let colors = app.colors();
        Layout {
            Text(strings.file_changed_on_disk_title)
                .font(.headline)
            Text(strings.file_changed_on_disk_explanation)
            Layout {
                Button(strings.overwrite_disk) {
                    if let Some(path) = (app.workspace).active_file {
                        app.send(Command::Overwrite(path));
                    }
                    app.reload_visible = false;
                }
                Button(strings.reload_and_discard_edits) {
                    if let Some(path) = (app.workspace).active_file {
                        app.send(Command::Reload(path));
                    }
                    app.reload_visible = false;
                }
                Button(strings.keep_editing) {
                    app.reload_visible = false;
                }
            }
            .mode(.horizontal)
            .spacing(8)
        }
        .padding(20)
        .spacing(12)
        .background(colors.surface_container_high)
        .color(colors.on_surface)
    }
}
