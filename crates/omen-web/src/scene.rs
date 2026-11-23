use crate::WindowError;
use omen_core::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Browser scene wire version. The matching host rejects incompatible versions.
pub const SCENE_VERSION: u32 = 1;
/// One accessible DOM node. Text is always assigned with textContent, never HTML.
#[derive(Clone, Serialize)]
pub struct DomNode {
    /// Stable runtime identity.
    pub id: String,
    /// Native HTML tag.
    pub tag: String,
    /// Safe text content (never includes password values).
    pub text: String,
    /// DOM attributes such as roles and accessible labels.
    pub attributes: BTreeMap<String, String>,
    /// CSS property overrides.
    pub style: BTreeMap<String, String>,
    /// Input property value, assigned as a property rather than an HTML attribute.
    pub value: Option<String>,
    /// Checked input property.
    pub checked: Option<bool>,
    /// Whether the node accepts an activation event.
    pub actionable: bool,
    /// Gesture topic names enabled for this node.
    pub gestures: Vec<String>,
    /// Optional entry/property animation sampled by the shared host.
    pub motion: Option<Motion>,
    /// Child nodes in visual source order.
    pub children: Vec<DomNode>,
}
/// Finite motion samples, using the same core easing/spring model as desktop.
#[derive(Clone, Serialize)]
pub struct Motion {
    /// Millisecond settling deadline.
    pub duration_ms: f64,
    /// Normalized progress sampled for browser interpolation.
    pub samples: Vec<f32>,
    /// Optional entry transition (fade or slide).
    pub entry: Option<String>,
    /// Target opacity.
    pub opacity: f32,
    /// Target physical translation.
    pub offset: (f32, f32),
}
/// Complete browser scene, with no callback closures crossing the boundary.
#[derive(Clone, Serialize)]
pub struct Scene {
    /// Wire version.
    pub version: u32,
    /// Root DOM node.
    pub root: DomNode,
}
/// Browser-to-Rust events. Input values are bounded by the host/ABI before decoding.
#[derive(Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum BrowserEvent {
    /// Button, tap, or assistive-technology activation.
    Activate {
        /// Stable node identity.
        id: String,
    },
    /// A committed single-line input value.
    Input {
        /// Stable node identity.
        id: String,
        /// Current value.
        value: String,
    },
    /// A native checkbox/switch edit.
    Check {
        /// Stable node identity.
        id: String,
        /// Current checked state.
        checked: bool,
    },
    /// Dismiss the top presentation.
    Dismiss {
        /// Presentation node identity.
        id: String,
    },
    /// Completed long press or directional swipe.
    Gesture {
        /// Stable node identity.
        id: String,
        /// long_press or swipe_left/right/up/down.
        gesture: String,
    },
}
#[derive(Clone, Default)]
struct Handler {
    action: Option<Action>,
    text: Option<Binding<String>>,
    checked: Option<Binding<bool>>,
    changed: Option<Action>,
    asynchronous: Option<AsyncAction>,
    context: Option<TaskContext>,
    error: Option<ErrorAction>,
    task_handle: Option<Value<Option<TaskHandle>>>,
    gestures: BTreeMap<String, Action>,
}
/// Renderer-independent retained scene engine. Native hosts can test this without a browser.
pub struct WebEngine {
    runtime: Runtime,
    handlers: BTreeMap<String, Handler>,
    foreground: bool,
}
impl WebEngine {
    /// Own one application's view/state graph.
    pub fn new(view: impl View) -> Self {
        Self {
            runtime: Runtime::new(view.into_node()),
            handlers: BTreeMap::new(),
            foreground: true,
        }
    }
    /// Schedule an asynchronous host render after reactive writes.
    pub fn set_waker(&mut self, wake: impl Fn() + 'static) {
        self.runtime.set_waker(wake);
    }
    /// Suspend mount resources, preserving view state for foreground restoration.
    pub fn set_lifecycle(&mut self, state: AppLifecycle) -> Result<(), WindowError> {
        self.foreground = state == AppLifecycle::Foreground;
        self.runtime.set_lifecycle(state);
        if !self.foreground {
            self.runtime.render()?;
            self.handlers.clear();
        }
        Ok(())
    }
    /// Reconcile the view tree and build a keyed DOM scene.
    pub fn render(
        &mut self,
        viewport: Viewport,
        system_dark: bool,
        reduced_motion: bool,
    ) -> Result<Scene, WindowError> {
        use omen_core::{DeviceEnvironment, DevicePlatform, ThemeMode};
        self.render_device(
            DeviceEnvironment {
                platform: DevicePlatform::Web,
                appearance: if system_dark {
                    ThemeMode::Dark
                } else {
                    ThemeMode::Light
                },
                ..DeviceEnvironment::default()
            }
            .with_viewport(viewport),
            reduced_motion,
        )
    }

