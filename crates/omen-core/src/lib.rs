//! Renderer-independent, owned view trees for Omen.
#![deny(missing_docs)]

use std::{fmt, rc::Rc};

mod editor;
pub use editor::*;
mod desktop_toolbar;
pub use desktop_toolbar::*;
mod desktop_scene;
pub use desktop_scene::*;
mod file_picker;
pub use file_picker::*;
mod design;
mod localization;
mod motion;
mod navigation;
pub use design::*;
pub use localization::*;
pub use motion::*;
pub use navigation::*;
mod platform;
pub use platform::*;
mod appearance;
mod controls;
mod observation;
mod reactive;
mod tasks;
pub use appearance::*;
pub use controls::*;
pub use reactive::*;
pub use tasks::*;

/// Converts a public view builder into a renderer-independent node.
pub trait View {
    /// Consume this view, preserving its children, style, and callbacks.
    fn into_node(self) -> ViewNode;
}

/// A native desktop operation attached to an activatable Omen control.
#[derive(Clone, Debug, PartialEq)]
pub enum DesktopAction {
    /// Write text to the operating system clipboard.
    CopyText(String),
    /// Reveal a file or folder in the system file manager.
    RevealPath(std::path::PathBuf),
}

/// A reusable, main-thread UI callback. Captures must be owned and `'static`.
#[derive(Clone)]
pub struct Action(Rc<dyn Fn()>);

impl Action {
    /// Store a callback without invoking it during view construction.
    pub fn new(callback: impl Fn() + 'static) -> Self {
        Self(Rc::new(callback))
    }

    /// Invoke the callback in response to an event.
    pub fn invoke(&self) {
        (self.0)();
    }
}

impl PartialEq for Action {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Action(..)")
    }
}

/// Child arrangement for a Layout container.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LayoutMode {
    /// Arrange children in a horizontal row.
    Horizontal,
    /// Arrange children in a vertical column (the default).
    #[default]
    Vertical,
    /// Overlay children; later children paint above earlier children.
    Stack,
}

/// Semantic text sizes, resolved by the rendering backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    /// Small supporting text.
    Caption,
    /// Compact supporting text.
    Footnote,
    /// Emphasized body text.
    Headline,
    /// Secondary heading.
    Title2,
    /// Default body text.
    Body,
    /// Section heading.
    Title,
    /// Large page heading.
    LargeTitle,
}

/// Semantic font weights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    /// Thin weight.
    Thin,
    /// Light weight.
    Light,
    /// Semibold weight.
    Semibold,
    /// Heavy weight.
    Heavy,
    /// Normal weight.
    Regular,
    /// Medium weight.
    Medium,
    /// Bold weight.
    Bold,
}

/// Initial semantic color palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    /// Theme main text.
    Primary,
    /// Theme window background.
    Canvas,
    /// Theme raised surface.
    Surface,
    /// Theme accent.
    Accent,
    /// Theme border or separator.
    Border,
    /// Red accent.
    Red,
    /// Orange accent.
    Orange,
    /// Purple accent.
    Purple,
    /// Neutral gray.
    Gray,
    /// Raised panel surface.
    Panel,
    /// An explicit 24-bit RGB color.
    Rgb(u32),
    /// A 32-bit RGBA color, with alpha in the low byte.
    Rgba(u32),
    /// White foreground.
    White,
    /// Black foreground.
    Black,
    /// Blue accent.
    Blue,
    /// Muted foreground.
    Secondary,
    /// Green success accent.
    Green,
    /// Dark canvas.
    Dark,
}

/// Logical corner of a parent layout used to position a pinned child.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinCorner {
    /// Top and leading.
    TopLeading,
    /// Top and trailing.
    TopTrailing,
    /// Bottom and leading.
    BottomLeading,
    /// Bottom and trailing.
    BottomTrailing,
}

impl PinCorner {
    /// Whether this corner uses the trailing and bottom edges.
    pub fn edges(self) -> (bool, bool) {
        (
            matches!(self, Self::TopTrailing | Self::BottomTrailing),
            matches!(self, Self::BottomLeading | Self::BottomTrailing),
        )
    }
}

