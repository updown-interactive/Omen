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
