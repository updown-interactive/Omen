//! GPUI rendering adapter for Omen's renderer-independent view tree.
#![deny(missing_docs)]

use gpui::{
    AnyElement, App, Context, Div, FontWeight, IntoElement, Render, SharedString, Window,
    WindowOptions, div, prelude::*, px, rgb,
};
#[cfg(not(any(target_os = "ios", target_os = "android")))]
use gpui::{Bounds, TitlebarOptions, WindowBounds, size};
use omen_core::{Color, Font, LayoutMode, Style, View, ViewKind, ViewNode, Weight};
use std::{cell::RefCell, rc::Rc};

mod design_canvas;
mod device_shell;
mod focus_scroll;
mod motion;
mod presentation;
use motion::animate;
mod icons;
mod input;
#[cfg(target_os = "macos")]
mod macos_app_icon;
#[cfg(not(any(target_os = "ios", target_os = "android")))]
mod scenes;
mod split;
#[cfg(not(any(target_os = "ios", target_os = "android")))]
mod titlebar;

/// An error opening the native desktop window.
#[derive(Debug, thiserror::Error)]
pub enum WindowError {
    /// GPUI could not create a native window.
    #[error("failed to open Omen window: {0}")]
    OpenWindow(String),
    /// The initial reactive view could not be constructed.
    #[error(transparent)]
    Reactive(#[from] omen_core::RuntimeError),
}

/// A retained Omen tree rendered by GPUI. Actions survive normal GPUI rerenders.
pub struct OmenWindow {
    runtime: omen_core::Runtime,
    wake_installed: bool,
    visibility_subscription: Option<gpui::Subscription>,
    close_actions: Vec<omen_core::Action>,
    blur_actions: Vec<omen_core::Action>,
    activation_subscription: Option<gpui::Subscription>,
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    scenes: scenes::Windows,
}
impl OmenWindow {
    /// Create an isolated reactive window from a root view.
    pub fn new(view: impl View) -> Self {
        let root = view.into_node();
        let (close_actions, blur_actions) = window_actions(&root);
        Self {
            runtime: omen_core::Runtime::new(root),
            close_actions,
            blur_actions,
            activation_subscription: None,
            wake_installed: false,
            visibility_subscription: None,
            #[cfg(not(any(target_os = "ios", target_os = "android")))]
            scenes: scenes::Windows::default(),
        }
    }
}
impl Drop for OmenWindow {
    fn drop(&mut self) {
        for action in &self.close_actions {
            action.invoke();
        }
    }
}
fn window_actions(root: &ViewNode) -> (Vec<omen_core::Action>, Vec<omen_core::Action>) {
    fn visit(
        node: &ViewNode,
        close: &mut Vec<omen_core::Action>,
        blur: &mut Vec<omen_core::Action>,
    ) {
        close.extend(node.style.on_window_close.iter().cloned());
        blur.extend(node.style.on_window_blur.iter().cloned());
        match &node.kind {
            ViewKind::Group(children)
            | ViewKind::Layout(children)
            | ViewKind::List(children)
            | ViewKind::Card(children)
            | ViewKind::Presentation { children, .. } => {
                for child in children {
                    visit(child, close, blur);
                }
            }
            _ => {}
        }
    }
    let (mut close, mut blur) = (Vec::new(), Vec::new());
    visit(root, &mut close, &mut blur);
    (close, blur)
}
impl Render for OmenWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.wake_installed {
            let weak = cx.entity().downgrade();
            let app = cx.to_async();
            let executor = cx.foreground_executor().clone();
            self.runtime.set_waker(move || {
                let weak = weak.clone();
                let app = app.clone();
                executor
                    .spawn(async move {
                        app.update(|cx| {
                            let _ = weak.update(cx, |_, cx| cx.notify());
                        });
                    })
                    .detach();
            });
            self.wake_installed = true;
        }
        if self.visibility_subscription.is_none() {
            native_trace(&format!("first view render: {:?}", window.viewport_size()));
            let weak = cx.entity().downgrade();
            self.visibility_subscription =
                Some(window.observe_window_visibility(move |visibility, _, cx| {
                    let _ = weak.update(cx, |app, cx| {
                        app.runtime.set_lifecycle(if visibility.is_visible() {
                            omen_core::AppLifecycle::Foreground
                        } else {
                            omen_core::AppLifecycle::Background
                        });
                        if !visibility.is_visible() {
                            let _ = app.runtime.render();
                            app.runtime.take_tasks();
                        }
                        cx.notify();
                    });
                }));
        }
        let viewport = window.viewport_size();
        use omen_core::{
            DeviceEnvironment, DevicePlatform, KeyboardViewport, PointerCapabilities, ThemeMode,
            Viewport,
        };
        let insets = window.insets();
        let platform = DevicePlatform::native();
        let mobile = matches!(platform, DevicePlatform::Ios | DevicePlatform::Android);
        let device = DeviceEnvironment {
            pixel_ratio: window.scale_factor(),
            #[cfg(target_os = "android")]
            text_scale: gpui_android::text_scale(),
            platform,
            pointer: PointerCapabilities {
                touch: mobile,
                mouse: !mobile,
            },
            keyboard: KeyboardViewport::from_insets(logical_insets(insets.ime)),
            appearance: if system_dark(window) {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            ..DeviceEnvironment::default()
        }
        .with_viewport(Viewport {
            width: viewport.width.into(),
            height: viewport.height.into(),
            safe_area: logical_insets(insets.safe_area),
        });
        self.runtime.set_device_environment(device);
        let root = match self.runtime.render() {
            Ok(root) => root,
            Err(error) => {
                eprintln!("Omen view error: {error}");
                ViewNode::new(ViewKind::Text("Unable to display this view".into()))
            }
        };
        let (close_actions, blur_actions) = window_actions(&root);
        if window.is_visible() {
            self.blur_actions = blur_actions;
        }
        if self.activation_subscription.is_none() && !self.blur_actions.is_empty() {
            self.activation_subscription =
                Some(cx.observe_window_activation(window, |app, window, _| {
                    if !window.is_window_active() {
                        for action in &app.blur_actions {
                            action.invoke();
                        }
                    }
                }));
        }
        // A hidden runtime temporarily renders an empty tree; root teardown
        // registrations must survive hiding or a transient render failure.
        if !close_actions.is_empty() {
            self.close_actions = close_actions;
        }
        for job in self.runtime.take_tasks() {
            cx.foreground_executor().spawn(job.run()).detach();
        }
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        if window.is_visible() {
            let mut requested = Vec::new();
            scenes::declarations(&root, &mut requested);
            // Opening a window during rendering disrupts GPUI's dependency collection.
            let owner = cx.entity().downgrade();
            cx.defer(move |cx| {
                let _ = owner.update(cx, |this, cx| this.scenes.reconcile(requested, cx));
            });
        }
        let tokens = root_theme(&root).resolve(system_dark(window));
        div()
            .id("omen-window")
            .size_full()
            .overflow_y_scroll()
            .bg(rgb(tokens.canvas))
            .text_color(rgb(tokens.primary))
            .text_size(px(tokens.body_size))
            .child(OmenTree { root })
    }
}

/// Convert an Omen tree into native elements with persistent control state.
pub fn into_element(view: &ViewNode) -> AnyElement {
    OmenTree { root: view.clone() }.into_any_element()
}

#[derive(IntoElement)]
struct OmenTree {
    root: ViewNode,
}
impl gpui::RenderOnce for OmenTree {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        input::initialize(cx);
        focus_scroll::begin(window, cx);
        presentation::reconcile(&self.root, window, cx);
        let dismiss = presentation::dismiss_action(&self.root);
        let root_focus = window
            .use_keyed_state("omen-root-focus", cx, |_, cx| {
                FocusState(cx.focus_handle().tab_index(0).tab_stop(false))
            })
            .read(cx)
            .0
            .clone();
        if window.focused(cx).is_none() {
            root_focus.focus(window, cx);
        }
        // A named view resolves through transparent groups. Avoid an auto-sized
        // wrapper between a viewport-sized root and its percentage height.
        let mut root = &self.root;
        while let ViewKind::Group(children) = &root.kind {
            if root.style.is_transparent_group() && children.len() == 1 {
                root = &children[0];
            } else {
                break;
            }
        }
        input::keyboard_root(
            div()
                .size_full()
                .id("omen-tree")
                .track_focus(&root_focus)
                .on_key_down(move |event, window, cx| {
                    if event.keystroke.key == "escape"
                        && let Some(dismiss) = &dismiss
                    {
                        dismiss.invoke();
                        window.refresh();
                        cx.stop_propagation();
                    }
                })
                .child(render_node(
                    root,
                    &mut 0,
                    RenderSettings::new(window, cx),
                    window,
                    cx,
                )),
        )
    }
}

pub(crate) fn fills_height(node: &ViewNode) -> bool {
    if node
        .style
        .frame
        .is_some_and(|frame| frame.height == f32::INFINITY)
    {
        return true;
    }
    match &node.kind {
        ViewKind::Group(children) if children.len() == 1 => fills_height(&children[0]),
        ViewKind::Presentation { children, .. } => children.first().is_some_and(fills_height),
        _ => false,
    }
}

fn root_theme(node: &ViewNode) -> omen_core::Theme {
    if let Some(theme) = node.style.theme {
        return theme;
    }
    if let ViewKind::Group(children) = &node.kind
        && let [child] = children.as_slice()
    {
        return root_theme(child);
    }
    omen_core::Theme::default()
}
fn logical_insets(insets: gpui::Edges<gpui::Pixels>) -> omen_core::Insets {
    omen_core::Insets {
        top: insets.top.into(),
        right: insets.right.into(),
        bottom: insets.bottom.into(),
        left: insets.left.into(),
    }
}

fn system_dark(window: &Window) -> bool {
    matches!(
        window.appearance(),
        gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
    )
}
#[derive(Clone, Copy)]
struct RenderSettings {
    tokens: omen_core::DesignTokens,
    direction: omen_core::LayoutDirection,
    insets: omen_core::Insets,
    inert: bool,
    reduced_motion: bool,
    parent_stretches: bool,
    parent_layout: LayoutMode,
    focus_scroll: Option<usize>,
    viewport_width: f32,
    container_width: f32,
    flow_width: Option<f32>,
    grid_flow: bool,
    adaptive_flow: bool,
    scale: f32,
}
impl RenderSettings {
    fn new(window: &Window, cx: &App) -> Self {
        Self {
            tokens: omen_core::Theme::default().resolve(system_dark(window)),
            direction: omen_core::LayoutDirection::LeftToRight,
            insets: {
                let insets = window.insets().effective();
                omen_core::Insets {
                    top: insets.top.into(),
                    right: insets.right.into(),
                    bottom: insets.bottom.into(),
                    left: insets.left.into(),
                }
            },
            inert: false,
            reduced_motion: cx.reduce_motion(),
            parent_stretches: true,
            parent_layout: LayoutMode::Vertical,
            focus_scroll: None,
            viewport_width: window.viewport_size().width.into(),
            container_width: window.viewport_size().width.into(),
            flow_width: None,
            grid_flow: false,
            adaptive_flow: false,
            scale: 1.,
        }
    }
}
struct FocusState(gpui::FocusHandle);

#[inline(always)]
fn render_node(
    node: &ViewNode,
    next_id: &mut usize,
    settings: RenderSettings,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    render_node_with_pin(node, next_id, settings, None, window, cx)
}

