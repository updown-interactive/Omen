//! Omen's public application API for declarative views and retained state.
//!
//! Import [`prelude`] for the macros, builders, modifiers and state handles used
//! in application code. [`main!`] starts an application; [`view!`] constructs a
//! tree or declares a reusable named view. Supporting [`state!`], [`locale!`] and
//! [`environment!`] declarations organize logic independently of file placement.
//!
//! # Retained state and a bound control
//!
//! ```
//! use omen::prelude::*;
//!
//! state! {
//!     CounterState {
//!         count = 0
//!         enabled = true
//!         pub fn increment(&mut self) { self.count += 1; }
//!     }
//! }
//! view! {
//!     Counter {
//!         @State state = CounterState()
//!         Layout {
//!             Text("Count: {state.count}").font(.title)
//!             Button("Increment") { state.increment() }.padding(.md)
//!             Toggle("Enabled", $state.enabled)
//!         }.spacing(.md)
//!     }
//! }
//! let mut runtime = Runtime::new(view! { Counter() }.into_node());
//! let _rendered = runtime.render().unwrap();
//! ```
//!
//! State is retained by view identity. Plain fields read values; `$state.field`
//! gives a two-way binding into the same model. Actions run on activation, not
//! while the tree is constructed. Synchronous mutations batch notifications.
//!
//! # Component families
//!
//! - Content and actions: [`Text`], [`Button`], [`Image`], [`Icon`].
//! - Forms: [`TextField`], [`SecureField`], [`Checkbox`], [`Toggle`].
//! - Containers and status: [`Layout`], [`List`], [`Card`], [`Progress`], [`Divider`].
//! - Navigation: [`NavigationStack`], [`NavigationBar`], [`TabView`], [`Tab`].
//! - Shared behavior: [`ViewExt`] provides styling, bindings, presentations,
//!   accessibility, lifecycle and animation modifiers.
//!
//! The optional `omen-components` crate supplies higher-level reusable views.
//! Source guides are in `docs/view-library.md` and `docs/reactive-state.md`.
//!
//! # Backends and features
//!
//! The default `native` feature enables GPUI startup. Use `default-features = false`
//! for backend-independent view construction and testing, or enable `web` for
//! the browser backend. Starting an application requires an appropriate backend.
//! Controls share a semantic model; platform input, clipboard and accessibility
//! capabilities still depend on the backend. See `docs/cross-platform.md`.
#![deny(missing_docs)]
extern crate self as omen;
pub use omen_ui::*;
/// Common macros, views, state and styling APIs for application code.
pub mod prelude {
    pub use omen_ui::*;
}