/// Insets in logical pixels measured inward from a parent layout corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pin {
    /// Parent corner.
    pub corner: PinCorner,
    /// Horizontal inset.
    pub x: u16,
    /// Vertical inset.
    pub y: u16,
}

/// A semantic drop position in a declarative flow layout.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowPlacement {
    /// Insert before the target.
    Before,
    /// Insert after the target.
    After,
    /// Append inside a compatible container.
    Into,
    /// Reposition a pinned child within its parent.
    Pinned(Pin),
}

/// A mapped component in one versioned preview frame.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewPlacement {
    /// Document revision and frame identity.
    pub scope: String,
    /// Source byte range containing this component and its descendants.
    pub source: std::ops::Range<usize>,
    /// Human-readable drag label.
    pub label: String,
    /// Whether the component can be moved from its parent.
    pub draggable: bool,
    /// Axis of the target's parent, when sibling placement is supported.
    pub axis: Option<LayoutMode>,
    /// Whether the target can accept a child.
    pub container: bool,
    /// Receives the dragged source offset and the resolved placement.
    pub drop_from: Binding<(usize, FlowPlacement)>,
}

/// A component-library item that can be dropped into a preview.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewInsert {
    /// Document revision and target frame.
    pub scope: String,
    /// Component label for the drag ghost.
    pub label: String,
    /// Dismiss the component chooser after the native drag begins.
    pub begin: Action,
    /// Receives the destination source offset and resolved placement.
    pub insert: Binding<(usize, FlowPlacement)>,
}

/// An explicit two-axis component size editable from the preview.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewResize {
    /// Original logical dimensions and the commit callback.
    pub size: Binding<[f32; 2]>,
}

/// A frame position binding and the scale of its Studio canvas.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewFrameDrag {
    /// Select the frame when its header is clicked.
    pub select: Action,
    /// Canvas position in unscaled logical units.
    pub position: Binding<[i32; 2]>,
    /// Display pixels per canvas unit.
    pub scale: f32,
}

/// Pointer-driven navigation on a Studio canvas.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PreviewGesture {
    /// Move the artwork by this many screen pixels.
    Pan([f32; 2]),
    /// Scale around a point relative to the canvas viewport.
    Zoom {
        /// Relative scale multiplier.
        factor: f32,
        /// Anchor in viewport pixels.
        anchor: [f32; 2],
    },
}

/// Decorative enclosure around a Studio viewport. Insets are in canvas logical pixels.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewShell {
    IPhone,
    AndroidPhone,
    IPad,
    AndroidTablet,
    Mac,
    Windows,
    Linux,
    MobileBrowser,
    TabletBrowser,
    DesktopBrowser,
    Custom,
}

impl PreviewShell {
    /// Side, top, and bottom chrome outside the editable viewport.
    pub fn insets(self) -> [f32; 3] {
        match self {
            Self::IPhone => [9., 29., 20.],
            Self::AndroidPhone => [7., 24., 16.],
            Self::IPad => [16., 24., 16.],
            Self::AndroidTablet => [12., 20., 12.],
            Self::MobileBrowser => [8., 39., 18.],
            Self::TabletBrowser => [13., 42., 13.],
            Self::DesktopBrowser => [2., 38., 2.],
            Self::Mac | Self::Windows | Self::Linux => [2., 29., 2.],
            Self::Custom => [2., 2., 2.],
        }
    }
}