fn render_node_with_pin(
    node: &ViewNode,
    next_id: &mut usize,
    settings: RenderSettings,
    pin_parent: Option<&design_canvas::Placement<'_>>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let id = *next_id;
    *next_id += 1;
    let mut settings = settings;
    let preview = node.style.preview.as_deref();
    if let Some(width) = preview.and_then(|preview| preview.width) {
        settings.viewport_width = width;
        settings.container_width = width;
    }
    if let Some(scale) = preview.and_then(|preview| preview.scale) {
        settings.scale = scale.clamp(0.1, 2.);
    }
    let scale = settings.scale;
    let px = move |value| gpui::px(value * scale);
    if let Some(theme) = node.style.theme {
        settings.tokens = theme.resolve(system_dark(window));
    }
    if let Some(insets) = node.style.safe_area_insets {
        settings.insets = insets;
    }
    let style = node.style.for_container(
        settings.viewport_width,
        settings.container_width,
        settings.tokens,
        settings.insets,
    );
    let content_width =
        style.content_width(settings.container_width, settings.tokens, settings.insets);
    let grid = style.grid_metrics(content_width, settings.tokens);
    let key = SharedString::from(
        style
            .runtime_id
            .clone()
            .unwrap_or_else(|| format!("omen-{id}")),
    );
    if let Some(locale) = &style.locale {
        settings.direction = locale.direction;
    }
    if let Some(direction) = style.direction {
        settings.direction = direction;
    }
    if let Some(reduced) = style.reduced_motion {
        settings.reduced_motion = reduced;
    }
    let navigation = preview.is_some_and(|preview| preview.navigation.is_some());
    let scroll_handle = if navigation || style.scrollable || style.horizontal_scrollable {
        let handle = window
            .use_keyed_state(SharedString::from(format!("{key}-scroll")), cx, |_, _| {
                gpui::ScrollHandle::new()
            })
            .read(cx)
            .clone();
        if style.follow_scroll_end && handle.max_offset().y + handle.offset().y <= px(1.) {
            handle.scroll_to_bottom();
        }
        if !navigation {
            focus_scroll::register(id, handle.clone(), window, cx);
        }
        Some(handle)
    } else {
        None
    };
    let mut child_settings = settings;
    child_settings.container_width = content_width;
    if matches!(node.kind, ViewKind::Card(_))
        && style.padding.is_none()
        && style.padding_token.is_none()
    {
        child_settings.container_width =
            (content_width - 2. * settings.tokens.space(omen_core::Space::Large) - 2.).max(0.);
    }
    child_settings.grid_flow = grid.is_some();
    child_settings.adaptive_flow = style.adaptive_direction.is_some() || grid.is_some();
    child_settings.flow_width = grid.map(|(_, width)| width);
    if grid.is_none()
        && style.mode == Some(LayoutMode::Horizontal)
        && let ViewKind::Layout(children) = &node.kind
    {
        let spacing = style
            .spacing
            .or_else(|| {
                style
                    .spacing_token
                    .map(|token| settings.tokens.space(token))
            })
            .unwrap_or(0.);
        child_settings.flow_width =
            omen_core::remaining_row_width(children, content_width, spacing);
    }
    if scroll_handle.is_some() && !navigation {
        child_settings.focus_scroll = Some(id);
    }
    if matches!(
        node.kind,
        ViewKind::Layout(_) | ViewKind::Card(_) | ViewKind::List(_) | ViewKind::Group(_)
    ) {
        child_settings.parent_stretches =
            style.alignment.is_none() && style.mode.unwrap_or_default() != LayoutMode::Horizontal;
    }
    child_settings.parent_layout = match node.kind {
        ViewKind::Layout(_) => style.mode.unwrap_or_default(),
        ViewKind::Group(_) | ViewKind::Card(_) | ViewKind::List(_) => LayoutMode::Vertical,
        _ => settings.parent_layout,
    };
    if let Some(edges) = style.safe_area {
        use omen_core::SafeArea::*;
        if matches!(edges, All | Vertical) {
            child_settings.insets.top = 0.;
            child_settings.insets.bottom = 0.;
        }
        if matches!(edges, All | Horizontal) {
            child_settings.insets.left = 0.;
            child_settings.insets.right = 0.;
        }
    }
    let placement_axis = preview.and_then(|preview| preview.placement.as_ref());
    let adapted_placement = if settings.adaptive_flow {
        placement_axis
            .filter(|placement| placement.axis.is_some())
            .map(|placement| {
                let mut placement = placement.clone();
                placement.axis = Some(if settings.grid_flow {
                    LayoutMode::Horizontal
                } else {
                    settings.parent_layout
                });
                placement
            })
    } else {
        None
    };
    let placement = adapted_placement
        .as_ref()
        .or(placement_axis)
        .map(|placement| {
            (
                placement,
                Rc::new(std::cell::Cell::new(gpui::Bounds::default())),
            )
        });
    let element =
        match &node.kind {
            ViewKind::List(rows) => {
                if let Some(height) = style.virtual_row_height {
                    let rows = rows.clone();
                    let settings = child_settings;
                    let list_key = key.clone();
                    div().size_full().min_h_0().child(
                        gpui::uniform_list(list_key, rows.len(), move |range, window, cx| {
                            range
                                .map(|index| {
                                    let mut id = index * 1000;
                                    div().h(px(height)).w_full().overflow_hidden().child(
                                        render_node(&rows[index], &mut id, settings, window, cx),
                                    )
                                })
                                .collect::<Vec<_>>()
                        })
                        .size_full(),
                    )
                } else {
                    rows.iter().fold(div().flex().flex_col(), |element, row| {
                        let row_id = *next_id;
                        *next_id += 1;
                        let mut row_settings = child_settings;
                        row_settings.parent_stretches = true;
                        element.child(
                            div()
                                .flex()
                                .flex_col()
                                .id(SharedString::from(format!("omen-row-{row_id}")))
                                .role(gpui::Role::ListItem)
                                .child(render_node(row, next_id, row_settings, window, cx)),
                        )
                    })
                }
            }
            ViewKind::Group(children) => render_children(
                div()
                    .flex()
                    .flex_col()
                    .when(fills_height(node), |element| element.h_full()),
                children,
                next_id,
                placement.as_ref(),
                child_settings,
                false,
                window,
                cx,
            ),
            ViewKind::Card(children) => render_children(
                div()
                    .flex()
                    .flex_col()
                    .p(px(settings.tokens.space(omen_core::Space::Large)))
                    .gap(px(settings.tokens.space(omen_core::Space::Medium)))
                    .rounded(px(settings.tokens.radius))
                    .bg(rgb(settings.tokens.surface))
                    .border_1()
                    .border_color(rgb(settings.tokens.border)),
                children,
                next_id,
                placement.as_ref(),
                child_settings,
                false,
                window,
                cx,
            ),
            ViewKind::Layout(children) => {
                let mode = style.mode.unwrap_or_default();
                let element = if let Some((columns, _)) = grid {
                    div().grid().grid_cols(columns).w_full().min_w_0()
                } else {
                    match mode {
                        LayoutMode::Horizontal => {
                            let row = div().flex().items_center();
                            if settings.direction == omen_core::LayoutDirection::RightToLeft {
                                row.flex_row_reverse()
                            } else {
                                row.flex_row()
                            }
                        }
                        LayoutMode::Vertical => div().flex().flex_col(),
                        LayoutMode::Stack => div()
                            .grid()
                            .grid_cols_max_content(1)
                            .grid_rows_max_content(1),
                    }
                };
                let element = if let Some(preview) =
                    preview.filter(|p| p.grid.is_some() || p.viewport_size.is_some())
                {
                    element.relative().child(design_canvas::grid(
                        preview.grid.map(|color| settings.tokens.color(color)),
                        preview.viewport_size.clone(),
                        preview.grid_transform,
                    ))
                } else {
                    element
                };
                render_children(
                    element,
                    children,
                    next_id,
                    placement.as_ref(),
                    child_settings,
                    grid.is_none() && mode == LayoutMode::Stack,
                    window,
                    cx,
                )
            }
            _ => {
                let content = render_leaf(
                    node,
                    &style,
                    &key,
                    LeafSettings {
                        own: settings,
                        child: child_settings,
                    },
                    next_id,
                    window,
                    cx,
                );
                return finish_node(
                    node,
                    &style,
                    key,
                    settings,
                    scroll_handle,
                    placement,
                    pin_parent,
                    content,
                    window,
                    cx,
                );
            }
        };
    let content = NativeContent {
        element,
        role: if matches!(node.kind, ViewKind::List(_)) {
            gpui::Role::List
        } else {
            gpui::Role::Group
        },
        label: String::new(),
        activation: None,
        checked: None,
    };
    finish_node(
        node,
        &style,
        key,
        settings,
        scroll_handle,
        placement,
        pin_parent,
        content,
        window,
        cx,
    )
}

struct NativeContent {
    element: Div,
    role: gpui::Role,
    label: String,
    activation: Option<omen_core::Action>,
    checked: Option<bool>,
}

// Keep leaf construction and event wiring out of each recursive container frame,
// including in debug builds where GPUI style builders create large temporaries.
#[derive(Clone, Copy)]
struct LeafSettings {
    own: RenderSettings,
    child: RenderSettings,
}

