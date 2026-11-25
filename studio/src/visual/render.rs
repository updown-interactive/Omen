//! Maps literal Omen source properties to the native Omen view builders.
use super::{Profile, VisualFrame, VisualTree};
use omen::{
    AdaptiveGrid, AdaptiveLayout, Alignment, AnyView, AppBarPlatform, Binding, Breakpoint, Button,
    Card, Color, Divider, FlowPlacement, Font, Gap, Image, Insets, Layout, LayoutDirection,
    LayoutMode, Locale, Navigator, PinCorner, PreviewPlacement, ReactiveView, ResponsiveStyle,
    Route, SafeArea, Space, Text, TextField, Theme, ThemeMode, Value, View, ViewExt, ViewKind,
    ViewNode, Weight,
};
use omen_macro_core::inspector::InspectedNode;
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet, HashMap},
    rc::Rc,
    sync::Arc,
};

pub type Select = Rc<dyn Fn(usize)>;
type FormValues = Rc<RefCell<HashMap<(String, u64, usize), Value<String>>>>;

#[derive(Clone, Default)]
pub struct InteractivePreview {
    inputs: FormValues,
    navigator: Navigator,
}

impl InteractivePreview {
    pub fn reset(&self) {
        self.inputs.borrow_mut().clear();
        self.navigator.reset();
    }
}

#[derive(Clone)]
pub struct Reorder {
    pub scope: String,
    pub move_node: Rc<dyn Fn(usize, usize, FlowPlacement)>,
    pub resize: Rc<dyn Fn(usize, [f32; 2])>,
}

#[derive(Clone, Copy, PartialEq)]
pub struct InsertionGuide {
    parent: usize,
    edge: Option<(usize, FlowPlacement)>,
}

impl InsertionGuide {
    pub fn new(tree: &VisualTree, target: usize, position: Option<FlowPlacement>) -> Option<Self> {
        let placement = position.unwrap_or(FlowPlacement::Into);
        let parent = tree.insertion_parent(target, placement)?;
        let participates_in_flow = |child: usize| {
            !tree.nodes[child]
                .properties
                .iter()
                .any(|property| property.name == "pin")
        };
        let children = || {
            tree.children(Some(parent))
                .filter(|&child| participates_in_flow(child))
        };
        let append = || children().last().map(|child| (child, FlowPlacement::After));
        let edge = match position {
            Some(position) if participates_in_flow(target) => Some((target, position)),
            Some(_) => children()
                .find(|&child| child > target)
                .map(|child| (child, FlowPlacement::Before))
                .or_else(append),
            None => append(),
        };
        Some(Self { parent, edge })
    }

    pub fn label(self, tree: &VisualTree, width: f32) -> String {
        let style = layout_style(&tree.nodes[self.parent], width);
        let horizontal = style.mode == Some(LayoutMode::Horizontal);
        let alignment = match style.alignment {
            Some(Alignment::Leading) => "Leading",
            Some(Alignment::Trailing) => "Trailing",
            Some(Alignment::Center) => "Centered",
            Some(Alignment::Top) => "Top",
            Some(Alignment::Bottom) => "Bottom",
            Some(Alignment::TopLeading) => "Top leading",
            Some(Alignment::TopTrailing) => "Top trailing",
            Some(Alignment::BottomLeading) => "Bottom leading",
            Some(Alignment::BottomTrailing) => "Bottom trailing",
            None if horizontal => "Vertically centered",
            None => "Horizontal stretch",
        };
        format!(
            "{} · {alignment}",
            if horizontal { "Row" } else { "Column" }
        )
    }
}

fn layout_style(node: &InspectedNode, width: f32) -> omen::Style {
    let mut view = component(node, None).unwrap_or_else(|| AnyView::new(Layout::new()));
    for property in &node.properties {
        if matches!(
            property.name.as_str(),
            "alignment"
                | "mode"
                | "horizontal"
                | "vertical"
                | "stack"
                | "direction"
                | "adaptive"
                | "frame"
                | "minWidth"
                | "padding"
                | "paddingToken"
        ) && let Some(updated) = modifier(view.clone(), &property.name, &property.value)
        {
            view = updated;
        }
    }
    view.into_node().style.for_container(
        width,
        width,
        Theme::default().resolve(false),
        Insets::default(),
    )
}

pub fn component_label(node: &InspectedNode, width: f32) -> String {
    if !matches!(node.component.as_str(), "Layout" | "AdaptiveLayout") {
        return node.component.clone();
    }
    format!("{} · {}", node.component, layout_kind(node, width))
}

