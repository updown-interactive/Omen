//! Navigation, tab, and presentation views built on retained reactive boundaries.
use crate::*;
use std::rc::Rc;

/// A route stack with a retained root and uniquely identified destination visits.
///
/// The DSL accepts a navigator binding and registers typed named-property
/// destinations automatically. Returning to a retained visit preserves its state;
/// popping a visit ends its lifetime. [`Self::new`] preserves the legacy
/// string-route factory API, while [`Self::from_navigator`] powers the typed DSL.
/// This renders Omen controls rather than an OS navigation widget.
///
/// ```
/// # use omen_ui as omen;
/// use omen::view;
/// view! { Detail(title: String) { Text(title).navigationTitle("Detail") } }
/// view! { Home {
///     @State navigator = Navigator()
///     NavigationStack($navigator) {
///         Button("Open detail") { navigator.push(Detail(title: "Details")) }
///     }
/// } }
/// let home = view! { Home() };
/// ```
pub struct NavigationStack {
    navigator: Navigator,
    destination: Rc<dyn Fn(Route) -> ViewNode>,
    children: Vec<ViewNode>,
}
impl NavigationStack {
    /// Create a stack. The factory constructs the screen for each pushed route.
    pub fn new<V: View>(navigator: Navigator, destination: impl Fn(Route) -> V + 'static) -> Self {
        Self {
            navigator,
            destination: Rc::new(move |route| destination(route).into_node()),
            children: Vec::new(),
        }
    }
    /// Create a stack whose pushed views supply their own typed destinations.
    pub fn from_navigator(navigator: impl Into<Binding<Navigator>>) -> Self {
        Self::new(navigator.into().get(), |_| AnyView::group(Vec::new()))
    }
    /// Add root content.
    pub fn children(mut self, children: impl IntoIterator<Item = ViewNode>) -> Self {
        self.children.extend(children);
        self
    }
}
impl View for NavigationStack {
    fn into_node(self) -> ViewNode {
        let navigator = self.navigator;
        let destination = self.destination;
        let root = self.children;
        ViewNode::new(ViewKind::Reactive(ReactiveView::new(
            "omen::NavigationStack",
            move |_| {
                let entries = navigator.entries();
                let active = entries.last();
                let content = active.map_or_else(
                    || AnyView::group(root.clone()).into_node(),
                    |entry| {
                        navigator
                            .destination(entry.id)
                            .unwrap_or_else(|| destination(entry.route.clone()))
                    },
                );
                let fallback_title =
                    active.map_or_else(String::new, |entry| entry.route.name.clone());
                let back = active.map(|_| {
                    let navigator = navigator.clone();
                    Action::new(move || {
                        navigator.pop();
                    })
                });
                let mut rows = vec![
                    AnyView::group(if active.is_none() {
                        vec![content.clone()]
                    } else {
                        Vec::new()
                    })
                    .key("root")
                    .into_node(),
                ];
                for entry in &entries {
                    rows.push(
                        AnyView::group(if active.is_some_and(|active| active.id == entry.id) {
                            vec![content.clone()]
                        } else {
                            Vec::new()
                        })
                        .key(entry.id)
                        .into_node(),
                    );
                }
                let mut stack = Layout::new()
                    .child(retained_group(rows).key("navigation-pages"))
                    .into_node();
                stack.style.navigation_chrome = Some(NavigationChrome::new(move |pages| {
                    let mut page = Style::default();
                    read_app_bar_style(pages, &mut page);
                    let title = page
                        .navigation_title
                        .clone()
                        .unwrap_or_else(|| fallback_title.clone());
                    navigation_app_bar(&page, title, back.clone(), AppBarPlatform::current())
                        .map(View::into_node)
                }));
                Ok(stack)
            },
        )))
    }
}
fn read_app_bar_style(node: &ViewNode, page: &mut Style) {
    page.navigation_title = page
        .navigation_title
        .take()
        .or_else(|| node.style.navigation_title.clone());
    page.app_bar = page.app_bar.take().or_else(|| node.style.app_bar.clone());
    page.background = page.background.or(node.style.background);
    page.color = page.color.or(node.style.color);
    page.theme = page.theme.or(node.style.theme);
    page.direction = page.direction.or(node.style.direction);
    page.locale = page.locale.take().or_else(|| node.style.locale.clone());
    if let ViewKind::Group(children) = &node.kind
        && let Some(child) = children
            .iter()
            .find(|child| !matches!(&child.kind, ViewKind::Group(children) if children.is_empty()))
    {
        read_app_bar_style(child, page);
    }
}

