//! Live output for the current or most recent run session.
use crate::app::StudioEnvironment;
use omen::view;

view! {
    pub(super) DebugConsole {
        @Environment app: StudioEnvironment
        @Environment viewport
        let colors = app.colors();
        let output = app.console_output;
        let status = app.run_status;
        let height = (viewport.height * 0.27).clamp(125., 280.);
        Layout {
            Layout {
                Text("Debug Console").fontSize(12).weight(.semibold)
                Layout {}.fill()
                Button("Hide") { app.toggle_console(); }
                    .frame(52, 22).fontSize(11).cornerRadius(4)
                    .hoverBackground(colors.surface_container_high)
                    .accessibilityLabel("Close Debug Console panel")
            }
            .mode(.horizontal).frame(.infinity, 32).padding(.sm)
            .background(colors.surface_container)
            Divider()
            Layout {
                if !output.is_empty() {
                    Text(output).fontFamily("monospace").fontSize(11).lineHeight(16)
                } else if status.failed {
                    Text(status.message).fontFamily("monospace").fontSize(11).color(omen::Color::Red)
                } else {
                    Text(if status.active { "Waiting for run output…" } else { "Run the project to see its output." })
                        .fontSize(11).color(colors.on_surface_variant)
                }
            }
            .padding(8).fill().followScrollEnd()
            .background(colors.surface)
        }
        .frame(.infinity, height)
        .color(colors.on_surface)
    }
}
