//! A compact project form with native input and clear action hierarchy.
use crate::app::StudioEnvironment;
use crate::strings::StudioStrings;
use omen::{Binding, view};
use omen_cli::project::manifest::TargetPlatform;

view! {
    pub ProjectCreator {
        @Environment app: StudioEnvironment
        @Localize strings = StudioStrings()
        let colors = app.colors();
        let settings = app.settings;
        let creating = app.creating_project();
        let location = app.project_parent;
        Layout {
            Layout {
                Text(strings.new_project)
                    .fontSize(24)
                    .weight(.semibold)
                Text(strings.set_up_your_app_and_choose_where)
                    .fontSize(13)
                    .color(colors.on_surface_variant)
            }
            .spacing(6)
            Divider()
            Layout {
                Layout {
                    ProjectField(
                        title: "Project name".to_string(),
                        placeholder: "my-app".to_string(),
                        value: app.project_name_input()
                    )
                    .fill()
                    ProjectField(
                        title: "Application ID".to_string(),
                        placeholder: "com.example.myapp".to_string(),
                        value: app.project_identifier()
                    )
                    .fill()
                }
                .mode(.horizontal)
                .spacing(16)
                Text(strings.use_lowercase_letters_numbers_hyphens_or_underscores)
                    .font(.caption)
                    .color(colors.on_surface_variant)
                Layout {
                    Text(strings.location)
                        .fontSize(12)
                        .weight(.semibold)
                    Layout {
                        Layout {
                            TextField("Parent folder")
                                .value($app.project_parent)
                                .fontSize(13)
                                .frame(.infinity, 38)
                                .background(colors.surface)
                                .cornerRadius(7)
                        }
                        .padding(1)
                        .background(colors.outline)
                        .cornerRadius(8).fill()
                        Button(strings.choose_folder) {}
                        .filePicker(app.folder_picker(true))
                        .frame(128, 40)
                        .background(colors.surface_container)
                        .color(colors.accent_foreground)
                        .cornerRadius(8)
                    }
                    .mode(.horizontal)
                    .spacing(10)
                    if location.is_empty() {
                        Text(strings.choose_a_folder_for_your_new_project)
                            .font(.caption)
                            .color(colors.on_surface_variant)
                    } else {
                        Text(std::path::Path::new(&location).join(app.project_name).display().to_string())
                            .font(.caption)
                            .color(colors.on_surface_variant)
                    }
                }
                .spacing(8)
                Layout {
                    Text(strings.target_platforms)
                        .fontSize(12)
                        .weight(.semibold)
                    Layout {
                        PlatformChoice(
                            target: TargetPlatform::Desktop,
                            title: "Desktop".to_string(),
                            detail: "macOS, Windows, Linux".to_string()
                        )
                        .fill()
                        PlatformChoice(
                            target: TargetPlatform::Android,
                            title: "Android".to_string(),
                            detail: "Phones and tablets".to_string()
                        )
                        .fill()
                    }
                    .mode(.horizontal)
                    .spacing(12)
                    Layout {
                        PlatformChoice(
                            target: TargetPlatform::Ios,
                            title: "iOS".to_string(),
                            detail: "iPhone and iPad".to_string()
                        )
                        .fill()
                        PlatformChoice(
                            target: TargetPlatform::Web,
                            title: "Web".to_string(),
                            detail: "Browser applications".to_string()
                        )
                        .fill()
                    }
                    .mode(.horizontal)
                    .spacing(12)
                    Text(strings.select_at_least_one_platform_build_tools)
                        .font(.caption)
                        .color(colors.on_surface_variant)
                }
                .spacing(10)
            }
            .spacing(14)
            .fill()
            .frame(.infinity, .infinity)
            .scrollable()
            if !(app.creation_error).is_empty() {
                Text(app.creation_error)
                    .font(.caption)
                    .color(if creating { colors.accent_foreground } else { omen::Color::Red })
                    .padding(8)
                    .frame(.infinity, 40)
                    .background(colors.surface_container)
                    .cornerRadius(6)
            }
            Divider()
            Layout {
                Layout {}
                .fill()
                Button(strings.cancel) { app.cancel_create(); }
                .frame(88, 38)
                .background(colors.surface_container)
                .color(colors.on_surface)
                .cornerRadius(8)
                Button(if creating { "Creating…" } else { "Create project" }) { app.submit_create(); }
                .frame(144, 38)
                .background(colors.primary)
                .color(colors.on_primary)
                .cornerRadius(8)
                .weight(.semibold)
            }
            .mode(.horizontal)
            .spacing(10)
        }
        .padding(24)
        .spacing(16)
        .frame(640, 580)
        .theme(settings.theme())
        .background(colors.surface_container_high)
        .color(colors.on_surface)
    }
}

view! {
    ProjectField(title: String, placeholder: String, value: Binding<String>) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        Layout {
            Text(title.clone())
                .fontSize(12)
                .weight(.semibold)
            Layout {
                TextField(placeholder)
                    .value(value)
                    .accessibilityLabel(title)
                    .fontSize(13)
                    .frame(.infinity, 38)
                    .background(colors.surface)
                    .cornerRadius(7)
            }
            .padding(1)
            .background(colors.outline)
            .cornerRadius(8)
        }
        .spacing(8)
        .frame(.infinity, 64)
    }
}

view! {
    PlatformChoice(target: TargetPlatform, title: String, detail: String) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let selected = app.target(target).get();
        Layout {
            Layout {
                Checkbox(title)
                    .checked(app.target(target))
                    .fontSize(13)
                    .weight(.medium)
                Text(detail)
                    .font(.caption)
                    .color(colors.on_surface_variant)
            }
            .padding(10)
            .spacing(4)
            .frame(.infinity, 64)
            .background(if selected { colors.selection } else { colors.surface_container })
            .cornerRadius(7)
        }
        .padding(1)
        .background(if selected { colors.primary } else { colors.outline })
        .cornerRadius(8)
    }
}
