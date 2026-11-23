//! Declarative native desktop windows.
use crate::{Binding, ViewNode};
use std::{fmt, rc::Rc};

/// An independently rendered desktop window requested by a view.
/// The native host retains a window by key until its binding becomes false.
#[derive(Clone)]
pub struct DesktopScene {
    /// Stable identity within the owner window.
    pub key: String,
    /// Closing the native window writes false to this binding.
    pub visible: Binding<bool>,
    content: Rc<dyn Fn() -> ViewNode>,
}
impl DesktopScene {
    /// Construct a window declaration with an independently owned view factory.
    pub fn new(
        key: String,
        visible: Binding<bool>,
        content: impl Fn() -> ViewNode + 'static,
    ) -> Self {
        Self {
            key,
            visible,
            content: Rc::new(content),
        }
    }
    /// Construct the initial tree for a new window.
    pub fn content(&self) -> ViewNode {
        (self.content)()
    }
}
impl PartialEq for DesktopScene {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.visible == other.visible
            && Rc::ptr_eq(&self.content, &other.content)
    }
}
impl fmt::Debug for DesktopScene {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DesktopScene")
            .field("key", &self.key)
            .field("visible", &self.visible)
            .finish_non_exhaustive()
    }
}
