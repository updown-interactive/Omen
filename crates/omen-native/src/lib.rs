//! Example filesystem, notification, and camera plugins with a desktop host adapter.
#![deny(missing_docs)]
#![forbid(unsafe_code)]
mod desktop;
#[cfg(unix)]
mod filesystem;
#[cfg(not(unix))]
#[path = "filesystem_unsupported.rs"]
mod filesystem;
pub use desktop::*;
pub use filesystem::*;
use omen_plugin::*;

/// Cross-platform operation and event schemas shared with JNI, Swift, and WASM hosts.
pub use omen_plugin::standard::*;