/// Renderer metadata for Studio previews.
#[doc(hidden)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PreviewStyle {
    /// Device enclosure and its light/dark window appearance.
    pub shell: Option<(PreviewShell, bool)>,
    /// Dot color for the scrollable design surface.
    pub grid: Option<Color>,
    /// World origin and scale of the canvas grid, in viewport pixels.
    pub grid_transform: Option<[f32; 3]>,
    /// Receives the canvas viewport dimensions after native layout.
    pub viewport_size: Option<Binding<[f32; 2]>>,
    /// Receives native pan and zoom gestures on a clipped preview surface.
    pub navigation: Option<Binding<PreviewGesture>>,
    /// Local layout width for responsive modifiers in a native preview subtree.
    pub width: Option<f32>,
    /// Pixel scale for a native preview subtree, independent of layout breakpoints.
    pub scale: Option<f32>,
    /// Position a frame in Studio's scrollable canvas.
    pub position: Option<[f32; 2]>,
    /// Keep floating design controls above the canvas and receive their own pointer events.
    pub floating_panel: bool,
    /// Present Studio menu content at window scale, with its own padding and overflow.
    pub menu_panel: bool,
    /// Native design drag source and drop target.
    pub placement: Option<PreviewPlacement>,
    /// Drag source for a component-library row.
    pub insert: Option<PreviewInsert>,
    /// Resizing is available for a selected component with a fixed frame.
    pub resize: Option<PreviewResize>,
    /// Native drag handle for repositioning a preview frame.
    pub frame_drag: Option<PreviewFrameDrag>,
    /// Draw the native preview's safe-area guides.
    pub safe_area: bool,
    /// Nesting depth of a Layout whose type and bounds are visible in Design.
    pub layout_depth: Option<usize>,
    /// Show this flow container’s alignment guides during insertion.
    pub layout_guide: bool,
    /// Highlight the insertion edge of this child, or the first slot of an empty layout.
    pub insertion_edge: Option<FlowPlacement>,
    /// Native design surface outline color.
    pub outline: Option<Color>,
    /// Show the design outline persistently when selected.
    pub selected: bool,
}
/// Explicit style overrides. Unset values inherit the backend's defaults.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Style {
    /// Human-readable name for design and inspection tools.
    pub name: Option<String>,
    /// Native design canvas and preview configuration.
    #[doc(hidden)]
    pub preview: Option<Rc<PreviewStyle>>,
    /// Persistent code-editor document and language-service bridge.
    pub code_document: Option<EditorDocument>,
    /// Native title-bar toolbar host, installed by desktop_window.
    #[doc(hidden)]
    pub desktop_toolbar: Option<f32>,
    /// Independent native desktop windows declared by this node.
    pub desktop_scenes: Vec<DesktopScene>,
    /// Native path prompt opened when this control is activated.
    pub file_picker: Option<FilePicker>,
    /// Optional native desktop operation performed when a control is activated.
    pub desktop_action: Option<DesktopAction>,
    /// Background applied while the pointer is over this control.
    pub hover_background: Option<Color>,
    /// Minimum width in logical pixels.
    pub min_width: Option<f32>,
    /// Independently scroll overflowing content horizontally.
    pub horizontal_scrollable: bool,
    /// Root callback invoked on native window teardown, independently of visibility.
    pub on_window_close: Option<Action>,
    /// Native callback when the window loses activation.
    pub on_window_blur: Option<Action>,
    /// Initial desktop window configuration on the application root.
    pub desktop_window: Option<DesktopWindow>,
    /// Portable key chord and callback pairs handled within this subtree.
    pub shortcuts: Vec<(String, Action)>,
    /// Native multiline editing on TextField nodes (used by TextEditor).
    pub multiline: bool,
    /// Fixed-height native List rows rendered only in the visible range.
    pub virtual_row_height: Option<f32>,
    /// Native pointer-resizing handle configuration.
    pub resize_handle: Option<ResizeHandle>,
    /// Secondary-click action, for context presentations.
    pub on_context_menu: Option<Action>,
    /// Grow to consume remaining space along the parent layout axis.
    pub fill: bool,
    /// Truncate overflowing text with an ellipsis (...) instead of wrapping onto multiple lines.
    pub truncate: bool,
    /// Clip and independently scroll overflowing content vertically.
    pub scrollable: bool,
    /// Keep a native vertical scroll region at the end while it is scrolled to the end.
    pub follow_scroll_end: bool,
    /// Long-press action for pointer/touch hosts.
    pub on_long_press: Option<Action>,
    /// Physical directional swipe actions.
    pub on_swipe: Vec<(SwipeDirection, Action)>,
    /// Reconcile keyed child lifetimes in internal navigation containers.
    #[doc(hidden)]
    pub keyed_children: bool,
    /// Theme override inherited by descendants.
    pub theme: Option<Theme>,
    /// Locale metadata inherited by descendants.
    pub locale: Option<Locale>,
    /// Explicit logical layout direction override.
    pub direction: Option<LayoutDirection>,
    /// Responsive overrides keyed by minimum viewport width.
    pub responsive: Vec<(Breakpoint, ResponsiveStyle)>,
    /// Layout direction resolved from local content width.
    pub adaptive_direction: Option<AdaptiveDirection>,
    /// Minimum cell width for a wrapping grid.
    pub min_item_width: Option<f32>,
    /// Edges that consume the current safe-area insets.
    pub safe_area: Option<SafeArea>,
    /// Explicit insets for an embedded surface.
    pub safe_area_insets: Option<Insets>,
    /// Padding from the inherited design-token scale.
    pub padding_token: Option<Space>,
    /// Spacing from the inherited design-token scale.
    pub spacing_token: Option<Space>,
    /// Stable presentation host type, including while its content is closed.
    #[doc(hidden)]
    pub presentation_kind: Option<PresentationKind>,
    /// Internal navigation Back action exposed to native platform hosts.
    #[doc(hidden)]
    pub navigation_back: bool,
    /// Tab metadata consumed by TabView.
    pub tab: Option<TabItem>,
    /// Title consumed by NavigationStack.
    pub navigation_title: Option<String>,
    /// Per-screen appearance and navigation control overrides.
    pub app_bar: Option<Rc<AppBarOptions>>,
    /// Target platform for an automatic Back button.
    #[doc(hidden)]
    pub app_bar_platform: Option<AppBarPlatform>,
    /// Deferred chrome for a retained navigation layout.
    #[doc(hidden)]
    pub navigation_chrome: Option<NavigationChrome>,
    /// Optional style animation timing.
    pub animation: Option<Animation>,
    /// Entry transition.
    pub transition: Option<Transition>,
    /// Target opacity.
    pub opacity: Option<f32>,
    /// Target translation in logical pixels.
    pub offset: Option<(f32, f32)>,
    /// Disable interpolation and entry transitions in this subtree.
    pub reduced_motion: Option<bool>,
    /// Stable explicit identity within a parent.
    pub key: Option<String>,
    /// Compiler-assigned structural source identity.
    #[doc(hidden)]
    pub source_site: Option<&'static str>,
    /// Resolved identity used by native controls.
    #[doc(hidden)]
    pub runtime_id: Option<String>,
    /// Shared typed environment values introduced at this node.
    pub environment: Option<Environment>,
    /// Called when this node becomes visible.
    pub on_appear: Option<Action>,
    /// Called when this node leaves the visible tree.
    pub on_disappear: Option<Action>,
    /// Cleanup-producing effect keyed by a dependency snapshot.
    pub effect: Option<Effect>,
    /// Pointer/keyboard action on a non-button view.
    pub on_tap: Option<Action>,
    /// Asynchronous action run on activation.
    pub async_action: Option<AsyncAction>,
    /// Asynchronous work run for this node's mounted lifetime.
    pub mount_task: Option<AsyncAction>,
    /// Handler for asynchronous failures.
    pub on_error: Option<ErrorAction>,
    /// Runtime task owner, supplied during reconciliation.
    #[doc(hidden)]
    pub task_context: Option<TaskContext>,
    /// Optional binding that receives the latest task handle.
    pub task_handle: Option<Value<Option<TaskHandle>>>,
    /// Explicit dimensions in logical pixels.
    pub frame: Option<Frame>,
    /// Content alignment.
    pub alignment: Option<Alignment>,
    /// Optional out-of-flow positioning relative to the parent layout.
    pub pin: Option<Pin>,
    /// Explicit font size in logical pixels, overriding semantic font.
    pub font_size: Option<f32>,
    /// Explicit line height in logical pixels.
    pub line_height: Option<f32>,
    /// Font family override.
    pub font_family: Option<String>,
    /// Accessible semantic overrides.
    pub accessibility: Accessibility,
    /// Additional keyboard-focus participation for non-control views.
    pub focusable: bool,
    /// Focus an editable native control once when it first becomes active.
    pub autofocus: bool,
    /// One-shot focus request for a native container; consumed when active.
    pub focus_request: Option<Binding<bool>>,
    /// Keep sequential keyboard navigation inside this group.
    pub focus_scope: bool,
    /// Traverse a native focus scope with unmodified Up/Down keys.
    pub focus_arrows: bool,
    /// Shared editable text, when this is a text field.
    pub text_value: Option<Binding<String>>,
    /// Shared checked state, when this is a checkbox or toggle.
    pub checked: Option<Binding<bool>>,
    /// Callback after a control value changes.
    pub on_change: Option<Action>,
    /// Layout child arrangement; unset means vertical.
    pub mode: Option<LayoutMode>,
    /// Text size.
    pub font: Option<Font>,
    /// Text weight.
    pub weight: Option<Weight>,
    /// Foreground color.
    pub color: Option<Color>,
    /// Background color.
    pub background: Option<Background>,
    /// Uniform padding in logical pixels.
    pub padding: Option<f32>,
    /// Container gap in logical pixels.
    pub spacing: Option<f32>,
    /// Corner radius in logical pixels.
    pub corner_radius: Option<f32>,
}

