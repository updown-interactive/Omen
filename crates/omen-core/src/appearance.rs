//! Typed appearance values shared by all renderers.
use crate::Color;

/// A string that is not a supported hexadecimal color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("expected a hex color in #RGB, #RGBA, #RRGGBB, or #RRGGBBAA format")]
pub struct InvalidHexColor;

impl Color {
    /// Parse a hexadecimal color with an optional leading `#`.
    /// Four- and eight-digit values use CSS channel order with alpha last.
    pub fn from_hex(value: &str) -> Result<Self, InvalidHexColor> {
        let value = value.trim();
        let digits = value.strip_prefix('#').unwrap_or(value);
        if !matches!(digits.len(), 3 | 4 | 6 | 8)
            || !digits.bytes().all(|digit| digit.is_ascii_hexdigit())
        {
            return Err(InvalidHexColor);
        }
        let bits = u32::from_str_radix(digits, 16).map_err(|_| InvalidHexColor)?;
        let expand = |shift: u32| ((bits >> shift) & 0xf) * 17;
        Ok(match digits.len() {
            3 => Self::Rgb((expand(8) << 16) | (expand(4) << 8) | expand(0)),
            4 => Self::Rgba((expand(12) << 24) | (expand(8) << 16) | (expand(4) << 8) | expand(0)),
            6 => Self::Rgb(bits),
            _ => Self::Rgba(bits),
        })
    }
}

// String conversion is total so applications can bind dynamic values.
// Use from_hex to report invalid values before applying a modifier.
impl From<&str> for Color {
    fn from(value: &str) -> Self {
        Self::from_hex(value).unwrap_or(Self::Rgba(0))
    }
}

impl From<String> for Color {
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}

impl From<&String> for Color {
    fn from(value: &String) -> Self {
        Self::from(value.as_str())
    }
}

/// Explicit width and height in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// Width in logical pixels; positive infinity fills the parent width.
    pub width: f32,
    /// Height in logical pixels; positive infinity fills the parent height.
    pub height: f32,
}
/// Placement of children within their container.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    /// Horizontal leading edge, vertically centered.
    Leading,
    /// Center on both axes.
    Center,
    /// Horizontal trailing edge, vertically centered.
    Trailing,
    /// Top edge, horizontally centered.
    Top,
    /// Bottom edge, horizontally centered.
    Bottom,
    /// Top leading corner.
    TopLeading,
    /// Top trailing corner.
    TopTrailing,
    /// Bottom leading corner.
    BottomLeading,
    /// Bottom trailing corner.
    BottomTrailing,
}
/// A two-stop linear gradient. Zero degrees points upward, clockwise thereafter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearGradient {
    /// Initial color.
    pub from: Color,
    /// Final color.
    pub to: Color,
    /// Direction in degrees.
    pub angle: f32,
}
impl LinearGradient {
    /// Create a two-color gradient.
    pub fn new(from: impl Into<Color>, to: impl Into<Color>, angle: f32) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            angle,
        }
    }
}
/// A solid or linear-gradient background.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Background {
    /// Solid fill.
    Color(Color),
    /// Linear gradient.
    Gradient(LinearGradient),
}
impl From<Color> for Background {
    fn from(color: Color) -> Self {
        Self::Color(color)
    }
}
impl From<LinearGradient> for Background {
    fn from(gradient: LinearGradient) -> Self {
        Self::Gradient(gradient)
    }
}

impl From<&str> for Background {
    fn from(value: &str) -> Self {
        Color::from(value).into()
    }
}

impl From<String> for Background {
    fn from(value: String) -> Self {
        Color::from(value).into()
    }
}

impl From<&String> for Background {
    fn from(value: &String) -> Self {
        Color::from(value).into()
    }
}