pub fn layout_kind(node: &InspectedNode, width: f32) -> &'static str {
    match layout_style(node, width).mode.unwrap_or_default() {
        LayoutMode::Horizontal => "Row",
        LayoutMode::Vertical => "Column",
        LayoutMode::Stack => "Stack",
    }
}

pub fn diagnostics(tree: &VisualTree) -> Vec<String> {
    let mut warnings = BTreeSet::new();
    for (index, node) in tree.nodes.iter().enumerate() {
        if linked_view(node, &tree.linked_views).is_none()
            && component(node, tree.assets.get(&index)).is_none()
        {
            warnings.insert(format!(
                "{} needs a literal argument or an available component implementation.",
                node.component
            ));
        }
        let mut view = AnyView::new(Layout::new());
        for property in node
            .properties
            .iter()
            .filter(|property| property.modifier_range.is_some())
        {
            if let Some(updated) = modifier(view.clone(), &property.name, &property.value) {
                view = updated;
            } else {
                warnings.insert(format!(
                    ".{} is unavailable in this preview.",
                    property.name
                ));
            }
        }
    }
    warnings.into_iter().collect()
}

#[derive(Clone, Copy)]
pub enum FrameInteraction<'a> {
    Design,
    Interactive(&'a InteractivePreview),
    Inert,
}

pub fn frame_content(
    tree: &VisualTree,
    frame: &VisualFrame,
    selection: Option<usize>,
    select: Select,
    interaction: FrameInteraction<'_>,
    reorder: &Reorder,
    guide: Option<InsertionGuide>,
) -> AnyView {
    let placement = Reorder {
        scope: format!("{}:{}", reorder.scope, frame.id),
        move_node: reorder.move_node.clone(),
        resize: reorder.resize.clone(),
    };
    let linked_budget = Cell::new(2048);
    let children: Vec<_> = tree
        .children(None)
        .map(|index| {
            render_node(
                tree,
                index,
                selection,
                &select,
                match interaction {
                    FrameInteraction::Design => FrameNodeMode::Design,
                    FrameInteraction::Interactive(session) => FrameNodeMode::Interactive(session, 0),
                    FrameInteraction::Inert => FrameNodeMode::Inert,
                },
                &placement,
                f32::from(frame.width),
                guide,
                0,
                &linked_budget,
            )
        })
        .collect();
    let interactive = match interaction {
        FrameInteraction::Interactive(session) => Some(session),
        _ => None,
    };
    let children = if let Some(interaction) = interactive.filter(|_| {
        tree.nodes
            .iter()
            .filter_map(|node| node.preview_route.as_ref())
            .any(|route| destination(tree, route).is_some())
    }) {
        vec![preview_navigation(
            children,
            tree.clone(),
            interaction.clone(),
            placement,
            frame.clone(),
        )]
    } else {
        children
    };
    let theme = if frame.dark {
        Theme::dark()
    } else {
        Theme::light()
    };
    let view = Layout::new()
        .children(children)
        .frame(frame.width, frame.height)
        .min_width(frame.width)
        .corner_radius(match frame.profile {
            Profile::IPhone | Profile::Android | Profile::WebMobile => 14.,
            Profile::IPad | Profile::AndroidTablet | Profile::WebTablet => 8.,
            _ => 0.,
        })
        .scrollable()
        .background(Color::Canvas)
        .color(Color::Primary)
        .theme(theme)
        .locale(Locale::new(frame.locale.clone()));
    let insets = if matches!(
        frame.profile,
        Profile::Android | Profile::IPhone | Profile::IPad | Profile::AndroidTablet
    ) {
        Insets {
            top: 24.,
            bottom: 16.,
            ..Default::default()
        }
    } else {
        Insets::default()
    };
    let view = view.safe_area_insets(insets);
    let mut node = view.into_node();
    let preview = node.style.preview_mut();
    preview.width = Some(f32::from(frame.width));
    preview.safe_area = frame.safe_area;
    preview.inert = matches!(interaction, FrameInteraction::Inert);
    AnyView::new(node)
}

