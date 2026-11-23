//! View builders and declarative macros reexported by the public `omen` facade.
//!
//! Disable default features for backend-independent construction, testing and
//! API documentation. Application code normally imports from `omen`. The
//! component families and shared modifiers are documented on their types and
//! [`ViewExt`]; renderer service coverage remains platform dependent.
//!
//! ```
//! use omen_ui::{View, view};
//! let greeting = view! { Text("Hello").font(.title) };
//! let _node = greeting.into_node();
//! ```
#![deny(missing_docs)]
extern crate self as omen_ui;

pub use omen_core::*;
pub use omen_macros::{environment, locale, main, state, view};

pub use omen_format as format;
pub use omen_format::{
    AddressOptions, ByteOptions, ByteStandard, CalendarId, CurrencyCode, CurrencyDisplay,
    CurrencyOptions, CurrencySign, DataRateOptions, DataRateUnit, DateOptions, DateStyle,
    DateTimeOptions, Decimal, DurationOptions, DurationStyle, FormatContext, FormatError,
    FormatOption, Formatter, FormatterDescriptor, HourCycle, Instant, ListOptions, ListStyle,
    ListType, LocaleId, Measurement, MeasurementOptions, MeasurementSystem, Money, NumberOptions,
    NumberingSystem, PercentOptions, PercentScale, PhoneFormat, PhoneNumber, PhoneOptions,
    PlainDate, PlainDateTime, PlainTime, PostalAddress, RelativeOptions, RelativeStyle,
    RoundingMode, TextOptions, TextTransform, TimeOptions, TimeStyle, TimeZoneId, Unit, UnitWidth,
    ZoneOffset, ZonedDateTime,
};

/// Backend entry points used only by generated application code.
#[doc(hidden)]
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
pub mod __private {
    #[cfg(target_os = "android")]
    pub use omen_gpui::{AndroidApp, run_android_view};
    pub use omen_gpui::{WindowError, run_view};
}

/// Browser backend entry points and native transport helpers.
#[cfg(feature = "web")]
pub use omen_web as web;
/// Backend entry points used by generated browser application code.
#[doc(hidden)]
#[cfg(all(feature = "web", any(target_arch = "wasm32", not(feature = "native"))))]
pub mod __private {
    pub use omen_web::{WindowError, run_view};
}

/// A common owned view representation for conditional branches and modifier chains.
#[derive(Clone, Debug)]
pub struct AnyView(ViewNode);

impl AnyView {
    /// Erase a builder's type into the common runtime node representation.
    pub fn new(view: impl View) -> Self {
        Self(view.into_node())
    }
    /// Group zero or more siblings without imposing an additional layout.
    pub fn group(children: Vec<ViewNode>) -> Self {
        Self(ViewNode::new(ViewKind::Group(children)))
    }
}
impl View for AnyView {
    fn into_node(self) -> ViewNode {
        self.0
    }
}

/// An owned text label with typography and accessibility modifiers.
///
/// Inside `view!`, literals interpolate local values, state and fields without
/// `format!`. Use `{{` and `}}` for literal braces. The Rust builder accepts an
/// already constructed string. Text has no child block.
///
/// ```
/// # use omen_ui as omen;
/// use omen::view;
/// let name = "Ada";
/// let greeting = view! { Text("Hello, {name}").font(.title).color(.primary) };
/// ```
pub struct Text(ViewNode);
impl Text {
    /// Create text from a string or owned string value.
    pub fn new(content: impl std::fmt::Display) -> Self {
        Self(ViewNode::new(ViewKind::Text(content.to_string())))
    }
}
impl View for Text {
    fn into_node(self) -> ViewNode {
        self.0
    }
}

/// A layout that uses a column in compact space and a row in wider space.
#[derive(Default)]
pub struct AdaptiveLayout(Layout);

impl AdaptiveLayout {
    /// Create an empty container.
    pub fn new() -> Self {
        Self::default()
    }
    /// Append one child view.
    pub fn child(mut self, child: impl View) -> Self {
        self.0 = self.0.child(child);
        self
    }
    /// Append already constructed children in source order.
    pub fn children(mut self, children: impl IntoIterator<Item = ViewNode>) -> Self {
        self.0 = self.0.children(children);
        self
    }
}
impl View for AdaptiveLayout {
    fn into_node(self) -> ViewNode {
        self.0.adaptive(AdaptiveDirection::default()).into_node()
    }
}

/// A grid with as many equal-width columns as fit its minimum item width.
#[derive(Default)]
pub struct AdaptiveGrid(Layout);

