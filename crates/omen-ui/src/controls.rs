//! Omen-owned content and form controls.
use crate::*;

macro_rules! content_view {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        pub struct $name(ViewNode);
        impl $name {
            /// Create a view from an asset path or registered icon name.
            pub fn new(source: impl Into<String>) -> Self {
                Self(ViewNode::new(ViewKind::$name(source.into())))
            }
        }
        impl View for $name {
            fn into_node(self) -> ViewNode {
                self.0
            }
        }
    };
}
content_view!(
    Image,
    r#"A raster image loaded from an accessible asset path.

The native renderer uses contain fitting, a 96 × 96 default frame and an error
fallback. Browser paths resolve against served assets. There is no shared remote
image downloader. Always supply a meaningful accessible description.

```
# use omen_ui as omen;
use omen::view;
let photo = view! { Image("assets/photo.png").frame(160, 100)
    .accessibilityLabel("A mountain reflected in a lake") };
```"#
);
content_view!(
    Icon,
    r#"A named icon or local native SVG asset.

Built-in names are `check`, `close`, `plus`, `search`, `settings` and `info`.
Native icons default to 24 × 24 and use the foreground color; the web renderer
uses text-symbol fallbacks. A local SVG path is a native renderer capability.

```
# use omen_ui as omen;
use omen::view;
let completed = view! { Icon("check").color(.green).accessibilityLabel("Completed") };
```"#
);

macro_rules! input_view {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        pub struct $name(ViewNode);
        impl $name {
            /// Create an empty control with a placeholder and default accessible name.
            pub fn new(label: impl Into<String>) -> Self {
                Self(ViewNode::new(ViewKind::$name {
                    label: label.into(),
                    value: TextValue::new(String::new()),
                }))
            }
        }
        impl View for $name {
            fn into_node(self) -> ViewNode {
                self.0
            }
        }
    };
}
input_view!(
    TextField,
    r#"A single-line text field with a placeholder and default accessible label.

Starts empty without a supplied value. Bind a retained string with `.value($name)`.
User edits update the binding before `onChange`; programmatic writes do not call
that callback. Desktop selection/IME and clipboard support do not imply the same
coverage on unfinished mobile backends. Multiline editing is not implemented.

```
# use omen_ui as omen;
use omen::view;
view! { Profile {
    @State name = ""
    TextField("Name").value($name).accessibilityLabel("Full name")
} }
let profile = view! { Profile() };
```"#
);
input_view!(
    SecureField,
    r#"A masked single-line input using the same bindings as TextField.

The desktop adapter masks rendered/native/accessibility values and blocks
copy/cut export. Paste and selection replacement remain available. Platform
service coverage varies; masking is not encrypted credential storage.

```
# use omen_ui as omen;
use omen::view;
view! { Login {
    @State password = ""
    SecureField("Password").value($password).accessibilityLabel("Password")
} }
let login = view! { Login() };
```"#
);

macro_rules! bool_view {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        pub struct $name(ViewNode);
        impl $name {
            /// Create an unchecked control with a label.
            pub fn new(label: impl Into<String>) -> Self {
                Self(ViewNode::new(ViewKind::$name {
                    label: label.into(),
                    value: BoolValue::new(false),
                }))
            }
        }
        impl View for $name {
            fn into_node(self) -> ViewNode {
                self.0
            }
        }
    };
}
bool_view!(
    Checkbox,
    r#"A labeled Boolean selection, unchecked unless supplied a binding.

`.checked($accepted)` and the DSL constructor's second binding argument are
interchangeable. Activation updates shared state and then calls `onChange`.
Keyboard/accessibility activation depends on the backend's service coverage.

```
# use omen_ui as omen;
use omen::view;
view! { Terms {
    @State accepted = false
    Checkbox("Accept the terms", $accepted)
} }
let terms = view! { Terms() };
```"#
);
bool_view!(
    Toggle,
    r#"A labeled on/off setting backed by a Boolean binding.

Starts off without an explicit value. Use `$state.field`, not a Boolean snapshot,
when the setting should update application state. User edits call `onChange`
after writing through the binding; programmatic changes do not.

```
# use omen_ui as omen;
use omen::view;
view! { Preferences {
    @State enabled = true
    Toggle("Notifications").checked($enabled)
} }
let preferences = view! { Preferences() };
```"#
);

/// Determinate completion progress, clamped to the range zero through one.
///
/// Non-finite input becomes zero. This is not an indeterminate spinner. The
/// semantic value exposes zero/one bounds; give the operation an accessible name.
///
/// ```
/// # use omen_ui as omen;
/// use omen::view;
/// let upload = view! { Progress(0.65).accessibilityLabel("Upload progress") };
/// ```
pub struct Progress(ViewNode);
impl Progress {
    /// Create a progress indicator. Non-finite values become zero.
    pub fn new(fraction: impl Into<f64>) -> Self {
        let fraction = fraction.into();
        Self(ViewNode::new(ViewKind::Progress(if fraction.is_finite() {
            fraction.clamp(0., 1.) as f32
        } else {
            0.
        })))
    }
}
impl View for Progress {
    fn into_node(self) -> ViewNode {
        self.0
    }
}

/// A horizontal separator, rendered natively as a one-pixel themed rule.
///
/// Accepts no constructor arguments or child block.
///
/// ```
/// # use omen_ui as omen;
/// use omen::view;
/// let sections = view! { Layout { Text("Account") Divider() Text("Preferences") } };
/// ```
#[derive(Default)]
pub struct Divider;
impl Divider {
    /// Create a separator.
    pub fn new() -> Self {
        Self
    }
}
impl View for Divider {
    fn into_node(self) -> ViewNode {
        ViewNode::new(ViewKind::Divider)
    }
}

/// Empty space along the parent Layout’s axis.
///
/// `Gap()` adds eight logical pixels. Use `Gap(16)` for a custom size.
pub struct Gap(f32);

impl Default for Gap {
    fn default() -> Self {
        Self::new()
    }
}

impl Gap {
    /// Create a gap of eight logical pixels.
    pub fn new() -> Self {
        Self(8.)
    }

    /// Create a gap with an explicit size in logical pixels.
    pub fn sized(size: impl Into<f64>) -> Self {
        let size = size.into() as f32;
        Self(if size.is_finite() { size.max(0.) } else { 0. })
    }
}

impl View for Gap {
    fn into_node(self) -> ViewNode {
        ViewNode::new(ViewKind::Gap(self.0))
    }
}

/// A native multiline plain-text editor with selection, clipboard and IME input.
/// Syntax highlighting and language-server features are separate capabilities.
pub struct TextEditor(ViewNode);
impl TextEditor {
    /// Create an empty editor with a placeholder and accessible name.
    pub fn new(label: impl Into<String>) -> Self {
        let mut node = TextField::new(label).into_node();
        node.style.multiline = true;
        Self(node)
    }
}
impl View for TextEditor {
    fn into_node(self) -> ViewNode {
        self.0
    }
}
