//! Visible split geometry, separate from persisted preferred widths.
use super::LayoutPreferences;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelGeometry {
    pub sidebar_width: f32,
    pub preview_width: f32,
    pub editor_width: f32,
    pub sidebar_limit: f32,
    pub preview_limit: f32,
}
impl LayoutPreferences {
    pub fn geometry(&self, viewport_width: f32) -> PanelGeometry {
        let width = if viewport_width.is_finite() {
            viewport_width.max(0.)
        } else {
            0.
        };
        let handles = 6. * (u8::from(self.sidebar_visible) + u8::from(self.preview_visible)) as f32;
        let sidebar_limit = (width - if self.preview_visible { 280. } else { 0. } - 320. - handles)
            .clamp(180., 480.);
        let sidebar_width = if self.sidebar_visible {
            self.sidebar_width.min(sidebar_limit)
        } else {
            0.
        };
        let preview_limit = (width - sidebar_width - 320. - handles).clamp(280., 600.);
        let preview_width = if self.preview_visible {
            self.preview_width.min(preview_limit)
        } else {
            0.
        };
        PanelGeometry {
            sidebar_width,
            preview_width,
            editor_width: (width - sidebar_width - preview_width - handles).max(0.),
            sidebar_limit,
            preview_limit,
        }
    }
}