impl AdaptiveGrid {
    /// Create an empty container.
    pub fn new() -> Self {
        Self::default()
    }
    /// Append one child view.
    pub fn child(mut self, child: impl View) -> Self {
        self.0 = self.0.child(child);
        self
    }
    /// Append already constructed children in source order.
    pub fn children(mut self, children: impl IntoIterator<Item = ViewNode>) -> Self {
        self.0 = self.0.children(children);
        self
    }
}
impl View for AdaptiveGrid {
    fn into_node(self) -> ViewNode {
        self.0.min_item_width(240.).into_node()
    }
}

/// A labeled action that sizes to its content and padding by default.
///
/// The label supplies the default accessible name. Activation invokes the
/// callback on the UI thread; construction does not invoke it. Keep synchronous
/// work short, or use [`Button::new_async`] / the DSL's async action block.
/// Keyboard and assistive-technology activation depend on the platform backend.
///
/// ```
/// # use omen_ui as omen;
/// use omen::view;
/// let compact = view! { Button("Save") {}.padding(.md) };
/// let fixed = view! { Button("Save") {}.frame(240, 44) };
/// let full = view! { Button("Save") {}.frame(.infinity, 44) };
/// let asynchronous = view! { Button("Load") async {} };
/// ```
pub struct Button(ViewNode);
impl Button {
    /// Create an asynchronous button with cancellation and error handling.
    pub fn new_async<F, Fut>(label: impl Into<String>, factory: F) -> AnyView
    where
        F: Fn() -> Fut + 'static,
        Fut: std::future::Future + 'static,
        Fut::Output: TaskOutput,
    {
        Button::new(label, || {}).on_tap_async(factory)
    }
    /// Create a button; the callback runs on click, never during construction.
    pub fn new(label: impl Into<String>, action: impl Fn() + 'static) -> Self {
        Self(ViewNode::new(ViewKind::Button {
            label: label.into(),
            action: Action::new(action),
        }))
    }
}
impl View for Button {
    fn into_node(self) -> ViewNode {
        self.0
    }
}

macro_rules! container {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Default)]
        pub struct $name {
            children: Vec<ViewNode>,
        }
        impl $name {
            /// Create an empty container.
            pub fn new() -> Self {
                Self::default()
            }
            /// Append one child view.
            pub fn child(mut self, view: impl View) -> Self {
                self.children.push(view.into_node());
                self
            }
            /// Append already constructed children in source order.
            pub fn children(mut self, children: impl IntoIterator<Item = ViewNode>) -> Self {
                self.children.extend(children);
                self
            }
        }
        impl View for $name {
            fn into_node(self) -> ViewNode {
                ViewNode::new(ViewKind::$name(self.children))
            }
        }
    };
}
container!(
    Layout,
    r#"A container that arranges children vertically, horizontally, or in a stack.

Vertical is the default. `spacing` separates vertical/horizontal children; stack
mode overlays them. `alignment` controls placement rather than arrangement.

```
# use omen_ui as omen;
use omen::view;
let row = view! {
    Layout { Icon("check") Text("Complete") }.mode(.horizontal).spacing(.md)
};
```

Use `.horizontal(.medium)` for a breakpoint-specific arrangement. Use
[`ViewExt::fill`] and [`ViewExt::scrollable`] to build bounded scroll regions."#
);
container!(
    Card,
    r#"A vertical surface with themed padding, spacing, border and rounding.

Defaults use large-token padding and medium-token spacing. Standard modifiers
replace those defaults. A card is not automatically clickable or a heading.

```
# use omen_ui as omen;
use omen::view;
let summary = view! {
    Card { Text("Summary").font(.headline) Text("All changes saved") }
        .spacing(.md).cornerRadius(12)
};
```"#
);
container!(
    List,
    r#"An eager vertical list with list and list-item semantics.

Use stable, unique IDs for movable stateful rows. Keyed identity is implemented;
virtualization is not. Scrolling requires an explicitly bounded scroll region.

```
# use omen_ui as omen;
use omen::view;
let rows = view! {
    List(vec![1_u64, 2], id: |id| *id) { id in Text("Row {id}") }
        .spacing(.sm)
};
```"#
);