/// Owned node contents, independent from compiler AST types and GPUI.
#[derive(Clone, Debug)]
pub enum ViewKind {
    /// A foreground presentation above its base view.
    Presentation {
        /// Base then presented content.
        children: Vec<ViewNode>,
        /// Placement and interaction style.
        kind: PresentationKind,
        /// Dismissal action, also used for Escape.
        dismiss: Action,
    },
    /// A dependency-tracked named view boundary.
    Reactive(ReactiveView),
    /// Owned text.
    Text(String),
    /// A label and its event callback.
    Button {
        /// Visible button title.
        label: String,
        /// Click callback.
        action: Action,
    },
    /// Raster image from a local filesystem path.
    Image(String),
    /// SVG icon asset name or local SVG path.
    Icon(String),
    /// Single-line editable text with a visible placeholder.
    TextField {
        /// Placeholder and default accessible name.
        label: String,
        /// Initial/shared control value.
        value: TextValue,
    },
    /// Single-line masked text input.
    SecureField {
        /// Placeholder and accessible name.
        label: String,
        /// Shared secret; never included in Debug output.
        value: TextValue,
    },
    /// A checkbox with a text label.
    Checkbox {
        /// Visible label.
        label: String,
        /// Shared checked state.
        value: BoolValue,
    },
    /// An on/off switch with a text label.
    Toggle {
        /// Visible label.
        label: String,
        /// Shared on/off state.
        value: BoolValue,
    },
    /// A padded, rounded panel of child views.
    Card(Vec<ViewNode>),
    /// Determinate completion fraction, clamped to 0 through 1.
    Progress(f32),
    /// A horizontal rule.
    Divider,
    /// Fixed spacing in logical pixels along the parent layout’s axis.
    Gap(f32),
    /// Children arranged according to the node's layout mode.
    Layout(Vec<ViewNode>),
    /// Eager rows with optional stable keyed identity.
    List(Vec<ViewNode>),
    /// Layout-transparent group used by control flow.
    Group(Vec<ViewNode>),
}