#[inline(never)]
fn render_leaf(
    node: &ViewNode,
    style: &Style,
    key: &SharedString,
    settings: LeafSettings,
    next_id: &mut usize,
    window: &mut Window,
    cx: &mut App,
) -> NativeContent {
    let LeafSettings {
        own: settings,
        child: child_settings,
    } = settings;
    let px = |value| gpui::px(value * settings.scale);
    let preview = style.preview.as_deref();
    let mut role = gpui::Role::Group;
    let mut label = String::new();
    let mut activation = None;
    let mut checked = None;
    let element = match &node.kind {
        ViewKind::Presentation { .. } => {
            presentation::render(node, key, next_id, child_settings, window, cx)
        }
        ViewKind::Reactive(_) => div().child("Unresolved reactive view"),
        ViewKind::Text(text) => {
            role = gpui::Role::Label;
            label = text.clone();
            div()
                .when(!style.truncate, |el| el.flex().flex_col())
                .child(text.clone())
        }
        ViewKind::Image(path) => {
            role = gpui::Role::Image;
            let frame = style.frame.unwrap_or(omen_core::Frame {
                width: 96.,
                height: 96.,
            });
            let inset = style.padding.unwrap_or(0.).max(0.) * 2.;
            div()
                .flex()
                .flex_col()
                .w(px(96.))
                .h(px(96.))
                .overflow_hidden()
                .child(
                    gpui::img(std::path::PathBuf::from(path))
                        .w(px((frame.width - inset).max(0.)))
                        .h(px((frame.height - inset).max(0.)))
                        .object_fit(gpui::ObjectFit::Contain)
                        .rounded(px(style.corner_radius.unwrap_or(0.).max(0.)))
                        .with_fallback(|| {
                            div()
                                .child("Image unavailable")
                                .text_color(rgb(0x94a3b8))
                                .into_any_element()
                        }),
                )
        }
        ViewKind::Icon(name) => {
            role = gpui::Role::Image;
            label = name.clone();
            div().size(px(24.)).child(icons::IconElement {
                source: name.clone(),
            })
        }
        ViewKind::Button {
            label: title,
            action,
        } => {
            role = gpui::Role::Button;
            label = title.clone();
            activation = Some(action.clone());
            let back_icon = style.navigation_back.then(|| {
                let chevron = style.app_bar_platform.unwrap_or_else(omen_core::AppBarPlatform::current).uses_chevron();
                let rtl = settings.direction == omen_core::LayoutDirection::RightToLeft;
                let source = match (chevron, rtl) {
                    (true, false) => "chevron-left",
                    (true, true) => "chevron-right",
                    (false, false) => "arrow-left",
                    (false, true) => "arrow-right",
                };
                div().size(px(20.)).child(icons::IconElement { source: source.into() })
            });
            let button = div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .when(!style.navigation_back, |button| button.child(title.clone()))
                .children(back_icon)
                .cursor_pointer()
                .block_mouse_except_scroll();
            if settings.parent_stretches {
                if settings.direction == omen_core::LayoutDirection::RightToLeft {
                    button.self_end()
                } else {
                    button.self_start()
                }
            } else {
                button
            }
        }
        ViewKind::TextField { label, value } | ViewKind::SecureField { label, value } => {
            let value = style
                .text_value
                .clone()
                .unwrap_or_else(|| value.clone().into());
            let label = style
                .accessibility
                .label
                .clone()
                .unwrap_or_else(|| label.clone());
            let secure = matches!(node.kind, ViewKind::SecureField { .. });
            let input = window.use_keyed_state(key.clone(), cx, |_, cx| {
                input::TextInput::new(
                    label.clone(),
                    value.clone(),
                    secure,
                    style.on_change.clone(),
                    style.accessibility.hint.clone(),
                    cx,
                )
            });
            input.update(cx, |input, _| {
                input.configure(
                    label,
                    value,
                    style.on_change.clone(),
                    style.accessibility.hint.clone(),
                )
            });
            let mut bg_color: gpui::Hsla = style
                .background
                .as_ref()
                .and_then(|bg| match bg {
                    omen_core::Background::Color(c) => {
                        Some(gpui::rgba(settings.tokens.rgba(*c)).into())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| {
                    rgb(if style.multiline {
                        settings.tokens.canvas
                    } else {
                        settings.tokens.surface
                    })
                    .into()
                });
            bg_color.a = 1.0;
            let field_inert =
                settings.inert || preview.is_some_and(|preview| preview.outline.is_some());
            input.update(cx, |input, cx| {
                input.focus_handle = input.focus_handle.clone().tab_stop(!field_inert);
                input.rtl = settings.direction == omen_core::LayoutDirection::RightToLeft;
                input.multiline = style.multiline;
                input.code_document = style.code_document.clone();
                input.accent = rgb(settings.tokens.accent).into();
                input.background = bg_color;
                if style.autofocus && !field_inert && !input.autofocus_applied {
                    input.focus_handle.focus(window, cx);
                    input.autofocus_applied = true;
                }
            });
            let field_focus = input.read(cx).focus_handle.clone();
            let marker = focus_scroll::handle(settings.focus_scroll, window, cx)
                .map(|scroll| focus_scroll::marker(field_focus, scroll, &key, window, cx));
            let field = if field_inert {
                let value = input.read(cx).display_snapshot();
                div().child(value).into_any_element()
            } else {
                input.into_any_element()
            };
            div()
                .when(style.multiline, |element| element.size_full().min_h_0())
                .when(!style.multiline, |element| element.w(px(260.)))
                .p(px(10.))
                .rounded(px(8.))
                .bg(rgb(if style.multiline {
                    settings.tokens.canvas
                } else {
                    settings.tokens.surface
                }))
                .child(field)
                .children(marker)
        }
        ViewKind::Checkbox {
            label: title,
            value,
        }
        | ViewKind::Toggle {
            label: title,
            value,
        } => {
            let value = style
                .checked
                .clone()
                .unwrap_or_else(|| value.clone().into());
            let is_checked = value.get();
            checked = Some(is_checked);
            let toggle = matches!(node.kind, ViewKind::Toggle { .. });
            role = if toggle {
                gpui::Role::Switch
            } else {
                gpui::Role::CheckBox
            };
            label = title.clone();
            let on_change = style.on_change.clone();
            activation = Some(omen_core::Action::new(move || {
                value.set(!value.get());
                if let Some(action) = &on_change {
                    action.invoke();
                }
            }));
            let indicator = if toggle {
                let track = div()
                    .flex()
                    .items_center()
                    .w(px(42.))
                    .h(px(24.))
                    .p(px(2.))
                    .rounded_full()
                    .bg(rgb(if is_checked {
                        settings.tokens.accent
                    } else {
                        settings.tokens.border
                    }));
                if is_checked {
                    track.justify_end()
                } else {
                    track.justify_start()
                }
                .child(
                    div()
                        .size(px(20.))
                        .rounded_full()
                        .bg(rgb(settings.tokens.canvas)),
                )
            } else {
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(22.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(rgb(settings.tokens.border))
                    .bg(rgb(if is_checked {
                        settings.tokens.accent
                    } else {
                        settings.tokens.surface
                    }))
                    .text_color(rgb(if is_checked {
                        settings.tokens.canvas
                    } else {
                        settings.tokens.primary
                    }))
                    .child(if is_checked { "✓" } else { "" })
            };
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(10.))
                .p(px(6.))
                .cursor_pointer()
                .block_mouse_except_scroll()
                .child(indicator)
                .child(title.clone())
        }
        ViewKind::Progress(fraction) => {
            role = gpui::Role::ProgressIndicator;
            div()
                .w(px(180.))
                .h(px(8.))
                .rounded(px(4.))
                .overflow_hidden()
                .bg(rgb(settings.tokens.border))
                .child(div().h_full().w(gpui::relative(*fraction)).bg(gpui::rgba(
                    settings.tokens.rgba(style.color.unwrap_or(Color::Accent)),
                )))
        }
        ViewKind::Gap(size) => {
            let gap = div().flex_none().self_stretch();
            let gap = match settings.parent_layout {
                LayoutMode::Horizontal => gap.w(px(*size)),
                LayoutMode::Vertical => gap.h(px(*size)),
                LayoutMode::Stack => gap.size(px(0.)),
            };
            gap.when(
                preview.is_some_and(|preview| preview.outline.is_some()),
                |gap| gap.bg(gpui::rgba((settings.tokens.accent << 8) | 0x20)),
            )
        }
        ViewKind::Divider => {
            role = gpui::Role::Splitter;
            div().w_full().h(px(1.)).bg(rgb(settings.tokens.border))
        }
        ViewKind::List(_) | ViewKind::Group(_) | ViewKind::Card(_) | ViewKind::Layout(_) => {
            unreachable!("container nodes are rendered before leaf construction")
        }
    };
    NativeContent {
        element,
        role,
        label,
        activation,
        checked,
    }
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn finish_node(
    node: &ViewNode,
    style: &Style,
    key: SharedString,
    settings: RenderSettings,
    scroll_handle: Option<gpui::ScrollHandle>,
    placement: Option<design_canvas::Placement<'_>>,
    pin_parent: Option<&design_canvas::Placement<'_>>,
    content: NativeContent,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let NativeContent {
        mut element,
        role,
        label,
        mut activation,
        checked,
    } = content;
    let preview = style.preview.as_deref();
    let px = |value| gpui::px(value * settings.scale);
    element = apply_style(element, &node.kind, style, settings);
    if preview.is_some_and(|p| p.width.is_some()) {
        element = element.debug_selector(|| "studio-device-screen".into());
    }
    if let Some((shell, dark)) = preview.and_then(|p| p.shell) {
        element = device_shell::render(element, shell, dark, settings.scale);
    }
    if let Some(preview) = preview
        && let Some(depth) = preview.layout_depth
    {
        element = element.relative().child(design_canvas::layout_outline(
            style.mode.unwrap_or_default(),
            depth,
            preview.selected,
        ));
    }
    if let Some(preview) = preview
        && (preview.layout_guide || preview.insertion_edge.is_some())
    {
        let rtl = settings.direction == omen_core::LayoutDirection::RightToLeft;
        let parent_guide = preview.layout_guide.then(|| {
            let default_padding = if matches!(node.kind, ViewKind::Card(_)) {
                settings.tokens.space(omen_core::Space::Large)
            } else {
                0.
            };
            let padding = style
                .padding
                .or_else(|| {
                    style
                        .padding_token
                        .map(|token| settings.tokens.space(token))
                })
                .unwrap_or(default_padding)
                .max(0.);
            let mut insets = [padding; 4];
            if let Some(edges) = style.safe_area {
                use omen_core::SafeArea::*;
                if matches!(edges, All | Vertical) {
                    insets[0] += settings.insets.top.max(0.);
                    insets[2] += settings.insets.bottom.max(0.);
                }
                if matches!(edges, All | Horizontal) {
                    insets[1] += settings.insets.right.max(0.);
                    insets[3] += settings.insets.left.max(0.);
                }
            }
            if matches!(node.kind, ViewKind::Card(_)) {
                insets = insets.map(|inset| inset + 1. / settings.scale);
            }
            design_canvas::LayoutGuide {
                insets: insets.map(px),
                alignment: style.alignment.map(alignment_axes),
            }
        });
        let horizontal = if parent_guide.is_some() {
            style.min_item_width.is_some() || style.mode == Some(LayoutMode::Horizontal)
        } else {
            placement.as_ref().and_then(|(target, _)| target.axis) == Some(LayoutMode::Horizontal)
        };
        element = element.relative().child(design_canvas::insertion_guides(
            parent_guide,
            preview.insertion_edge,
            horizontal,
            rtl,
            settings.tokens.accent,
        ));
    }
    if let Some(color) = preview
        .filter(|preview| preview.layout_depth.is_none())
        .and_then(|preview| preview.outline)
    {
        let color = gpui::rgba(settings.tokens.rgba(color));
        let outline_group = SharedString::from(format!("{key}-design-outline"));
        let mut outline = div().absolute().inset_0().border_1();
        if preview.is_some_and(|preview| preview.selected) {
            outline = outline.border_color(color);
        } else {
            outline = outline
                .border_color(gpui::transparent_black())
                .group_hover(outline_group.clone(), move |style| {
                    style.border_color(color)
                });
        }
        element = element.relative().group(outline_group).child(outline);
    }
    if preview.is_some_and(|preview| preview.safe_area) {
        let color = rgb(settings.tokens.accent);
        let insets = settings.insets;
        element = element.relative().child(
            div()
                .absolute()
                .top(px(insets.top))
                .bottom(px(insets.bottom))
                .left(px(insets.left))
                .right(px(insets.right))
                .border_1()
                .border_color(color),
        );
    }
    if let Some(action) = &style.on_tap {
        activation = Some(action.clone());
    }
    if let (Some(action), Some(context)) = (&style.async_action, &style.task_context) {
        let action = action.clone();
        let context = context.clone();
        let error = style.on_error.clone();
        let handle = style.task_handle.clone();
        activation = Some(omen_core::Action::new(move || {
            let task = context.spawn(&action, error.clone());
            if let Some(binding) = &handle {
                binding.set(Some(task));
            }
        }));
    }
    let role = node
        .style
        .accessibility
        .role
        .map(accessibility_role)
        .unwrap_or(role);
    let label = node
        .style
        .accessibility
        .label
        .as_ref()
        .unwrap_or(&label)
        .clone();
    if let Some((_, bounds)) = &placement {
        element = design_canvas::placement_bounds(element.relative(), bounds);
    }
    if let Some(resize) = preview.and_then(|preview| preview.resize.as_ref()) {
        element = element.relative().child(design_canvas::resize_handle(
            &key,
            resize,
            settings.scale,
            settings.tokens.accent,
            window,
            cx,
        ));
    }
    let mut element = element.id(key.clone()).role(role);
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    if let Some(height) = style.desktop_toolbar {
        let background = titlebar::background(node, settings.tokens, system_dark(window));
        element = titlebar::render(element, height, &key, background, window, cx);
    }

    if let Some(handle) = &scroll_handle {
        element = element.track_scroll(handle);
        if let Some(gestures) = preview.and_then(|preview| preview.navigation.clone()) {
            element = design_canvas::navigate(element, &key, handle.clone(), gestures, window, cx);
        }
    }
    if let Some(drag) = preview.and_then(|preview| preview.frame_drag.as_ref()) {
        element = design_canvas::move_frame(
            element,
            &key,
            drag,
            preview.is_some_and(|p| p.position.is_some()),
            window,
            cx,
        );
    }
    if let Some(insert) = preview.and_then(|preview| preview.insert.as_ref()) {
        element = design_canvas::insert(element, insert);
    }
    if let Some((placement, bounds)) = placement {
        let rtl = settings.direction == omen_core::LayoutDirection::RightToLeft;
        if let Some(parent) = pin_parent {
            element = design_canvas::move_pinned(
                element,
                (placement, bounds.clone()),
                parent,
                rtl,
                settings.scale,
                window,
                cx,
            );
        }
        element = design_canvas::placement(element, placement, bounds, rtl, settings.tokens.accent);
    }
    if style.scrollable {
        element = element.overflow_y_scroll();
    }
    if style.horizontal_scrollable {
        element = element.overflow_x_scroll();
    }
    if !settings.inert
        && let Some(handle) = &style.resize_handle
    {
        element = split::configure(element, &key, handle);
    }
    if !settings.inert
        && let Some(action) = style.on_context_menu.clone()
    {
        let update_position = presentation::context_position_updater(window, cx);
        element = element.on_mouse_down(gpui::MouseButton::Right, move |event, window, cx| {
            update_position(event.position, cx);
            action.invoke();
            window.refresh();
            cx.stop_propagation();
        });
    }
    if !label.is_empty() {
        element = element.aria_label(label);
    }
    if let Some(hint) = &style.accessibility.hint {
        element = element.aria_description(hint.clone());
    }
    if let Some(selected) = style.accessibility.selected {
        element = element.aria_selected(selected);
    }
    if let Some(checked) = checked {
        element = element.aria_toggled(if checked {
            gpui::Toggled::True
        } else {
            gpui::Toggled::False
        });
    }
    if let ViewKind::Progress(fraction) = node.kind {
        element = element
            .aria_numeric_value(f64::from(fraction))
            .aria_min_numeric_value(0.)
            .aria_max_numeric_value(1.);
    }
    if !settings.inert && !style.shortcuts.is_empty() {
        let shortcuts = style.shortcuts.clone();
        element = element.on_key_down(move |event, window, cx| {
            for (chord, action) in &shortcuts {
                if let Ok(key) = gpui::Keystroke::parse(chord)
                    && key.key == event.keystroke.key
                    && key.modifiers == event.keystroke.modifiers
                {
                    action.invoke();
                    window.refresh();
                    cx.stop_propagation();
                    return;
                }
            }
        });
    }
    if (style.file_picker.is_some() || style.desktop_action.is_some()) && activation.is_none() {
        activation = Some(omen_core::Action::new(|| {}));
    }
    if style.focus_scope && !settings.inert {
        element = element.tab_group();
    }
    let wants_focus = activation.is_some()
        || style.focusable
        || style.focus_request.is_some()
        || style.focus_scope;
    if settings.inert {
        activation = None;
    }
    if wants_focus {
        let focus = window
            .use_keyed_state(SharedString::from(format!("{key}-focus")), cx, |_, cx| {
                FocusState(cx.focus_handle().tab_stop(true).tab_index(0))
            })
            .read(cx)
            .0
            .clone()
            .tab_stop(!settings.inert && (activation.is_some() || style.focusable));
        if !settings.inert {
            if let Some(request) = &style.focus_request
                && request.get()
            {
                focus.focus(window, cx);
                request.set(false);
            }
            let click_focus = focus.clone();
            let a11y_focus = focus.clone();
            let pointer_focus = focus.clone();
            if activation.is_some() || style.focusable {
                element = element.on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                    pointer_focus.focus(window, cx);
                    cx.stop_propagation();
                });
            }
            element = element
                .track_focus(&focus)
                .tab_index(0)
                .tab_stop(activation.is_some() || style.focusable)
                .focus_visible(move |style| {
                    style.border_2().border_color(rgb(settings.tokens.accent))
                })
                .on_a11y_action(gpui::AccessibleAction::Focus, move |_, window, cx| {
                    a11y_focus.focus(window, cx)
                });
            if (activation.is_some() || style.focusable)
                && let Some(scroll) = focus_scroll::handle(settings.focus_scroll, window, cx)
            {
                element = element.child(focus_scroll::marker(
                    focus.clone(),
                    scroll,
                    &key,
                    window,
                    cx,
                ));
            }
            if style.focus_scope {
                element = input::keyboard_scope(element, focus.clone(), style.focus_arrows);
            }
            if let Some(action) = activation.filter(|_| !settings.inert) {
                let native = style.desktop_action.clone();
                let keyboard_native = native.clone();
                let accessible_native = native.clone();
                let picker = style.file_picker.clone();
                let keyboard_picker = picker.clone();
                let accessible_picker = picker.clone();
                let keyboard = action.clone();
                let accessible = action.clone();
                element = element
                    .on_click(move |event, window, cx| {
                        // Enter/Space are handled on key-down below. GPUI also
                        // synthesizes a keyboard click on release; don't dispatch twice.
                        if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                            cx.stop_propagation();
                            return;
                        }
                        click_focus.focus(window, cx);
                        activate_control(&action, &picker, &native, cx);
                        window.refresh();
                        cx.stop_propagation();
                    })
                    .on_key_down(move |event, window, cx| {
                        if !event.is_held
                            && event.keystroke.modifiers == gpui::Modifiers::default()
                            && matches!(event.keystroke.key.as_str(), "enter" | "space")
                        {
                            activate_control(&keyboard, &keyboard_picker, &keyboard_native, cx);
                            window.refresh();
                            cx.stop_propagation();
                        }
                    })
                    .on_a11y_action(gpui::AccessibleAction::Click, move |_, window, cx| {
                        activate_control(&accessible, &accessible_picker, &accessible_native, cx);
                        window.refresh();
                    });
            }
        }
    }
    element = animate(element, style, &key, settings.reduced_motion, window, cx);
    // Motion uses relative offsets for ordinary views. Artboards use the
    // canvas camera's absolute position as their final layout coordinates.
    if preview.is_some_and(|preview| preview.floating_panel) {
        element = element
            .occlude()
            .debug_selector(|| "studio-floating-panel".into())
            .on_scroll(|_, _, cx| cx.stop_propagation());
    }
    if let Some([left, top]) = preview.and_then(|preview| preview.position) {
        element = element.absolute().left(px(left)).top(px(top));
    }
    element.into_any_element()
}

fn activate_control(
    action: &omen_core::Action,
    picker: &Option<omen_core::FilePicker>,
    native: &Option<omen_core::DesktopAction>,
    cx: &mut App,
) {
    match native {
        Some(omen_core::DesktopAction::CopyText(text)) => {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()))
        }
        Some(omen_core::DesktopAction::RevealPath(path)) => cx.reveal_path(path),
        None => {}
    }
    if let Some(picker) = picker {
        let picker = picker.clone();
        let response = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: picker.files,
            directories: picker.directories,
            multiple: picker.multiple,
            prompt: picker.prompt.clone().map(Into::into),
        });
        cx.foreground_executor()
            .spawn(async move {
                let result = match response.await {
                    Ok(result) => result.map_err(|error| error.to_string()),
                    Err(error) => Err(format!("File picker closed unexpectedly: {error}")),
                };
                picker.complete(result);
            })
            .detach();
    } else {
        action.invoke();
    }
}

fn render_children(
    mut element: Div,
    children: &[ViewNode],
    next_id: &mut usize,
    placement: Option<&design_canvas::Placement<'_>>,
    settings: RenderSettings,
    overlay: bool,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    let mut pinned = Vec::new();
    for child in children {
        if let ViewKind::Group(children) = &child.kind
            && child.style.is_transparent_group()
        {
            element = render_children(
                element, children, next_id, placement, settings, overlay, window, cx,
            );
            continue;
        }
        let mut child_settings = settings;
        if child.style.pin.is_none()
            && (settings.grid_flow || child.style.fill)
            && let Some(width) = settings.flow_width
        {
            child_settings.container_width = width;
        }
        if overlay {
            child_settings.parent_stretches = true;
        }
        let pin = child.style.pin;
        let pin_parent = if pin.is_some() { placement } else { None };
        let child = render_node_with_pin(child, next_id, child_settings, pin_parent, window, cx);
        if let Some(pin) = pin {
            let (trailing, bottom) = pin.corner.edges();
            let right = trailing ^ (settings.direction == omen_core::LayoutDirection::RightToLeft);
            let x = px(f32::from(pin.x) * settings.scale);
            let y = px(f32::from(pin.y) * settings.scale);
            let layer = div().absolute().child(child);
            let layer = if right { layer.right(x) } else { layer.left(x) };
            let layer = if bottom {
                layer.bottom(y)
            } else {
                layer.top(y)
            };
            pinned.push(layer);
        } else if overlay {
            element = element.child(
                div()
                    .flex()
                    .flex_col()
                    .col_start(1)
                    .row_start(1)
                    .child(child),
            );
        } else {
            element = element.child(child);
        }
    }

    element.when(!pinned.is_empty(), |element| {
        element.relative().children(pinned)
    })
}

fn alignment_axes(alignment: omen_core::Alignment) -> (u8, u8) {
    use omen_core::Alignment::*;
    match alignment {
        Leading | TopLeading => (0, if alignment == Leading { 1 } else { 0 }),
        Trailing | TopTrailing => (2, if alignment == Trailing { 1 } else { 0 }),
        Center => (1, 1),
        Top => (1, 0),
        Bottom => (1, 2),
        BottomLeading => (0, 2),
        BottomTrailing => (2, 2),
    }
}

