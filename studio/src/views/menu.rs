//! Shared presentation for Studio menus.
use omen::{AnyView, LayoutDirection, View};

pub(crate) fn panel(content: impl View) -> AnyView {
    let mut panel = content.into_node();
    panel.style.direction = Some(LayoutDirection::LeftToRight);
    panel.style.preview_mut().menu_panel = true;
    AnyView::new(panel)
}
