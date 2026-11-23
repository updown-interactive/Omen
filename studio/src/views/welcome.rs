//! Session-aware startup routing and panel-free onboarding.
use super::new_project::ProjectCreator;
use crate::strings::StudioStrings;
use crate::views::StudioShell;
use crate::{app::StudioEnvironment, workspace::Command};
use omen::view;
use std::path::PathBuf;

view! {
    pub StudioLauncher {
        @Environment app: StudioEnvironment
        @Localize strings = StudioStrings()
        let colors=app.colors();
        let settings = app.settings;
        Layout {
            if !app.ready {
                Layout {Text(strings.restoring_workspace)
                        .fontSize(14)
                        .color(colors.on_surface_variant)}
                .fill()
                .alignment(.center)
            } else if app.editor_visible {
                StudioShell()
            } else {
                WelcomeScreen()
            }
        }
        .frame(.infinity,.infinity)
        .background(colors.surface)
        .color(colors.on_surface)
        .theme(settings.theme())
        .task(move ||app.receive())
        .onWindowClose(move ||app.window_close())
    }
}

view! {
    WelcomeScreen {
        @Environment app: StudioEnvironment
        @Localize strings = StudioStrings()
        let colors = app.colors();
        let settings = app.settings;
        let state = app.workspace;
        Layout {
            Layout {
                Layout {
                    Icon("file-omen-view").frame(34,34).color(colors.accent_foreground)
                }
                .frame(64, 64)
                .alignment(.center)
                .cornerRadius(16)
                .background(colors.selection)
                Text(strings.welcome_to_omen_studio)
                    .fontSize(32)
                    .weight(.semibold)
                Layout {
                    Layout {
                        Icon("file-plus").frame(20,20).color(colors.accent_foreground)
                        Button(strings.create_a_new_project) { app.create(); }
                        .frame(.infinity, 34)
                        .background(colors.selection)
                        .color(colors.on_selection)
                        .cornerRadius(8)
                        Text(strings.choose_your_platforms_build_with_omen)
                            .font(.caption)
                            .color(colors.on_surface_variant)
                    }
                    .spacing(8).padding(14)
                    .frame(310, 113)
                    .background(colors.surface_container).cornerRadius(11)
                    Layout {
                        Icon("folder-open").frame(20,20).color(colors.accent_foreground)
                        Button(strings.open_an_existing_project) {}
                        .filePicker(app.folder_picker(false))
                        .frame(.infinity, 34)
                        .background(colors.surface_container_high)
                        .cornerRadius(8)
                        Text(strings.select_a_project_folder_on_your_computer)
                            .font(.caption)
                            .color(colors.on_surface_variant)
                    }
                    .spacing(8).padding(14)
                    .frame(310, 113)
                    .background(colors.surface_container).cornerRadius(11)
                }
                .mode(.horizontal)
                .spacing(14)
                .padding(5)
                Layout {
                    Text(strings.recent_projects)
                        .fontSize(13)
                        .weight(.semibold)
                        .color(colors.on_surface_variant)
                    if !app.ready {
                        Text(strings.restoring_your_workspace)
                            .font(.caption)
                            .color(colors.on_surface_variant)
                    } else if state.recent.is_empty() {
                        Layout {
                            Text(strings.your_next_project_starts_here)
                                .fontSize(14)
                            Text(strings.projects_you_open_or_create_will_appear)
                                .font(.caption)
                                .color(colors.on_surface_variant)
                        }
                        .padding(24)
                        .spacing(8)
                        .frame(.infinity, 100)
                        .background(colors.surface_container)
                        .cornerRadius(10)
                    } else {
                        List(
                            state.recent,
                            id: |path| path.display().to_string()
                        ) { path in
                            RecentProject(path: path)
                        }
                        .virtualized(52.)
                        .frame(.infinity, 145)
                        .background(colors.surface_container)
                        .cornerRadius(10)
                    }
                }
                .spacing(12)
                .frame(634, 180)
                if !state.message.is_empty() && state.message != "Ready" && state.message != "Restoring workspace…" {
                    Text(state.message)
                        .font(.caption)
                        .color(colors.accent_foreground)
                }
                if !state.documents.is_empty() {Button(strings.recover_unsaved_files) {app.editor_visible = true;}
                    .fontSize(12)
                    .color(colors.accent_foreground)}
            }
            .alignment(.center)
            .spacing(14)
            .padding(20)
            .fill()
            .frame(.infinity, .infinity)
            .scrollable()
        }
        .frame(.infinity, .infinity)
        .theme(settings.theme())
        .gradient(colors.surface, colors.canvas, 140)
        .color(colors.on_surface)
        .dialog($app.creation_visible, move || view! { ProjectCreator() })

    }
}

view! {
    RecentProject(path: PathBuf) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let label = format!("{}   ·   {}", name, path.display());
        let accessible = format!("Open project {}", path.display());
        Button(label) { app.send(Command::OpenProject(path.clone())); }
        .frame(.infinity, 52)
        .padding(12)
        .fontSize(13)
        .color(colors.on_surface)
        .accessibilityLabel(accessible)
    }
}
