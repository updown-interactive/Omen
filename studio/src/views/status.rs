use crate::app::StudioEnvironment;
use omen::view;
view! {
    pub StudioStatusBar {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let state = app.workspace;
        let run = app.run_status;
        let console_visible = app.console_visible;
        let count = state.unsaved_count();
        Layout {
            Icon(if state.message == "Ready" { "check" } else { "info" })
                .frame(12,12).color(if state.message == "Ready" { omen::Color::Green } else { colors.on_surface_variant })
            Text(state.message).fontSize(11).truncate().fill()
            if !run.message.is_empty() {
                Text(run.message).fontSize(11).truncate().fill()
                    .color(if run.failed { omen::Color::Red } else { colors.on_surface_variant })
            }
            if count > 0 {
                Text("{count} unsaved").fontSize(11).color(colors.on_surface_variant)
            }
            if state.root.is_some() {
                Button("Debug Console") { app.toggle_console(); }
                    .frame(105, 21).cornerRadius(4).fontSize(11)
                    .color(if console_visible { colors.on_selection } else { colors.on_surface_variant })
                    .background(if console_visible { colors.selection } else { colors.surface_container })
                    .hoverBackground(colors.surface_container_high)
                    .accessibilityLabel(if console_visible { "Hide Debug Console" } else { "Show Debug Console" })
            }
        }
        .mode(.horizontal)
        .spacing(7)
        .padding(6)
        .frame(.infinity, 26)
        .background(colors.surface_container)
        .color(colors.on_surface)
    }
}