fn apply_style(mut element: Div, kind: &ViewKind, style: &Style, settings: RenderSettings) -> Div {
    let px = |value| gpui::px(value * settings.scale);
    let row_control = matches!(
        kind,
        ViewKind::Button { .. } | ViewKind::Checkbox { .. } | ViewKind::Toggle { .. }
    );
    let rtl = settings.direction == omen_core::LayoutDirection::RightToLeft;
    if row_control && rtl {
        element = element.flex_row_reverse();
    }
    let vertical = !row_control && style.mode != Some(LayoutMode::Horizontal);
    if style.fill {
        element = element.flex_1().min_w_0().min_h_0();
    }
    if style.truncate {
        element = element.min_w_0().truncate();
    }
    if style.scrollable || style.horizontal_scrollable {
        element = element.min_w_0().min_h_0().overflow_hidden();
    }
    if let Some(width) = style.min_width {
        element = element.min_w(px(width));
    }
    element = element.text_align(if rtl {
        gpui::TextAlign::Right
    } else {
        gpui::TextAlign::Left
    });
    if style.theme.is_some() {
        element = element
            .bg(rgb(settings.tokens.canvas))
            .text_color(rgb(settings.tokens.primary))
            .text_size(px(settings.tokens.body_size));
    }
    if let Some(token) = style.padding_token {
        element = element.p(px(settings.tokens.space(token).max(0.)));
    }
    if let Some(token) = style.spacing_token {
        element = element.gap(px(settings.tokens.space(token).max(0.)));
    }
    if let Some(font) = style.font {
        element = element.text_size(px(match font {
            Font::Caption => 12.,
            Font::Footnote => 13.,
            Font::Body => 16.,
            Font::Headline => 18.,
            Font::Title2 => 20.,
            Font::Title => 24.,
            Font::LargeTitle => 36.,
        }));
    }
    if let Some(size) = style.font_size {
        element = element.text_size(px(size.max(1.)));
    }
    if let Some(height) = style.line_height {
        element = element.line_height(px(height.max(1.)));
    }
    if let Some(family) = &style.font_family {
        element = element.font_family(SharedString::from(family.clone()));
    }
    if let Some(weight) = style.weight {
        element = element.font_weight(match weight {
            Weight::Thin => FontWeight::THIN,
            Weight::Light => FontWeight::LIGHT,
            Weight::Regular => FontWeight::NORMAL,
            Weight::Medium => FontWeight::MEDIUM,
            Weight::Semibold => FontWeight::SEMIBOLD,
            Weight::Bold => FontWeight::BOLD,
            Weight::Heavy => FontWeight::BLACK,
        });
    }
    if let Some(color) = style.color {
        element = element.text_color(gpui::rgba(settings.tokens.rgba(color)));
    }
    if let Some(color) = style.hover_background {
        let color = gpui::rgba(settings.tokens.rgba(color));
        element = element.hover(move |style| style.bg(color));
    }
    if let Some(background) = style.background {
        element = match background {
            omen_core::Background::Color(color) => {
                element.bg(gpui::rgba(settings.tokens.rgba(color)))
            }
            omen_core::Background::Gradient(gradient) => element.bg(gpui::linear_gradient(
                gradient.angle,
                gpui::linear_color_stop(gpui::rgba(settings.tokens.rgba(gradient.from)), 0.),
                gpui::linear_color_stop(gpui::rgba(settings.tokens.rgba(gradient.to)), 1.),
            )),
        };
    }
    if let Some(frame) = style.frame {
        element = if frame.width == f32::INFINITY {
            element.w_full()
        } else {
            element.w(px(frame.width.max(0.)))
        };
        element = if frame.height == f32::INFINITY {
            element.h_full()
        } else {
            element.h(px(frame.height.max(0.)))
        };
    }
    if let Some(alignment) = style.alignment {
        let (mut x, y) = alignment_axes(alignment);
        if rtl {
            x = 2 - x;
        }
        let (items, justify) = if vertical { (x, y) } else { (y, x) };
        element = match items {
            0 => element.items_start(),
            2 => element.items_end(),
            _ => element.items_center(),
        };
        element = match justify {
            0 => element.justify_start(),
            2 => element.justify_end(),
            _ => element.justify_center(),
        };
    }
    if let Some(padding) = style.padding {
        element = element.p(px(padding.max(0.)));
    }
    if let Some(spacing) = style.spacing {
        element = element.gap(px(spacing.max(0.)));
    }
    if let Some(radius) = style.corner_radius {
        element = element.rounded(px(radius.max(0.)));
    }
    if let Some(edges) = style.safe_area {
        use omen_core::SafeArea::*;
        let insets = settings.insets;
        let base = style
            .padding
            .or_else(|| {
                style
                    .padding_token
                    .map(|token| settings.tokens.space(token))
            })
            .unwrap_or(0.)
            .max(0.);
        if matches!(edges, All | Vertical) {
            element = element
                .pt(px(base + insets.top.max(0.)))
                .pb(px(base + insets.bottom.max(0.)));
        }
        if matches!(edges, All | Horizontal) {
            element = element
                .pl(px(base + insets.left.max(0.)))
                .pr(px(base + insets.right.max(0.)));
        }
    }
    element
}

fn accessibility_role(role: omen_core::AccessibilityRole) -> gpui::Role {
    use omen_core::AccessibilityRole as O;
    match role {
        O::Text => gpui::Role::Label,
        O::Heading => gpui::Role::Heading,
        O::Image => gpui::Role::Image,
        O::Button => gpui::Role::Button,
        O::Checkbox => gpui::Role::CheckBox,
        O::Switch => gpui::Role::Switch,
        O::TextField => gpui::Role::TextInput,
        O::Progress => gpui::Role::ProgressIndicator,
        O::List => gpui::Role::List,
        O::ListItem => gpui::Role::ListItem,
        O::Group => gpui::Role::Group,
        O::Navigation => gpui::Role::Navigation,
        O::Tab => gpui::Role::Tab,
        O::TabList => gpui::Role::TabList,
        O::TabPanel => gpui::Role::TabPanel,
        O::Dialog => gpui::Role::Dialog,
    }
}

/// Wait without blocking the native UI thread.
pub async fn sleep(duration: std::time::Duration) {
    async_io::Timer::after(duration).await;
}

/// Open a desktop window displaying a constructed Omen view.
///
/// Call from the main thread. Named views retain state and rebuild on observed writes.
/// # Errors
/// Returns an error if GPUI cannot open the native window.
#[cfg(not(target_os = "android"))]
pub fn run_view(view: impl View) -> Result<(), WindowError> {
    run_with_application(view, gpui_platform::application())
}

#[cfg(target_os = "android")]
pub use gpui_android::AndroidApp;

/// Run a view on Android's NativeActivity application thread.
#[cfg(target_os = "android")]
pub fn run_android_view(app: AndroidApp, view: impl View) -> Result<(), WindowError> {
    let platform = gpui_android::AndroidPlatform::new(app)
        .map_err(|error| WindowError::OpenWindow(error.to_string()))?;
    run_with_application(view, gpui::Application::with_platform(platform))
}

/// Android requires the NativeActivity entry point rather than a process main.
#[cfg(target_os = "android")]
pub fn run_view(_view: impl View) -> Result<(), WindowError> {
    Err(WindowError::OpenWindow(
        "Use the generated android_main entry point".into(),
    ))
}

fn run_with_application(
    view: impl View,
    application: gpui::Application,
) -> Result<(), WindowError> {
    let root = view.into_node();
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    let desktop = root.style.desktop_window.clone().unwrap_or_default();
    let mut initial = OmenWindow::new(root);
    initial.runtime.prepare()?;
    let error = Rc::new(RefCell::new(None));
    let error_in_app = Rc::clone(&error);
    native_trace("entering GPUI platform loop");
    application.run(move |cx: &mut App| {
        native_trace("GPUI platform launch callback");
        #[cfg(target_os = "macos")]
        macos_app_icon::apply_bundle_icon();
        cx.set_app_identity(
            &app_setting("OMEN_APP_ID", option_env!("OMEN_APP_ID"), "dev.omen.app"),
            &app_setting("OMEN_APP_NAME", option_env!("OMEN_APP_NAME"), "Omen"),
        );
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        let bounds = Bounds::centered(
            None,
            size(
                px(window_dimension(
                    "OMEN_WINDOW_WIDTH",
                    option_env!("OMEN_WINDOW_WIDTH"),
                    desktop.width,
                )),
                px(window_dimension(
                    "OMEN_WINDOW_HEIGHT",
                    option_env!("OMEN_WINDOW_HEIGHT"),
                    desktop.height,
                )),
            ),
            cx,
        );
        #[cfg(any(target_os = "ios", target_os = "android"))]
        let options = WindowOptions {
            titlebar: None,
            ..Default::default()
        };
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(
                px(desktop.min_width.max(64.)),
                px(desktop.min_height.max(64.)),
            )),
            titlebar: Some(titlebar::options(
                &desktop,
                app_setting(
                    "OMEN_WINDOW_TITLE",
                    option_env!("OMEN_WINDOW_TITLE"),
                    &desktop.title,
                ),
            )),
            app_owns_titlebar_drag: desktop.toolbar.is_some(),
            window_decorations: titlebar::decorations(&desktop),
            ..Default::default()
        };
        let result = cx.open_window(options, |_, cx| cx.new(|_| initial));
        if let Err(cause) = result {
            *error_in_app.borrow_mut() = Some(WindowError::OpenWindow(cause.to_string()));
            cx.quit();
        } else {
            native_trace("native window opened");
            cx.activate(true);
        }
    });
    error.borrow_mut().take().map_or(Ok(()), Err)
}

