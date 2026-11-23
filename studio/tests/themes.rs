use omen::{Color, Theme};
use omen_studio::theme::{LayoutPreferences, ThemePreset};

fn rgb(color: Color) -> u32 {
    Theme::dark().resolve(true).color(color)
}
fn luminance(color: Color) -> f64 {
    let rgb = rgb(color);
    let channel = |shift: u32| {
        let value = f64::from((rgb >> shift) & 255_u32) / 255.;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
}
fn contrast(a: Color, b: Color) -> f64 {
    let a = luminance(a);
    let b = luminance(b);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[test]
fn default_dark_uses_the_configured_palette() {
    let colors = ThemePreset::Xcode.colors(true);
    assert_eq!(
        [
            colors.canvas,
            colors.grid,
            colors.surface,
            colors.on_surface,
            colors.surface_container,
            colors.surface_container_high,
            colors.primary,
            colors.secondary,
            colors.on_surface_variant,
            colors.outline,
            colors.selection,
            colors.on_selection,
        ]
        .map(rgb),
        [
            0x080809, 0x1c1c20, 0x0d0d0f, 0xeaeaed, 0x111113, 0x1b1b1e, 0xe9ad76, 0xbcc4d0,
            0x92929a, 0x29292d, 0x241e19, 0xf0c59d,
        ]
    );
}

#[test]
fn presets_have_distinct_layered_palettes_in_both_appearances() {
    for dark in [false, true] {
        for preset in ThemePreset::ALL {
            let colors = preset.colors(dark);
            assert_ne!(
                colors.surface,
                colors.surface_container,
                "{}",
                preset.name()
            );
            assert_ne!(
                colors.surface_container,
                colors.surface_container_high,
                "{}",
                preset.name()
            );
            let theme = colors.theme(dark).resolve(dark);
            assert_eq!(theme.canvas, rgb(colors.surface));
            assert_eq!(theme.primary, rgb(colors.on_surface));
            assert_eq!(theme.accent, 0xe9ad76);
            assert_eq!(theme.surface, rgb(colors.surface_container_high));
        }
        assert_ne!(
            ThemePreset::Xcode.colors(dark),
            ThemePreset::Midnight.colors(dark)
        );
        assert_ne!(
            ThemePreset::Xcode.colors(dark),
            ThemePreset::Graphite.colors(dark)
        );
    }
}

#[test]
fn foreground_and_selection_roles_remain_readable_on_their_surfaces() {
    for preset in ThemePreset::ALL {
        for dark in [false, true] {
            let colors = preset.colors(dark);
            for (foreground, background) in [
                (colors.on_surface, colors.surface),
                (colors.on_surface, colors.surface_container_high),
                (colors.on_surface_variant, colors.surface_container_high),
                (colors.accent_foreground, colors.surface_container_high),
                (colors.on_primary, colors.primary),
                (colors.secondary, colors.surface),
                (colors.on_selection, colors.selection),
            ] {
                assert!(
                    contrast(foreground, background) >= 4.5,
                    "{} dark={dark}: contrast {}",
                    preset.name(),
                    contrast(foreground, background)
                );
            }
        }
    }
}

#[test]
fn legacy_layout_fields_remain_readable_after_appearance_moves_to_settings() {
    let legacy = r#"{"dark":false,"sidebar_visible":false,"preview_visible":true,"sidebar_width":315.0,"preview_width":390.0}"#;
    let layout: LayoutPreferences = serde_json::from_str(legacy).unwrap();
    assert!(!layout.sidebar_visible);
    assert_eq!(layout.sidebar_width, 315.);
    assert_eq!(layout.preview_width, 390.);
}

#[test]
fn visible_geometry_reserves_editor_space_without_rewriting_preferences() {
    for sidebar_visible in [false, true] {
        for preview_visible in [false, true] {
            let layout = LayoutPreferences {
                sidebar_visible,
                preview_visible,
                sidebar_width: 480.,
                preview_width: 600.,
                active_sidebar: Default::default(),
            };
            for width in [1000., 1100., 1600.] {
                let geometry = layout.geometry(width);
                assert!(geometry.editor_width >= 320.);
                assert_eq!(geometry.sidebar_width > 0., sidebar_visible);
                assert_eq!(geometry.preview_width > 0., preview_visible);
                assert_eq!(layout.sidebar_width, 480.);
                assert_eq!(layout.preview_width, 600.);
            }
        }
    }
}
