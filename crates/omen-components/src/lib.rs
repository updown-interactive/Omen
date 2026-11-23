//! Reusable Omen views built entirely with view!, ordinary properties, and modifiers.
//!
//! Import these names into your application and compose them with named arguments.
//!
//! | View | Purpose |
//! | --- | --- |
//! | [`SectionHeader`] | Heading and supporting text |
//! | [`StatusBadge`] | Textual status with compact emphasis |
//! | [`PrimaryButton`] | Gradient action button |
//! | [`EmptyState`] | Informational empty-content card |
//! | [`LoadingCard`] | Titled determinate progress |
//! | [`FormRow`] | Labeled editable text binding |
//! | [`PreferenceRow`] | Labeled Boolean binding |
//!
//! Applications normally import `view`, state and bindings from `omen` and add
//! `omen-components` as a separate dependency. All properties are required;
//! named arguments may appear in any order. These are ordinary views declared
//! with `view!`, with the same state, identity and modifier system as built-ins.
#![deny(missing_docs)]
use omen_ui::{Action, Binding, view};

view! {
    /// A title and supporting subtitle arranged vertically.
    ///
    /// # Properties
    ///
    /// - `title: String`: heading text, rendered with title typography and semibold weight.
    /// - `subtitle: String`: required supporting text, rendered in the secondary color.
    ///
    /// The title has heading accessibility semantics. Children have six logical pixels of spacing.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use omen_ui as omen;
    /// use omen::view;
    /// use omen_components::SectionHeader;
    /// let header = view! { SectionHeader(title: "Account", subtitle: "Manage your profile") };
    /// ```
    pub SectionHeader(title: String, subtitle: String) {
        Layout {
            Text(title).font(.title).weight(.semibold).accessibilityRole(.heading)
            Text(subtitle).font(.body).color(.secondary)
        }.spacing(6)
    }
}
view! {
    /// A compact green status label on a rounded panel background.
    ///
    /// # Properties
    ///
    /// - `label: String`: the complete visible status, for example `"Connected"`.
    ///
    /// Uses semibold caption typography, six logical pixels of padding and corner radius.
    /// Include the status in the text rather than communicating it through color alone.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use omen_ui as omen;
    /// use omen::view;
    /// use omen_components::StatusBadge;
    /// let status = view! { StatusBadge(label: "Connected") };
    /// ```
    pub StatusBadge(label: String) {
        Text(label).font(.caption).weight(.semibold).color(.green)
            .padding(6).background(.panel).cornerRadius(6)
    }
}
view! {
    /// An action button with white text and a blue-to-purple gradient.
    ///
    /// # Properties
    ///
    /// - `title: String`: the visible and accessible button label.
    /// - `action: omen::Action`: a shared synchronous callback, invoked on activation.
    ///
    /// Uses twelve logical pixels of padding and an eight-pixel corner radius.
    /// Like the built-in button, its default width follows its content. Use a frame
    /// for a fixed or full-width button. For asynchronous work use the built-in
    /// `Button` async action syntax instead of blocking this callback.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use omen_ui as omen;
    /// use omen::{Action, view};
    /// use omen_components::PrimaryButton;
    /// let save = Action::new(|| println!("Save"));
    /// let button = view! { PrimaryButton(title: "Save", action: save).frame(.infinity, 44) };
    /// ```
    pub PrimaryButton(title: String, action: Action) {
        Button(title) { action.invoke(); }.padding(12).gradient(.blue, .purple, 90)
            .color(.white).cornerRadius(8)
    }
}
view! {
    /// A centered informational card for an empty collection or unavailable content.
    ///
    /// # Properties
    ///
    /// - `title: String`: the short heading describing the empty state.
    /// - `message: String`: supporting explanation or next steps.
    ///
    /// Contains an accessible information icon, a heading and secondary body text.
    /// This view has no action property; compose an adjacent button when an action is needed.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use omen_ui as omen;
    /// use omen::view;
    /// use omen_components::EmptyState;
    /// let empty = view! { EmptyState(title: "No tasks", message: "Create a task to get started.") };
    /// ```
    pub EmptyState(title: String, message: String) {
        Card {
            Icon("info").color(.secondary).accessibilityLabel("Information")
            Text(title).font(.headline).accessibilityRole(.heading)
            Text(message).font(.body).color(.secondary)
        }.alignment(.center).spacing(8)
    }
}
view! {
    /// A titled card containing determinate progress.
    ///
    /// # Properties
    ///
    /// - `title: String`: the visible heading.
    /// - `fraction: f32`: completion fraction, clamped to zero through one by `Progress`.
    ///
    /// The progress indicator has the accessible label `"Completion"`. This is not an
    /// indeterminate spinner. Use an `f32` literal for the named fraction property.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use omen_ui as omen;
    /// use omen::view;
    /// use omen_components::LoadingCard;
    /// let loading = view! { LoadingCard(title: "Uploading", fraction: 0.5_f32) };
    /// ```
    pub LoadingCard(title: String, fraction: f32) {
        Card {
            Text(title).font(.headline)
            Progress(fraction).accessibilityLabel("Completion")
        }
    }
}
view! {
    /// A caption label above a bound single-line text field.
    ///
    /// # Properties
    ///
    /// - `label: String`: the caption, placeholder and default accessible field name.
    /// - `value: omen::Binding<String>`: the same two-way storage read by the application.
    ///
    /// Supply `$state.field` in a reactive view, or a shared `TextValue` in ordinary Rust.
    /// Do not create a fresh value on each rebuild if the edit should persist.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use omen_ui as omen;
    /// use omen::{TextValue, view};
    /// use omen_components::FormRow;
    /// let name = TextValue::new(String::from("Ada"));
    /// let row = view! { FormRow(label: "Name", value: name.clone()) };
    /// assert_eq!(name.get(), "Ada");
    /// ```
    pub FormRow(label: String, value: Binding<String>) {
        Layout {
            Text(label.clone()).font(.caption).color(.secondary)
            TextField(label).value(value)
        }.spacing(4)
    }
}
view! {
    /// A labeled toggle backed by a shared Boolean binding.
    ///
    /// # Properties
    ///
    /// - `title: String`: the visible and accessible setting label.
    /// - `value: omen::Binding<bool>`: two-way checked state.
    ///
    /// Activation writes through the binding. Use `$state.field` to share the view's
    /// retained model rather than passing a snapshot Boolean.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use omen_ui as omen;
    /// use omen::{BoolValue, view};
    /// use omen_components::PreferenceRow;
    /// let notifications = BoolValue::new(true);
    /// let row = view! { PreferenceRow(title: "Notifications", value: notifications.clone()) };
    /// assert!(notifications.get());
    /// ```
    pub PreferenceRow(title: String, value: Binding<bool>) {
        Toggle(title).checked(value)
    }
}
