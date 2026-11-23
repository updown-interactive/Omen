//! Observable route stacks and presentation metadata.
use crate::Value;
use std::collections::BTreeMap;

/// An application route, with optional string parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    /// Route name chosen by the application.
    pub name: String,
    /// Application parameters, such as a record ID.
    pub parameters: BTreeMap<String, String>,
}
impl Route {
    /// Create a route.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            parameters: BTreeMap::new(),
        }
    }
    /// Add a named parameter.
    pub fn parameter(mut self, name: impl Into<String>, value: impl ToString) -> Self {
        self.parameters.insert(name.into(), value.to_string());
        self
    }
}
impl From<&str> for Route {
    fn from(name: &str) -> Self {
        Self::new(name)
    }
}
impl From<String> for Route {
    fn from(name: String) -> Self {
        Self::new(name)
    }
}
/// One uniquely identified visit to a route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteEntry {
    /// Stable identity of this visit; repeated routes have different IDs.
    pub id: u64,
    /// Application route.
    pub route: Route,
}
#[derive(Clone, Debug, Default)]
struct NavigationState {
    entries: Vec<RouteEntry>,
    next_id: u64,
    views: BTreeMap<u64, crate::ViewNode>,
}
/// A clonable, observable navigation controller. Clones share one path.
#[derive(Clone, Debug)]
pub struct Navigator(Value<NavigationState>);
impl Navigator {
    /// Create an empty path (showing the stack's root).
    pub fn new() -> Self {
        Self(Value::new(NavigationState::default()))
    }
    /// Read the current route visits, tracking the active reactive boundary.
    pub fn entries(&self) -> Vec<RouteEntry> {
        self.0.get().entries
    }
    /// Push a new visit. Returns false only if the visit ID space is exhausted.
    pub fn push(&self, route: impl Into<Route>) -> bool {
        let route = route.into();
        self.0.update(|state| {
            let Some(id) = state.next_id.checked_add(1) else {
                return false;
            };
            state.next_id = id;
            state.entries.push(RouteEntry { id, route });
            true
        })
    }
    /// Push a typed view, retaining its destination under a unique visit identity.
    pub fn push_view(&self, view: impl crate::View) -> bool {
        let route = Route::new(std::any::type_name_of_val(&view));
        let node = view.into_node();
        self.0.update(|state| {
            let Some(id) = state.next_id.checked_add(1) else {
                return false;
            };
            state.next_id = id;
            state.views.insert(id, node);
            state.entries.push(RouteEntry { id, route });
            true
        })
    }
    /// Read the typed destination for a visit, if one was registered.
    pub fn destination(&self, id: u64) -> Option<crate::ViewNode> {
        self.0.get().views.get(&id).cloned()
    }
    /// Pop one visit; the root cannot be popped.
    pub fn pop(&self) -> Option<Route> {
        self.0.update(|state| {
            state.entries.pop().map(|entry| {
                state.views.remove(&entry.id);
                entry.route
            })
        })
    }
    /// Replace the current visit with a fresh identity.
    pub fn replace(&self, route: impl Into<Route>) -> bool {
        let route = route.into();
        self.0.update(|state| {
            let Some(id) = state.next_id.checked_add(1) else {
                return false;
            };
            state.next_id = id;
            if let Some(entry) = state.entries.pop() {
                state.views.remove(&entry.id);
            }
            state.entries.push(RouteEntry { id, route });
            true
        })
    }
    /// Return to the root, disposing all pushed visits on reconciliation.
    pub fn reset(&self) {
        self.0.update(|state| {
            state.entries.clear();
            state.views.clear();
        });
    }
}
/// Platform conventions for an Omen app bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppBarPlatform {
    /// iPhone and iPad use a leading chevron and centered title.
    Ios,
    /// Android uses a leading arrow and a leading title.
    Android,
    /// macOS uses a chevron and a leading title.
    Mac,
    /// Other desktops and browsers use a leading arrow and title.
    Desktop,
}

impl AppBarPlatform {
    /// Default conventions for the current host.
    pub const fn current() -> Self {
        if cfg!(target_os = "ios") {
            Self::Ios
        } else if cfg!(target_os = "android") {
            Self::Android
        } else if cfg!(target_os = "macos") {
            Self::Mac
        } else {
            Self::Desktop
        }
    }

    /// Whether the target uses a chevron for its Back control.
    pub fn uses_chevron(self) -> bool {
        matches!(self, Self::Ios | Self::Mac)
    }
}

/// Per-screen overrides consumed by an enclosing navigation stack.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AppBarOptions {
    /// Hide the entire app bar.
    pub hidden: bool,
    /// Suppress the automatic Back button on this screen.
    pub back_hidden: bool,
    /// Override the platform's title alignment.
    pub center_title: Option<bool>,
    /// Override the app bar's fill.
    pub background: Option<crate::Background>,
    /// Override text and icon color in the app bar.
    pub foreground: Option<crate::Color>,
}

type ChromeFactory = dyn Fn(&crate::ViewNode) -> Option<crate::ViewNode>;

/// Builds a stack's chrome after the active page has resolved.
#[doc(hidden)]
#[derive(Clone)]
pub struct NavigationChrome(std::rc::Rc<ChromeFactory>);

impl NavigationChrome {
    /// Store a builder that reads the resolved page tree.
    pub fn new(build: impl Fn(&crate::ViewNode) -> Option<crate::ViewNode> + 'static) -> Self {
        Self(std::rc::Rc::new(build))
    }

    pub(crate) fn build(&self, pages: &crate::ViewNode) -> Option<crate::ViewNode> {
        (self.0)(pages)
    }
}

impl std::fmt::Debug for NavigationChrome {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("NavigationChrome")
    }
}

impl PartialEq for NavigationChrome {
    fn eq(&self, other: &Self) -> bool {
        std::rc::Rc::ptr_eq(&self.0, &other.0)
    }
}

/// Overlay placement and interaction style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentationKind {
    /// Bottom-aligned panel.
    Sheet,
    /// Centered confirmation or alert.
    Dialog,
    /// Anchored panel that leaves surrounding content interactive.
    Popover,
    /// Pointer-anchored context menu that leaves the document interactive.
    ContextMenu,
    /// Centered blocking presentation.
    Modal,
}
/// Metadata attached to a tab's content group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabItem {
    /// Stable tab identifier.
    pub id: String,
    /// Visible tab title.
    pub title: String,
}

impl Default for Navigator {
    fn default() -> Self {
        Self::new()
    }
}
