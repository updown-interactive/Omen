//! Theme tokens and viewport-dependent layout.
use crate::{Color, Frame, LayoutMode};
/// Theme selection. System is resolved by the native backend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeMode {
    /// Follow the operating system.
    System,
    /// Light surfaces and dark text.
    Light,
    /// Dark surfaces and light text.
    #[default]
    Dark,
}
/// Spacing token names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    /// Four logical pixels by default.
    Small,
    /// Eight logical pixels by default.
    Medium,
    /// Sixteen logical pixels by default.
    Large,
    /// Twenty-four logical pixels by default.
    ExtraLarge,
}
/// Overridable semantic colors and dimensions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DesignTokens {
    /// Main text color.
    pub primary: u32,
    /// Supporting text color.
    pub secondary: u32,
    /// Window canvas color.
    pub canvas: u32,
    /// Raised surface color.
    pub surface: u32,
    /// Accent color.
    pub accent: u32,
    /// Separator color.
    pub border: u32,
    /// Spacing scale, ordered small through extra large.
    pub spacing: [f32; 4],
    /// Default panel radius.
    pub radius: f32,
    /// Default body font size.
    pub body_size: f32,
}
/// A theme with optional custom design tokens.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Theme {
    /// Color scheme selection.
    pub mode: ThemeMode,
    /// Explicit palette/size overrides; otherwise follows the selected scheme.
    pub tokens: Option<DesignTokens>,
}
impl From<ThemeMode> for Theme {
    fn from(mode: ThemeMode) -> Self {
        Self { mode, tokens: None }
    }
}
impl Theme {
    /// Light theme.
    pub fn light() -> Self {
        ThemeMode::Light.into()
    }
    /// Dark theme.
    pub fn dark() -> Self {
        ThemeMode::Dark.into()
    }
    /// Follow native appearance.
    pub fn system() -> Self {
        ThemeMode::System.into()
    }
    /// Use application design tokens.
    pub fn with_tokens(mut self, tokens: DesignTokens) -> Self {
        self.tokens = Some(tokens);
        self
    }
    /// Resolve tokens, supplying the current system appearance.
    pub fn resolve(self, system_dark: bool) -> DesignTokens {
        self.tokens.unwrap_or_else(|| {
            let dark =
                self.mode == ThemeMode::Dark || (self.mode == ThemeMode::System && system_dark);
            DesignTokens {
                primary: if dark { 0xf8fafc } else { 0x0f172a },
                secondary: if dark { 0x94a3b8 } else { 0x475569 },
                canvas: if dark { 0x101827 } else { 0xf8fafc },
                surface: if dark { 0x1e293b } else { 0xffffff },
                accent: 0x2563eb,
                border: if dark { 0x334155 } else { 0xcbd5e1 },
                spacing: [4., 8., 16., 24.],
                radius: 12.,
                body_size: 16.,
            }
        })
    }
}
impl DesignTokens {
    /// Resolve a spacing token to logical pixels.
    pub fn space(self, token: Space) -> f32 {
        self.spacing[match token {
            Space::Small => 0,
            Space::Medium => 1,
            Space::Large => 2,
            Space::ExtraLarge => 3,
        }]
    }
    /// Resolve a semantic or fixed color to an RGB value.
    pub fn color(self, color: Color) -> u32 {
        match color {
            Color::Primary => self.primary,
            Color::Secondary => self.secondary,
            Color::Canvas => self.canvas,
            Color::Surface | Color::Panel => self.surface,
            Color::Accent => self.accent,
            Color::Border => self.border,
            Color::White => 0xffffff,
            Color::Black => 0,
            Color::Blue => 0x2563eb,
            Color::Green => 0x22c55e,
            Color::Dark => 0x101827,
            Color::Red => 0xef4444,
            Color::Orange => 0xf97316,
            Color::Purple => 0x8b5cf6,
            Color::Gray => 0x64748b,
            Color::Rgb(value) => value & 0xffffff,
            Color::Rgba(value) => value >> 8,
        }
    }