    /// Render with browser-provided input, display and viewport capabilities.
    pub fn render_device(
        &mut self,
        device: omen_core::DeviceEnvironment,
        reduced_motion: bool,
    ) -> Result<Scene, WindowError> {
        let viewport = device.size;
        let system_dark = device.appearance == omen_core::ThemeMode::Dark;
        self.runtime.set_device_environment(device);
        let root = self.runtime.render()?;
        let mut handlers = BTreeMap::new();
        let mut count = 0;
        let settings = Settings {
            tokens: inherited_theme(&root).resolve(system_dark),
            direction: LayoutDirection::LeftToRight,
            parent_layout: LayoutMode::Vertical,
            insets: viewport.safe_area,
            reduced: reduced_motion,
            system_dark,
            viewport_width: viewport.width,
        };
        let root = build(&root, settings, viewport.width, &mut handlers, &mut count)?;
        self.handlers = handlers;
        Ok(Scene {
            version: SCENE_VERSION,
            root,
        })
    }
    /// Dispatch only events that match a mounted node's registered capabilities.
    pub fn dispatch(&self, event: BrowserEvent) -> bool {
        if !self.foreground {
            return false;
        }
        let id = match &event {
            BrowserEvent::Activate { id }
            | BrowserEvent::Input { id, .. }
            | BrowserEvent::Check { id, .. }
            | BrowserEvent::Dismiss { id }
            | BrowserEvent::Gesture { id, .. } => id,
        };
        let Some(handler) = self.handlers.get(id).cloned() else {
            return false;
        };
        match event {
            BrowserEvent::Activate { .. } | BrowserEvent::Dismiss { .. } => {
                if let (Some(action), Some(context)) = (handler.asynchronous, handler.context) {
                    let task = context.spawn(&action, handler.error);
                    if let Some(binding) = handler.task_handle {
                        binding.set(Some(task));
                    }
                } else if let Some(action) = handler.action {
                    action.invoke();
                } else {
                    return false;
                }
            }
            BrowserEvent::Input { value, .. } => {
                let Some(binding) = handler.text else {
                    return false;
                };
                binding.set(value);
                if let Some(changed) = handler.changed {
                    changed.invoke();
                }
            }
            BrowserEvent::Check { checked, .. } => {
                let Some(binding) = handler.checked else {
                    return false;
                };
                binding.set(checked);
                if let Some(changed) = handler.changed {
                    changed.invoke();
                }
            }
            BrowserEvent::Gesture { gesture, .. } => {
                let Some(action) = handler.gestures.get(&gesture) else {
                    return false;
                };
                action.invoke();
            }
        }
        true
    }
    /// Drain lifecycle/action tasks for a local host executor.
    pub fn take_tasks(&mut self) -> Vec<TaskJob> {
        self.runtime.take_tasks()
    }
}
#[derive(Clone, Copy)]
struct Settings {
    tokens: DesignTokens,
    direction: LayoutDirection,
    parent_layout: LayoutMode,
    insets: Insets,
    reduced: bool,
    system_dark: bool,
    viewport_width: f32,
}
fn pixels(value: f32) -> String {
    format!("{}px", if value.is_finite() { value.max(0.) } else { 0. })
}
fn color(tokens: DesignTokens, value: Color) -> String {
    let rgba = tokens.rgba(value);
    if rgba & 0xff == 0xff {
        format!("#{:06x}", rgba >> 8)
    } else {
        format!("#{rgba:08x}")
    }
}
fn build(
    node: &ViewNode,
    mut settings: Settings,
    width: f32,
    handlers: &mut BTreeMap<String, Handler>,
    count: &mut usize,
) -> Result<DomNode, WindowError> {
    *count += 1;
    if *count > 10000 {
        return Err(WindowError::SceneLimit);
    }
    if let Some(theme) = node.style.theme {
        settings.tokens = theme.resolve(settings.system_dark);
    }
    if let Some(insets) = node.style.safe_area_insets {
        settings.insets = insets;
    }
    let style = node.style.for_container(
        settings.viewport_width,
        width,
        settings.tokens,
        settings.insets,
    );
    let mut child_width = style.content_width(width, settings.tokens, settings.insets);
    let grid = style.grid_metrics(child_width, settings.tokens);
    if matches!(node.kind, ViewKind::Card(_))
        && style.padding.is_none()
        && style.padding_token.is_none()
    {
        child_width = (child_width - 2. * settings.tokens.space(Space::Large) - 2.).max(0.);
    }
    if let Some(locale) = &style.locale {
        settings.direction = locale.direction;
    }
    if let Some(direction) = style.direction {
        settings.direction = direction;
    }
    if let Some(reduced) = style.reduced_motion {
        settings.reduced |= reduced;
    }
    let mut child_settings = settings;
    let id = style
        .runtime_id
        .clone()
        .unwrap_or_else(|| format!("node-{count}"));
    let mut dom = DomNode {
        id: id.clone(),
        tag: "div".into(),
        text: String::new(),
        attributes: BTreeMap::new(),
        style: BTreeMap::new(),
        value: None,
        checked: None,
        actionable: false,
        gestures: Vec::new(),
        motion: None,
        children: Vec::new(),
    };
    let mut handler = Handler {
        changed: style.on_change.clone(),
        asynchronous: style.async_action.clone(),
        context: style.task_context.clone(),
        error: style.on_error.clone(),
        task_handle: style.task_handle.clone(),
        ..Default::default()
    };
    let css = &mut dom.style;
    css.insert("box-sizing".into(), "border-box".into());
    css.insert("min-width".into(), "0".into());
    css.insert("display".into(), "flex".into());
    css.insert("flex-direction".into(), "column".into());
    dom.attributes.insert(
        "dir".into(),
        if settings.direction == LayoutDirection::RightToLeft {
            "rtl"
        } else {
            "ltr"
        }
        .into(),
    );
    if let Some(locale) = &style.locale {
        dom.attributes.insert("lang".into(), locale.id.clone());
    }
    if style.theme.is_some() || id == "root" {
        css.insert("color".into(), format!("#{:06x}", settings.tokens.primary));
        css.insert(
            "background".into(),
            format!("#{:06x}", settings.tokens.canvas),
        );
        css.insert("font-size".into(), pixels(settings.tokens.body_size));
        css.insert(
            "--omen-surface".into(),
            format!("#{:06x}", settings.tokens.surface),
        );
        css.insert(
            "color-scheme".into(),
            if settings.tokens.canvas < 0x808080 {
                "dark"
            } else {
                "light"
            }
            .into(),
        );
    }
    let children: &[ViewNode] = match &node.kind {
        ViewKind::Reactive(_) => {
            dom.text = "Unresolved view".into();
            &[]
        }
        ViewKind::Text(text) => {
            dom.tag = "span".into();
            dom.text = text.clone();
            &[]
        }
        ViewKind::Button { label, action } => {
            dom.tag = "button".into();
            css.insert("width".into(), "fit-content".into());
            css.insert("flex".into(), "none".into());
            dom.text = if style.navigation_back {
                if settings.direction == LayoutDirection::RightToLeft {
                    "→"
                } else {
                    "←"
                }
                .into()
            } else {
                label.clone()
            };
            dom.attributes.insert("type".into(), "button".into());
            handler.action = Some(action.clone());
            &[]
        }
        ViewKind::Image(path) => {
            dom.tag = "img".into();
            dom.attributes.insert("src".into(), path.clone());
            dom.attributes.insert(
                "alt".into(),
                style.accessibility.label.clone().unwrap_or_default(),
            );
            css.insert("object-fit".into(), "contain".into());
            &[]
        }
        ViewKind::Icon(name) => {
            dom.tag = "span".into();
            dom.attributes.insert("role".into(), "img".into());
            dom.attributes.insert("data-icon".into(), name.clone());
            dom.text = match name.as_str() {
                "check" => "✓",
                "close" => "×",
                "plus" => "+",
                "search" => "⌕",
                "settings" => "⚙",
                _ => "ⓘ",
            }
            .into();
            &[]
        }
        ViewKind::TextField { label, value } | ViewKind::SecureField { label, value } => {
            dom.tag = if style.multiline { "textarea" } else { "input" }.into();
            dom.attributes.insert(
                "type".into(),
                if matches!(node.kind, ViewKind::SecureField { .. }) {
                    "password"
                } else {
                    "text"
                }
                .into(),
            );
            dom.attributes.insert("placeholder".into(), label.clone());
            dom.attributes.insert("aria-label".into(), label.clone());
            let binding = style
                .text_value
                .clone()
                .unwrap_or_else(|| value.clone().into());
            dom.value = Some(binding.get());
            handler.text = Some(binding);
            &[]
        }
        ViewKind::Checkbox { label, value } | ViewKind::Toggle { label, value } => {
            dom.tag = "input".into();
            dom.attributes.insert("type".into(), "checkbox".into());
            dom.attributes.insert("aria-label".into(), label.clone());
            dom.attributes
                .insert("data-visible-label".into(), label.clone());
            if matches!(node.kind, ViewKind::Toggle { .. }) {
                dom.attributes.insert("role".into(), "switch".into());
            }
            let binding = style
                .checked
                .clone()
                .unwrap_or_else(|| value.clone().into());
            dom.checked = Some(binding.get());
            handler.checked = Some(binding);
            &[]
        }
        ViewKind::Progress(fraction) => {
            dom.tag = "progress".into();
            dom.attributes.insert("max".into(), "1".into());
            dom.attributes.insert("value".into(), fraction.to_string());
            &[]
        }
        ViewKind::Gap(size) => {
            css.insert("flex".into(), "none".into());
            css.insert("align-self".into(), "stretch".into());
            match settings.parent_layout {
                LayoutMode::Horizontal => {
                    css.insert("width".into(), pixels(*size));
                }
                LayoutMode::Vertical => {
                    css.insert("height".into(), pixels(*size));
                }
                LayoutMode::Stack => {
                    css.insert("width".into(), "0px".into());
                    css.insert("height".into(), "0px".into());
                }
            }
            &[]
        }
        ViewKind::Divider => {
            dom.tag = "hr".into();
            css.insert(
                "border".into(),
                format!("0.5px solid #{:06x}", settings.tokens.border),
            );
            &[]
        }
        ViewKind::Layout(children) => {
            child_settings.parent_layout = style.mode.unwrap_or_default();
            if let Some((columns, _)) = grid {
                css.insert("display".into(), "grid".into());
                css.insert(
                    "grid-template-columns".into(),
                    format!("repeat({columns}, minmax(0, 1fr))"),
                );
                css.insert("width".into(), "100%".into());
            } else {
                match child_settings.parent_layout {
                    LayoutMode::Horizontal => {
                        css.insert("flex-direction".into(), "row".into());
                        css.insert("align-items".into(), "center".into());
                    }
                    LayoutMode::Stack => {
                        css.insert("display".into(), "grid".into());
                    }
                    _ => {}
                }
            }
            children
        }
        ViewKind::Card(children) => {
            child_settings.parent_layout = LayoutMode::Vertical;
            css.insert(
                "padding".into(),
                pixels(settings.tokens.space(Space::Large)),
            );
            css.insert("gap".into(), pixels(settings.tokens.space(Space::Medium)));
            css.insert("border-radius".into(), pixels(settings.tokens.radius));
            css.insert(
                "background".into(),
                format!("#{:06x}", settings.tokens.surface),
            );
            css.insert(
                "border".into(),
                format!("1px solid #{:06x}", settings.tokens.border),
            );
            children
        }
        ViewKind::List(children) => {
            child_settings.parent_layout = LayoutMode::Vertical;
            dom.attributes.insert("role".into(), "list".into());
            children
        }
        ViewKind::Group(children) => {
            if style.is_transparent_group() {
                css.insert("display".into(), "contents".into());
            } else {
                child_settings.parent_layout = LayoutMode::Vertical;
            }
            children
        }
        ViewKind::Presentation {
            children,
            kind,
            dismiss,
        } => {
            dom.attributes.insert(
                "data-presentation".into(),
                format!("{kind:?}").to_lowercase(),
            );
            handler.action = Some(dismiss.clone());
            children
        }
    };
    if let Some(action) = &style.on_tap {
        handler.action = Some(action.clone());
    }
    if let Some(action) = &style.on_long_press {
        handler.gestures.insert("long_press".into(), action.clone());
    }
    for (direction, action) in &style.on_swipe {
        handler.gestures.insert(
            format!("swipe_{direction:?}").to_lowercase(),
            action.clone(),
        );
    }
    dom.gestures = handler.gestures.keys().cloned().collect();
    dom.actionable = handler.action.is_some() || handler.asynchronous.is_some();
    if dom.actionable && dom.tag != "button" && !matches!(node.kind, ViewKind::Presentation { .. })
    {
        dom.attributes.insert("role".into(), "button".into());
        dom.attributes.insert("tabindex".into(), "0".into());
    }
    if let Some(kind) = style.presentation_kind {
        dom.attributes.insert(
            "data-presentation".into(),
            format!("{kind:?}").to_lowercase(),
        );
    }
    if style.navigation_back {
        dom.attributes
            .insert("data-omen-back".into(), "true".into());
    }
    if style.focusable {
        dom.attributes.insert("tabindex".into(), "0".into());
    }
    if let Some(role) = style.accessibility.role {
        dom.attributes.insert(
            "role".into(),
            match role {
                AccessibilityRole::Text => "text",
                AccessibilityRole::Heading => "heading",
                AccessibilityRole::Image => "img",
                AccessibilityRole::Button => "button",
                AccessibilityRole::Checkbox => "checkbox",
                AccessibilityRole::Switch => "switch",
                AccessibilityRole::TextField => "textbox",
                AccessibilityRole::Progress => "progressbar",
                AccessibilityRole::List => "list",
                AccessibilityRole::ListItem => "listitem",
                AccessibilityRole::Group => "group",
                AccessibilityRole::Navigation => "navigation",
                AccessibilityRole::Tab => "tab",
                AccessibilityRole::TabList => "tablist",
                AccessibilityRole::TabPanel => "tabpanel",
                AccessibilityRole::Dialog => "dialog",
            }
            .into(),
        );
        if role == AccessibilityRole::Heading {
            dom.attributes.insert("aria-level".into(), "2".into());
        }
    }
    if let Some(label) = &style.accessibility.label {
        dom.attributes.insert("aria-label".into(), label.clone());
    }
    if let Some(hint) = &style.accessibility.hint {
        dom.attributes
            .insert("aria-description".into(), hint.clone());
    }
    if let Some(selected) = style.accessibility.selected {
        dom.attributes
            .insert("aria-selected".into(), selected.to_string());
    }
    if style.fill {
        css.insert("flex".into(), "1 1 0%".into());
        css.insert("min-width".into(), "0".into());
        css.insert("min-height".into(), "0".into());
    }
    if style.scrollable {
        css.insert("overflow-x".into(), "hidden".into());
        css.insert("overflow-y".into(), "auto".into());
        css.insert("min-height".into(), "0".into());
    }
    if style.horizontal_scrollable {
        css.insert("overflow-x".into(), "auto".into());
        css.insert("overflow-y".into(), "hidden".into());
    }
    if let Some(width) = style.min_width {
        css.insert("min-width".into(), pixels(width));
    }
    if let Some(frame) = style.frame {
        css.insert(
            "width".into(),
            if frame.width == f32::INFINITY {
                "100%".into()
            } else {
                pixels(frame.width)
            },
        );
        css.insert(
            "height".into(),
            if frame.height == f32::INFINITY {
                "100%".into()
            } else {
                pixels(frame.height)
            },
        );
    }
    let padding = style.padding.or_else(|| {
        style
            .padding_token
            .map(|token| settings.tokens.space(token))
    });
    if let Some(padding) = padding {
        css.insert("padding".into(), pixels(padding));
    }
    if let Some(spacing) = style.spacing.or_else(|| {
        style
            .spacing_token
            .map(|token| settings.tokens.space(token))
    }) {
        css.insert("gap".into(), pixels(spacing));
    }
    if let Some(edges) = style.safe_area {
        use SafeArea::*;
        let base = padding.unwrap_or(0.);
        if matches!(edges, All | Vertical) {
            css.insert("padding-top".into(), pixels(base + settings.insets.top));
            css.insert(
                "padding-bottom".into(),
                pixels(base + settings.insets.bottom),
            );
            child_settings.insets.top = 0.;
            child_settings.insets.bottom = 0.;
        }
        if matches!(edges, All | Horizontal) {
            css.insert("padding-left".into(), pixels(base + settings.insets.left));
            css.insert("padding-right".into(), pixels(base + settings.insets.right));
            child_settings.insets.left = 0.;
            child_settings.insets.right = 0.;
        }
    }
    if let Some(color_value) = style.color {
        css.insert("color".into(), color(settings.tokens, color_value));
    }
    if let Some(hover) = style.hover_background {
        css.insert(
            "--omen-hover-background".into(),
            color(settings.tokens, hover),
        );
        dom.attributes
            .insert("data-omen-hover-background".into(), String::new());
    }
    if let Some(background) = style.background {
        css.insert(
            "background".into(),
            match background {
                Background::Color(value) => color(settings.tokens, value),
                Background::Gradient(gradient) => format!(
                    "linear-gradient({}deg, {}, {})",
                    gradient.angle,
                    color(settings.tokens, gradient.from),
                    color(settings.tokens, gradient.to)
                ),
            },
        );
    }
    if let Some(radius) = style.corner_radius {
        css.insert("border-radius".into(), pixels(radius));
    }
    let font_size = style.font_size.or_else(|| {
        style.font.map(|font| match font {
            Font::Caption => 12.,
            Font::Footnote => 13.,
            Font::Body => 16.,
            Font::Headline => 18.,
            Font::Title2 => 20.,
            Font::Title => 24.,
            Font::LargeTitle => 36.,
        })
    });
    if let Some(size) = font_size {
        css.insert("font-size".into(), pixels(size));
    }
    if let Some(height) = style.line_height {
        css.insert("line-height".into(), pixels(height));
    }
    if let Some(family) = style.font_family {
        css.insert("font-family".into(), family);
    }
    if let Some(weight) = style.weight {
        css.insert(
            "font-weight".into(),
            match weight {
                Weight::Thin => 100,
                Weight::Light => 300,
                Weight::Regular => 400,
                Weight::Medium => 500,
                Weight::Semibold => 600,
                Weight::Bold => 700,
                Weight::Heavy => 900,
            }
            .to_string(),
        );
    }
    if let Some(alignment) = style.alignment {
        use Alignment::*;
        let (x, y) = match alignment {
            Leading => (0, 1),
            Trailing => (2, 1),
            Center => (1, 1),
            Top => (1, 0),
            Bottom => (1, 2),
            TopLeading => (0, 0),
            TopTrailing => (2, 0),
            BottomLeading => (0, 2),
            BottomTrailing => (2, 2),
        };
        let vertical = style.mode.unwrap_or_default() != LayoutMode::Horizontal;
        let values = ["flex-start", "center", "flex-end"];
        css.insert(
            "align-items".into(),
            values[if vertical { x } else { y }].into(),
        );
        css.insert(
            "justify-content".into(),
            values[if vertical { y } else { x }].into(),
        );
    }
    let offset = style.offset.unwrap_or_default();
    let opacity = style.opacity.unwrap_or(1.);
    if style.offset.is_some() {
        css.insert(
            "transform".into(),
            format!("translate({}px, {}px)", offset.0, offset.1),
        );
    }
    if style.opacity.is_some() {
        css.insert("opacity".into(), opacity.to_string());
    }
    if !settings.reduced && (style.animation.is_some() || style.transition.is_some()) {
        let animation = style.animation.unwrap_or_default();
        dom.motion = Some(Motion {
            duration_ms: animation.duration.as_secs_f64() * 1000.,
            samples: (0..=60)
                .map(|step| animation.sample(animation.duration.mul_f64(step as f64 / 60.)))
                .collect(),
            entry: style.transition.map(|transition| {
                match transition {
                    Transition::Fade => "fade",
                    Transition::Slide => "slide",
                }
                .into()
            }),
            opacity,
            offset,
        });
    }
    let flow_width = grid.map(|(_, width)| width).or_else(|| {
        (matches!(node.kind, ViewKind::Layout(_)) && style.mode == Some(LayoutMode::Horizontal))
            .then(|| {
                let spacing = style
                    .spacing
                    .or_else(|| {
                        style
                            .spacing_token
                            .map(|token| settings.tokens.space(token))
                    })
                    .unwrap_or(0.);
                omen_core::remaining_row_width(children, child_width, spacing)
            })
            .flatten()
    });
    for child in children {
        let pin = child.style.pin;
        let width = flow_width
            .filter(|_| pin.is_none() && (grid.is_some() || child.style.fill))
            .unwrap_or(child_width);
        let mut child = build(child, child_settings, width, handlers, count)?;
        if matches!(node.kind, ViewKind::List(_)) {
            child
                .attributes
                .entry("role".into())
                .or_insert("listitem".into());
        }
        if let Some(pin) = pin {
            let (trailing, bottom) = pin.corner.edges();
            let right = trailing ^ (settings.direction == LayoutDirection::RightToLeft);
            dom.style
                .entry("position".into())
                .or_insert_with(|| "relative".into());
            child.style.insert("position".into(), "absolute".into());
            child.style.insert(
                if right { "right" } else { "left" }.into(),
                pixels(f32::from(pin.x)),
            );
            child.style.insert(
                if bottom { "bottom" } else { "top" }.into(),
                pixels(f32::from(pin.y)),
            );
        } else if grid.is_none() && style.mode == Some(LayoutMode::Stack) {
            child.style.insert("grid-area".into(), "1 / 1".into());
        }
        dom.children.push(child);
    }
    handlers.insert(id, handler);
    Ok(dom)
}

fn inherited_theme(node: &ViewNode) -> Theme {
    if let Some(theme) = node.style.theme {
        return theme;
    }
    if let ViewKind::Group(children) = &node.kind
        && let [child] = children.as_slice()
    {
        return inherited_theme(child);
    }
    Theme::default()
}
