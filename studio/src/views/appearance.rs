use crate::strings::StudioStrings;
use crate::{
    app::StudioEnvironment,
    settings::{AppearanceMode, AutoSaveMode, EditorSettings},
    theme::{StudioColors, ThemePreset},
    visual::{EditorMode, Profile},
};
use omen::{Binding, View, ViewExt, ViewNode, view};

view! {
    pub SettingsButton {
        @State let settings_open = false
        Layout { Icon("settings").frame(18, 18) }
            .frame(28, 28)
            .alignment(.center)
            .cornerRadius(4)
            .onTap(move || settings_open = true)
            .accessibilityRole(.button)
            .accessibilityLabel("Settings")
            .accessibilityHint("Open settings")
            .dialog($settings_open, move || view! { SettingsPanel(visible: $settings_open) })
    }

    SettingsPanel(visible: Binding<bool>) {
        @Environment app: StudioEnvironment
        @Localize strings = StudioStrings()
        @State section = SettingsSection::Appearance
        let colors = app.colors();
        let settings = app.settings;
        let status = (app.workspace).message;
        let active = section;
        let selection: Binding<SettingsSection> = $section.into();
        Layout {
            Layout {
                Text(strings.settings).fontSize(20).weight(.semibold)
                    .padding(6)
                Layout {
                    for item in SettingsSection::ALL {
                        SettingsSectionItem(section: item, selected: active == item, selection: selection.clone(), colors: colors)
                            .key(item.label())
                    }
                }
                .spacing(4)
                .accessibilityRole(.tabList).accessibilityLabel("Settings sections")
            }
            .padding(12).spacing(10)
            .frame(148, .infinity)
            .background(colors.surface_container)

            Layout {}.frame(1, .infinity).background(colors.outline)

            Layout {
                Text(active.label()).fontSize(20).weight(.semibold)
                Divider()
                Layout {
                    if active == SettingsSection::Appearance { AppearancePane() }
                    if active == SettingsSection::Editor { EditorPane() }
                    if active == SettingsSection::Design { DesignPane() }
                    if active == SettingsSection::Account { AccountPane() }
                }.accessibilityRole(.tabPanel).accessibilityLabel(format!("{} settings", active.label()))
                Layout {}.fill()
                if status.starts_with("Could not save settings.json:") || status.contains("Settings recovery:") || status.starts_with("Could not save account.json:") || status.contains("Account recovery:") {
                    Text(status).font(.caption).color(colors.accent_foreground)
                }
                Layout {
                    if active == SettingsSection::Account {
                        Layout {}.fill()
                    } else {
                        Text(strings.changes_apply_immediately_and_save_automatically)
                            .font(.caption).color(colors.on_surface_variant).fill()
                    }
                    Button(strings.done) { visible.set(false); }
                        .frame(78, 30).cornerRadius(6)
                        .background(colors.selection).color(colors.on_selection)
                }.mode(.horizontal).spacing(12)
            }
            .padding(16).spacing(10).fill().frame(.infinity, .infinity)
        }
        .mode(.horizontal).alignment(.topLeading).frame(676, 444)
        .theme(settings.theme())
        .background(colors.surface_container_high)
        .color(colors.on_surface)
    }

    AccountPane {
        @Environment app: StudioEnvironment
        let saved = app.account;
        @State name = saved.name.clone()
        @State email = saved.email.clone()
        let colors = app.colors();
        let changed = name.trim() != saved.name || email.trim() != saved.email;
        Layout {
            Layout {
                Text("Name").fontSize(13).weight(.semibold)
                TextField("Full name").value($name)
                    .frame(.infinity, 34).cornerRadius(6).background(colors.surface)
                Text("Used in the Created by header of new files.")
                    .fontSize(11).color(colors.on_surface_variant)
            }.spacing(6)
            Layout {
                Text("Email").fontSize(13).weight(.semibold)
                TextField("Email address").value($email)
                    .frame(.infinity, 34).cornerRadius(6).background(colors.surface)
                Text("Stored in your local account profile.")
                    .fontSize(11).color(colors.on_surface_variant)
            }.spacing(6)

            Layout {
                Button("Save Account") { app.save_account(&name, &email); }
                    .frame(110, 30).cornerRadius(6)
                    .background(colors.selection).color(colors.on_selection)
                if !changed && (!saved.name.is_empty() || !saved.email.is_empty()) {
                    Text("Saved").font(.caption).color(colors.on_surface_variant)
                }
            }.mode(.horizontal).spacing(10)
        }.frame(.infinity, 296).padding(2).spacing(16).scrollable()
    }

    AppearancePane {
        @Environment app: StudioEnvironment
        @Localize strings = StudioStrings()
        let colors = app.colors();
        let settings = app.settings;
        Layout {
            Text(strings.interface_appearance).fontSize(13).weight(.semibold)
            Layout {
                for (mode, label) in [(AppearanceMode::Light, "Light"), (AppearanceMode::Dark, "Dark")] {
                    Button(label) { app.set_appearance(mode); }
                        .frame(104, 32).cornerRadius(6)
                        .background(if settings.appearance.mode == mode { colors.selection } else { colors.surface_container })
                        .color(if settings.appearance.mode == mode { colors.on_selection } else { colors.on_surface })
                        .accessibilityLabel(format!("Use {} appearance", label.to_lowercase()))
                }
            }.mode(.horizontal).spacing(6)
            Divider()
            Text(strings.color_theme).fontSize(13).weight(.semibold)
            Layout {
                for preset in ThemePreset::ALL { ThemeChoice(preset: preset) }
            }.mode(.horizontal).spacing(6)
        }.frame(.infinity, 296).padding(2).spacing(12).scrollable()
    }

    ThemeChoice(preset: ThemePreset) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let settings = app.settings;
        let selected = settings.appearance.theme == preset;
        let swatch = preset.colors(settings.appearance.mode == AppearanceMode::Dark);
        Layout {
            Layout {
                Layout {}.frame(8, 8).cornerRadius(2).background(swatch.primary)
            }
            .frame(18, 18).cornerRadius(5).alignment(.center)
            .background(swatch.surface)
            Text(preset.name()).fontSize(12).weight(.medium)
        }.mode(.horizontal).spacing(8).padding(9).frame(.infinity, 38).fill()
            .cornerRadius(6)
            .background(if selected { colors.selection } else { colors.surface_container })
            .color(if selected { colors.on_selection } else { colors.on_surface })
            .onTap(move || app.select_theme(preset))
            .accessibilityRole(.button)
            .accessibilityLabel(format!("Select {} theme", preset.name()))
    }

    EditorPane {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        Layout {
            AutoSaveSettings()
            Divider()
            Text("Typography").fontSize(13).weight(.semibold)
            EditorStepper(control: EditorControl::FontSize)
            EditorStepper(control: EditorControl::LineHeight)
            EditorStepper(control: EditorControl::LetterSpacing)
            Divider()
            Button("Reset typography") {
                app.update_settings(|settings| {
                    let defaults = EditorSettings::default();
                    settings.editor.font_size = defaults.font_size;
                    settings.editor.line_height = defaults.line_height;
                    settings.editor.letter_spacing = defaults.letter_spacing;
                });
            }.fontSize(12).padding(6).color(colors.accent_foreground)
        }.frame(.infinity, 296).padding(2).spacing(8).scrollable()
    }

    AutoSaveSettings {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let settings = (app.settings).editor;
        let selected = settings.auto_save;
        Layout {
            Text("Auto Save").fontSize(13).weight(.semibold)
            Layout {
                for mode in AutoSaveMode::ALL {
                    Button(mode.label()) { app.update_settings(|settings| settings.editor.auto_save = mode); }
                        .fontSize(11).frame(.infinity, 30).fill().cornerRadius(5)
                        .background(if selected == mode { colors.selection } else { colors.surface_container })
                        .color(if selected == mode { colors.on_selection } else { colors.on_surface })
                        .accessibilityLabel(format!("Auto Save: {}", mode.label()))
                }
            }.mode(.horizontal).spacing(5)
            Text(selected.description()).fontSize(11).color(colors.on_surface_variant)
            if selected == AutoSaveMode::AfterDelay {
                EditorStepper(control: EditorControl::AutoSaveDelay)
            }
        }.spacing(7)
    }

    EditorStepper(control: EditorControl) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let value = control.display(&(app.settings).editor);
        let label = control.label();
        let decrease = format!("Decrease {}", label.to_lowercase());
        let increase = format!("Increase {}", label.to_lowercase());
        Layout {
            Layout {
                Text(label).fontSize(13).weight(.medium)
                Text(control.description()).fontSize(11).color(colors.on_surface_variant)
            }.spacing(2).fill()
            Layout {
                Button("−") { app.update_settings(|settings| control.step(&mut settings.editor, -1)); }
                    .frame(28, 28).cornerRadius(4).accessibilityLabel(decrease)
                Layout { Text(value).fontFamily("monospace").fontSize(12) }
                    .frame(72, 28).alignment(.center)
                Button("+") { app.update_settings(|settings| control.step(&mut settings.editor, 1)); }
                    .frame(28, 28).cornerRadius(4).accessibilityLabel(increase)
            }.mode(.horizontal).background(colors.surface).cornerRadius(5)
        }.mode(.horizontal).spacing(12).padding(9).cornerRadius(6)
            .background(colors.surface_container)
    }

    DesignPane {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let settings = (app.settings).design;
        Layout {
            Text("Default editor mode").fontSize(13).weight(.semibold)
            Layout {
                for (label, mode) in [("Design", EditorMode::Design), ("Code", EditorMode::Code), ("Split", EditorMode::Split)] {
                    Button(label) { app.update_settings(|settings| settings.design.initial_mode = mode); }
                        .frame(72, 28).cornerRadius(5)
                        .background(if settings.initial_mode == mode { colors.selection } else { colors.surface_container })
                        .color(if settings.initial_mode == mode { colors.on_selection } else { colors.on_surface })
                        .accessibilityLabel(format!("Use {label} as the default editor mode"))
                }
            }.mode(.horizontal).spacing(6)
            Text("Applies to @Design files without a saved mode.")
                .fontSize(11).color(colors.on_surface_variant)
            Divider()
            Text("Default frame").fontSize(13).weight(.semibold)
            AdaptiveGrid {
                for profile in Profile::ALL {
                    Button(profile.label()) { app.update_settings(|settings| settings.design.frame_profile = profile); }
                        .fontSize(11).frame(.infinity, 25).cornerRadius(5)
                        .background(if settings.frame_profile == profile { colors.selection } else { colors.surface_container })
                        .color(if settings.frame_profile == profile { colors.on_selection } else { colors.on_surface })
                        .accessibilityLabel(format!("Use {} as the default frame", profile.label()))
                }
            }.min_item_width(106).spacing(5)
            Checkbox("Show canvas grid", app.design_grid()).fontSize(12)
        }.frame(.infinity, 296).padding(2).spacing(8).scrollable()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsSection {
    Appearance,
    Editor,
    Design,
    Account,
}

impl SettingsSection {
    const ALL: [Self; 4] = [Self::Appearance, Self::Editor, Self::Design, Self::Account];

    fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Editor => "Editor",
            Self::Design => "Design",
            Self::Account => "Account",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Appearance => "settings",
            Self::Editor => "file-code",
            Self::Design => "file-omen-view",
            Self::Account => "file",
        }
    }
}

