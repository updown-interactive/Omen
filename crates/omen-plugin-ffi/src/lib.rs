//! Narrow C ABI for Swift and a JNI C shim. Callback buffers are borrowed only during calls.
#![deny(missing_docs)]
use omen_plugin::{ErrorCode, MAX_MESSAGE_BYTES, NativeBridge, PluginError, PluginResult};
use std::{
    collections::BTreeMap,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};
/// Host callback. Copy bytes before returning; never unwind across this boundary.
/// The numeric context is owned by the embedding host, never dereferenced by Rust.
pub type SendCallback = unsafe extern "C" fn(context: u64, data: *const u8, len: usize) -> bool;
static BRIDGES: OnceLock<Mutex<BTreeMap<u64, NativeBridge>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
fn bridges() -> &'static Mutex<BTreeMap<u64, NativeBridge>> {
    BRIDGES.get_or_init(Mutex::default)
}
fn lock() -> std::sync::MutexGuard<'static, BTreeMap<u64, NativeBridge>> {
    bridges()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
/// Retrieve a bridge to register typed plugin proxies in the application's Rust runtime.
pub fn bridge(handle: u64) -> PluginResult<NativeBridge> {
    lock()
        .get(&handle)
        .cloned()
        .ok_or_else(|| PluginError::new(ErrorCode::Unavailable, "unknown bridge handle"))
}
/// Create a bridge. Returns zero for a missing callback or exhausted handle space.
/// # Safety
/// The callback and its context must remain valid until destroy returns and all previously
/// dispatched callbacks finish. The callback must not throw/unwind and must copy its buffer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn omen_bridge_create(send: Option<SendCallback>, context: u64) -> u64 {
    let Some(send) = send else {
        return 0;
    };
    let Ok(handle) = NEXT.fetch_update(Ordering::AcqRel, Ordering::Acquire, |next| {
        next.checked_add(1)
    }) else {
        return 0;
    };
    let bridge = NativeBridge::new(move |bytes: &[u8]| {
        // The embedding host supplies this function and guarantees its lifetime above.
        if unsafe { send(context, bytes.as_ptr(), bytes.len()) } {
            Ok(())
        } else {
            Err(PluginError::new(
                ErrorCode::Native,
                "native transport rejected message",
            ))
        }
    });
    lock().insert(handle, bridge);
    handle
}
/// Deliver a UTF-8 JSON response/event. Invalid input returns false and allocates no handle.
/// # Safety
/// `data` must identify `len` readable bytes for this call. Null is accepted only with length zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn omen_bridge_receive(handle: u64, data: *const u8, len: usize) -> bool {
    if len == 0 || len > MAX_MESSAGE_BYTES || data.is_null() {
        return false;
    }
    let Ok(bridge) = bridge(handle) else {
        return false;
    };
    // Borrowing is bounded by the caller's pointer/length contract; bytes are parsed synchronously.
    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bridge.receive(bytes).is_ok()
    }))
    .unwrap_or(false)
}
/// Detach a bridge and fail pending calls. Repeated destruction is harmless.
/// Hosts must unregister their callback before freeing its associated context.
#[unsafe(no_mangle)]
pub extern "C" fn omen_bridge_destroy(handle: u64) {
    let removed = lock().remove(&handle);
    if let Some(bridge) = removed {
        bridge.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    unsafe extern "C" fn accept(_context: u64, _data: *const u8, _len: usize) -> bool {
        true
    }
    #[test]
    fn opaque_handles_validate_lengths_and_detach_without_cross_allocator_buffers() {
        // The callback is a static function and remains valid for the entire test.
        let handle = unsafe { omen_bridge_create(Some(accept), 7) };
        assert_ne!(handle, 0);
        assert!(bridge(handle).is_ok());
        // Null and oversized inputs are rejected before dereferencing the pointer.
        assert!(!unsafe { omen_bridge_receive(handle, std::ptr::null(), 0) });
        assert!(!unsafe { omen_bridge_receive(handle, std::ptr::null(), MAX_MESSAGE_BYTES + 1) });
        let message = b"malformed";
        assert!(!unsafe { omen_bridge_receive(handle, message.as_ptr(), message.len()) });
        omen_bridge_destroy(handle);
        omen_bridge_destroy(handle);
        assert!(bridge(handle).is_err());
    }
}