/// A constructed UI node with style overrides.
#[derive(Clone, Debug)]
pub struct ViewNode {
    /// Contents and children.
    pub kind: ViewKind,
    /// Explicit styling.
    pub style: Style,
}

impl Style {
    /// Configure an isolated native preview surface.
    #[doc(hidden)]
    pub fn preview_mut(&mut self) -> &mut PreviewStyle {
        Rc::make_mut(self.preview.get_or_insert_with(Rc::default))
    }

    /// Apply matching responsive rules in increasing breakpoint order.
    pub fn for_width(&self, width: f32) -> Self {
        let mut style = self.clone();
        let mut rules = style.responsive.clone();
        rules.sort_by_key(|(breakpoint, _)| *breakpoint);
        for (breakpoint, rule) in rules {
            if width < breakpoint.min_width() {
                continue;
            }
            if let Some(mode) = rule.mode {
                style.mode = Some(mode);
            }
            if let Some(padding) = rule.padding {
                style.padding = Some(padding);
                style.padding_token = None;
            }
            if let Some(spacing) = rule.spacing {
                style.spacing = Some(spacing);
                style.spacing_token = None;
            }
            if let Some(frame) = rule.frame {
                style.frame = Some(frame);
            }
            if let Some(font_size) = rule.font_size {
                style.font_size = Some(font_size);
            }
        }
        style
    }
    /// Width offered to this node after applying its explicit frame constraints.
    pub fn constrained_width(&self, width: f32) -> f32 {
        self.frame
            .filter(|frame| frame.width.is_finite())
            .map_or(width, |frame| frame.width)
            .max(self.min_width.unwrap_or(0.))
    }

