//! Application configuration stored independently from workspace and panel layout.
use crate::{
    services,
    theme::{StudioColors, ThemePreset},
    visual::{EditorMode, Profile},
};
use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppearanceMode {
    Light,
    #[default]
    Dark,
}
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct AppearanceSettings {
    pub theme: ThemePreset,
    pub mode: AppearanceMode,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoSaveMode {
    #[default]
    Off,
    AfterDelay,
    OnFocusChange,
    OnWindowChange,
}

impl AutoSaveMode {
    pub const ALL: [Self; 4] = [
        Self::Off,
        Self::AfterDelay,
        Self::OnFocusChange,
        Self::OnWindowChange,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::AfterDelay => "After Delay",
            Self::OnFocusChange => "On Focus Change",
            Self::OnWindowChange => "On Window Change",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Off => "Save files manually.",
            Self::AfterDelay => "Save each file after you stop editing it.",
            Self::OnFocusChange => "Save a file when you leave its editor or switch tabs.",
            Self::OnWindowChange => "Save edited files when the Studio window loses focus.",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct EditorSettings {
    pub font_size: u8,
    pub line_height: u8,
    pub letter_spacing: f32,
    pub auto_save: AutoSaveMode,
    pub auto_save_delay_ms: u32,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            font_size: 13,
            line_height: 20,
            letter_spacing: 0.5,
            auto_save: AutoSaveMode::Off,
            auto_save_delay_ms: 1000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct DesignSettings {
    pub initial_mode: EditorMode,
    pub frame_profile: Profile,
    pub show_grid: bool,
}

impl Default for DesignSettings {
    fn default() -> Self {
        Self {
            initial_mode: EditorMode::Design,
            frame_profile: Profile::IPhone,
            show_grid: true,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct StudioSettings {
    pub appearance: AppearanceSettings,
    pub editor: EditorSettings,
    pub design: DesignSettings,
}
impl StudioSettings {
    pub fn sanitize(&mut self) {
        self.editor.auto_save_delay_ms = self.editor.auto_save_delay_ms.clamp(250, 10_000);
        self.editor.font_size = self.editor.font_size.clamp(10, 24);
        self.editor.line_height = self.editor.line_height.clamp(self.editor.font_size + 2, 40);
        self.editor.letter_spacing = if self.editor.letter_spacing.is_finite() {
            self.editor.letter_spacing.clamp(0., 3.)
        } else {
            EditorSettings::default().letter_spacing
        };
    }

    pub fn colors(&self) -> StudioColors {
        self.appearance
            .theme
            .colors(self.appearance.mode == AppearanceMode::Dark)
    }
    pub fn theme(&self) -> omen::Theme {
        self.colors()
            .theme(self.appearance.mode == AppearanceMode::Dark)
    }
    /// Worker-only startup load; missing files migrate legacy appearance once.
    pub fn load(directory: &Path) -> io::Result<Self> {
        match fs::read(directory.join("settings.json")) {
            Ok(bytes) => {
                let mut settings: Self =
                    serde_json::from_slice(&bytes).map_err(io::Error::other)?;
                settings.sanitize();
                Ok(settings)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                #[derive(Deserialize)]
                #[serde(default)]
                struct Legacy {
                    preset: ThemePreset,
                    dark: bool,
                }
                impl Default for Legacy {
                    fn default() -> Self {
                        Self {
                            preset: ThemePreset::Xcode,
                            dark: true,
                        }
                    }
                }
                let legacy = services::read_json::<Legacy>(&directory.join("layout.json"))?;
                let settings = Self {
                    appearance: AppearanceSettings {
                        theme: legacy.preset,
                        mode: if legacy.dark {
                            AppearanceMode::Dark
                        } else {
                            AppearanceMode::Light
                        },
                    },
                    ..Self::default()
                };
                settings.save(directory)?;
                Ok(settings)
            }
            Err(error) => Err(error),
        }
    }
    /// Persist readable JSON atomically on the filesystem worker.
    pub fn save(&self, directory: &Path) -> io::Result<()> {
        let mut bytes = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        bytes.push(b'\n');
        services::atomic_write(&directory.join("settings.json"), &bytes)
    }
}