struct SettingsSectionItem {
    section: SettingsSection,
    selected: bool,
    selection: Binding<SettingsSection>,
    colors: StudioColors,
}

impl View for SettingsSectionItem {
    fn into_node(self) -> ViewNode {
        let Self {
            section,
            selected,
            selection,
            colors,
        } = self;
        view! {
            Layout {
                Icon(section.icon()).frame(15, 15)
                Text(section.label()).fontSize(13).weight(.medium)
            }.mode(.horizontal).spacing(9).padding(8).frame(.infinity, 34)
                .cornerRadius(6)
                .background(if selected { colors.selection } else { colors.surface_container })
                .hoverBackground(if selected { colors.selection } else { colors.surface_container_high })
                .color(if selected { colors.on_selection } else { colors.on_surface })
                .accessibilityRole(.tab).accessibilityLabel(section.label())
        }
        .on_tap(move || selection.set(section))
        .accessibility_selected(selected)
        .into_node()
    }
}

#[derive(Clone, Copy)]
enum EditorControl {
    AutoSaveDelay,
    FontSize,
    LineHeight,
    LetterSpacing,
}

impl EditorControl {
    fn label(self) -> &'static str {
        match self {
            Self::AutoSaveDelay => "Auto Save delay",
            Self::FontSize => "Font size",
            Self::LineHeight => "Line height",
            Self::LetterSpacing => "Letter spacing",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::AutoSaveDelay => "Idle time before saving an edited file.",
            Self::FontSize => "Size of text in the code editor.",
            Self::LineHeight => "Vertical space used by each line.",
            Self::LetterSpacing => "Extra space between characters.",
        }
    }

    fn display(self, settings: &EditorSettings) -> String {
        match self {
            Self::AutoSaveDelay => format!("{} ms", settings.auto_save_delay_ms),
            Self::FontSize => format!("{} px", settings.font_size),
            Self::LineHeight => format!("{} px", settings.line_height),
            Self::LetterSpacing => format!("{:.1} px", settings.letter_spacing),
        }
    }

    fn step(self, settings: &mut EditorSettings, delta: i8) {
        match self {
            Self::AutoSaveDelay => {
                settings.auto_save_delay_ms = settings
                    .auto_save_delay_ms
                    .saturating_add_signed(i32::from(delta) * 250)
            }
            Self::FontSize => settings.font_size = settings.font_size.saturating_add_signed(delta),
            Self::LineHeight => {
                settings.line_height = settings.line_height.saturating_add_signed(delta)
            }
            Self::LetterSpacing => settings.letter_spacing += f32::from(delta) * 0.5,
        }
    }
}