    /// Resolve a semantic or fixed color to RGBA, preserving explicit alpha.
    pub fn rgba(self, color: Color) -> u32 {
        match color {
            Color::Rgba(value) => value,
            _ => (self.color(color) << 8) | 0xff,
        }
    }
}
/// Insets in logical pixels. Desktop content viewports normally have zero insets.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    /// Top inset.
    pub top: f32,
    /// Right inset.
    pub right: f32,
    /// Bottom inset.
    pub bottom: f32,
    /// Left inset.
    pub left: f32,
}
/// Safe-area edges to inset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SafeArea {
    /// All four edges.
    All,
    /// Top and bottom edges.
    Vertical,
    /// Leading and trailing edges.
    Horizontal,
    /// No automatic insets.
    None,
}
/// Mobile-first viewport thresholds in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Breakpoint {
    /// Width below 600 pixels.
    Compact,
    /// Width of at least 600 pixels.
    Medium,
    /// Width of at least 1024 pixels.
    Large,
    /// Width of at least 1440 pixels.
    ExtraLarge,
}
impl std::fmt::Display for Breakpoint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl Breakpoint {
    /// Minimum width at which this breakpoint applies.
    pub fn min_width(self) -> f32 {
        match self {
            Self::Compact => 0.,
            Self::Medium => 600.,
            Self::Large => 1024.,
            Self::ExtraLarge => 1440.,
        }
    }
}
/// Observable viewport data supplied by the renderer to named views.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// Content width.
    pub width: f32,
    /// Content height.
    pub height: f32,
    /// Insets not usable for content.
    pub safe_area: Insets,
}
impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 800.,
            height: 520.,
            safe_area: Insets::default(),
        }
    }
}
impl Viewport {
    /// Current width category.
    pub fn breakpoint(self) -> Breakpoint {
        if self.width >= 1440. {
            Breakpoint::ExtraLarge
        } else if self.width >= 1024. {
            Breakpoint::Large
        } else if self.width >= 600. {
            Breakpoint::Medium
        } else {
            Breakpoint::Compact
        }
    }
}
/// Responsive overrides. Matching rules apply from narrowest to widest.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ResponsiveStyle {
    /// Child arrangement override.
    pub mode: Option<LayoutMode>,
    /// Padding override.
    pub padding: Option<f32>,
    /// Spacing override.
    pub spacing: Option<f32>,
    /// Frame override.
    pub frame: Option<Frame>,
    /// Font size override.
    pub font_size: Option<f32>,
}

/// Orientation of the logical content viewport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    /// Height is at least as large as width.
    Portrait,
    /// Width is greater than height.
    Landscape,
}

/// Platform exposing the Omen viewport.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DevicePlatform {
    /// Android native host.
    Android,
    /// iOS native host.
    Ios,
    /// macOS native host.
    MacOs,
    /// Windows native host.
    Windows,
    /// Linux native host.
    Linux,
    /// Browser host.
    Web,
    /// Host platform is unavailable.
    #[default]
    Unknown,
}

impl DevicePlatform {
    /// Compile target used by a native host.
    pub const fn native() -> Self {
        if cfg!(target_os = "android") {
            Self::Android
        } else if cfg!(target_os = "ios") {
            Self::Ios
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Unknown
        }
    }
}

/// Available input capabilities. Touch and mouse can both be available.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PointerCapabilities {
    /// A touch input is available.
    pub touch: bool,
    /// A fine pointer such as a mouse or trackpad is available.
    pub mouse: bool,
}

/// Occlusion caused by the on-screen keyboard in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct KeyboardViewport {
    /// An on-screen keyboard is occluding the viewport.
    pub visible: bool,
    /// Obscured edges.
    pub insets: Insets,
}

impl KeyboardViewport {
    /// Derive visibility from reported keyboard insets.
    pub fn from_insets(insets: Insets) -> Self {
        Self {
            visible: [insets.top, insets.right, insets.bottom, insets.left]
                .iter()
                .any(|&inset| inset > 0.),
            insets,
        }
    }
}