fn preview_navigation(
    root: Vec<ViewNode>,
    tree: VisualTree,
    session: InteractivePreview,
    placement: Reorder,
    frame: VisualFrame,
) -> ViewNode {
    ViewNode::new(ViewKind::Reactive(ReactiveView::new(
        "studio::PreviewNavigation",
        move |_| {
            let entries = session.navigator.entries();
            let Some(entry) = entries.last() else {
                // A transparent group preserves the root geometry in both modes.
                return Ok(AnyView::group(root.clone()).into_node());
            };
            let page =
                render_destination(&tree, entry, &session, &placement, f32::from(frame.width));
            let title = page
                .style
                .navigation_title
                .clone()
                .unwrap_or_else(|| entry.route.name.clone());
            let background = page
                .style
                .background
                .unwrap_or_else(|| Color::Canvas.into());
            let navigator = session.navigator.clone();
            let back = omen::Action::new(move || {
                navigator.pop();
            });
            let platform = match frame.profile {
                Profile::IPhone | Profile::IPad => AppBarPlatform::Ios,
                Profile::Android | Profile::AndroidTablet => AppBarPlatform::Android,
                Profile::Mac => AppBarPlatform::Mac,
                _ => AppBarPlatform::Desktop,
            };
            let bar = omen::navigation_app_bar(&page.style, title, Some(back), platform);
            let theme = page.style.theme;
            let screen = Layout::new()
                .children(bar.into_iter().map(View::into_node))
                .child(page.key(entry.id))
                .frame(frame.width, frame.height)
                .background(background);
            let mut node = match theme {
                Some(theme) => screen.theme(theme).into_node(),
                None => screen.into_node(),
            };
            node.style.preview_mut().width = Some(f32::from(frame.width));
            Ok(node)
        },
    )))
}

#[derive(Clone, Copy)]
enum FrameNodeMode<'a> {
    Design,
    Interactive(&'a InteractivePreview, u64),
    Inert,
}

fn render_node(
    tree: &VisualTree,
    index: usize,
    selection: Option<usize>,
    select: &Select,
    mode: FrameNodeMode<'_>,
    reorder: &Reorder,
    width: f32,
    guide: Option<InsertionGuide>,
    depth: usize,
    linked_budget: &Cell<usize>,
) -> ViewNode {
    let node = &tree.nodes[index];
    let is_layout = matches!(
        node.component.as_str(),
        "Layout" | "AdaptiveLayout" | "AdaptiveGrid"
    );
    let linked = linked_view(node, &tree.linked_views);
    let mut view = styled_component(tree, index, linked.is_some());
    if let omen::ViewKind::Layout(children) | omen::ViewKind::Card(children) = &mut view.kind {
        if let Some(linked) = linked {
            let mut trail = vec![
                tree.declaration_name().unwrap_or_default(),
                node.component.as_str(),
            ];
            *children = linked
                .children(None)
                .map(|child| {
                    render_linked_node(linked, child, &tree.linked_views, &mut trail, linked_budget)
                })
                .collect();
        } else {
            *children = tree
                .children(Some(index))
                .map(|child| {
                    render_node(
                        tree,
                        child,
                        selection,
                        select,
                        mode,
                        reorder,
                        width,
                        guide,
                        depth + usize::from(is_layout),
                        linked_budget,
                    )
                })
                .collect();
        }
    }
    match mode {
        FrameNodeMode::Interactive(session, visit) => {
            return interactive_node(view, tree, index, session, visit);
        }
        FrameNodeMode::Inert => return view.into_node(),
        FrameNodeMode::Design => {}
    }
    let select = select.clone();
    let mut view = view.on_tap(move || select(index)).into_node();
    let container = tree.flow_container(index);
    if container {
        let mode = if view.style.min_item_width.is_some() {
            LayoutMode::Horizontal
        } else {
            layout_style(node, width).mode.unwrap_or_default()
        };
        if let omen::ViewKind::Layout(children) | omen::ViewKind::Card(children) = &mut view.kind {
            for child in children.iter_mut() {
                if let Some(placement) = &mut child.style.preview_mut().placement {
                    placement.axis = Some(mode);
                }
            }
            if children.is_empty() && view.style.frame.is_none() {
                view.style.frame = Some(omen::Frame {
                    width: f32::INFINITY,
                    height: 26.,
                });
            }
        }
    }
    let pinned = view.style.pin.is_some();
    let preview = view.style.preview_mut();
    preview.outline = Some(if selection == Some(index) {
        Color::Accent
    } else {
        Color::Border
    });
    preview.selected = selection == Some(index);
    preview.layout_depth = is_layout.then_some(depth);
    if let Some(guide) = guide {
        preview.layout_guide = guide.parent == index;
        if let Some((target, placement)) = guide.edge {
            if target == index {
                preview.insertion_edge = Some(placement);
            }
        } else if guide.parent == index {
            preview.insertion_edge = Some(FlowPlacement::Into);
        }
    }
    let move_node = reorder.move_node.clone();
    let source = node.name_range.start;
    if selection == Some(index)
        && let Some((size, _)) = super::placement::fixed_size(node)
    {
        let resize = reorder.resize.clone();
        preview.resize = Some(omen::PreviewResize {
            size: Binding::new(move || size, move |dimensions| resize(index, dimensions)),
        });
    }
    preview.placement = Some(PreviewPlacement {
        scope: reorder.scope.clone(),
        source: node.range.clone(),
        label: node.component.clone(),
        draggable: !pinned
            && tree
                .parent(index)
                .is_some_and(|parent| tree.flow_container(parent)),
        axis: None,
        container,
        drop_from: Binding::new(
            move || (source, FlowPlacement::Into),
            move |(origin, placement)| move_node(origin, source, placement),
        ),
    });
    view
}