/// Styling, bindings and behavior shared by every Omen view.
///
/// Later explicit values replace earlier overrides. Rust methods use snake_case
/// and typed values; the `view!` DSL uses camelCase and symbolic enum values.
/// [`Self::spacing`] is meaningful on Layout, Card and List; the DSL validates
/// modifier context and arity before generating builders.
///
/// - Size and arrangement: [`Self::frame`], [`Self::padding`], [`Self::spacing`],
///   [`Self::alignment`], [`Self::fill`], [`Self::scrollable`].
/// - Appearance: [`Self::font`], [`Self::color`], [`Self::background`],
///   [`Self::gradient`], [`Self::corner_radius`], [`Self::theme`].
/// - Editable controls: [`Self::value`], [`Self::checked`], [`Self::on_change`].
/// - Presentations: [`Self::sheet`], [`Self::dialog`], [`Self::popover`], [`Self::modal`].
/// - State/lifecycle: [`Self::environment`], [`Self::on_tap`], [`Self::on_appear`],
///   [`Self::on_disappear`], [`Self::effect`], [`Self::task`].
/// - Semantics: [`Self::accessibility_label`], [`Self::accessibility_hint`],
///   [`Self::accessibility_role`], [`Self::focusable`].
pub trait ViewExt: View + Sized {
    /// Prevent a flex child from shrinking below this logical width.
    fn min_width(self, width: impl Into<f64>) -> AnyView {
        let width = width.into() as f32;
        let mut node = self.into_node();
        node.style.min_width = Some(if width.is_finite() { width.max(0.) } else { 0. });
        AnyView(node)
    }
    /// Make a bounded view an independent horizontal scroll region.
    fn scrollable_horizontal(self) -> AnyView {
        let mut node = self.into_node();
        node.style.horizontal_scrollable = true;
        AnyView(node)
    }

    /// Register a native application-root teardown callback. Unlike on_disappear,
    /// this does not run while the window is temporarily hidden.
    fn on_window_close(self, action: impl Fn() + 'static) -> AnyView {
        let mut node = self.into_node();
        node.style.on_window_close = Some(Action::new(action));
        AnyView(node)
    }

    /// Register a callback when the native window loses activation.
    fn on_window_blur(self, action: impl Fn() + 'static) -> AnyView {
        let mut node = self.into_node();
        node.style.on_window_blur = Some(Action::new(action));
        AnyView(node)
    }

    /// Bind a persistent code document, bypassing whole-string text bindings.
    fn document(self, document: EditorDocument) -> AnyView {
        let mut base = self.into_node();
        base.style.multiline = true;
        let mut node = ViewNode::new(ViewKind::Reactive(ReactiveView::new(
            "omen::CodeDocument",
            move |_| {
                let _ = document.state.get();
                let mut node = base.clone();
                node.style.code_document = Some(document.clone());
                Ok(node)
            },
        )));
        node.style.multiline = true;
        AnyView(node)
    }

    /// Open a native file or folder chooser when activated (GPUI desktop).
    /// The picker replaces the control's ordinary click action.
    fn file_picker(self, picker: FilePicker) -> AnyView {
        let mut node = self.into_node();
        node.style.file_picker = Some(picker);
        AnyView(node)
    }

    /// Present a separate GPUI desktop window while the binding is true.
    /// Content owns its environment and may configure its initial desktop_window.
    /// Native close writes false; reopening creates a fresh view lifetime.
    fn window<V: View>(
        self,
        key: impl Into<String>,
        visible: impl Into<Binding<bool>>,
        content: impl Fn() -> V + 'static,
    ) -> AnyView {
        let base = self.into_node();
        let key = key.into();
        let visible = visible.into();
        let content = std::rc::Rc::new(content);
        let metadata = base.style.desktop_window.clone();
        let environment = base.style.environment.clone();
        let mut node = ViewNode::new(ViewKind::Reactive(ReactiveView::new(
            "omen::DesktopScene",
            move |_| {
                let mut body = AnyView::group(vec![base.clone()]).into_node();
                if visible.get() {
                    let content = content.clone();
                    body.style.desktop_scenes.push(DesktopScene::new(
                        key.clone(),
                        visible.clone(),
                        move || content().into_node(),
                    ));
                }
                Ok(body)
            },
        )));
        node.style.desktop_window = metadata;
        node.style.environment = environment;
        AnyView(node)
    }

    /// Configure the initial desktop window without backend-specific application code.
    fn desktop_window(self, window: DesktopWindow) -> AnyView {
        let mut node = self.into_node();
        if !cfg!(any(
            target_arch = "wasm32",
            target_os = "ios",
            target_os = "android"
        )) && let Some(toolbar) = window.toolbar.clone()
        {
            let height = toolbar.resolved_height();
            let environment = node.style.environment.clone();
            let theme = node.style.theme;
            let locale = node.style.locale.clone();
            let direction = node.style.direction;
            let mut host = ViewNode::new(ViewKind::Reactive(ReactiveView::new(
                "omen::DesktopToolbar",
                move |_| Ok(toolbar.content()),
            )));
            host.style.key = Some("omen-native-toolbar".into());
            host.style.desktop_toolbar = Some(height);
            node.style.fill = true;
            let mut shell = Layout::new()
                .child(AnyView(host))
                .child(AnyView(node))
                .frame(f32::INFINITY, f32::INFINITY)
                .into_node();
            shell.style.environment = environment;
            shell.style.theme = theme;
            shell.style.locale = locale;
            shell.style.direction = direction;
            node = shell;
        }
        node.style.desktop_window = Some(window);
        AnyView(node)
    }
    /// Handle a GPUI-compatible key chord while a descendant has focus.
    fn shortcut(self, chord: impl Into<String>, action: impl Fn() + 'static) -> AnyView {
        let mut node = self.into_node();
        node.style
            .shortcuts
            .push((chord.into(), Action::new(action)));
        AnyView(node)
    }

