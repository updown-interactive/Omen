//! Searchable project item chooser.
use crate::{app::StudioEnvironment, strings::StudioStrings};
use omen::view;

view! {
    pub CreationPalette {
        @Environment app: StudioEnvironment
        @Localize strings = StudioStrings()
        let colors=app.colors();
        let settings = app.settings;
        let query=(app.palette_query).to_lowercase();
        let dark=settings.appearance.mode==crate::settings::AppearanceMode::Dark;
        Layout {
            Layout {
                Text(strings.create_new)
                    .fontSize(19)
                    .weight(.semibold)
                Text(strings.create_new_description)
                    .fontSize(11)
                    .color(colors.on_surface_variant)
            }.spacing(3)
            Layout {
                TextField("Search types…")
                    .autofocus(true)
                    .value($app.palette_query)
                    .onChange(move ||app.palette_index.set(0))
                    .frame(.infinity,32)
                    .background(colors.surface)
                    .cornerRadius(6)
            }
            .padding(1)
            .background(colors.outline)
            .cornerRadius(7)
            .shortcut("down",move ||app.palette_move(true))
            .shortcut("up",move ||app.palette_move(false))
            .shortcut("enter",move ||app.activate_palette())
            Layout {
                for (index,kind) in crate::workspace::file_kind::StudioFileKind::CREATABLE.into_iter().filter(|kind|kind.matches(&query)).enumerate() {
                    Layout {
                        Icon(kind.icon())
                            .frame(20,20)
                            .minWidth(20)
                            .color(omen::Color::Rgb(kind.tint(dark)))
                        Layout {
                            Text(kind.title())
                                .fontSize(12)
                                .weight(.semibold)
                            Text(kind.description())
                                .fontSize(11)
                                .truncate()
                                .color(colors.on_surface_variant)
                        }
                        .spacing(1)
                        .fill()
                        .alignment(.topLeading)
                    }
                    .mode(.horizontal)
                    .spacing(8)
                    .padding(5)
                    .frame(.infinity,38)
                    .key(kind.title())
                    .cornerRadius(7)
                    .background(if app.palette_index==index {colors.selection} else {colors.surface_container_high})
                    .hoverBackground(colors.selection)
                    .onTap(move ||app.begin_typed_entry(kind))
                    .accessibilityRole(.button)
                    .accessibilityLabel(kind.label())
                    .accessibilityHint(kind.description())
                }
                if !crate::workspace::file_kind::StudioFileKind::CREATABLE.into_iter().any(|kind|kind.matches(&query)) {
                    Text(strings.no_matching_file_types)
                        .fontSize(12)
                        .padding(16)
                        .color(colors.on_surface_variant)
                }
            }
            .spacing(2)
            .frame(.infinity,278)
            .scrollable()
            Layout {
                Text("↑ ↓ Select   ·   ↵ Continue   ·   Esc Close")
                    .fontSize(10)
                    .color(colors.on_surface_variant)
                    .fill()
                Button(strings.cancel) {app.palette_visible = false;}
                    .fontSize(11)
                    .frame(65,26)
                    .background(colors.surface_container)
                    .cornerRadius(5)
            }.mode(.horizontal).spacing(8)
        }
        .padding(14)
        .spacing(8)
        .frame(464,440)
        .focusScope(true)
        .theme(settings.theme())
        .background(colors.surface_container_high)
        .color(colors.on_surface)
    }
}

view! {
    QuickOpenLoader {
        @Environment app: StudioEnvironment
        let loader = app.clone();
        Layout {}.frame(0, 0).task(move || {
            let app = loader.clone();
            async move { app.load_quick_open_files().await; }
        })
    }

    pub QuickOpenPalette {
        @Environment app: StudioEnvironment
        @Localize strings = StudioStrings()
        let colors = app.colors();
        let settings = app.settings;
        let query = (app.quick_open_query).trim().to_lowercase();
        let root = (app.workspace).root;
        let files: Vec<_> = app.quick_open_files.get().into_iter().filter(|path| {
            if query.is_empty() {
                return true;
            }
            let display = root.as_ref().and_then(|r| path.strip_prefix(r).ok()).unwrap_or(path).to_string_lossy().to_lowercase();
            display.contains(&query)
        }).take(100).enumerate().map(|(index, path)| {
            let display = root.as_ref().and_then(|r| path.strip_prefix(r).ok()).unwrap_or(&path).display().to_string();
            let icon = crate::state::TreeRow {
                path: path.clone(),
                depth: 0,
                directory: false,
                expanded: false,
            }.icon();
            (index, path, display, icon)
        }).collect();
        Layout {
            QuickOpenLoader().key(format!("quick-open:{root:?}"))
            Text(strings.quick_open)
                .fontSize(19)
                .weight(.semibold)
            Layout {
                TextField("Search project files…")
                    .autofocus(true)
                    .focusRequest($app.quick_open_focus)
                    .value($app.quick_open_query)
                    .onChange(move || app.quick_open_index.set(0))
                    .frame(.infinity,32)
                    .background(colors.surface)
                    .cornerRadius(6)
            }
            .padding(1)
            .background(colors.outline)
            .cornerRadius(7)
            .shortcut("down", move || app.quick_open_move(true))
            .shortcut("up", move || app.quick_open_move(false))
            .shortcut("enter", move || app.activate_quick_open())
            Layout {
                for (index, open_path, display, icon) in files.iter().cloned() {
                    Layout {
                        Icon(icon).frame(16, 16)
                        Text(display.clone()).fontSize(12).truncate().fill()
                    }
                    .mode(.horizontal)
                    .spacing(8)
                    .padding(6)
                    .frame(.infinity, 30)
                    .key(display)
                    .cornerRadius(6)
                    .background(if app.quick_open_index == index { colors.selection } else { colors.surface_container_high })
                    .hoverBackground(colors.selection)
                    .onTap(move || {
                        app.quick_open_visible = false;
                        app.send(crate::workspace::Command::OpenFile(open_path.clone()));
                    })
                    .accessibilityRole(.button)
                }
                if files.is_empty() {
                    Text(strings.no_matching_files)
                        .fontSize(12)
                        .padding(16)
                        .color(colors.on_surface_variant)
                }
            }
            .spacing(2)
            .frame(.infinity, 278)
            .scrollable()
            Layout {
                Text("↑ ↓ Select   ·   ↵ Open   ·   Esc Close")
                    .fontSize(10)
                    .color(colors.on_surface_variant)
                    .fill()
                Button(strings.cancel) {
                    app.quick_open_visible = false;
                    app.focus_editor();
                }
                    .fontSize(11)
                    .frame(65, 26)
                    .background(colors.surface_container)
                    .cornerRadius(5)
            }.mode(.horizontal).spacing(8)
        }
        .padding(14)
        .spacing(8)
        .frame(500, 440)
        .focusScope(true)
        .theme(settings.theme())
        .background(colors.surface_container_high)
        .color(colors.on_surface)
        .shortcut("escape", move || {
            app.quick_open_visible = false;
            app.focus_editor();
        })
    }
}