fn destination<'a>(tree: &'a VisualTree, route: &Route) -> Option<&'a VisualTree> {
    let destination = tree.linked_views.get(&route.name)?;
    (destination.parameters.len() == route.parameters.len()
        && destination
            .parameters
            .iter()
            .all(|name| route.parameters.contains_key(name)))
    .then_some(destination)
}

fn interactive_node(
    mut view: ViewNode,
    tree: &VisualTree,
    index: usize,
    session: &InteractivePreview,
    visit: u64,
) -> ViewNode {
    if tree.nodes[index].component == "TextField" {
        let key = (
            tree.declaration_name().unwrap_or_default().to_owned(),
            visit,
            index,
        );
        let value = session
            .inputs
            .borrow_mut()
            .entry(key)
            .or_insert_with(|| Value::new(String::new()))
            .clone();
        return view.value(value).into_node();
    }
    if let omen::ViewKind::Button { action, .. } = &mut view.kind
        && let Some(route) = tree.nodes[index].preview_route.as_ref()
        && destination(tree, route).is_some()
    {
        let session = session.clone();
        let route = route.clone();
        *action = omen::Action::new(move || {
            let entries = session.navigator.entries();
            session.inputs.borrow_mut().retain(|(_, visit, _), _| {
                *visit == 0 || entries.iter().any(|entry| entry.id == *visit)
            });
            session.navigator.push(route.clone());
        });
    }
    view
}

fn render_destination(
    tree: &VisualTree,
    entry: &omen::RouteEntry,
    session: &InteractivePreview,
    reorder: &Reorder,
    width: f32,
) -> ViewNode {
    let route = &entry.route;
    let Some(destination) = destination(tree, route) else {
        return Text::new("Destination preview unavailable").into_node();
    };
    let mut page = destination.clone();
    page.linked_views = tree.linked_views.clone();
    for node in &mut page.nodes {
        for property in &mut node.properties {
            let name = property.value.trim();
            let name = name.strip_suffix(".clone()").unwrap_or(name).trim();
            if let Some(value) = route.parameters.get(name) {
                property.value = format!("{value:?}");
            }
        }
    }
    let select: Select = Rc::new(|_| {});
    let budget = Cell::new(2048);
    let mut children = page.children(None).map(|index| {
        render_node(
            &page,
            index,
            None,
            &select,
            FrameNodeMode::Interactive(session, entry.id),
            reorder,
            width,
            None,
            0,
            &budget,
        )
    });
    let title = page
        .children(None)
        .next()
        .and_then(|index| {
            page.nodes[index]
                .properties
                .iter()
                .find(|property| property.name == "navigationTitle")
        })
        .and_then(|property| literal(&property.value))
        .unwrap_or_else(|| route.name.clone());
    let content = match (children.next(), children.next()) {
        (Some(node), None) => node,
        (Some(first), Some(second)) => Layout::new()
            .children([first, second].into_iter().chain(children))
            .into_node(),
        _ => Layout::new().into_node(),
    };
    content.navigation_title(title).into_node()
}

type LinkedViews = BTreeMap<String, Arc<VisualTree>>;

fn linked_view<'a>(node: &InspectedNode, views: &'a LinkedViews) -> Option<&'a VisualTree> {
    if node
        .properties
        .iter()
        .any(|property| property.modifier_range.is_none())
    {
        return None;
    }
    views
        .get(&node.component)
        .filter(|view| view.parameters.is_empty())
        .map(Arc::as_ref)
}

