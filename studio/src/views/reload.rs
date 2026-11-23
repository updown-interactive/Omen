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
            Text(strings.discard_edits_and_reload)
                .font(.headline)
            Text(strings.this_replaces_this_tab_s_unsaved_text)
            Button(strings.reload_and_discard_edits) {
                if let Some(path) = (app.workspace).active_file { app.send(Command::Reload(path)); }
                app.reload_visible = false;
            }
            Button(strings.keep_editing) { app.reload_visible = false; }
        }
        .padding(20)
        .spacing(12)
        .background(colors.surface_container_high)
        .color(colors.on_surface)
    }
}
