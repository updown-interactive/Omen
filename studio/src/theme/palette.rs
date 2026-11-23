//! Studio semantic colors for each preset and appearance mode.
use omen::{Color, DesignTokens, Theme};
use serde::{Deserialize, Serialize};

const PRIMARY: Color = Color::Rgb(0xe9ad76);
const ON_PRIMARY: Color = Color::Rgb(0x27180f);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreset {
    #[default]
    Xcode,
    Midnight,
    Graphite,
}
impl ThemePreset {
    pub const ALL: [Self; 3] = [Self::Xcode, Self::Midnight, Self::Graphite];
    pub fn name(self) -> &'static str {
        match self {
            Self::Xcode => "Default",
            Self::Midnight => "Midnight",
            Self::Graphite => "Graphite",
        }
    }
    pub fn colors(self, dark: bool) -> StudioColors {
        let colors = match (self, dark) {
            (Self::Xcode, true) => {
                return StudioColors {
                    canvas: Color::Rgb(0x080809),
                    grid: Color::Rgb(0x1c1c20),
                    surface: Color::Rgb(0x0d0d0f),
                    on_surface: Color::Rgb(0xeaeaed),
                    surface_container: Color::Rgb(0x111113),
                    surface_container_high: Color::Rgb(0x1b1b1e),
                    primary: PRIMARY,
                    accent_foreground: PRIMARY,
                    on_primary: ON_PRIMARY,
                    secondary: Color::Rgb(0xbcc4d0),
                    on_surface_variant: Color::Rgb(0x92929a),
                    outline: Color::Rgb(0x29292d),
                    selection: Color::Rgb(0x241e19),
                    on_selection: Color::Rgb(0xf0c59d),
                };
            }
            (Self::Xcode, false) => [
                0xffffff, 0x252527, 0xf3f3f5, 0xe7e7ea, 0x7652a6, 0x64646b, 0xceced3, 0xdbeaff,
                0x002f6c,
            ],
            (Self::Midnight, true) => [
                0x151c2b, 0xe8edf6, 0x1d2638, 0x293449, 0xbaa6f9, 0xabb7cc, 0x3a4860, 0x2d466e,
                0xffffff,
            ],
            (Self::Midnight, false) => [
                0xfafbff, 0x202a3b, 0xedf1f9, 0xe0e6f2, 0x7252a3, 0x59667b, 0xc7d0df, 0xd6e4ff,
                0x173c7a,
            ],
            (Self::Graphite, true) => [
                0x1c1c1c, 0xececec, 0x262626, 0x333333, 0xc7b89a, 0xaeaeae, 0x454545, 0x3b4c5d,
                0xffffff,
            ],
            (Self::Graphite, false) => [
                0xfdfcfb, 0x2a2928, 0xf1efec, 0xe4e1dd, 0x765b35, 0x62605d, 0xcbc6bf, 0xdde7f0,
                0x274868,
            ],
        };
        let [
            surface,
            on_surface,
            surface_container,
            surface_container_high,
            secondary,
            on_surface_variant,
            outline,
            selection,
            on_selection,
        ] = colors.map(Color::Rgb);
        StudioColors {
            canvas: mix(surface, surface_container_high, if dark { 20 } else { 75 }),
            grid: mix(surface_container, outline, if dark { 72 } else { 64 }),
            surface,
            on_surface,
            surface_container,
            surface_container_high,
            primary: PRIMARY,
            accent_foreground: if dark { PRIMARY } else { Color::Rgb(0x81441e) },
            on_primary: ON_PRIMARY,
            secondary,
            on_surface_variant,
            outline,
            selection,
            on_selection,
        }
    }
}

/// Shared color roles available through `StudioEnvironment::colors()`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StudioColors {
    /// Design work surface beneath the frames.
    pub canvas: Color,
    /// Low-contrast canvas grid.
    pub grid: Color,
    /// Editor and workspace canvas.
    pub surface: Color,
    /// Primary text on the canvas and container surfaces.
    pub on_surface: Color,
    /// Navigator and inspector panels.
    pub surface_container: Color,
    /// Toolbar, inputs, tabs and elevated presentations.
    pub surface_container_high: Color,
    /// Primary accent for actions, focus and the caret.
    pub primary: Color,
    /// Accent for text and icons on the theme's surfaces.
    pub accent_foreground: Color,
    /// Text and icons on a primary-colored control.
    pub on_primary: Color,
    /// Complementary accent for supporting highlights.
    pub secondary: Color,
    /// Muted supporting text.
    pub on_surface_variant: Color,
    /// Panel and control separators.
    pub outline: Color,
    /// Selected navigator rows and appearance choices.
    pub selection: Color,
    /// Foreground on a selected row.
    pub on_selection: Color,
}
impl StudioColors {
    pub fn theme(self, dark: bool) -> Theme {
        let base = if dark { Theme::dark() } else { Theme::light() };
        let fallback = base.resolve(dark);
        // Omen's existing primary/secondary tokens represent foreground text.
        // Map Studio's accent roles without changing that public framework contract.
        base.with_tokens(DesignTokens {
            primary: fallback.color(self.on_surface),
            secondary: fallback.color(self.on_surface_variant),
            canvas: fallback.color(self.surface),
            surface: fallback.color(self.surface_container_high),
            accent: fallback.color(self.primary),
            border: fallback.color(self.outline),
            spacing: [4., 8., 12., 20.],
            radius: 6.,
            body_size: 13.,
        })
    }
}

fn mix(base: Color, tint: Color, percent: u32) -> Color {
    let (Color::Rgb(base), Color::Rgb(tint)) = (base, tint) else {
        return base;
    };
    let channel = |shift: u32| {
        let base = (base >> shift) & 255;
        let tint = (tint >> shift) & 255;
        ((base * (100 - percent) + tint * percent) / 100) << shift
    };
    Color::Rgb(channel(16) | channel(8) | channel(0))
}
