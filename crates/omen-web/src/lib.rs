//! Retained DOM scene adapter for browser, Android WebView, and iOS WKWebView hosts.
#![deny(missing_docs)]
mod scene;
pub use scene::*;
#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::{native_bridge, permission_broker, run_view, sleep};
#[cfg(not(target_arch = "wasm32"))]
/// Browser entry point. Native applications should select the desktop backend instead.
pub fn run_view(_view: impl omen_core::View) -> Result<(), WindowError> {
    Err(WindowError::Unsupported)
}
/// Browser host startup/rendering failure.
#[derive(Debug, thiserror::Error)]
pub enum WindowError {
    /// A reactive view failed to reconcile.
    #[error(transparent)]
    Runtime(#[from] omen_core::RuntimeError),
    /// Requested browser startup from a non-WASM target.
    #[error("the web backend requires wasm32-unknown-unknown")]
    Unsupported,
    /// An application already owns this WASM instance.
    #[error("one application may run per WASM instance")]
    AlreadyRunning,
    /// Host frame size/encoding failed.
    #[error("browser scene exceeds the supported frame limit")]
    SceneLimit,
}
