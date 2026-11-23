//! Explicit shared values and semantic descriptions for retained controls.
use std::{cell::RefCell, fmt, rc::Rc};

/// A shared control value. Native control events update this value and repaint.
///
/// Reads inside named views track dependencies; writes invalidate those views.
/// Debug output deliberately redacts all values.
pub struct Value<T>(Rc<ValueInner<T>>);
struct ValueInner<T> {
    value: RefCell<T>,
    signal: crate::observation::Signal,
}
impl<T> Clone for Value<T> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}
impl<T> Value<T> {
    /// Create a shared observable value.
    pub fn new(value: T) -> Self {
        Self(Rc::new(ValueInner {
            value: RefCell::new(value),
            signal: crate::observation::Signal::new(),
        }))
    }
    /// Borrow a value while tracking a dependency, without cloning the model.
    pub fn read<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        self.0.signal.track();
        read(&self.0.value.borrow())
    }
    /// Replace the value and invalidate views that read it.
    pub fn set(&self, value: T) {
        *self.0.value.borrow_mut() = value;
        self.0.signal.notify();
    }
    /// Mutate in place and notify observers once after the borrow is released.
    pub fn update<R>(&self, update: impl FnOnce(&mut T) -> R) -> R {
        let result = update(&mut self.0.value.borrow_mut());
        self.0.signal.notify();
        result
    }
}
impl<T: Clone> Value<T> {
    /// Read a snapshot and register a dependency in the current view build.
    pub fn get(&self) -> T {
        self.0.signal.track();
        self.0.value.borrow().clone()
    }
}
impl<T> PartialEq for Value<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl<T> fmt::Debug for Value<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Value(<redacted>)")
    }
}
impl<T: Clone + fmt::Display> fmt::Display for Value<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.get(), f)
    }
}
/// An observable application model, tracked at model granularity.
pub type Observable<T> = Value<T>;
/// A two-way value or model-field projection.
pub struct Binding<T>(Rc<BindingFns<T>>);
struct BindingFns<T> {
    get: Box<dyn Fn() -> T>,
    set: Box<dyn Fn(T)>,
}
impl<T> Clone for Binding<T> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}
impl<T> Binding<T> {
    /// Create a binding from read and write operations.
    pub fn new(get: impl Fn() -> T + 'static, set: impl Fn(T) + 'static) -> Self {
        Self(Rc::new(BindingFns {
            get: Box::new(get),
            set: Box::new(set),
        }))
    }
    /// Read the current value, tracking any underlying observable reads.
    pub fn get(&self) -> T {
        (self.0.get)()
    }
    /// Write through the binding.
    pub fn set(&self, value: T) {
        (self.0.set)(value);
    }
}
impl<T: Clone + 'static> From<Value<T>> for Binding<T> {
    fn from(value: Value<T>) -> Self {
        let read = value.clone();
        Self::new(move || read.get(), move |next| value.set(next))
    }
}
impl<T> PartialEq for Binding<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl<T> fmt::Debug for Binding<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Binding(<redacted>)")
    }
}
/// Shared editable text, including secure-field values.
pub type TextValue = Value<String>;
/// Shared checkbox or toggle value.
pub type BoolValue = Value<bool>;

/// Backend-independent accessibility roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessibilityRole {
    /// Static text.
    Text,
    /// Heading.
    Heading,
    /// Image or icon.
    Image,
    /// Action button.
    Button,
    /// Checkbox.
    Checkbox,
    /// On/off switch.
    Switch,
    /// Editable text.
    TextField,
    /// Completion indicator.
    Progress,
    /// Collection of rows.
    List,
    /// One row.
    ListItem,
    /// A named group of related content.
    Group,
    /// Navigation region.
    Navigation,
    /// Tab selector.
    Tab,
    /// Tab selector collection.
    TabList,
    /// Selected tab content.
    TabPanel,
    /// Dialog surface.
    Dialog,
}
/// Accessible overrides; controls also supply default names, roles, and values.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Accessibility {
    /// Accessible name, overriding the visible label.
    pub label: Option<String>,
    /// Supplementary spoken description.
    pub hint: Option<String>,
    /// Explicit semantic role.
    pub role: Option<AccessibilityRole>,
    /// Selection state for a tab.
    pub selected: Option<bool>,
}

