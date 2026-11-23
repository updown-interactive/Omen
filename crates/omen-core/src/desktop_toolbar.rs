//! Declarative content hosted beside the desktop window controls.
use crate::{View, ViewNode};
use std::{fmt, rc::Rc};

/// Custom Omen content in a desktop window's native title-bar area.
/// Set [`crate::DesktopWindow::toolbar`] before opening the window.
/// Toolbar content shares the window's runtime and application environments.
#[derive(Clone)]
pub struct DesktopToolbar {
    /// Height in logical pixels. Native hosts clamp this to 32..96 pixels.
    pub height: f32,
    content: Rc<dyn Fn() -> ViewNode>,
}
impl DesktopToolbar {
    /// Construct a 44-pixel toolbar from ordinary Omen views.
    pub fn new<V: View>(content: impl Fn() -> V + 'static) -> Self {
        Self {
            height: 44.,
            content: Rc::new(move || content().into_node()),
        }
    }
    /// Resolve the configured height, rejecting nonfinite and extreme values.
    pub fn resolved_height(&self) -> f32 {
        if self.height.is_finite() {
            self.height.clamp(32., 96.)
        } else {
            44.
        }
    }
    /// Construct the toolbar content within its owning window's runtime.
    pub fn content(&self) -> ViewNode {
        (self.content)()
    }
}
impl PartialEq for DesktopToolbar {
    fn eq(&self, other: &Self) -> bool {
        self.height == other.height && Rc::ptr_eq(&self.content, &other.content)
    }
}
impl fmt::Debug for DesktopToolbar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DesktopToolbar")
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}