/// Reactive snapshot of the host window and its capabilities.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeviceEnvironment {
    /// Logical viewport dimensions and safe area.
    pub size: Viewport,
    /// Width in logical pixels.
    pub width: f32,
    /// Height in logical pixels.
    pub height: f32,
    /// Orientation derived from the viewport.
    pub orientation: Orientation,
    /// Insets reserved for system UI.
    pub safe_area: Insets,
    /// Physical pixels per logical pixel.
    pub pixel_ratio: f32,
    /// Platform reported by the host.
    pub platform: DevicePlatform,
    /// Available touch and fine pointers.
    pub pointer: PointerCapabilities,
    /// Keyboard occlusion.
    pub keyboard: KeyboardViewport,
    /// Preferred text multiplier, defaulting to one.
    pub text_scale: f32,
    /// Current system appearance.
    pub appearance: ThemeMode,
}

impl Default for DeviceEnvironment {
    fn default() -> Self {
        Self {
            size: Viewport::default(),
            width: 0.,
            height: 0.,
            orientation: Orientation::Portrait,
            safe_area: Insets::default(),
            pixel_ratio: 1.,
            platform: DevicePlatform::native(),
            pointer: PointerCapabilities::default(),
            keyboard: KeyboardViewport::default(),
            text_scale: 1.,
            appearance: ThemeMode::Light,
        }
        .with_viewport(Viewport::default())
    }
}

impl DeviceEnvironment {
    /// Update the logical viewport and its derived fields together.
    pub fn with_viewport(mut self, viewport: Viewport) -> Self {
        self.size = viewport;
        self.width = viewport.width;
        self.height = viewport.height;
        self.orientation = if viewport.width > viewport.height {
            Orientation::Landscape
        } else {
            Orientation::Portrait
        };
        self.safe_area = viewport.safe_area;
        self
    }
}

/// Width available to a view at its position in the layout tree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResponsiveLayout {
    /// Width in logical pixels.
    pub width: f32,
    /// Width below 600 logical pixels.
    pub compact: bool,
    /// Width between 600 and 1024 logical pixels.
    pub medium: bool,
    /// Width at least 1024 logical pixels.
    pub expanded: bool,
}

impl ResponsiveLayout {
    /// Classify a logical width.
    pub fn new(width: f32) -> Self {
        Self {
            width,
            compact: width < Breakpoint::Medium.min_width(),
            medium: (Breakpoint::Medium.min_width()..Breakpoint::Large.min_width())
                .contains(&width),
            expanded: width >= Breakpoint::Large.min_width(),
        }
    }
}

/// Child direction chosen from the width available to the container.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdaptiveDirection {
    /// Direction in compact space.
    pub compact: LayoutMode,
    /// Direction in medium space, falling back to expanded.
    pub medium: Option<LayoutMode>,
    /// Direction in expanded space.
    pub expanded: LayoutMode,
}

impl Default for AdaptiveDirection {
    fn default() -> Self {
        Self {
            compact: LayoutMode::Vertical,
            medium: None,
            expanded: LayoutMode::Horizontal,
        }
    }
}

impl AdaptiveDirection {
    /// Choose a direction for the available content width.
    pub fn resolve(self, width: f32) -> LayoutMode {
        let layout = ResponsiveLayout::new(width);
        if layout.compact {
            self.compact
        } else if layout.medium {
            self.medium.unwrap_or(self.expanded)
        } else {
            self.expanded
        }
    }
}

/// Distribute a row's remaining space when the widths of its fixed children are known.
#[doc(hidden)]
pub fn remaining_row_width(children: &[crate::ViewNode], width: f32, spacing: f32) -> Option<f32> {
    let mut assigned = 0.;
    let mut flexible = 0;
    let mut count: usize = 0;
    for child in children.iter().filter(|child| child.style.pin.is_none()) {
        count += 1;
        let style = &child.style;
        if !style.responsive.is_empty() {
            return None;
        }
        if let Some(frame) = style.frame.filter(|frame| frame.width.is_finite()) {
            assigned += frame.width.max(style.min_width.unwrap_or(0.));
        } else if style.fill && style.min_width.is_none() {
            flexible += 1;
        } else {
            return None;
        }
    }
    (flexible > 0).then(|| {
        ((width - assigned - count.saturating_sub(1) as f32 * spacing.max(0.)) / flexible as f32)
            .max(0.)
    })
}