fn styled_component(tree: &VisualTree, index: usize, linked: bool) -> ViewNode {
    let node = &tree.nodes[index];
    let mut view = if linked {
        AnyView::new(Layout::new())
    } else {
        component(node, tree.assets.get(&index)).unwrap_or_else(|| {
            AnyView::new(Text::new(format!("{} preview unavailable", node.component)))
        })
    };
    // Style the shell before attaching children so modifier trials stay shallow.
    for property in node
        .properties
        .iter()
        .filter(|property| property.modifier_range.is_some())
    {
        if let Some(updated) = modifier(view.clone(), &property.name, &property.value) {
            view = updated;
        }
    }
    view.key(index).into_node()
}

fn render_linked_node<'a>(
    tree: &'a VisualTree,
    index: usize,
    views: &'a LinkedViews,
    trail: &mut Vec<&'a str>,
    budget: &Cell<usize>,
) -> ViewNode {
    if budget.get() == 0 {
        return Text::new("Preview limit reached").key(index).into_node();
    }
    budget.set(budget.get() - 1);
    let node = &tree.nodes[index];
    let linked = linked_view(node, views)
        .filter(|_| trail.len() < 32 && !trail.contains(&node.component.as_str()));
    let mut view = styled_component(tree, index, linked.is_some());
    if let omen::ViewKind::Layout(children) | omen::ViewKind::Card(children) = &mut view.kind {
        if let Some(linked) = linked {
            trail.push(&node.component);
            *children = linked
                .children(None)
                .take(budget.get().saturating_add(1))
                .map(|child| render_linked_node(linked, child, views, trail, budget))
                .collect();
            trail.pop();
        } else {
            *children = tree
                .children(Some(index))
                .take(budget.get().saturating_add(1))
                .map(|child| render_linked_node(tree, child, views, trail, budget))
                .collect();
        }
    }
    view
}

fn component(node: &InspectedNode, image: Option<&String>) -> Option<AnyView> {
    let label = || {
        node.properties
            .iter()
            .find(|property| property.key == "arg:0")
            .and_then(|property| literal(&property.value))
    };
    Some(match node.component.as_str() {
        "Layout" => AnyView::new(Layout::new()),
        "AdaptiveLayout" => AnyView::new(AdaptiveLayout::new()),
        "AdaptiveGrid" => AnyView::new(AdaptiveGrid::new()),
        "Card" => AnyView::new(Card::new()),
        "Text" => AnyView::new(Text::new(static_text(label()?)?)),
        "Button" => AnyView::new(Button::new(label()?, || {})),
        "Image" => AnyView::new(Image::new(image?.as_str())),
        "TextField" => AnyView::new(TextField::new(label()?)),
        "Divider" => AnyView::new(Divider::new()),
        "Gap" => AnyView::new(
            if let Some(size) = node
                .properties
                .iter()
                .find(|property| property.key == "arg:0")
            {
                Gap::sized(number(&size.value)?)
            } else {
                Gap::new()
            },
        ),
        _ => return None,
    })
}

fn literal(value: &str) -> Option<String> {
    syn::parse_str::<syn::LitStr>(value)
        .ok()
        .map(|literal| literal.value())
}

fn static_text(source: String) -> Option<String> {
    if !source.contains(['{', '}']) {
        return Some(source);
    }
    let mut result = String::with_capacity(source.len());
    let mut chars = source.chars();
    while let Some(ch) = chars.next() {
        if matches!(ch, '{' | '}') && chars.next() != Some(ch) {
            return None;
        }
        result.push(ch);
    }
    Some(result)
}

fn number(value: &str) -> Option<f64> {
    let value = value.trim();
    value
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite() && (0. ..=16_384.).contains(number))
}

macro_rules! token {
    ($value:expr, $type:ident, $($name:literal => $variant:ident),+ $(,)?) => {
        match $value { $($name => $type::$variant,)+ _ => return None }
    };
}

fn preview_color(value: &str) -> Option<Color> {
    if let Some(text) = literal(value) {
        return Color::from_hex(&text).ok();
    }
    Some(
        token!(value, Color, ".primary" => Primary, ".secondary" => Secondary, ".canvas" => Canvas, ".surface" => Surface, ".accent" => Accent, ".border" => Border, ".red" => Red, ".orange" => Orange, ".purple" => Purple, ".gray" => Gray, ".panel" => Panel, ".white" => White, ".black" => Black, ".blue" => Blue, ".green" => Green, ".dark" => Dark),
    )
}