/// Construct the app bar for a screen in a navigation stack.
///
/// Studio previews supply the simulated platform so they use the same controls
/// as the running application.
pub fn navigation_app_bar(
    page: &Style,
    title: String,
    back: Option<Action>,
    platform: AppBarPlatform,
) -> Option<AnyView> {
    let defaults = AppBarOptions::default();
    let options = page.app_bar.as_deref().unwrap_or(&defaults);
    if options.hidden {
        return None;
    }
    let back = back.filter(|_| !options.back_hidden);
    let centered = options
        .center_title
        .unwrap_or(platform == AppBarPlatform::Ios);
    let height = if platform == AppBarPlatform::Android {
        56.
    } else {
        44.
    };
    let inset = if back.is_some() { 44. } else { 16. };
    let leading = if let Some(action) = back {
        let mut button = Button::new("Back", move || action.invoke())
            .frame(inset, height)
            .accessibility_label("Back")
            .into_node();
        button.style.navigation_back = true;
        button.style.app_bar_platform = Some(platform);
        AnyView::new(button)
    } else {
        Layout::new().frame(inset, 1.)
    };
    let label = Text::new(title)
        .font_size(17.)
        .weight(Weight::Semibold)
        .truncate()
        .accessibility_role(AccessibilityRole::Heading);
    let label = if centered {
        Layout::new()
            .child(label)
            .alignment(Alignment::Center)
            .fill()
    } else {
        label.fill()
    };
    let background = options
        .background
        .or(page.background)
        .unwrap_or_else(|| Color::Surface.into());
    let mut bar = Layout::new().child(leading).child(label);
    if centered {
        bar = bar.child(Layout::new().frame(inset, 1.));
    }
    let mut bar = bar
        .mode(LayoutMode::Horizontal)
        .alignment(Alignment::Center)
        .frame(f64::INFINITY, height)
        .background(background)
        .color(
            options
                .foreground
                .or(page.color)
                .unwrap_or_else(|| app_bar_foreground(background)),
        )
        .accessibility_role(AccessibilityRole::Navigation)
        .key("navigation-bar");
    if let Some(theme) = page.theme {
        bar = bar.theme(theme);
    }
    if let Some(locale) = &page.locale {
        bar = bar.locale(locale.clone());
    }
    if let Some(direction) = page.direction {
        bar = bar.direction(direction);
    }
    Some(bar)
}

fn app_bar_foreground(background: Background) -> Color {
    let rgb = match background {
        Background::Color(Color::Rgb(rgb)) => rgb,
        Background::Color(Color::Rgba(rgba)) if rgba & 0xff == 0xff => rgba >> 8,
        Background::Color(Color::Black | Color::Dark) => return Color::White,
        Background::Color(Color::White) => return Color::Black,
        _ => return Color::Primary,
    };
    let linear = |shift: u32| {
        let component = f64::from((rgb >> shift) & 255) / 255.;
        if component <= 0.04045 {
            component / 12.92
        } else {
            ((component + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance = 0.2126 * linear(16) + 0.7152 * linear(8) + 0.0722 * linear(0);
    if luminance < 0.179 {
        Color::White
    } else {
        Color::Black
    }
}

/// A horizontal themed bar containing a title and optional action views.
///
/// NavigationStack already supplies its own back/title bar. Compose this view
/// for standalone/custom bars when an additional bar is intentional.
///
/// ```
/// # use omen_ui as omen;
/// use omen::view;
/// let bar = view! { NavigationBar("Tasks") { Button("Add") {} } };
/// ```
pub struct NavigationBar {
    title: String,
    children: Vec<ViewNode>,
}
impl NavigationBar {
    /// Create a bar with a title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            children: Vec::new(),
        }
    }
    /// Add action views after the title.
    pub fn children(mut self, children: impl IntoIterator<Item = ViewNode>) -> Self {
        self.children.extend(children);
        self
    }
}
impl View for NavigationBar {
    fn into_node(self) -> ViewNode {
        Layout::new()
            .child(
                Text::new(self.title)
                    .font(Font::Headline)
                    .accessibility_role(AccessibilityRole::Heading),
            )
            .children(self.children)
            .mode(LayoutMode::Horizontal)
            .spacing_token(Space::Large)
            .padding_token(Space::Medium)
            .background(Color::Surface)
            .into_node()
    }
}
/// Named content owned by a [`TabView`].
///
/// The first argument is a stable ID; the second is its visible title. IDs must
/// be unique within their parent. Add a Layout inside the tab when its children
/// need spacing or arrangement. Use Tab only as content of a TabView.
///
/// ```
/// # use omen_ui as omen;
/// use omen::{view, Value};
/// let selected = Value::new(String::from("home"));
/// let tabs = view! { TabView(selected) { Tab("home", "Home") { Text("Welcome") } } };
/// ```
pub struct Tab {
    item: TabItem,
    children: Vec<ViewNode>,
}
impl Tab {
    /// Create tab metadata.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            item: TabItem {
                id: id.into(),
                title: title.into(),
            },
            children: Vec::new(),
        }
    }
    /// Add tab content.
    pub fn children(mut self, children: impl IntoIterator<Item = ViewNode>) -> Self {
        self.children.extend(children);
        self
    }
}
impl View for Tab {
    fn into_node(self) -> ViewNode {
        let mut node = AnyView::group(self.children).key(&self.item.id).into_node();
        node.style.tab = Some(self.item);
        node
    }
}
/// A bound String selection with retained per-tab view state.
///
/// Supply Tab children with unique IDs. Unknown selections display the first
/// tab; unrelated children cause `RuntimeError::InvalidTabContent` when rendered.
/// Switching retains state for tabs still present, while removing a tab disposes
/// its content lifetime. This is an Omen tab UI, not an OS tab-bar widget.
///
/// ```
/// # use omen_ui as omen;
/// use omen::view;
/// view! { Settings {
///     @State selected = "account"
///     TabView($selected) {
///         Tab("account", "Account") { Text("Account settings") }
///         Tab("privacy", "Privacy") { Text("Privacy settings") }
///     }
/// } }
/// let settings = view! { Settings() };
/// ```
pub struct TabView {
    selection: Binding<String>,
    children: Vec<ViewNode>,
}
impl TabView {
    /// Create a tab view; an unknown selection displays the first tab.
    pub fn new(selection: impl Into<Binding<String>>) -> Self {
        Self {
            selection: selection.into(),
            children: Vec::new(),
        }
    }
    /// Add Tab children.
    pub fn children(mut self, children: impl IntoIterator<Item = ViewNode>) -> Self {
        self.children.extend(children);
        self
    }
}
impl View for TabView {
    fn into_node(self) -> ViewNode {
        let tabs = self.children;
        let selection = self.selection;
        ViewNode::new(ViewKind::Reactive(ReactiveView::new(
            "omen::TabView",
            move |_| {
                let selected = selection.get();
                let mut flattened = Vec::new();
                flatten_tabs(&tabs, &mut flattened)?;
                let tabs = &flattened;
                let active = tabs
                    .iter()
                    .filter_map(|tab| tab.style.tab.as_ref())
                    .find(|tab| tab.id == selected)
                    .or_else(|| tabs.iter().find_map(|tab| tab.style.tab.as_ref()));
                let mut buttons = Vec::new();
                let mut pages = Vec::new();
                for tab in tabs {
                    let Some(item) = &tab.style.tab else {
                        continue;
                    };
                    let is_active = active.is_some_and(|active| active.id == item.id);
                    let binding = selection.clone();
                    let id = item.id.clone();
                    buttons.push(
                        Button::new(item.title.clone(), move || binding.set(id.clone()))
                            .color(if is_active {
                                Color::Accent
                            } else {
                                Color::Secondary
                            })
                            .padding_token(Space::Medium)
                            .accessibility_role(AccessibilityRole::Tab)
                            .accessibility_selected(is_active)
                            .key(&item.id)
                            .into_node(),
                    );
                    let content = if is_active {
                        if let ViewKind::Group(children) = &tab.kind {
                            children.clone()
                        } else {
                            vec![tab.clone()]
                        }
                    } else {
                        Vec::new()
                    };
                    pages.push(
                        AnyView::group(content)
                            .accessibility_role(AccessibilityRole::TabPanel)
                            .key(&item.id)
                            .into_node(),
                    );
                }
                Ok(Layout::new()
                    .child(
                        Layout::new()
                            .children(buttons)
                            .mode(LayoutMode::Horizontal)
                            .spacing_token(Space::Medium)
                            .accessibility_role(AccessibilityRole::TabList)
                            .key("tab-bar"),
                    )
                    .child(retained_group(pages).key("tab-pages"))
                    .into_node())
            },
        )))
    }
}

