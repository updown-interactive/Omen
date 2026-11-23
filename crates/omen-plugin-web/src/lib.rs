//! Web/WASM transport for Omen's native plugin protocol.
//! Install the matching JavaScript host before starting bridge plugins.
#![deny(missing_docs)]
#![forbid(unsafe_code)]
#[cfg(target_arch = "wasm32")]
mod bindings {
    use omen_plugin::{ErrorCode, NativeBridge, PluginError, PluginResult};
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen(module = "/../../platforms/web/host.mjs")]
    extern "C" {
        #[wasm_bindgen(js_name=sendMessage)]
        fn send_message(context: u32, bytes: &[u8]) -> bool;
    }
    /// A browser host endpoint. Keep it alive while its Rust plugin proxies are registered.
    #[wasm_bindgen]
    pub struct WebBridge {
        bridge: NativeBridge,
    }
    #[wasm_bindgen]
    impl WebBridge {
        /// Bind a context previously installed with createNativeHost in JavaScript.
        #[wasm_bindgen(constructor)]
        pub fn new(context: u32) -> Self {
            Self {
                bridge: NativeBridge::new(move |bytes: &[u8]| -> PluginResult<()> {
                    if send_message(context, bytes) {
                        Ok(())
                    } else {
                        Err(PluginError::new(
                            ErrorCode::Unavailable,
                            "web host is detached",
                        ))
                    }
                }),
            }
        }
        /// Deliver a response/event from the JavaScript host.
        pub fn receive(&self, bytes: &[u8]) -> Result<(), JsValue> {
            self.bridge
                .receive(bytes)
                .map_err(|error| JsValue::from_str(&error.to_string()))
        }
        /// Detach, failing pending work. Also close the matching JavaScript host.
        pub fn close(&self) {
            self.bridge.close();
        }
    }
    impl WebBridge {
        /// Register proxies from application Rust code against this bridge.
        pub fn native_bridge(&self) -> NativeBridge {
            self.bridge.clone()
        }
    }
}
#[cfg(target_arch = "wasm32")]
pub use bindings::WebBridge;