    /// Render fixed-height List rows on demand on the native backend.
    fn virtualized(self, row_height: f32) -> AnyView {
        let mut node = self.into_node();
        node.style.virtual_row_height = Some(if row_height.is_finite() {
            row_height.max(1.)
        } else {
            28.
        });
        AnyView(node)
    }
    /// Turn a narrow view into a keyboard and pointer panel resize handle.
    fn resize_handle(
        self,
        width: impl Into<Binding<f32>>,
        min: f32,
        max: f32,
        reverse: bool,
    ) -> AnyView {
        let mut node = self.into_node();
        node.style.resize_handle = Some(ResizeHandle {
            width: width.into(),
            min,
            max,
            reverse,
        });
        node.style.focusable = true;
        AnyView(node)
    }
    /// Handle secondary-click using the existing Omen presentation system.
    fn on_context_menu(self, action: impl Fn() + 'static) -> AnyView {
        let mut node = self.into_node();
        node.style.on_context_menu = Some(Action::new(action));
        AnyView(node)
    }

    /// Consume remaining space along the parent layout axis, allowing shrinkage.
    fn fill(self) -> AnyView {
        let mut node = self.into_node();
        node.style.fill = true;
        AnyView(node)
    }
    /// Truncate overflowing text with an ellipsis (...) instead of wrapping onto multiple lines.
    fn truncate(self) -> AnyView {
        let mut node = self.into_node();
        node.style.truncate = true;
        AnyView(node)
    }
    /// Make this view an independent vertical scroll region.
    /// Give it a bounded frame or use fill inside a bounded parent.
    fn scrollable(self) -> AnyView {
        let mut node = self.into_node();
        node.style.scrollable = true;
        AnyView(node)
    }

    /// Scroll through output, following new content when already at the end.
    fn follow_scroll_end(self) -> AnyView {
        let mut node = self.into_node();
        node.style.scrollable = true;
        node.style.follow_scroll_end = true;
        AnyView(node)
    }