pub(crate) fn present<V: View>(
    base: ViewNode,
    visible: Binding<bool>,
    kind: PresentationKind,
    content: impl Fn() -> V + 'static,
) -> AnyView {
    let site = base.style.source_site;
    let key = base.style.key.clone();
    let theme = base.style.theme;
    let locale = base.style.locale.clone();
    let direction = base.style.direction;
    let title = base.style.navigation_title.clone();
    let app_bar = base.style.app_bar.clone();
    let mut node = ViewNode::new(ViewKind::Reactive(ReactiveView::new(
        "omen::Presentation",
        move |_| {
            let base = AnyView::group(vec![base.clone()]).key("base").into_node();
            let mut body = if !visible.get() {
                AnyView::group(vec![base]).into_node()
            } else {
                let binding = visible.clone();
                ViewNode::new(ViewKind::Presentation {
                    children: vec![
                        base,
                        AnyView::group(vec![content().into_node()])
                            .key("presented")
                            .into_node(),
                    ],
                    kind,
                    dismiss: Action::new(move || binding.set(false)),
                })
            };
            body.style.keyed_children = true;
            body.style.presentation_kind = Some(kind);
            Ok(body)
        },
    )));
    node.style.source_site = site;
    node.style.key = key;
    node.style.theme = theme;
    node.style.locale = locale;
    node.style.direction = direction;
    node.style.navigation_title = title;
    node.style.app_bar = app_bar;
    AnyView(node)
}

fn retained_group(children: Vec<ViewNode>) -> AnyView {
    let mut node = AnyView::group(children).into_node();
    node.style.keyed_children = true;
    AnyView(node)
}

fn flatten_tabs(nodes: &[ViewNode], output: &mut Vec<ViewNode>) -> Result<(), RuntimeError> {
    for node in nodes {
        if node.style.tab.is_some() {
            output.push(node.clone());
        } else if let ViewKind::Group(children) = &node.kind {
            flatten_tabs(children, output)?;
        } else {
            return Err(RuntimeError::InvalidTabContent);
        }
    }
    Ok(())
}
