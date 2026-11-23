//! Experimental Android platform integration for GPUI, independent of Omen.
#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
pub use android::*;
#[cfg(target_os = "android")]
pub use android_activity::AndroidApp;

#[cfg(any(target_os = "android", test))]
mod touch;