/// Launch the original Phase 1 welcome example.
pub fn run() {
    let heading = ViewNode {
        kind: ViewKind::Text("Welcome to Omen".into()),
        style: Style {
            font: Some(Font::LargeTitle),
            ..Default::default()
        },
    };
    let subtitle = ViewNode::new(ViewKind::Text("A Rust-first UI framework".into()));
    let root = ViewNode {
        kind: ViewKind::Layout(vec![heading, subtitle]),
        style: Style {
            padding: Some(32.),
            spacing: Some(16.),
            background: Some(Color::Dark.into()),
            color: Some(Color::White),
            ..Default::default()
        },
    };
    if let Err(error) = run_view(root) {
        eprintln!("{error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{InputEvent, TestAppContext, point};
    use omen_ui::view;
    use std::cell::Cell;

    #[gpui::test]
    fn declaration_completion_adds_import_and_undoes_both_in_one_step(cx: &mut TestAppContext) {
        use omen_ui::{
            EditorCompletion, EditorCompletionEdit, EditorDocument, EditorState, Value, ViewExt,
        };
        let insert = "view! {\n    pub MyView { Text(\"Hi\") }\n}";
        let import = "use omen::prelude::*;\n\n";
        let name = insert.find("MyView").unwrap();
        let model = Value::new(EditorState {
            text: "view".into(),
            selection: 4..4,
            reveal: Some(4..4),
            completions: std::sync::Arc::new(vec![EditorCompletion {
                label: "view!".into(),
                insert: insert.into(),
                range: 0..4,
                placeholders: vec![name..name + 6, insert.len()..insert.len()],
                additional_edits: vec![EditorCompletionEdit {
                    range: 0..0,
                    insert: import.into(),
                }],
            }]),
            ..Default::default()
        });
        let root = view! { TextEditor("Rust").frame(600,400) }
            .document(EditorDocument::new(model.clone(), |_| {}));
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        vcx.simulate_keystrokes("enter");
        vcx.run_until_parked();
        assert_eq!(model.get().text, format!("{import}{insert}"));
        assert_eq!(
            model.get().selection,
            import.len() + name..import.len() + name + 6
        );
        vcx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-z"
        } else {
            "ctrl-z"
        });
        vcx.run_until_parked();
        assert_eq!(model.get().text, "view");
    }

    #[gpui::test]
    fn omen_snippet_selects_defaults_and_tabs_across_unicode_edits(cx: &mut TestAppContext) {
        use omen_ui::{EditorCompletion, EditorDocument, EditorState, Value, ViewExt};
        let insert = "TextField(\"Name\").value($text)";
        let label = insert.find("Name").unwrap();
        let binding = insert.find("text").unwrap();
        let model = Value::new(EditorState {
            text: "Tex".into(),
            selection: 3..3,
            reveal: Some(3..3),
            completions: std::sync::Arc::new(vec![EditorCompletion {
                label: "TextField".into(),
                insert: insert.into(),
                range: 0..3,
                additional_edits: Vec::new(),
                placeholders: vec![
                    label..label + 4,
                    binding..binding + 4,
                    insert.len()..insert.len(),
                ],
            }]),
            ..Default::default()
        });
        let root = view! { TextEditor("Rust").frame(600,400) }
            .document(EditorDocument::new(model.clone(), |_| {}));
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        vcx.simulate_keystrokes("enter");
        vcx.run_until_parked();
        assert_eq!(model.get().selection, label..label + 4);
        vcx.simulate_input("👋友");
        vcx.run_until_parked();
        vcx.simulate_keystrokes("tab");
        vcx.run_until_parked();
        assert_eq!(model.get().selection, binding + 3..binding + 7);
        vcx.simulate_keystrokes("shift-tab");
        vcx.run_until_parked();
        assert_eq!(model.get().selection, label..label + 7);
        vcx.simulate_keystrokes("tab");
        vcx.run_until_parked();
        vcx.simulate_input("name");
        vcx.run_until_parked();
        vcx.simulate_keystrokes("tab");
        vcx.run_until_parked();
        assert_eq!(model.get().selection, insert.len() + 3..insert.len() + 3);
        assert_eq!(model.get().text, "TextField(\"👋友\").value($name)");
        let undo = if cfg!(target_os = "macos") {
            "cmd-z"
        } else {
            "ctrl-z"
        };
        for _ in 0..12 {
            if model.get().text == "Tex" {
                break;
            }
            vcx.simulate_keystrokes(undo);
            vcx.run_until_parked();
        }
        assert_eq!(model.get().text, "Tex");
    }

    #[gpui::test]
    fn persistent_code_editor_accepts_completion_and_can_undo_it(cx: &mut TestAppContext) {
        use omen_ui::{EditorCompletion, EditorDocument, EditorState, Value, ViewExt};
        let model = Value::new(EditorState {
            text: "hel".into(),
            selection: 3..3,
            reveal: Some(3..3),
            completions: std::sync::Arc::new(vec![EditorCompletion {
                label: "hello_world".into(),
                insert: "hello_world".into(),
                placeholders: Vec::new(),
                additional_edits: Vec::new(),
                range: 0..3,
            }]),
            ..Default::default()
        });
        let root = view! {TextEditor("Rust").frame(600,400)}
            .document(EditorDocument::new(model.clone(), |_| {}));
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        vcx.simulate_keystrokes("enter");
        vcx.run_until_parked();
        assert_eq!(model.get().text, "hello_world");
        assert!(model.get().completions.is_empty());
        vcx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-z"
        } else {
            "ctrl-z"
        });
        vcx.run_until_parked();
        assert_eq!(model.get().text, "hel");
    }

    #[gpui::test]
    fn persistent_code_editor_auto_indents_and_undoes(cx: &mut TestAppContext) {
        use omen_ui::{EditorDocument, EditorState, Value, ViewExt};
        let model = Value::new(EditorState {
            text: "fn main() {".into(),
            selection: 11..11,
            reveal: Some(11..11),
            ..Default::default()
        });
        let document = EditorDocument::new(model.clone(), |_| {});
        let root = view! {TextEditor("Code").frame(600,400).fontFamily("monospace").lineHeight(20)}
            .document(document);
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        vcx.simulate_keystrokes("enter");
        vcx.run_until_parked();
        assert_eq!(model.get().text, "fn main() {\n    ");
        vcx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-z"
        } else {
            "ctrl-z"
        });
        vcx.run_until_parked();
        assert_eq!(model.get().text, "fn main() {");
        vcx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-shift-z"
        } else {
            "ctrl-shift-z"
        });
        vcx.run_until_parked();
        assert_eq!(model.get().text, "fn main() {\n    ");
        model.update(|state| {
            state.text = "// external change\n".into();
            state.selection = 0..0;
            state.reveal = Some(0..0);
            state.history_epoch += 1;
        });
        vcx.run_until_parked();
        vcx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-z"
        } else {
            "ctrl-z"
        });
        vcx.run_until_parked();
        assert_eq!(
            model.get().text,
            "// external change\n",
            "reload resets stale undo history"
        );
    }

    #[gpui::test]
    fn pointer_context_menus_copy_to_native_clipboard_and_dismiss_outside(cx: &mut TestAppContext) {
        use omen_ui::{BoolValue, ViewExt};
        let visible = BoolValue::new(false);
        let open = visible.clone();
        let close = visible.clone();
        let root = view! {Layout {Text("Explorer")}.frame(800,600)}
            .on_context_menu(move || open.set(true))
            .context_menu(visible.clone(), move || {
                let close = close.clone();
                view! {Button("Copy path") {close.set(false);}.frame(160,32)}.desktop_action(
                    omen_core::DesktopAction::CopyText("/project/src/main.rs".into()),
                )
            });
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.simulate_event(gpui::MouseDownEvent {
            position: point(px(200.), px(200.)),
            button: gpui::MouseButton::Right,
            ..Default::default()
        });
        vcx.run_until_parked();
        assert!(visible.get());
        vcx.simulate_click(point(px(212.), px(212.)), Default::default());
        vcx.run_until_parked();
        assert!(!visible.get());
        vcx.update(|_, cx| {
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("/project/src/main.rs".into())
            )
        });
        vcx.simulate_event(gpui::MouseDownEvent {
            position: point(px(100.), px(100.)),
            button: gpui::MouseButton::Right,
            ..Default::default()
        });
        vcx.run_until_parked();
        assert!(visible.get());
        vcx.simulate_click(point(px(600.), px(500.)), Default::default());
        vcx.run_until_parked();
        assert!(
            !visible.get(),
            "clicks on the underlying explorer dismiss the menu"
        );
    }

    #[gpui::test]
    fn nested_explorer_menu_stays_at_the_clicked_row_after_pointer_movement(
        cx: &mut TestAppContext,
    ) {
        use omen_ui::{BoolValue, ViewExt};
        let visible = BoolValue::new(false);
        let open = visible.clone();
        let close = visible.clone();
        let root = view! {Layout {Layout {
            Button("main.rs") {}.frame(260,28).onContextMenu(move ||open.set(true))
        }.padding(12)}.frame(800,600).alignment(.topLeading)}
        .context_menu(visible.clone(), move || {
            let close = close.clone();
            view! {Button("Open selected file") {close.set(false);}.frame(200,28)}
        });
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.simulate_event(gpui::MouseDownEvent {
            position: point(px(130.), px(24.)),
            button: gpui::MouseButton::Right,
            ..Default::default()
        });
        vcx.simulate_event(gpui::MouseMoveEvent {
            position: point(px(700.), px(500.)),
            ..Default::default()
        });
        vcx.run_until_parked();
        assert!(visible.get());
        vcx.simulate_click(point(px(144.), px(36.)), Default::default());
        vcx.run_until_parked();
        assert!(
            !visible.get(),
            "the menu stays beside the clicked explorer row"
        );
    }

    #[gpui::test]
    fn dropdown_popovers_dismiss_outside_on_escape_and_when_the_trigger_toggles(
        cx: &mut TestAppContext,
    ) {
        use omen_ui::{BoolValue, ViewExt};
        let visible = BoolValue::new(false);
        let toggle = visible.clone();
        let root =
            view! { Button("File") { toggle.update(|value| *value = !*value); }.frame(80, 36) }
                .popover(
                    visible.clone(),
                    || view! { Button("Menu item") {}.frame(160, 32) },
                );
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.simulate_click(point(px(20.), px(20.)), Default::default());
        vcx.run_until_parked();
        assert!(visible.get());
        vcx.simulate_click(point(px(400.), px(300.)), Default::default());
        vcx.run_until_parked();
        assert!(!visible.get(), "clicking outside dismisses the dropdown");
        vcx.simulate_click(point(px(20.), px(20.)), Default::default());
        vcx.run_until_parked();
        assert!(visible.get());
        vcx.simulate_keystrokes("escape");
        vcx.run_until_parked();
        assert!(!visible.get());
        vcx.simulate_click(point(px(20.), px(20.)), Default::default());
        vcx.run_until_parked();
        assert!(visible.get());
        vcx.simulate_click(point(px(20.), px(20.)), Default::default());
        vcx.run_until_parked();
        assert!(
            !visible.get(),
            "trigger click closes without immediately reopening"
        );
    }

    #[gpui::test]
    fn titlebar_drag_stays_armed_across_redraw_and_resets_on_release(cx: &mut TestAppContext) {
        type States = Rc<RefCell<Vec<Rc<Cell<bool>>>>>;
        struct DragRoot {
            states: States,
        }
        impl Render for DragRoot {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                self.states
                    .borrow_mut()
                    .push(titlebar::drag_state("drag-regression", window, cx));
                titlebar::render(
                    div().id("content"),
                    44.,
                    "drag-regression",
                    0x343439,
                    window,
                    cx,
                )
            }
        }
        let states: States = Rc::default();
        let (_, vcx) = cx.add_window_view(|_, _| DragRoot {
            states: states.clone(),
        });
        vcx.run_until_parked();
        let state = states.borrow().last().unwrap().clone();
        vcx.simulate_mouse_down(
            point(px(150.), px(20.)),
            gpui::MouseButton::Left,
            Default::default(),
        );
        vcx.run_until_parked();
        assert!(state.get(), "blank titlebar space arms window dragging");
        vcx.update(|window, _| window.refresh());
        vcx.run_until_parked();
        let redrawn = states.borrow().last().unwrap().clone();
        assert!(Rc::ptr_eq(&state, &redrawn));
        assert!(
            redrawn.get(),
            "redrawing must not cancel an in-progress titlebar gesture"
        );
        vcx.simulate_mouse_up(
            point(px(150.), px(20.)),
            gpui::MouseButton::Left,
            Default::default(),
        );
        vcx.run_until_parked();
        assert!(!state.get());
    }

    #[test]
    fn titlebar_surface_matches_the_content_through_reactive_wrappers() {
        let mut content = ViewNode::new(ViewKind::Layout(Vec::new()));
        content.style.theme = Some(omen_core::Theme::light());
        content.style.background = Some(omen_core::Color::Rgb(0x343439).into());
        let toolbar = ViewNode::new(ViewKind::Group(vec![ViewNode::new(ViewKind::Group(vec![
            content,
        ]))]));
        assert_eq!(
            titlebar::background(&toolbar, omen_core::Theme::dark().resolve(true), true),
            0x343439
        );
    }

    #[gpui::test]
    fn desktop_toolbar_controls_are_interactive_and_body_inputs_keep_focus(
        cx: &mut TestAppContext,
    ) {
        use omen_ui::{DesktopToolbar, DesktopWindow, TextValue, Value, ViewExt};
        let count = Value::new(0);
        let clicked = count.clone();
        let text = TextValue::new(String::new());
        let root = view! { Layout { TextField("Body input").value(text.clone()) }.frame(.infinity, .infinity) }
            .desktop_window(DesktopWindow { toolbar: Some(DesktopToolbar::new(move || {
                let clicked = clicked.clone();
                view! { Layout { Button("Toolbar action") { clicked.update(|count| *count += 1); }.frame(160, 36) }.frame(.infinity, .infinity) }
            })), ..Default::default() });
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        let x = if cfg!(target_os = "macos") { 100. } else { 20. };
        vcx.simulate_click(point(px(x), px(20.)), Default::default());
        vcx.run_until_parked();
        assert_eq!(count.get(), 1);
        vcx.simulate_keystrokes("tab");
        vcx.simulate_keystrokes("x");
        vcx.run_until_parked();
        assert_eq!(text.get(), "x");
        vcx.simulate_click(point(px(x), px(20.)), Default::default());
        vcx.run_until_parked();
        assert_eq!(count.get(), 2);
        assert_eq!(text.get(), "x");
    }

    #[test]
    fn native_toolbar_configuration_preserves_standard_titlebars_when_absent() {
        let standard = omen_core::DesktopWindow::default();
        let options = titlebar::options(&standard, "Standard".into());
        assert!(!options.appears_transparent);
        assert!(options.traffic_light_position.is_none());
        let mut toolbar =
            omen_core::DesktopToolbar::new(|| ViewNode::new(ViewKind::Text("Tools".into())));
        toolbar.height = f32::NAN;
        let custom = omen_core::DesktopWindow {
            toolbar: Some(toolbar),
            ..Default::default()
        };
        let options = titlebar::options(&custom, "Custom".into());
        assert!(options.appears_transparent);
        assert_eq!(options.traffic_light_position.unwrap().y, px(14.));
    }

    #[gpui::test]
    fn desktop_scenes_open_separately_retain_identity_and_follow_native_close(
        cx: &mut TestAppContext,
    ) {
        use omen_ui::{BoolValue, ViewExt};
        let visible = BoolValue::new(false);
        let opened = visible.clone();
        let root = view! { Button("Open editor") { opened.set(true); }.padding(16) }.window(
            "editor",
            visible.clone(),
            || view! { Text("Independent editor") },
        );
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        assert_eq!(vcx.windows().len(), 1);
        vcx.simulate_click(point(px(20.), px(20.)), Default::default());
        vcx.run_until_parked();
        assert_eq!(vcx.windows().len(), 2);
        let owner = vcx.update(|window, _| window.window_handle().window_id());
        let editor = vcx
            .windows()
            .into_iter()
            .find(|handle| handle.window_id() != owner)
            .unwrap();
        vcx.update(|window, _| window.refresh());
        vcx.run_until_parked();
        assert_eq!(
            vcx.windows().len(),
            2,
            "redraw keeps the same editor window"
        );
        vcx.update(|_, cx| {
            editor
                .update(cx, |_, window, _| window.remove_window())
                .unwrap()
        });
        vcx.run_until_parked();
        assert!(
            !visible.get(),
            "native close updates the presentation binding"
        );
        assert_eq!(vcx.windows().len(), 1);
        visible.set(true);
        vcx.run_until_parked();
        assert_eq!(vcx.windows().len(), 2, "closed scene can reopen");
        visible.set(false);
        vcx.run_until_parked();
        assert_eq!(
            vcx.windows().len(),
            1,
            "binding dismissal closes only the editor"
        );
    }

    #[gpui::test]
    fn native_folder_picker_handles_mouse_keyboard_and_cancellation(cx: &mut TestAppContext) {
        use omen_ui::{FilePicker, ViewExt};
        let replies = Rc::new(RefCell::new(Vec::new()));
        let output = replies.clone();
        let picker = FilePicker::folders(move |result| output.borrow_mut().push(result));
        let root = view! { Button("Choose folder") {}.padding(16) }
            .file_picker(picker)
            .into_node();
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.simulate_click(point(px(20.), px(20.)), Default::default());
        vcx.run_until_parked();
        assert!(vcx.did_prompt_for_paths());
        vcx.simulate_path_prompt_response(|options| {
            assert!(options.directories);
            assert!(!options.files);
            assert!(!options.multiple);
            Some(vec![std::path::PathBuf::from("/chosen/project")])
        });
        vcx.run_until_parked();
        assert_eq!(
            replies.borrow()[0].as_ref().unwrap().as_ref().unwrap()[0],
            std::path::PathBuf::from("/chosen/project")
        );
        vcx.simulate_keystrokes("enter");
        vcx.run_until_parked();
        assert!(vcx.did_prompt_for_paths());
        vcx.simulate_path_prompt_response(|_| None);
        vcx.run_until_parked();
        assert!(matches!(replies.borrow()[1], Ok(None)));
    }

    #[gpui::test]
    fn modal_traps_tab_dismisses_with_escape_and_restores_focus(cx: &mut TestAppContext) {
        use omen_ui::{BoolValue, ViewExt};
        let visible = BoolValue::new(false);
        let opened = visible.clone();
        let root = view! {Button("Open") {opened.set(true);}.padding(16)}
            .modal(
                visible.clone(),
                || view! {Button("Modal action") {}.padding(16)},
            )
            .into_node();
        let (_entity, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.simulate_click(point(px(20.), px(20.)), Default::default());
        vcx.run_until_parked();
        assert!(visible.get());
        let modal_focus = vcx.update(|window, cx| window.focused(cx).unwrap());
        vcx.simulate_keystrokes("tab");
        vcx.run_until_parked();
        vcx.update(|window, cx| assert!(modal_focus.contains_focused(window, cx)));
        vcx.simulate_keystrokes("escape");
        vcx.run_until_parked();
        assert!(!visible.get());
        vcx.simulate_keystrokes("enter");
        vcx.run_until_parked();
        assert!(visible.get(), "focus returned to the opener");
        vcx.simulate_click(point(px(4.), px(4.)), Default::default());
        vcx.run_until_parked();
        assert!(
            !visible.get(),
            "backdrop dismissed without activating underlying content"
        );
    }
    #[gpui::test]
    fn hex_background_retains_alpha_in_native_style(cx: &mut TestAppContext) {
        let window = cx.add_empty_window();
        window.update(|window, cx| {
            let node = view! { Text("Swatch").background("#11223380") }.into_node();
            let settings = RenderSettings::new(window, cx);
            let mut element = apply_style(div(), &node.kind, &node.style, settings);
            assert_eq!(
                element.style().background,
                Some(gpui::rgba(0x11223380).into())
            );
        });
    }

    #[gpui::test]
    fn leading_button_labels_stay_against_the_menu_gutter(cx: &mut TestAppContext) {
        use gpui::Bounds;
        use omen_ui::{Alignment, Button, LayoutDirection, ViewExt};
        use std::cell::Cell;

        struct MenuRow {
            direction: LayoutDirection,
            label: Rc<Cell<Bounds<gpui::Pixels>>>,
        }

        impl Render for MenuRow {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let mut settings = RenderSettings::new(window, cx);
                settings.direction = self.direction;
                let button = Button::new("Menu item", || {})
                    .frame(200, 32)
                    .padding(6)
                    .alignment(Alignment::Leading)
                    .into_node();
                let content = render_leaf(
                    &button,
                    &button.style,
                    &SharedString::from("menu-row"),
                    LeafSettings {
                        own: settings,
                        child: settings,
                    },
                    &mut 0,
                    window,
                    cx,
                );
                let label = self.label.clone();
                apply_style(content.element, &button.kind, &button.style, settings)
                    .on_children_prepainted(move |bounds, _, _| label.set(bounds[0]))
            }
        }

        for direction in [LayoutDirection::LeftToRight, LayoutDirection::RightToLeft] {
            let label = Rc::new(Cell::new(Bounds::default()));
            let (_, vcx) = cx.add_window_view(|_, _| MenuRow {
                direction,
                label: label.clone(),
            });
            vcx.run_until_parked();
            let label = label.get();
            if direction == LayoutDirection::LeftToRight {
                assert_eq!(label.left(), px(6.));
            } else {
                assert_eq!(label.right(), px(194.));
            }
            assert!((f32::from(label.center().y) - 16.).abs() <= 1.);
        }
    }

    #[gpui::test]
    fn rtl_and_responsive_layouts_use_logical_row_order(cx: &mut TestAppContext) {
        use omen_ui::{Breakpoint, LayoutDirection, ResponsiveStyle};
        let clicks = Rc::new(RefCell::new(Vec::new()));
        let first = clicks.clone();
        let second = clicks.clone();
        let root=view! {Layout {
            Button("First"){first.borrow_mut().push(1);}.frame(100,50)
            Button("Second"){second.borrow_mut().push(2);}.frame(100,50)
        }.responsive(Breakpoint::Medium,ResponsiveStyle{mode:Some(LayoutMode::Horizontal),..Default::default()})
        .direction(LayoutDirection::RightToLeft).frame(200,50)}.into_node();
        let (_entity, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.simulate_click(point(px(20.), px(20.)), Default::default());
        vcx.run_until_parked();
        assert_eq!(*clicks.borrow(), [2]);
        vcx.simulate_click(point(px(120.), px(20.)), Default::default());
        vcx.run_until_parked();
        assert_eq!(*clicks.borrow(), [2, 1]);
    }
    #[gpui::test]
    fn adaptive_layout_and_grid_position_children_in_local_space(cx: &mut TestAppContext) {
        let clicks = Rc::new(RefCell::new(Vec::new()));
        let second = clicks.clone();
        let third = clicks.clone();
        let root = view! {
            Layout {
                Layout {
                    Text("First").frame(60,40)
                    Button("Second") { second.borrow_mut().push(2); }.frame(60,40)
                }.direction(.adaptive(compact: .vertical, expanded: .horizontal))
                AdaptiveGrid {
                    Text("A").frame(60,40)
                    Text("B").frame(60,40)
                    Button("C") { third.borrow_mut().push(3); }.frame(60,40)
                }.min_item_width(100)
            }.frame(240,300)
        }
        .into_node();
        let (_entity, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        vcx.simulate_click(point(px(20.), px(60.)), Default::default());
        vcx.simulate_click(point(px(20.), px(140.)), Default::default());
        assert_eq!(*clicks.borrow(), [2, 3]);
    }
    view! {
        NativeCounter() {
            @State let count = 0;
            Layout {
                Button("Increment") { count += 1; }.padding(16)
                Text("Native count: {count}")
            }
        }
    }
    view! {
        NativeTask(observed: Rc<Cell<i32>>) {
            @State let count = 0;
            observed.set(count);
            Text("Async count: {count}").task async { count = 7; }
        }
    }
    #[gpui::test]
    fn async_completion_wakes_the_native_window_without_an_input_event(cx: &mut TestAppContext) {
        let observed = Rc::new(Cell::new(0));
        let root = view! { NativeTask(observed: observed.clone()) }.into_node();
        let (_entity, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        assert_eq!(observed.get(), 7);
    }
    #[gpui::test]
    fn native_actions_invalidate_and_rebuild_stateful_views(cx: &mut TestAppContext) {
        let root = view! { NativeCounter() }.into_node();
        let (entity, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.simulate_click(point(px(20.), px(20.)), Default::default());
        vcx.run_until_parked();
        vcx.update(|_, cx| {
            entity.update(cx, |window, _| {
                fn contains(node: &ViewNode, expected: &str) -> bool {
                    match &node.kind {
                        ViewKind::Text(text) => text == expected,
                        ViewKind::Group(children) | ViewKind::Layout(children) => {
                            children.iter().any(|node| contains(node, expected))
                        }
                        _ => false,
                    }
                }
                assert!(contains(
                    &window.runtime.render().unwrap(),
                    "Native count: 1"
                ));
            });
        });
    }
    #[gpui::test]
    fn rendered_button_dispatches_click_and_survives_redraw(cx: &mut TestAppContext) {
        let clicks = Rc::new(Cell::new(0));
        let captured = Rc::clone(&clicks);
        let root = view! {
            Layout {
                if true {
                    Button("Click") { captured.set(captured.get() + 1); }
                        .padding(16).background(.blue).cornerRadius(8)
                }
            }.padding(8)
        }
        .into_node();
        let (_entity, cx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        assert_eq!(clicks.get(), 0);
        cx.simulate_click(point(px(20.), px(20.)), Default::default());
        cx.run_until_parked();
        assert_eq!(clicks.get(), 1);
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        cx.simulate_click(point(px(20.), px(20.)), Default::default());
        cx.run_until_parked();
        assert_eq!(clicks.get(), 2);
        cx.simulate_click(point(px(700.), px(400.)), Default::default());
        assert_eq!(clicks.get(), 2);
    }
    #[gpui::test]
    fn multiline_editor_inserts_lines_moves_vertically_and_preserves_clipboard_newlines(
        cx: &mut TestAppContext,
    ) {
        let value = omen_ui::TextValue::new(String::new());
        let root = view! { TextEditor("Code").value(value.clone()).frame(500, 300) }.into_node();
        let (_entity, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.simulate_click(point(px(20.), px(20.)), Default::default());
        vcx.simulate_input("first 👋");
        vcx.simulate_keystrokes("enter");
        vcx.simulate_input("second");
        assert_eq!(value.get(), "first 👋\nsecond");
        vcx.simulate_keystrokes("home up end");
        vcx.simulate_input("!");
        assert_eq!(value.get(), "first 👋!\nsecond");
        vcx.simulate_keystrokes("cmd-a cmd-c");
        vcx.update(|_, cx| {
            assert_eq!(
                cx.read_from_clipboard()
                    .and_then(|item| item.text())
                    .as_deref(),
                Some("first 👋!\nsecond")
            )
        });
        vcx.simulate_input("");
        vcx.simulate_keystrokes("cmd-v");
        assert_eq!(value.get(), "first 👋!\nsecond");
        vcx.simulate_keystrokes("tab");
        assert_eq!(value.get(), "first 👋!\nsecond    ");
        vcx.simulate_keystrokes("cmd-z");
        assert_eq!(value.get(), "first 👋!\nsecond");
        vcx.simulate_keystrokes("cmd-shift-z");
        assert_eq!(value.get(), "first 👋!\nsecond    ");
    }

    #[test]
    fn root_teardown_callbacks_run_when_the_window_owner_is_dropped() {
        use omen_ui::ViewExt;
        let calls = Rc::new(Cell::new(0));
        let callback = calls.clone();
        let view =
            omen_ui::Text::new("Root").on_window_close(move || callback.set(callback.get() + 1));
        let window = OmenWindow::new(view);
        assert_eq!(calls.get(), 0);
        drop(window);
        assert_eq!(calls.get(), 1);
    }

    #[gpui::test]
    fn shortcuts_and_secondary_click_actions_work_with_focused_inputs(cx: &mut TestAppContext) {
        let calls = Rc::new(Cell::new(0));
        let save = calls.clone();
        let menu = calls.clone();
        let root = view! {
            Layout { TextField("Name") }.frame(300, 100)
            .shortcut("cmd-s", move || save.set(save.get()+10))
            .onContextMenu(move || menu.set(menu.get()+1))
        }
        .into_node();
        let (_entity, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.simulate_click(point(px(20.), px(20.)), Default::default());
        vcx.simulate_keystrokes("cmd-s");
        assert_eq!(calls.get(), 10);
        vcx.simulate_mouse_down(
            point(px(20.), px(20.)),
            gpui::MouseButton::Right,
            Default::default(),
        );
        vcx.simulate_mouse_up(
            point(px(20.), px(20.)),
            gpui::MouseButton::Right,
            Default::default(),
        );
        assert_eq!(calls.get(), 11);
    }

    #[gpui::test]
    fn virtualized_lists_only_instantiate_visible_native_controls(cx: &mut TestAppContext) {
        use omen_ui::{Binding, List, TextField, ViewExt};
        let reads = Rc::new(Cell::new(0));
        let rows = (0..10_000)
            .map(|index| {
                let reads = reads.clone();
                TextField::new(format!("Row {index}"))
                    .value(Binding::new(
                        move || {
                            reads.set(reads.get() + 1);
                            String::new()
                        },
                        |_| {},
                    ))
                    .frame(200, 28)
                    .key(index)
                    .into_node()
            })
            .collect::<Vec<_>>();
        let root = List::new()
            .children(rows)
            .virtualized(28.)
            .frame(220, 200)
            .into_node();
        reads.set(0); // Binding dependency registration above is independent of native rendering.
        let (_entity, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        assert!(reads.get() > 0);
        assert!(
            reads.get() < 100,
            "offscreen controls were instantiated: {} reads",
            reads.get()
        );
    }

    #[gpui::test]
    fn horizontal_scroll_regions_keep_overflowing_tabs_selectable(cx: &mut TestAppContext) {
        let selected = Rc::new(Cell::new(0));
        let first = selected.clone();
        let second = selected.clone();
        let root = view! {
            Layout {
                Button("First") { first.set(1); }.frame(180, 40).minWidth(180)
                Button("Second") { second.set(2); }.frame(180, 40).minWidth(180)
            }.mode(.horizontal).frame(200, 40).scrollableHorizontal()
        }
        .into_node();
        let (_entity, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.simulate_click(point(px(20.), px(20.)), Default::default());
        assert_eq!(selected.get(), 1);
        vcx.simulate_event(gpui::ScrollEvent {
            position: point(px(100.), px(20.)),
            delta: gpui::ScrollDelta::Pixels(point(px(-180.), px(0.))),
            touch_phase: gpui::TouchPhase::Moved,
            ..Default::default()
        });
        vcx.run_until_parked();
        vcx.simulate_click(point(px(20.), px(20.)), Default::default());
        assert_eq!(
            selected.get(),
            2,
            "offscreen tabs become selectable after horizontal scrolling"
        );
    }

    #[gpui::test]
    fn renders_deeply_nested_native_controls_on_the_default_stack(cx: &mut TestAppContext) {
        use omen_ui::ViewExt;
        let clicked = omen_ui::Value::new(false);
        let action = clicked.clone();
        let leaf = omen_ui::Button::new("Deep control", move || action.set(true))
            .frame(160, 50)
            .into_node();
        let root = (0..40).fold(leaf, |child, _| {
            omen_ui::Layout::new().child(child).into_node()
        });
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.simulate_click(point(px(20.), px(20.)), Default::default());
        assert!(clicked.get());
    }

    #[gpui::test]
    fn native_preview_reports_viewport_size_with_grid_hidden(cx: &mut TestAppContext) {
        use omen_ui::ViewExt;
        let measured = omen_ui::Value::new([0.; 2]);
        let mut root = omen_ui::Layout::new().frame(320, 100).into_node();
        let preview = root.style.preview_mut();
        preview.viewport_size = Some(measured.clone().into());
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        assert_eq!(measured.get(), [320., 100.]);
    }

    #[gpui::test]
    fn canvas_routes_pointer_zoom_and_separates_frame_drag_from_pan(cx: &mut TestAppContext) {
        use omen_core::{PreviewFrameDrag, PreviewGesture};
        use omen_ui::ViewExt;

        let gesture = omen_ui::Value::new(PreviewGesture::Pan([0., 0.]));
        let frame_position = omen_ui::Value::new([4, 4]);
        let mut header = omen_ui::Layout::new().frame(90, 24).into_node();
        header.style.preview_mut().frame_drag = Some(PreviewFrameDrag {
            select: omen_core::Action::new(|| {}),
            position: frame_position.clone().into(),
            scale: 2.,
        });
        let mut canvas = omen_ui::Layout::new()
            .child(header)
            .frame(320, 240)
            .into_node();
        canvas.style.preview_mut().navigation = Some(gesture.clone().into());
        let root = omen_ui::Layout::new()
            .child(canvas)
            .padding(20.)
            .into_node();
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.simulate_event(gpui::ScrollEvent {
            position: point(px(130.), px(100.)),
            delta: gpui::ScrollDelta::Pixels(point(px(0.), px(45.))),
            modifiers: gpui::Modifiers {
                control: true,
                ..Default::default()
            },
            ..Default::default()
        });
        assert!(
            matches!(gesture.get(), PreviewGesture::Zoom { factor, anchor } if factor > 1. && anchor == [110., 80.])
        );

        let mut drag = |points: [(f32, f32); 3]| {
            let [start, first, last] = points.map(|(x, y)| point(px(x), px(y)));
            vcx.simulate_mouse_down(start, gpui::MouseButton::Left, Default::default());
            vcx.simulate_mouse_move(first, gpui::MouseButton::Left, Default::default());
            vcx.simulate_mouse_move(last, gpui::MouseButton::Left, Default::default());
            vcx.simulate_mouse_up(last, gpui::MouseButton::Left, Default::default());
        };
        drag([(150., 150.), (130., 140.), (105., 110.)]);
        assert!(matches!(gesture.get(), PreviewGesture::Pan([x, y]) if x < 0. && y < 0.));
        let last_pan = gesture.get();
        drag([(50., 30.), (42., 30.), (28., 24.)]);
        assert_eq!(
            frame_position.get(),
            [-7, 1],
            "drag keeps the original grab point at 200% zoom"
        );
        assert_eq!(gesture.get(), last_pan);
    }

    #[gpui::test]
    fn native_preview_selects_components_and_routes_sibling_drops(cx: &mut TestAppContext) {
        use omen_core::{Action, Binding, FlowPlacement, PreviewPlacement};
        use omen_ui::ViewExt;
        let selected = omen_ui::Value::new(None);
        let moved = omen_ui::Value::new(None);
        let action_ran = omen_ui::Value::new(false);
        let children = (0..2)
            .map(|index| {
                let selected = selected.clone();
                let original = action_ran.clone();
                let drop_result = moved.clone();
                let mut child =
                    omen_ui::Button::new(format!("Component {index}"), move || original.set(true))
                        .frame(160, 50)
                        .into_node();
                child.style.on_tap = Some(Action::new(move || selected.set(Some(index))));
                let preview = child.style.preview_mut();
                preview.outline = Some(Color::Accent);
                preview.placement = Some(PreviewPlacement {
                    scope: "view:layout".into(),
                    source: index..index + 1,
                    label: format!("Component {index}"),
                    draggable: true,
                    axis: Some(LayoutMode::Vertical),
                    container: false,
                    drop_from: Binding::new(
                        move || (index, FlowPlacement::Into),
                        move |(source, placement)| {
                            drop_result.set(Some((source, index, placement)))
                        },
                    ),
                });
                child
            })
            .collect::<Vec<_>>();
        let root = omen_ui::Layout::new().children(children).into_node();
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        let first = point(px(20.), px(20.));
        let second = point(px(20.), px(70.));
        vcx.simulate_click(first, Default::default());
        assert_eq!(selected.get(), Some(0));
        assert!(!action_ran.get());

        vcx.simulate_mouse_down(first, gpui::MouseButton::Left, Default::default());
        vcx.simulate_mouse_move(
            point(px(40.), px(30.)),
            gpui::MouseButton::Left,
            Default::default(),
        );
        vcx.simulate_mouse_move(second, gpui::MouseButton::Left, Default::default());
        vcx.simulate_mouse_up(second, gpui::MouseButton::Left, Default::default());
        assert_eq!(moved.get(), Some((0, 1, FlowPlacement::Before)));
        assert!(!action_ran.get());
    }

    #[gpui::test]
    fn resize_handles_capture_pointer_and_support_keyboard_adjustment(cx: &mut TestAppContext) {
        let width = omen_ui::Value::new(200_f32);
        let root = view! { Layout {}.frame(6, 300).resizeHandle(width.clone(), 180., 400., false) }
            .into_node();
        let (_entity, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.simulate_mouse_down(
            point(px(3.), px(30.)),
            gpui::MouseButton::Left,
            Default::default(),
        );
        vcx.simulate_mouse_move(
            point(px(20.), px(30.)),
            gpui::MouseButton::Left,
            Default::default(),
        );
        vcx.simulate_mouse_move(
            point(px(100.), px(30.)),
            gpui::MouseButton::Left,
            Default::default(),
        );
        vcx.simulate_mouse_up(
            point(px(100.), px(30.)),
            gpui::MouseButton::Left,
            Default::default(),
        );
        assert!(
            width.get() > 200.,
            "drag continues beyond the narrow handle"
        );
        vcx.simulate_click(point(px(3.), px(30.)), Default::default());
        let previous = width.get();
        vcx.simulate_keystrokes("right");
        assert_eq!(width.get(), (previous + 10.).min(400.));
    }

    #[gpui::test]
    fn three_panels_resize_and_keep_text_input_focus(cx: &mut TestAppContext) {
        let value = omen_ui::TextValue::new(String::new());
        let clicks = Rc::new(Cell::new(0));
        let captured = Rc::clone(&clicks);
        let root = view! {
            Layout {
                Text("Explorer").frame(100, .infinity)
                Layout {
                    TextField("Edit").value(value.clone())
                }.fill().frame(.infinity, .infinity).scrollable()
                Button("Preview") { captured.set(captured.get() + 1); }
                .frame(100, .infinity)
            }.mode(.horizontal).frame(.infinity, .infinity)
        }
        .into_node();
        use omen_ui::ViewExt;
        let root = ViewNode::new(ViewKind::Group(vec![root]))
            .popover(omen_ui::BoolValue::new(false), || {
                omen_ui::Text::new("Context")
            })
            .dialog(omen_ui::BoolValue::new(false), || {
                omen_ui::Text::new("Confirm")
            })
            .desktop_window(omen_core::DesktopWindow::default())
            .into_node();
        let (_entity, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        for width in [900., 700.] {
            vcx.simulate_resize(gpui::size(px(width), px(300.)));
            vcx.run_until_parked();
            vcx.simulate_click(point(px(width - 50.), px(280.)), Default::default());
            vcx.simulate_click(point(px(130.), px(20.)), Default::default());
            vcx.simulate_keystrokes("cmd-a");
            vcx.simulate_input("Hello 👩‍💻");
            assert_eq!(value.get(), "Hello 👩‍💻");
        }
        assert_eq!(clicks.get(), 2, "right panel follows the window edge");
    }

    #[gpui::test]
    fn layout_modes_arrange_and_hit_test_children(cx: &mut TestAppContext) {
        for (mode, position, expected) in [
            (LayoutMode::Horizontal, point(px(52.), px(20.)), 2),
            (LayoutMode::Vertical, point(px(20.), px(85.)), 2),
            (LayoutMode::Stack, point(px(20.), px(20.)), 2),
        ] {
            let clicks = Rc::new(RefCell::new(Vec::new()));
            let first = Rc::clone(&clicks);
            let second = Rc::clone(&clicks);
            let root = view! {
                Layout {
                    Button("") { first.borrow_mut().push(1); }.padding(16)
                    if true { Button("") { second.borrow_mut().push(2); }.padding(16) }
                }.mode(mode).padding(8)
            }
            .into_node();
            let (_entity, cx) = cx.add_window_view(|_, _| OmenWindow::new(root));
            cx.simulate_click(position, Default::default());
            cx.run_until_parked();
            assert_eq!(*clicks.borrow(), [expected], "mode: {mode:?}");
        }
    }
    #[gpui::test]
    fn text_fields_edit_unicode_and_secure_fields_do_not_export_secrets(cx: &mut TestAppContext) {
        use omen_ui::TextValue;
        let value = TextValue::new(String::new());
        let root = view! { TextField("Name").value(value.clone()) }.into_node();
        let (_entity, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.simulate_click(point(px(20.), px(20.)), Default::default());
        vcx.simulate_input("A👩‍💻");
        assert_eq!(value.get(), "A👩‍💻");
        vcx.simulate_keystrokes("backspace");
        assert_eq!(value.get(), "A");
        vcx.simulate_keystrokes("cmd-a");
        vcx.simulate_input("Ada");
        assert_eq!(value.get(), "Ada");
        vcx.simulate_keystrokes("cmd-a cmd-c");
        vcx.update(|_, cx| {
            assert_eq!(
                cx.read_from_clipboard()
                    .and_then(|item| item.text())
                    .as_deref(),
                Some("Ada")
            )
        });
        let secret = TextValue::new("secret-value".to_string());
        let root = view! { SecureField("Password").value(secret.clone()) }.into_node();
        let (_entity, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.simulate_click(point(px(20.), px(20.)), Default::default());
        vcx.update(|_, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("sentinel".to_string()))
        });
        vcx.simulate_keystrokes("cmd-a cmd-c cmd-x");
        assert_eq!(secret.get(), "secret-value");
        vcx.update(|_, cx| {
            assert_eq!(
                cx.read_from_clipboard()
                    .and_then(|item| item.text())
                    .as_deref(),
                Some("sentinel")
            )
        });
        vcx.simulate_input("new");
        assert_eq!(secret.get(), "new");
    }

    #[gpui::test]
    fn controls_support_tab_shift_tab_and_keyboard_activation(cx: &mut TestAppContext) {
        use omen_ui::{BoolValue, TextValue};
        let text = TextValue::new(String::new());
        let checked = BoolValue::new(false);
        let toggle = BoolValue::new(false);
        let calls = Rc::new(Cell::new(0));
        let callback = Rc::clone(&calls);
        let root = view! { Layout {
            TextField("Name").value(text.clone())
            Checkbox("Accept").checked(checked.clone())
            Toggle("Notify").checked(toggle.clone())
            Button("Submit") { callback.set(callback.get() + 1); }.padding(8)
        }.spacing(8) }
        .into_node();
        let (_entity, cx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        cx.simulate_keystrokes("tab");
        cx.simulate_input("Name");
        assert_eq!(text.get(), "Name");
        cx.simulate_keystrokes("tab space");
        cx.run_until_parked();
        assert!(
            checked.get(),
            "text={:?}, toggle={}",
            text.get(),
            toggle.get()
        );
        cx.simulate_keystrokes("tab space");
        cx.run_until_parked();
        assert!(toggle.get());
        cx.simulate_keystrokes("tab enter");
        assert_eq!(calls.get(), 1);
        cx.simulate_keystrokes("shift-tab space");
        cx.run_until_parked();
        assert!(!toggle.get());
        cx.simulate_keystrokes("shift-tab space");
        cx.run_until_parked();
        assert!(!checked.get());
    }
    #[gpui::test]
    fn buttons_size_to_content_unless_a_frame_sets_the_width(cx: &mut TestAppContext) {
        let clicks = Rc::new(Cell::new(0));
        let captured = clicks.clone();
        let root=view! {Layout {Button("Fit") {captured.set(captured.get()+1);}.padding(12)}.frame(300,80)}.into_node();
        let (_entity, window) = cx.add_window_view(|_, _| OmenWindow::new(root));
        window.simulate_click(point(px(250.), px(5.)), Default::default());
        assert_eq!(clicks.get(), 0);
        window.simulate_click(point(px(5.), px(5.)), Default::default());
        assert_eq!(clicks.get(), 1);

        let captured = clicks.clone();
        let root=view! {Layout {Button("Fixed") {captured.set(captured.get()+1);}.frame(160,40)}.frame(300,80)}.into_node();
        let (_entity, window) = cx.add_window_view(|_, _| OmenWindow::new(root));
        window.simulate_click(point(px(250.), px(5.)), Default::default());
        assert_eq!(clicks.get(), 1);
        window.simulate_click(point(px(100.), px(5.)), Default::default());
        assert_eq!(clicks.get(), 2);

        let captured = clicks.clone();
        let root=view! {Layout {Button("Full") {captured.set(captured.get()+1);}.frame(.infinity,40)}.frame(300,80)}.into_node();
        let (_entity, window) = cx.add_window_view(|_, _| OmenWindow::new(root));
        window.simulate_click(point(px(250.), px(5.)), Default::default());
        assert_eq!(clicks.get(), 3);
    }

    #[gpui::test]
    fn frame_and_alignment_position_children(cx: &mut TestAppContext) {
        let clicks = Rc::new(Cell::new(0));
        let captured = Rc::clone(&clicks);
        let root = view! { Layout {
            Button("") { captured.set(captured.get() + 1); }.padding(16)
        }.frame(120, 100).alignment(.bottomTrailing) }
        .into_node();
        let (_entity, cx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        cx.simulate_click(point(px(5.), px(5.)), Default::default());
        assert_eq!(clicks.get(), 0);
        cx.simulate_click(point(px(110.), px(90.)), Default::default());
        assert_eq!(clicks.get(), 1);
    }
    #[gpui::test]
    fn external_library_snippets_keep_tab_navigation_and_single_undo(cx: &mut TestAppContext) {
        use omen_core::{EditorDocument, EditorSnippet, EditorState, Value};
        use omen_ui::ViewExt;
        let model = Value::new(EditorState {
            text: "start ".into(),
            ..Default::default()
        });
        let root = view! { TextEditor("Rust").frame(600,400).autofocus(true) }
            .document(EditorDocument::new(model.clone(), |_| {}));
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        model.update(|state| {
            state.text = "start Text(\"Hello\")".into();
            state.selection = 12..17;
            state.reveal = Some(12..17);
            state.snippet_request = Some(EditorSnippet {
                stops: vec![12..17, 19..19],
                active: 0,
            });
        });
        vcx.run_until_parked();
        vcx.simulate_keystrokes("tab");
        vcx.run_until_parked();
        assert_eq!(model.get().selection, 19..19);
        vcx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-z"
        } else {
            "ctrl-z"
        });
        vcx.run_until_parked();
        assert_eq!(model.get().text, "start ");
    }
    #[gpui::test]
    fn editor_focus_and_window_activation_deliver_distinct_blur_events(cx: &mut TestAppContext) {
        use omen_core::{EditorDocument, EditorEvent, EditorState, Value};
        let editor_blurs = Rc::new(Cell::new(0));
        let count = editor_blurs.clone();
        let model = Value::new(EditorState {
            text: "let value = 1;".into(),
            reveal: Some(0..0),
            ..Default::default()
        });
        let document = EditorDocument::new(model, move |event| {
            if matches!(event, EditorEvent::Blur) {
                count.set(count.get() + 1);
            }
        });
        let window_blurs = Rc::new(Cell::new(0));
        let count = window_blurs.clone();
        let root = view! {
            Layout {
                TextEditor("Code").frame(240,70).document(document)
                Button("Leave editor") {}.frame(240,32)
            }.onWindowBlur(move || count.set(count.get() + 1))
        }
        .into_node();
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.update(|window, _| window.activate_window());
        vcx.run_until_parked();
        vcx.simulate_click(point(px(20.), px(84.)), Default::default());
        vcx.run_until_parked();
        assert_eq!(editor_blurs.get(), 1);
        assert_eq!(window_blurs.get(), 0);

        let (_, other) = cx.add_window_view(|_, _| OmenWindow::new(view! { Text("Other") }));
        other.update(|window, _| window.activate_window());
        other.run_until_parked();
        assert_eq!(window_blurs.get(), 1);
    }

    #[gpui::test]
    fn semantic_highlights_survive_enter_and_backspace(cx: &mut TestAppContext) {
        use omen_core::{EditorDocument, EditorState, SyntaxKind, SyntaxSpan, Value};
        let text = omen_core::TextSnapshot::from("let value = 42;");
        let id = text.identity();
        let model = Value::new(EditorState {
            text,
            highlight_identity: Some(id),
            selection_request: Some(0..0),
            highlights: std::sync::Arc::new(vec![SyntaxSpan {
                range: 4..9,
                kind: SyntaxKind::Parameter,
            }]),
            ..Default::default()
        });
        let document = EditorDocument::new(model.clone(), |_| {});
        let root = view! { TextEditor("Rust").autofocus(true).frame(500,300).document(document) }
            .into_node();
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        vcx.simulate_keystrokes("enter");
        vcx.run_until_parked();
        assert_eq!(model.get().text, "\nlet value = 42;");
        assert_eq!(model.get().highlights[0].range, 5..10);
        assert_eq!(
            model.get().highlight_identity,
            Some(model.get().text.identity())
        );
        vcx.simulate_keystrokes("backspace");
        vcx.run_until_parked();
        assert_eq!(model.get().highlights[0].range, 4..9);
        assert_eq!(model.get().highlights[0].kind, SyntaxKind::Parameter);
    }

    #[gpui::test]
    fn autofocus_input_accepts_typing_without_a_mouse_click(cx: &mut TestAppContext) {
        let text = omen_core::Value::new(String::new());
        let root = view! { TextField("Search").value(text.clone()).autofocus(true).frame(400,40) }
            .into_node();
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        vcx.simulate_keystrokes("b u t t o n");
        vcx.run_until_parked();
        assert_eq!(text.get(), "button");
    }
    #[gpui::test]
    fn library_popup_search_and_navigation_work_without_mouse_input(cx: &mut TestAppContext) {
        use omen_ui::ViewExt;
        let visible = omen_core::Value::new(true);
        let query = omen_core::Value::new(String::new());
        let selection = Rc::new(Cell::new(0));
        let inserted = Rc::new(Cell::new(0));
        let rows = selection.clone();
        let accepts = inserted.clone();
        let search = query.clone();
        let root =
            view! {Layout {Text("Editor")}.frame(700,500)}.dialog(visible.clone(), move || {
                let rows = rows.clone();
                let accepts = accepts.clone();
                view! {Layout {
                    TextField("Search views").value(search.clone()).autofocus(true)
                }.frame(400,200)
                .shortcut("down",move ||rows.set(rows.get()+1))
                .shortcut("enter",move ||accepts.set(accepts.get()+1))}
            });
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        vcx.simulate_keystrokes("b u t t o n down enter");
        vcx.run_until_parked();
        assert_eq!(query.get(), "button");
        assert_eq!(selection.get(), 1);
        assert_eq!(inserted.get(), 1);
        vcx.simulate_keystrokes("escape");
        vcx.run_until_parked();
        assert!(!visible.get());
        visible.set(true);
        vcx.run_until_parked();
        vcx.simulate_keystrokes("x");
        vcx.run_until_parked();
        assert_eq!(query.get(), "buttonx");
    }
    #[gpui::test]
    fn focus_requests_enter_inspector_scope_and_tab_stays_inside(cx: &mut TestAppContext) {
        let requested = omen_core::Value::new(false);
        let request = requested.clone();
        let value = omen_core::Value::new(String::new());
        let model = omen_core::Value::new(omen_core::EditorState::default());
        let back = model.clone();
        let toggle_back = model.clone();
        let root = view! { Layout {
            TextEditor("Editor").autofocus(true).document(omen_core::EditorDocument::new(model.clone(), |_| {})).frame(300,300)
            Layout {
                TextField("Inspector value").value(value.clone())
                Button("Inspector action") {}
            }.frame(240,300).focusRequest(requested.clone()).focusScope(true).focusArrows(true)
            .shortcut("cmd-i",move ||toggle_back.update(|state|state.reveal=Some(state.selection.clone())))
        }.mode(.horizontal).frame(600,300)
        .shortcut("cmd-i",move ||request.set(true))
        .shortcut("cmd-1",move ||back.update(|state|state.reveal=Some(state.selection.clone()))) }.into_node();
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        vcx.simulate_keystrokes("p");
        vcx.run_until_parked();
        assert_eq!(model.get().text, "p");
        let editor_focus = vcx.update(|window, cx| window.focused(cx).unwrap());
        vcx.simulate_keystrokes("cmd-i");
        vcx.run_until_parked();
        assert!(!requested.get());
        let inspector = vcx.update(|window, cx| window.focused(cx).unwrap());
        assert_ne!(editor_focus, inspector);
        vcx.simulate_keystrokes("down");
        vcx.run_until_parked();
        vcx.simulate_keystrokes("x");
        vcx.run_until_parked();
        assert_eq!(value.get(), "x");
        let field_focus = vcx.update(|window, cx| window.focused(cx).unwrap());
        vcx.simulate_keystrokes("down");
        vcx.run_until_parked();
        vcx.update(|window, cx| assert_ne!(field_focus, window.focused(cx).unwrap()));
        vcx.simulate_keystrokes("up");
        vcx.run_until_parked();
        vcx.update(|window, cx| assert_eq!(field_focus, window.focused(cx).unwrap()));
        for _ in 0..5 {
            vcx.simulate_keystrokes("tab");
            vcx.run_until_parked();
        }
        vcx.update(|window, cx| assert!(inspector.contains_focused(window, cx)));
        vcx.simulate_keystrokes("shift-tab");
        vcx.run_until_parked();
        vcx.update(|window, cx| assert!(inspector.contains_focused(window, cx)));
        vcx.simulate_keystrokes("cmd-i y");
        vcx.run_until_parked();
        assert_eq!(model.get().text, "py");
    }
    #[gpui::test]
    fn multi_cursor_occurrences_edit_undo_and_escape_as_one_operation(cx: &mut TestAppContext) {
        use omen_core::{EditorDocument, EditorState, Value};
        use omen_ui::ViewExt;
        let state = Value::new(EditorState {
            text: "café café café catastrophe".into(),
            selection: 0..0,
            ..Default::default()
        });
        let root = view! {TextEditor("Rust").autofocus(true).frame(600,300)}
            .document(EditorDocument::new(state.clone(), |_| {}));
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        let next = if cfg!(target_os = "macos") {
            "cmd-d"
        } else {
            "ctrl-d"
        };
        vcx.simulate_keystrokes(next);
        vcx.run_until_parked();
        assert_eq!(state.get().text.slice(state.get().selection), "café");
        vcx.simulate_keystrokes(next);
        vcx.run_until_parked();
        assert_eq!(state.get().secondary_selections.len(), 1);
        vcx.simulate_keystrokes("x");
        vcx.run_until_parked();
        assert_eq!(state.get().text, "x x café catastrophe");
        vcx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-z"
        } else {
            "ctrl-z"
        });
        vcx.run_until_parked();
        assert_eq!(state.get().text, "café café café catastrophe");
        assert_eq!(state.get().secondary_selections.len(), 1);
        vcx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-shift-l"
        } else {
            "ctrl-shift-l"
        });
        vcx.run_until_parked();
        assert_eq!(state.get().secondary_selections.len(), 2);
        vcx.simulate_keystrokes("q");
        vcx.run_until_parked();
        assert_eq!(state.get().text, "q q q catastrophe");
        vcx.simulate_keystrokes("left delete");
        vcx.run_until_parked();
        assert_eq!(state.get().text, "   catastrophe");
        vcx.simulate_keystrokes("escape");
        vcx.run_until_parked();
        assert!(state.get().secondary_selections.is_empty());
    }
    #[gpui::test]
    fn word_and_line_deletion_preserve_other_lines_and_undo(cx: &mut TestAppContext) {
        use omen_core::{EditorDocument, EditorState, Value};
        use omen_ui::ViewExt;
        let state = Value::new(EditorState {
            text: "keep\nhello café\nnext".into(),
            selection: 16..16,
            reveal: Some(16..16),
            ..Default::default()
        });
        let root = view! {TextEditor("Rust").autofocus(true).frame(600,300)}
            .document(EditorDocument::new(state.clone(), |_| {}));
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        vcx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "alt-backspace"
        } else {
            "ctrl-backspace"
        });
        vcx.run_until_parked();
        assert_eq!(state.get().text, "keep\nhello \nnext");
        vcx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-backspace"
        } else {
            "home ctrl-shift-k"
        });
        vcx.run_until_parked();
        if cfg!(target_os = "macos") {
            assert_eq!(state.get().text, "keep\n\nnext");
        }
        vcx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-z"
        } else {
            "ctrl-z"
        });
        vcx.run_until_parked();
        assert_eq!(state.get().text, "keep\nhello \nnext");
    }
    #[gpui::test]
    fn inspector_arrow_focus_scrolls_offscreen_inputs_into_view(cx: &mut TestAppContext) {
        let values: Vec<_> = (0..16)
            .map(|_| omen_core::Value::new(String::new()))
            .collect();
        let request = omen_core::Value::new(true);
        let root=view!{Layout {
            for (index,value) in values.iter().enumerate() {
                TextField(format!("Property {index}")).value(value.clone()).frame(.infinity,40)
            }
        }.spacing(4).frame(300,160).scrollable().focusScope(true).focusArrows(true).focusRequest(request.clone())}.into_node();
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        for _ in 0..13 {
            vcx.simulate_keystrokes("down");
            vcx.run_until_parked();
        }
        vcx.simulate_keystrokes("x");
        vcx.run_until_parked();
        assert!(values.iter().skip(8).any(|value| value.get() == "x"));
        let offset = vcx.update(|window, cx| {
            focus_scroll::handle(Some(0), window, cx)
                .unwrap()
                .offset()
                .y
        });
        assert!(offset < px(-200.));
        for _ in 0..12 {
            vcx.simulate_keystrokes("up");
            vcx.run_until_parked();
        }
        let offset = vcx.update(|window, cx| {
            focus_scroll::handle(Some(0), window, cx)
                .unwrap()
                .offset()
                .y
        });
        assert_eq!(offset, px(0.));
    }
    #[gpui::test]
    fn multicursor_paste_distributes_lines_in_document_order_and_undo_restores_selections(
        cx: &mut TestAppContext,
    ) {
        use omen_core::{EditorDocument, EditorState, Value};
        use omen_ui::ViewExt;
        let state = Value::new(EditorState {
            text: "cat dog cat".into(),
            selection: 0..0,
            ..Default::default()
        });
        let root = view! {TextEditor("Rust").autofocus(true).frame(600,300)}
            .document(EditorDocument::new(state.clone(), |_| {}));
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        vcx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-d cmd-d cmd-c"
        } else {
            "ctrl-d ctrl-d ctrl-c"
        });
        vcx.run_until_parked();
        vcx.update(|_, cx| {
            assert_eq!(
                cx.read_from_clipboard().unwrap().text().unwrap(),
                "cat\ncat"
            )
        });
        vcx.update(|_, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string("A\nB".into())));
        vcx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-v"
        } else {
            "ctrl-v"
        });
        vcx.run_until_parked();
        assert_eq!(state.get().text, "A dog B");
        vcx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-z"
        } else {
            "ctrl-z"
        });
        vcx.run_until_parked();
        assert_eq!(state.get().text, "cat dog cat");
        assert_eq!(state.get().secondary_selections.len(), 1);
        vcx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-x"
        } else {
            "ctrl-x"
        });
        vcx.run_until_parked();
        assert_eq!(state.get().text, " dog ");
    }
    #[gpui::test]
    fn vertical_cursors_share_typing_and_grapheme_backspace(cx: &mut TestAppContext) {
        use omen_core::{EditorDocument, EditorState, Value};
        use omen_ui::ViewExt;
        let state = Value::new(EditorState {
            text: "a👩‍💻b\nc👩‍💻d".into(),
            selection: 1..1,
            reveal: Some(1..1),
            ..Default::default()
        });
        let root = view! {TextEditor("Rust").autofocus(true).frame(600,300)}
            .document(EditorDocument::new(state.clone(), |_| {}));
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        vcx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-alt-down"
        } else {
            "ctrl-alt-down"
        });
        vcx.run_until_parked();
        assert_eq!(state.get().secondary_selections.len(), 1);
        vcx.simulate_keystrokes("right backspace");
        vcx.run_until_parked();
        assert_eq!(state.get().text, "ab\ncd");
        vcx.simulate_keystrokes("x");
        vcx.run_until_parked();
        assert_eq!(state.get().text, "axb\ncxd");
    }
    #[gpui::test]
    fn multicursor_indentation_preserves_selected_words_and_last_line_delete_removes_newline(
        cx: &mut TestAppContext,
    ) {
        use omen_core::{EditorDocument, EditorState, Value};
        use omen_ui::ViewExt;
        let state = Value::new(EditorState {
            text: "cat\ncat".into(),
            selection: 0..0,
            ..Default::default()
        });
        let root = view! {TextEditor("Rust").autofocus(true).frame(600,300)}
            .document(EditorDocument::new(state.clone(), |_| {}));
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        vcx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-d cmd-d tab"
        } else {
            "ctrl-d ctrl-d tab"
        });
        vcx.run_until_parked();
        assert_eq!(state.get().text, "    cat\n    cat");
        vcx.simulate_keystrokes("shift-tab");
        vcx.run_until_parked();
        assert_eq!(state.get().text, "cat\ncat");
        vcx.simulate_keystrokes("escape");
        vcx.run_until_parked();
        vcx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-shift-k"
        } else {
            "ctrl-shift-k"
        });
        vcx.run_until_parked();
        assert_eq!(state.get().text, "cat");
    }
    #[gpui::test]
    fn button_full_key_press_dispatches_once_across_redraw_and_ignores_repeat(
        cx: &mut TestAppContext,
    ) {
        let count = Rc::new(Cell::new(0));
        let calls = count.clone();
        let root =
            view! {Button("Add modifier") {calls.set(calls.get()+1);}.frame(200,40)}.into_node();
        let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
        vcx.run_until_parked();
        vcx.simulate_keystrokes("tab");
        vcx.run_until_parked();
        for (index, key) in ["enter", "space"].into_iter().enumerate() {
            vcx.update(|window, cx| {
                window.dispatch_event(
                    gpui::KeyDownEvent {
                        keystroke: gpui::Keystroke::parse(key).unwrap(),
                        is_held: false,
                        prefer_character_input: false,
                    }
                    .to_platform_input(),
                    cx,
                );
            });
            vcx.run_until_parked();
            assert_eq!(count.get(), index + 1);
            vcx.update(|window, cx| {
                window.dispatch_event(
                    gpui::KeyDownEvent {
                        keystroke: gpui::Keystroke::parse(key).unwrap(),
                        is_held: true,
                        prefer_character_input: false,
                    }
                    .to_platform_input(),
                    cx,
                );
                window.dispatch_event(
                    gpui::KeyUpEvent {
                        keystroke: gpui::Keystroke::parse(key).unwrap(),
                    }
                    .to_platform_input(),
                    cx,
                );
            });
            vcx.run_until_parked();
            assert_eq!(count.get(), index + 1);
        }
    }
}

fn native_trace(message: &str) {
    if cfg!(debug_assertions) && std::env::var_os("OMEN_NATIVE_TRACE").is_some() {
        eprintln!("Omen GPUI: {message}");
    }
}

fn app_setting(key: &str, compiled: Option<&str>, fallback: &str) -> String {
    std::env::var(key)
        .ok()
        .or_else(|| compiled.map(str::to_owned))
        .unwrap_or_else(|| fallback.to_owned())
}
#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn window_dimension(key: &str, compiled: Option<&str>, fallback: f32) -> f32 {
    app_setting(key, compiled, "")
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite() && (64.0..=16384.0).contains(value))
        .unwrap_or(fallback)
}