/// Configuration for a native horizontal panel resize handle.
#[derive(Clone, Debug, PartialEq)]
pub struct ResizeHandle {
    /// Width updated during dragging or keyboard adjustment.
    pub width: Binding<f32>,
    /// Smallest allowed width.
    pub min: f32,
    /// Largest allowed width.
    pub max: f32,
    /// Whether moving left increases the panel width.
    pub reverse: bool,
}

/// Initial desktop window metadata. The native host may override it with OMEN_WINDOW_*.
#[derive(Clone, Debug, PartialEq)]
pub struct DesktopWindow {
    /// Optional custom Omen toolbar alongside desktop window action buttons.
    /// None preserves the standard native title bar. Configure before opening.
    pub toolbar: Option<crate::DesktopToolbar>,
    /// Native title bar label.
    pub title: String,
    /// Initial logical width.
    pub width: f32,
    /// Initial logical height.
    pub height: f32,
    /// Minimum logical width.
    pub min_width: f32,
    /// Minimum logical height.
    pub min_height: f32,
}
impl Default for DesktopWindow {
    fn default() -> Self {
        Self {
            toolbar: None,
            title: "Omen".into(),
            width: 800.,
            height: 520.,
            min_width: 320.,
            min_height: 240.,
        }
    }
}

/// A value that can be injected into a view environment.
pub trait EnvironmentSource {
    /// The model type used for typed dependency lookup.
    type Model: 'static;
    /// Convert to the existing observable environment storage.
    fn into_environment(self) -> Value<Self::Model>;
}
impl<T: 'static> EnvironmentSource for Value<T> {
    type Model = T;
    fn into_environment(self) -> Value<T> {
        self
    }
}
/// Compiler support for reading ordinary and reactive environment fields.
#[doc(hidden)]
pub struct EnvironmentField<'a, T>(pub &'a T);
/// Compiler support for field snapshots.
#[doc(hidden)]
pub trait ReadEnvironmentField {
    type Output;
    fn snapshot(self) -> Self::Output;
}
impl<T: Clone> ReadEnvironmentField for &EnvironmentField<'_, T> {
    type Output = T;
    fn snapshot(self) -> T {
        self.0.clone()
    }
}
impl<T: Clone> ReadEnvironmentField for EnvironmentField<'_, Value<T>> {
    type Output = T;
    fn snapshot(self) -> T {
        self.0.get()
    }
}

/// Compiler support for projecting an environment field into a binding.
#[doc(hidden)]
pub struct EnvironmentProjection<T, F> {
    pub model: Value<T>,
    pub read: for<'a> fn(&'a T) -> &'a F,
    pub write: for<'a> fn(&'a mut T) -> &'a mut F,
}
/// Compiler support for ordinary and reactive field bindings.
#[doc(hidden)]
pub trait BindEnvironmentField {
    type Output;
    fn binding(self) -> Self::Output;
}
impl<T: 'static, F: Clone + 'static> BindEnvironmentField for &EnvironmentProjection<T, F> {
    type Output = Binding<F>;
    fn binding(self) -> Self::Output {
        let read = self.model.clone();
        let write = self.model.clone();
        let get = self.read;
        let set = self.write;
        Binding::new(
            move || read.read(|model| get(model).clone()),
            move |next| write.update(|model| *set(model) = next),
        )
    }
}
impl<T: 'static, F: Clone + 'static> BindEnvironmentField for EnvironmentProjection<T, Value<F>> {
    type Output = Binding<F>;
    fn binding(self) -> Self::Output {
        self.model.read(|model| (self.read)(model).clone()).into()
    }
}
impl Value<bool> {
    /// Invert this observable boolean and notify dependent views.
    pub fn toggle(&self) {
        self.update(|value| *value = !*value);
    }
}