    /// Invoke an action after a stationary long press on supported pointer hosts.
    fn on_long_press(self, action: impl Fn() + 'static) -> AnyView {
        let mut node = self.into_node();
        node.style.on_long_press = Some(Action::new(action));
        AnyView(node)
    }
    /// Invoke an action on a physical swipe direction on supported pointer hosts.
    fn on_swipe(self, direction: SwipeDirection, action: impl Fn() + 'static) -> AnyView {
        let mut node = self.into_node();
        node.style.on_swipe.push((direction, Action::new(action)));
        AnyView(node)
    }
    /// Set an inherited theme or color scheme.
    fn theme(self, theme: impl Into<Theme>) -> AnyView {
        let mut node = self.into_node();
        node.style.theme = Some(theme.into());
        AnyView(node)
    }
    /// Set inherited locale and its default layout direction.
    fn locale(self, locale: Locale) -> AnyView {
        let mut node = self.into_node();
        node.style.locale = Some(locale);
        AnyView(node)
    }
    /// Override logical leading/trailing layout direction.
    fn direction(self, direction: LayoutDirection) -> AnyView {
        let mut node = self.into_node();
        node.style.direction = Some(direction);
        AnyView(node)
    }
    /// Arrange children using an adaptive direction based on container width.
    fn adaptive(self, direction: AdaptiveDirection) -> AnyView {
        let mut node = self.into_node();
        node.style.adaptive_direction = Some(direction);
        AnyView(node)
    }
    /// Set the minimum logical cell width of an adaptive grid.
    fn min_item_width(self, width: impl Into<f64>) -> AnyView {
        let mut node = self.into_node();
        node.style.min_item_width = Some((width.into() as f32).max(1.));
        AnyView(node)
    }
    /// Apply overrides at or above a viewport breakpoint.
    fn responsive(self, breakpoint: Breakpoint, style: ResponsiveStyle) -> AnyView {
        let mut node = self.into_node();
        node.style.responsive.push((breakpoint, style));
        AnyView(node)
    }
    /// Consume safe-area insets at the selected edges.
    fn safe_area(self, edges: SafeArea) -> AnyView {
        let mut node = self.into_node();
        node.style.safe_area = Some(edges);
        AnyView(node)
    }
    /// Override safe-area insets for an embedded surface.
    fn safe_area_insets(self, insets: Insets) -> AnyView {
        let mut node = self.into_node();
        node.style.safe_area_insets = Some(insets);
        AnyView(node)
    }
    /// Resolve padding from inherited design tokens.
    fn padding_token(self, token: Space) -> AnyView {
        let mut node = self.into_node();
        node.style.padding_token = Some(token);
        node.style.padding = None;
        AnyView(node)
    }
    /// Resolve spacing from inherited design tokens.
    fn spacing_token(self, token: Space) -> AnyView {
        let mut node = self.into_node();
        node.style.spacing_token = Some(token);
        node.style.spacing = None;
        AnyView(node)
    }
    /// Mark a tab selector's selected state for assistive technology.
    fn accessibility_selected(self, selected: bool) -> AnyView {
        let mut node = self.into_node();
        node.style.accessibility.selected = Some(selected);
        AnyView(node)
    }
    /// Supply a title to the enclosing navigation stack.
    fn navigation_title(self, title: impl Into<String>) -> AnyView {
        let mut node = self.into_node();
        node.style.navigation_title = Some(title.into());
        AnyView(node)
    }
    /// Control whether the enclosing navigation stack displays its app bar.
    fn app_bar_visible(self, visible: bool) -> AnyView {
        let mut node = self.into_node();
        std::rc::Rc::make_mut(node.style.app_bar.get_or_insert_with(Default::default)).hidden =
            !visible;
        AnyView(node)
    }
    /// Control whether a pushed screen displays the automatic Back button.
    fn show_back_button(self, visible: bool) -> AnyView {
        let mut node = self.into_node();
        std::rc::Rc::make_mut(node.style.app_bar.get_or_insert_with(Default::default))
            .back_hidden = !visible;
        AnyView(node)
    }
    /// Override the default alignment of the current screen’s app bar title.
    fn center_title(self, centered: bool) -> AnyView {
        let mut node = self.into_node();
        std::rc::Rc::make_mut(node.style.app_bar.get_or_insert_with(Default::default))
            .center_title = Some(centered);
        AnyView(node)
    }
    /// Set the fill of the enclosing navigation stack’s app bar.
    fn app_bar_background(self, background: impl Into<Background>) -> AnyView {
        let mut node = self.into_node();
        std::rc::Rc::make_mut(node.style.app_bar.get_or_insert_with(Default::default)).background =
            Some(background.into());
        AnyView(node)
    }
    /// Set the text and icon color of the enclosing navigation stack’s app bar.
    fn app_bar_color(self, foreground: impl Into<Color>) -> AnyView {
        let mut node = self.into_node();
        std::rc::Rc::make_mut(node.style.app_bar.get_or_insert_with(Default::default)).foreground =
            Some(foreground.into());
        AnyView(node)
    }
    /// Present a bottom-aligned blocking panel while the binding is true.
    fn sheet<V: View>(
        self,
        visible: impl Into<Binding<bool>>,
        content: impl Fn() -> V + 'static,
    ) -> AnyView {
        navigation::present(
            self.into_node(),
            visible.into(),
            PresentationKind::Sheet,
            content,
        )
    }
    /// Present a blocking dialog while the binding is true.
    fn dialog<V: View>(
        self,
        visible: impl Into<Binding<bool>>,
        content: impl Fn() -> V + 'static,
    ) -> AnyView {
        navigation::present(
            self.into_node(),
            visible.into(),
            PresentationKind::Dialog,
            content,
        )
    }
    /// Present a nonblocking anchored panel while the binding is true.
    fn popover<V: View>(
        self,
        visible: impl Into<Binding<bool>>,
        content: impl Fn() -> V + 'static,
    ) -> AnyView {
        navigation::present(
            self.into_node(),
            visible.into(),
            PresentationKind::Popover,
            content,
        )
    }
    /// Present a context menu anchored to the pointer when it opens.
    fn context_menu<V: View>(
        self,
        visible: impl Into<Binding<bool>>,
        content: impl Fn() -> V + 'static,
    ) -> AnyView {
        navigation::present(
            self.into_node(),
            visible.into(),
            PresentationKind::ContextMenu,
            content,
        )
    }
    /// Set the background shown while hovering a control.
    fn hover_background(self, color: impl Into<Color>) -> AnyView {
        let mut node = self.into_node();
        node.style.hover_background = Some(color.into());
        AnyView(node)
    }
    /// Perform a native desktop operation when this control is activated.
    fn desktop_action(self, action: DesktopAction) -> AnyView {
        let mut node = self.into_node();
        node.style.desktop_action = Some(action);
        AnyView(node)
    }
    /// Present a blocking centered modal while the binding is true.
    fn modal<V: View>(
        self,
        visible: impl Into<Binding<bool>>,
        content: impl Fn() -> V + 'static,
    ) -> AnyView {
        navigation::present(
            self.into_node(),
            visible.into(),
            PresentationKind::Modal,
            content,
        )
    }
    /// Animate opacity and offset changes.
    fn animation(self, animation: Animation) -> AnyView {
        let mut node = self.into_node();
        node.style.animation = Some(animation);
        AnyView(node)
    }
    /// Apply an entry transition when a native node mounts.
    fn transition(self, transition: Transition) -> AnyView {
        let mut node = self.into_node();
        node.style.transition = Some(transition);
        AnyView(node)
    }
    /// Target opacity, clamped to zero through one.
    fn opacity(self, opacity: f32) -> AnyView {
        let mut node = self.into_node();
        node.style.opacity = Some(opacity.clamp(0., 1.));
        AnyView(node)
    }
    /// Target translation in logical pixels.
    fn offset(self, x: f32, y: f32) -> AnyView {
        let mut node = self.into_node();
        node.style.offset = Some((x, y));
        AnyView(node)
    }
    /// Honor an application accessibility preference by disabling motion.
    fn reduced_motion(self, reduced: bool) -> AnyView {
        let mut node = self.into_node();
        node.style.reduced_motion = Some(reduced);
        AnyView(node)
    }
    /// Give this element a name for design tools such as the Studio view tree.
    fn name(self, name: impl Into<String>) -> AnyView {
        let mut node = self.into_node();
        node.style.name = Some(name.into());
        AnyView(node)
    }
    /// Set explicit stable identity within a parent or list.
    fn key(self, key: impl ToString) -> AnyView {
        let mut node = self.into_node();
        node.style.key = Some(key.to_string());
        AnyView(node)
    }
    /// Compiler-assigned structural identity.
    #[doc(hidden)]
    fn identity_site(self, site: &'static str) -> AnyView {
        let mut node = self.into_node();
        node.style.source_site = Some(site);
        AnyView(node)
    }
    /// Add an observable model to descendants' typed environment.
    fn environment<T: 'static>(self, value: impl EnvironmentSource<Model = T>) -> AnyView {
        let mut node = self.into_node();
        node.style.environment = Some(
            node.style
                .environment
                .unwrap_or_default()
                .with(value.into_environment()),
        );
        AnyView(node)
    }
    /// Override the formatting context for this view subtree.
    fn format_context(self, context: impl Into<omen_format::FormatContext>) -> AnyView {
        self.environment(omen_core::Value::new(context.into()))
    }
    /// Format text content according to the active formatting context and options.
    fn format(self, option: impl Into<omen_format::FormatOption>) -> AnyView {
        let base = self.into_node();
        let option = option.into();
        let fallback_locale = base.style.locale.clone();
        let base_style = base.style.clone();
        let mut node = ViewNode::new(ViewKind::Reactive(ReactiveView::new(
            "omen::FormattedText",
            move |scope| {
                let context = if let Ok(ctx_val) = scope.environment::<omen_format::FormatContext>() {
                    ctx_val.get()
                } else if let Ok(loc_val) = scope.environment::<Locale>() {
                    let loc = loc_val.get();
                    omen_format::FormatContext::new(omen_format::LocaleId::new(&loc.id))
                } else if let Some(ref loc) = fallback_locale {
                    omen_format::FormatContext::new(omen_format::LocaleId::new(&loc.id))
                } else {
                    omen_format::FormatContext::default()
                };

                let mut result_node = base.clone();
                match &base.kind {
                    ViewKind::Text(raw) => {
                        let formatted = omen_format::format::format_dynamic(raw, &option, &context)
                            .unwrap_or_else(|_| raw.clone());
                        result_node.kind = ViewKind::Text(formatted);
                    }
                    _ => {}
                }
                Ok(result_node)
            },
        )));
        node.style = base_style;
        AnyView(node)
    }
    /// Run when the view mounts or reappears.
    fn on_appear(self, action: impl Fn() + 'static) -> AnyView {
        let mut node = self.into_node();
        node.style.on_appear = Some(Action::new(action));
        AnyView(node)
    }
    /// Run when the view disappears or its window closes.
    fn on_disappear(self, action: impl Fn() + 'static) -> AnyView {
        let mut node = self.into_node();
        node.style.on_disappear = Some(Action::new(action));
        AnyView(node)
    }
    /// Activate a view by pointer, keyboard, or assistive technology.
    fn on_tap(self, action: impl Fn() + 'static) -> AnyView {
        let mut node = self.into_node();
        node.style.on_tap = Some(Action::new(action));
        AnyView(node)
    }
    /// Run a cleanup-producing effect when its dependency changes.
    fn effect(
        self,
        key: impl std::fmt::Debug + PartialEq + 'static,
        setup: impl Fn() -> Action + 'static,
    ) -> AnyView {
        let mut node = self.into_node();
        node.style.effect = Some(Effect::new(key, setup));
        AnyView(node)
    }
    /// Run asynchronous work on activation; a newer invocation cancels the old one.
    fn on_tap_async<F, Fut>(self, factory: F) -> AnyView
    where
        F: Fn() -> Fut + 'static,
        Fut: std::future::Future + 'static,
        Fut::Output: TaskOutput,
    {
        let mut node = self.into_node();
        node.style.async_action = Some(AsyncAction::new(factory));
        AnyView(node)
    }
    /// Run asynchronous work for the mounted lifetime of this view.
    fn task<F, Fut>(self, factory: F) -> AnyView
    where
        F: Fn() -> Fut + 'static,
        Fut: std::future::Future + 'static,
        Fut::Output: TaskOutput,
    {
        let mut node = self.into_node();
        node.style.mount_task = Some(AsyncAction::new(factory));
        AnyView(node)
    }
    /// Handle a failed asynchronous action or mount task.
    fn on_error(self, handler: impl Fn(String) + 'static) -> AnyView {
        let mut node = self.into_node();
        node.style.on_error = Some(ErrorAction::new(handler));
        AnyView(node)
    }
    /// Receive the most recently started task for explicit cancellation/status.
    fn task_handle(self, binding: Value<Option<TaskHandle>>) -> AnyView {
        let mut node = self.into_node();
        node.style.task_handle = Some(binding);
        AnyView(node)
    }
    /// Set explicit dimensions in logical pixels.
    ///
    /// Positive infinity fills the parent axis. The DSL spells a full-width
    /// button as `.frame(.infinity, 44)`; Rust uses `f64::INFINITY`. A frame
    /// overrides the content-sized default of a button. Both dimensions are required.
    fn frame(self, width: impl Into<f64>, height: impl Into<f64>) -> AnyView {
        let mut node = self.into_node();
        node.style.frame = Some(Frame {
            width: width.into() as f32,
            height: height.into() as f32,
        });
        AnyView(node)
    }
    /// Pin this child to a logical corner of its parent layout.
    fn pin(self, corner: PinCorner, x: u16, y: u16) -> AnyView {
        let mut node = self.into_node();
        node.style.pin = Some(Pin { corner, x, y });
        AnyView(node)
    }

    /// Align content within a view or container.
    fn alignment(self, value: Alignment) -> AnyView {
        let mut node = self.into_node();
        node.style.alignment = Some(value);
        AnyView(node)
    }
    /// Apply a linear-gradient background.
    fn gradient(
        self,
        from: impl Into<Color>,
        to: impl Into<Color>,
        angle: impl Into<f64>,
    ) -> AnyView {
        self.background(LinearGradient::new(
            from.into(),
            to.into(),
            angle.into() as f32,
        ))
    }
    /// Override font size in logical pixels.
    fn font_size(self, value: impl Into<f64>) -> AnyView {
        let mut node = self.into_node();
        node.style.font_size = Some(value.into() as f32);
        AnyView(node)
    }
    /// Override line height in logical pixels.
    fn line_height(self, value: impl Into<f64>) -> AnyView {
        let mut node = self.into_node();
        node.style.line_height = Some(value.into() as f32);
        AnyView(node)
    }
    /// Override font family.
    fn font_family(self, value: impl Into<String>) -> AnyView {
        let mut node = self.into_node();
        node.style.font_family = Some(value.into());
        AnyView(node)
    }
    /// Supply a text field's shared value.
    fn value(self, value: impl Into<Binding<String>>) -> AnyView {
        let mut node = self.into_node();
        let binding = value.into();
        let _ = binding.get();
        node.style.text_value = Some(binding);
        AnyView(node)
    }
    /// Supply a checkbox or toggle's shared value.
    fn checked(self, value: impl Into<Binding<bool>>) -> AnyView {
        let mut node = self.into_node();
        let binding = value.into();
        let _ = binding.get();
        node.style.checked = Some(binding);
        AnyView(node)
    }
    /// Invoke a callback after a native control changes its value.
    fn on_change(self, callback: impl Fn() + 'static) -> AnyView {
        let mut node = self.into_node();
        node.style.on_change = Some(Action::new(callback));
        AnyView(node)
    }
    /// Override the accessible name.
    fn accessibility_label(self, value: impl Into<String>) -> AnyView {
        let mut node = self.into_node();
        node.style.accessibility.label = Some(value.into());
        AnyView(node)
    }
    /// Add an accessible description.
    fn accessibility_hint(self, value: impl Into<String>) -> AnyView {
        let mut node = self.into_node();
        node.style.accessibility.hint = Some(value.into());
        AnyView(node)
    }
    /// Override the semantic role.
    fn accessibility_role(self, value: AccessibilityRole) -> AnyView {
        let mut node = self.into_node();
        node.style.accessibility.role = Some(value);
        AnyView(node)
    }
    /// Focus an editable control once when it appears.
    fn autofocus(self, enabled: bool) -> AnyView {
        let mut node = self.into_node();
        node.style.autofocus = enabled;
        AnyView(node)
    }
    /// Focus this container when the binding becomes true, then clear the request.
    fn focus_request(self, request: impl Into<Binding<bool>>) -> AnyView {
        let mut node = self.into_node();
        let request = request.into();
        let _ = request.get();
        node.style.focus_request = Some(request);
        AnyView(node)
    }
    /// Keep Tab navigation within a group of controls.
    fn focus_scope(self, enabled: bool) -> AnyView {
        let mut node = self.into_node();
        node.style.focus_scope = enabled;
        AnyView(node)
    }
    /// Traverse this focus scope with Up/Down in addition to Tab/Shift-Tab.
    fn focus_arrows(self, enabled: bool) -> AnyView {
        let mut node = self.into_node();
        node.style.focus_arrows = enabled;
        AnyView(node)
    }
    /// Include a non-control view in sequential keyboard focus.
    fn focusable(self, value: bool) -> AnyView {
        let mut node = self.into_node();
        node.style.focusable = value;
        AnyView(node)
    }
    /// Set a Layout container's child arrangement.
    fn mode(self, value: LayoutMode) -> AnyView {
        let mut node = self.into_node();
        node.style.mode = Some(value);
        node.style.adaptive_direction = None;
        AnyView(node)
    }
    /// Set a semantic font size.
    fn font(self, value: Font) -> AnyView {
        let mut node = self.into_node();
        node.style.font = Some(value);
        AnyView(node)
    }
    /// Set font weight.
    fn weight(self, value: Weight) -> AnyView {
        let mut node = self.into_node();
        node.style.weight = Some(value);
        AnyView(node)
    }
    /// Set foreground color.
    fn color(self, value: impl Into<Color>) -> AnyView {
        let mut node = self.into_node();
        node.style.color = Some(value.into());
        AnyView(node)
    }
    /// Set text color using a theme token or hex string.
    fn text_color(self, value: impl Into<Color>) -> AnyView {
        self.color(value)
    }
    /// Set background color.
    fn background(self, value: impl Into<Background>) -> AnyView {
        let mut node = self.into_node();
        node.style.background = Some(value.into());
        AnyView(node)
    }
    /// Set uniform padding in logical pixels.
    fn padding(self, value: impl Into<f64>) -> AnyView {
        let mut node = self.into_node();
        node.style.padding_token = None;
        node.style.padding = Some(value.into() as f32);
        AnyView(node)
    }
    /// Set container gap in logical pixels.
    fn spacing(self, value: impl Into<f64>) -> AnyView {
        let mut node = self.into_node();
        node.style.spacing_token = None;
        node.style.spacing = Some(value.into() as f32);
        AnyView(node)
    }
    /// Set corner radius in logical pixels.
    fn corner_radius(self, value: impl Into<f64>) -> AnyView {
        let mut node = self.into_node();
        node.style.corner_radius = Some(value.into() as f32);
        AnyView(node)
    }
}
impl<T: View> ViewExt for T {}

mod controls;
pub use controls::*;
mod navigation;
pub use navigation::*;

/// Wait without blocking the desktop UI thread.
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
pub async fn sleep(duration: std::time::Duration) {
    omen_gpui::sleep(duration).await;
}

/// Typed native plugin traits, runtime, permissions, and bridge interfaces.
pub use omen_plugin as plugins;

/// Wait without blocking the browser event loop.
#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub async fn sleep(duration: std::time::Duration) {
    omen_web::sleep(duration).await;
}

/// In-place Boolean mutation used by concise action blocks.
pub trait BoolExt {
    /// Invert this value.
    fn toggle(&mut self);
}
impl BoolExt for bool {
    fn toggle(&mut self) {
        *self = !*self;
    }
}