fn modifier(view: AnyView, name: &str, value: &str) -> Option<AnyView> {
    if matches!(name, "direction" | "mode" | "adaptive")
        && let Some(direction) = omen_macro_core::inspector::adaptive_direction(value)
    {
        return Some(view.adaptive(direction));
    }
    Some(match name {
        "adaptive" if value.is_empty() => view.adaptive(omen::AdaptiveDirection::default()),
        "padding" | "spacing" | "paddingToken" | "spacingToken" => {
            let padding = name.starts_with("padding");
            if let Some(size) = number(value) {
                if padding { view.padding(size) } else { view.spacing(size) }
            } else {
                let space = token!(value, Space, ".sm" => Small, ".small" => Small, ".md" => Medium, ".medium" => Medium, ".lg" => Large, ".large" => Large, ".xl" => ExtraLarge, ".extraLarge" => ExtraLarge);
                if padding { view.padding_token(space) } else { view.spacing_token(space) }
            }
        }
        "mode" => view.mode(token!(value, LayoutMode, ".horizontal" => Horizontal, ".vertical" => Vertical, ".stack" => Stack)),
        "pin" => {
            let mut arguments = value.trim_end_matches(',').split(',').map(str::trim);
            let corner = token!(arguments.next()?, PinCorner, ".topLeading" => TopLeading, ".topTrailing" => TopTrailing, ".bottomLeading" => BottomLeading, ".bottomTrailing" => BottomTrailing);
            view.pin(corner, arguments.next()?.parse::<u16>().ok()?, arguments.next()?.parse::<u16>().ok()?)
        }
        "alignment" => view.alignment(token!(value, Alignment, ".leading" => Leading, ".trailing" => Trailing, ".center" => Center, ".top" => Top, ".bottom" => Bottom, ".topLeading" => TopLeading, ".topTrailing" => TopTrailing, ".bottomLeading" => BottomLeading, ".bottomTrailing" => BottomTrailing)),
        "font" => view.font(token!(value, Font, ".caption" => Caption, ".footnote" => Footnote, ".headline" => Headline, ".title2" => Title2, ".body" => Body, ".title" => Title, ".largeTitle" => LargeTitle)),
        "weight" => view.weight(token!(value, Weight, ".thin" => Thin, ".light" => Light, ".semibold" => Semibold, ".heavy" => Heavy, ".regular" => Regular, ".medium" => Medium, ".bold" => Bold)),
        "background" => view.background(preview_color(value)?),
        "color" | "textColor" | "textcolor" => view.color(preview_color(value)?),
        "hoverBackground" => view.hover_background(preview_color(value)?),
        "gradient" => {
            let mut parts = value.trim_end_matches(',').splitn(3, ',').map(str::trim);
            let from = preview_color(parts.next()?)?;
            let to = preview_color(parts.next()?)?;
            let angle = parts.next()?.parse::<f64>().ok().filter(|angle| angle.is_finite())?;
            view.gradient(from, to, angle)
        }
        "cornerRadius" => view.corner_radius(number(value)?),
        "fontSize" => view.font_size(number(value)?),
        "lineHeight" => view.line_height(number(value)?),
        "fontFamily" => view.font_family(literal(value)?),
        "min_item_width" | "minItemWidth" => view.min_item_width(number(value)?),
        "minWidth" => view.min_width(number(value)?),
        "frame" => {
            let (width, height) = value.split_once(',')?;
            let dimension = |value: &str| if value.trim() == ".infinity" { Some(f64::INFINITY) } else { number(value) };
            view.frame(dimension(width)?, dimension(height)?)
        }
        "horizontal" | "vertical" | "stack" => {
            let breakpoint = token!(value, Breakpoint, ".compact" => Compact, ".small" => Compact, ".medium" => Medium, ".large" => Large, ".extraLarge" => ExtraLarge);
            let mode = match name {
                "horizontal" => LayoutMode::Horizontal,
                "vertical" => LayoutMode::Vertical,
                _ => LayoutMode::Stack,
            };
            view.responsive(breakpoint, ResponsiveStyle { mode: Some(mode), ..Default::default() })
        }
        "theme" => view.theme(token!(value, ThemeMode, ".light" => Light, ".dark" => Dark, ".system" => System)),
        "direction" => view.direction(token!(value, LayoutDirection, ".leftToRight" => LeftToRight, ".rightToLeft" => RightToLeft)),
        "safeArea" => view.safe_area(token!(value, SafeArea, ".all" => All, ".vertical" => Vertical, ".horizontal" => Horizontal, ".none" => None)),
        "fill" => view.fill(),
        "scrollable" => view.scrollable(),
        "scrollableHorizontal" => view.scrollable_horizontal(),
        "truncate" => view.truncate(),
        "name" => view.name(literal(value)?),
        "navigationTitle" => view.navigation_title(literal(value)?),
        "appBarVisible" => view.app_bar_visible(value.trim().parse().ok()?),
        "showBackButton" => view.show_back_button(value.trim().parse().ok()?),
        "centerTitle" => view.center_title(value.trim().parse().ok()?),
        "appBarBackground" => view.app_bar_background(preview_color(value)?),
        "appBarColor" => view.app_bar_color(preview_color(value)?),
        "key" => view,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_preview_applies_hex_colors_and_gradient_stops() {
        let view = AnyView::new(Text::new("Swatch"));
        let view = modifier(view, "textcolor", r##""#abc""##).unwrap();
        let view = modifier(view, "hoverBackground", r##""#1234""##).unwrap();
        let view = modifier(view, "gradient", r##".accent, "#01234567", 90"##).unwrap();
        let style = view.into_node().style;
        assert_eq!(style.color, Some(Color::Rgb(0xaabbcc)));
        assert_eq!(style.hover_background, Some(Color::Rgba(0x11223344)));
        let Some(omen::Background::Gradient(gradient)) = style.background else {
            panic!("expected gradient");
        };
        assert_eq!(gradient.from, Color::Accent);
        assert_eq!(gradient.to, Color::Rgba(0x01234567));
        assert_eq!(gradient.angle, 90.);
    }

    #[test]
    fn adaptive_direction_is_shared_by_studio_and_the_compiler() {
        let source = "view! { Home { Layout {}.mode(.vertical) } }";
        let node = omen_macro_core::inspector::visual_nodes(source)
            .unwrap()
            .remove(0);
        let value = ".adaptive(compact: .vertical, expanded: .horizontal)";
        let patch = omen_macro_core::inspector::set_layout_direction(source, &node, value).unwrap();
        let mut source = source.to_string();
        source.replace_range(patch.range, &patch.replacement);
        assert!(source.contains(".direction(.adaptive("));
        let tree = VisualTree::parse(&source).unwrap();
        assert_eq!(component_label(&tree.nodes[0], 390.), "Layout · Column");
        assert_eq!(component_label(&tree.nodes[0], 1024.), "Layout · Row");
    }

    #[test]
    fn interactive_preview_pushes_literal_view_arguments_and_supports_back() {
        use omen::ViewKind;

        fn action(node: &ViewNode, label: &str) -> Option<omen::Action> {
            match &node.kind {
                ViewKind::Button {
                    label: text,
                    action,
                    ..
                } if text == label => Some(action.clone()),
                ViewKind::Layout(children) | ViewKind::Group(children) => {
                    children.iter().find_map(|node| action(node, label))
                }
                _ => None,
            }
        }
        fn has_text(node: &ViewNode, label: &str) -> bool {
            match &node.kind {
                ViewKind::Text(text) => text == label,
                ViewKind::Layout(children) | ViewKind::Group(children) => {
                    children.iter().any(|node| has_text(node, label))
                }
                _ => false,
            }
        }
        fn visible_page(frame: &ViewNode) -> &ViewNode {
            let ViewKind::Layout(children) = &frame.kind else {
                panic!("expected a simulator frame");
            };
            let mut page = &children[0];
            while let ViewKind::Group(children) = &page.kind {
                assert!(page.style.is_transparent_group());
                page = &children[0];
            }
            page
        }

        let previews = VisualTree::for_preview(
            r##"
            view! {
                @Design
                Overview(navigator: Navigator) {
                    let published_nav = navigator;
                    Layout {
                        Button("View releases") {
                            published_nav.push_view(view! {
                                ProjectDetail(
                                    title: "Published",
                                    count: "8",
                                    description: "The latest release shipped on Friday."
                                )
                            });
                        }
                        Button("Runtime only") {
                            published_nav.push_view(view! {
                                ProjectDetail(title: current_title(), count: "9",
                                    description: "Computed in Rust")
                            });
                        }
                    }.frame(.infinity, .infinity).background("#0D0D0F")
                }
                ProjectDetail(title: String, count: String, description: String) {
                    Layout {
                        Text(title.clone())
                        Text(count)
                        Text(description)
                    }.frame(.infinity, .infinity).background("#0D0D0F").navigationTitle(title)
                }
            }
        "##,
            None,
        )
        .unwrap();
        let tree = previews
            .iter()
            .find(|view| view.marked)
            .unwrap()
            .tree
            .as_ref()
            .unwrap();
        let reorder = Reorder {
            scope: "navigation".into(),
            move_node: Rc::new(|_, _, _| {}),
            resize: Rc::new(|_, _| {}),
        };
        let session = InteractivePreview::default();
        let frame = VisualFrame::new(1, Profile::IPhone);
        let edit = frame_content(
            tree,
            &frame,
            None,
            Rc::new(|_| {}),
            FrameInteraction::Design,
            &reorder,
            None,
        )
        .into_node();
        let mut runtime = omen::Runtime::new(
            frame_content(
                tree,
                &frame,
                None,
                Rc::new(|_| {}),
                FrameInteraction::Interactive(&session),
                &reorder,
                None,
            )
            .into_node(),
        );
        let overview = runtime.render().unwrap();
        let edit_page = visible_page(&edit);
        let interact_page = visible_page(&overview);
        assert_eq!(edit_page.style.frame, interact_page.style.frame);
        assert_eq!(edit_page.style.background, interact_page.style.background);
        action(&overview, "Runtime only").unwrap().invoke();
        assert!(session.navigator.entries().is_empty());

        action(&overview, "View releases").unwrap().invoke();
        let detail = runtime.render().unwrap();
        assert!(has_text(&detail, "Published"));
        assert!(has_text(&detail, "8"));
        assert!(has_text(&detail, "The latest release shipped on Friday."));
        let detail_screen = visible_page(&detail);
        assert_eq!(detail_screen.style.frame, detail.style.frame);
        assert_eq!(detail_screen.style.background, edit_page.style.background);
        let ViewKind::Layout(children) = &detail_screen.kind else {
            panic!("expected a navigation bar and detail page");
        };
        let bar = &children[0];
        assert_eq!(bar.style.background, edit_page.style.background);
        assert_eq!(bar.style.color, Some(Color::White));
        let ViewKind::Layout(controls) = &bar.kind else {
            panic!("expected app bar controls");
        };
        assert_eq!(
            controls[0].style.app_bar_platform,
            Some(AppBarPlatform::Ios)
        );
        action(&detail, "Back").unwrap().invoke();

        let overview = runtime.render().unwrap();
        assert!(action(&overview, "View releases").is_some());
        assert!(!has_text(&overview, "Published"));
    }

    #[test]
    fn nested_layout_guides_follow_depth_and_the_frame_breakpoint() {
        let source = r#"view! { Home {
            Layout {
                Layout { Text("Nested") }.horizontal(.medium)
            }.mode(.stack)
        } }"#;
        let tree = VisualTree::parse(source).unwrap();
        let select: Select = Rc::new(|_| {});
        let reorder = Reorder {
            scope: "layout-guides".into(),
            move_node: Rc::new(|_, _, _| {}),
            resize: Rc::new(|_, _| {}),
        };
        let frame = VisualFrame::new(1, Profile::WebDesktop);
        let content = frame_content(
            &tree,
            &frame,
            Some(1),
            select.clone(),
            FrameInteraction::Design,
            &reorder,
            None,
        )
        .into_node();
        let omen::ViewKind::Layout(roots) = content.kind else {
            panic!("frame layout");
        };
        let outer = &roots[0];
        let omen::ViewKind::Layout(children) = &outer.kind else {
            panic!("outer layout");
        };
        let inner = &children[0];
        assert_eq!(outer.style.preview.as_ref().unwrap().layout_depth, Some(0));
        assert_eq!(inner.style.preview.as_ref().unwrap().layout_depth, Some(1));
        assert_eq!(
            inner.style.for_width(390.).mode.unwrap_or_default(),
            LayoutMode::Vertical
        );
        assert_eq!(
            inner.style.for_width(1200.).mode,
            Some(LayoutMode::Horizontal)
        );
        assert_eq!(component_label(&tree.nodes[0], 390.), "Layout · Stack");
        assert_eq!(component_label(&tree.nodes[1], 1200.), "Layout · Row");

        let interactive = frame_content(
            &tree,
            &frame,
            None,
            select,
            FrameInteraction::Interactive(&InteractivePreview::default()),
            &reorder,
            None,
        )
        .into_node();
        let omen::ViewKind::Layout(children) = interactive.kind else {
            panic!("frame layout");
        };
        assert!(children[0].style.preview.is_none());
    }
}
