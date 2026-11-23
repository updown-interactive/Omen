//! Layout preferences, deliberately independent of project and document state.
use serde::{Deserialize, Serialize};
mod layout;
mod palette;
pub use layout::PanelGeometry;
pub use palette::{StudioColors, ThemePreset};

/// Shared compact header height for sidebar, editor tabs and inspector.
pub const PANEL_HEADER_HEIGHT: f32 = 36.;

omen::state! {
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub LayoutPreferences {
        sidebar_visible: bool = true;
        active_sidebar: crate::state::SidebarPane = crate::state::SidebarPane::Explorer;
        preview_visible: bool = true;
        sidebar_width: f32 = 240.;
        preview_width: f32 = 300.;
    }
}
impl LayoutPreferences {
    pub fn sanitize(&mut self) {
        self.sidebar_width = if self.sidebar_width.is_finite() {
            self.sidebar_width.clamp(180., 480.)
        } else {
            240.
        };
        self.preview_width = if self.preview_width.is_finite() {
            self.preview_width.clamp(280., 600.)
        } else {
            300.
        };
    }
}