    /// Space inside this node's frame, shared by runtime and renderers.
    pub fn content_width(&self, width: f32, tokens: DesignTokens, insets: Insets) -> f32 {
        let padding = self
            .padding
            .or_else(|| self.padding_token.map(|token| tokens.space(token)))
            .unwrap_or(0.)
            .max(0.);
        let safe = if matches!(self.safe_area, Some(SafeArea::All | SafeArea::Horizontal)) {
            insets.left.max(0.) + insets.right.max(0.)
        } else {
            0.
        };
        (self.constrained_width(width) - 2. * padding - safe).max(0.)
    }

    /// Resolve local adaptive direction after applying the legacy viewport rules.
    pub fn for_container(
        &self,
        viewport: f32,
        width: f32,
        tokens: DesignTokens,
        insets: Insets,
    ) -> Self {
        let mut style = self.for_width(viewport);
        if let Some(direction) = style.adaptive_direction {
            style.mode = Some(direction.resolve(style.content_width(width, tokens, insets)));
        }
        style
    }

    /// Column count and cell width for an adaptive grid.
    pub fn grid_metrics(&self, width: f32, tokens: DesignTokens) -> Option<(u16, f32)> {
        let item = self.min_item_width?;
        let gap = self
            .spacing
            .or_else(|| self.spacing_token.map(|token| tokens.space(token)))
            .unwrap_or(0.)
            .max(0.);
        let columns = ((width + gap) / (item.max(1.) + gap))
            .floor()
            .clamp(1., 256.) as u16;
        let cell = ((width + gap) / f32::from(columns) - gap).max(0.);
        Some((columns, cell))
    }

    /// Whether a grouping node adds no visual or semantic layout overrides.
    pub fn is_transparent_group(&self) -> bool {
        let mut style = self.clone();
        style.key = None;
        style.name = None;
        style.navigation_title = None;
        style.app_bar = None;
        style.desktop_window = None;
        style.source_site = None;
        style.runtime_id = None;
        style.environment = None;
        style.on_appear = None;
        style.on_disappear = None;
        style.on_window_close = None;
        style.on_window_blur = None;
        style.effect = None;
        style.mount_task = None;
        style.on_error = None;
        style.task_context = None;
        style.task_handle = None;
        style == Self::default()
    }
}

impl ViewNode {
    /// Construct a node without style overrides.
    pub fn new(kind: ViewKind) -> Self {
        Self {
            kind,
            style: Style::default(),
        }
    }
}

impl View for ViewNode {
    fn into_node(self) -> ViewNode {
        self
    }
}

/// Batch synchronous mutations, notifying each changed observable once on exit.
/// Nested batches flush at the outer boundary, after mutable value borrows end.
/// Do not hold a batch across asynchronous suspension points.
pub fn batch_updates<R>(update: impl FnOnce() -> R) -> R {
    observation::batch(update)
}
