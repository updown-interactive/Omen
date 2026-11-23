//! Run controls and the project-scoped device menu in the native toolbar.
use super::menu;
use crate::app::StudioEnvironment;
use omen::{Binding, view};
use omen_cli::studio::RunTarget;

view! {
    pub(super) RunControls(visible: Binding<bool>) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let status = app.run_status;
        let toggle = visible.clone();
        let dismiss = visible.clone();
        Layout {
            if status.active {
                Button("■  Stop") { app.stop_run(); }
                    .frame(72, 27)
                    .cornerRadius(5)
                    .hoverBackground(colors.surface_container)
                    .accessibilityLabel("Stop running application")
                if !status.stopping {
                    Button("↻  Restart") { app.restart_run(); }
                        .frame(86, 27)
                        .cornerRadius(5)
                        .hoverBackground(colors.surface_container)
                        .accessibilityLabel("Restart running application")
                }
            } else {
                Button("▶  Run  ▾") {
                    let opening = !toggle.get();
                    if opening { app.discover_run_devices(); }
                    toggle.set(opening);
                }
                .frame(86, 27)
                .cornerRadius(5)
                .background(colors.selection)
                .color(colors.on_selection)
                .hoverBackground(colors.surface_container)
                .accessibilityLabel("Choose a run device")
                .popover(visible, move || menu::panel(view! { RunDeviceMenu(visible: dismiss.clone()) }))
            }
        }.mode(.horizontal).spacing(3)
    }

    RunDeviceMenu(visible: Binding<bool>) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let settings = app.settings;
        let data = app.run_menu;
        let status = app.run_status;
        let rows = data.devices.targets.len();
        let issues = data.devices.issues.len();
        let content_height = (rows as f32 * 44. + issues as f32 * 32. + 12.).clamp(38., 308.);
        Layout {
            Text("Run on").fontSize(12).weight(.semibold).padding(8)
            Divider()
            if data.loading {
                Text("Finding available devices…").fontSize(12)
                    .color(colors.on_surface_variant).padding(8)
            } else {
                Layout {
                    for target in data.devices.targets {
                        RunDeviceRow(target: target.clone(), visible: visible.clone())
                            .key(format!("{}:{:?}", target.platform, target.device))
                    }
                    for issue in data.devices.issues {
                        Text(issue).fontSize(11).truncate().frame(.infinity, 30)
                            .color(colors.on_surface_variant).padding(6)
                    }
                }.frame(.infinity, content_height).spacing(2).scrollable()
            }
            if status.failed {
                Divider()
                Text(status.message).fontSize(11).truncate()
                    .frame(.infinity, 30).padding(6).color(omen::Color::Red)
            }
        }
        .frame(320, 48. + if data.loading { 36. } else { content_height } + if status.failed { 34. } else { 0. })
        .padding(4).spacing(2)
        .theme(settings.theme()).background(colors.surface_container_high).color(colors.on_surface)
    }

    RunDeviceRow(target: RunTarget, visible: Binding<bool>) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let selection = target.clone();
        Button(format!("{}  ·  {}", target.name, target.detail)) {
            visible.set(false);
            app.start_run(selection.clone());
        }
        .frame(.infinity, 42).alignment(.leading)
        .padding(8).cornerRadius(4).fontSize(12)
        .hoverBackground(colors.selection)
        .accessibilityLabel(format!("Run on {}", target.name))
    }
}
