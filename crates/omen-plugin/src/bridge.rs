//! Versioned asynchronous bridge shared by JNI, C/Swift, and JavaScript hosts.
use crate::*;
use futures::{channel::oneshot, future::BoxFuture};
use std::{
    collections::BTreeMap,
    sync::{
        Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
/// Outbound native-bound messages. IDs are strings to preserve u64 precision in JS.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum HostRequest {
    /// Invoke a declared operation.
    Call {
        /// Protocol major version.
        api_version: u32,
        /// Correlation ID.
        id: String,
        /// Registered plugin ID.
        plugin: String,
        /// Declared operation name.
        operation: String,
        /// Typed JSON request.
        payload: Json,
    },
    /// Best-effort cooperative cancellation of host work.
    Cancel {
        /// Protocol major version.
        api_version: u32,
        /// Correlation ID.
        id: String,
    },
}
/// Messages returned by a native host.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum HostMessage {
    /// Successful response.
    Response {
        /// Protocol version.
        api_version: u32,
        /// Correlation ID.
        id: String,
        /// Typed JSON response.
        payload: Json,
    },
    /// Failed response.
    Error {
        /// Protocol version.
        api_version: u32,
        /// Correlation ID.
        id: String,
        /// Structured error.
        error: PluginError,
    },
    /// Unsolicited plugin event.
    Event {
        /// Protocol version.
        api_version: u32,
        /// Owning plugin.
        plugin: String,
        /// Declared event topic.
        topic: String,
        /// Typed JSON event.
        payload: Json,
    },
}
/// A thread-safe native send callback. Implementations must copy any borrowed bytes.
pub trait HostTransport: Send + Sync + 'static {
    /// Queue one UTF-8 JSON message without blocking for its response.
    fn send(&self, message: &[u8]) -> PluginResult<()>;
}
impl<F: Fn(&[u8]) -> PluginResult<()> + Send + Sync + 'static> HostTransport for F {
    fn send(&self, message: &[u8]) -> PluginResult<()> {
        self(message)
    }
}
struct Pending {
    plugin: String,
    sender: oneshot::Sender<PluginResult<Json>>,
}
struct BridgeInner {
    transport: Arc<dyn HostTransport>,
    pending: Mutex<BTreeMap<String, Pending>>,
    contexts: Mutex<BTreeMap<String, PluginContext>>,
    next: AtomicU64,
    closed: AtomicBool,
}
/// Correlates native requests, responses, cancellation, and events across one host.
#[derive(Clone)]
pub struct NativeBridge(Arc<BridgeInner>);
impl NativeBridge {
    /// Attach a trusted transport callback. It may deliver responses synchronously.
    pub fn new(transport: impl HostTransport) -> Self {
        Self(Arc::new(BridgeInner {
            transport: Arc::new(transport),
            pending: Mutex::default(),
            contexts: Mutex::default(),
            next: AtomicU64::new(1),
            closed: AtomicBool::new(false),
        }))
    }
    /// Construct a plugin proxy using host-declared, application-reviewed capabilities.
    pub fn plugin(&self, descriptor: PluginDescriptor) -> BridgePlugin {
        BridgePlugin {
            descriptor,
            bridge: self.clone(),
        }
    }
    /// Receive and validate a host response/event. Unknown/stale response IDs are rejected.
    pub fn receive(&self, bytes: &[u8]) -> PluginResult<()> {
        if self.0.closed.load(Ordering::Acquire) {
            return Err(closed());
        }
        if bytes.len() > MAX_MESSAGE_BYTES {
            return Err(PluginError::new(
                ErrorCode::InvalidRequest,
                "bridge message exceeds limit",
            ));
        }
        let message: HostMessage = serde_json::from_slice(bytes)
            .map_err(|_| PluginError::new(ErrorCode::InvalidRequest, "malformed bridge message"))?;
        let version = match &message {
            HostMessage::Response { api_version, .. }
            | HostMessage::Error { api_version, .. }
            | HostMessage::Event { api_version, .. } => *api_version,
        };
        if version != API_VERSION {
            return Err(PluginError::new(
                ErrorCode::IncompatibleVersion,
                "bridge major version mismatch",
            ));
        }
        match message {
            HostMessage::Response { id, payload, .. } => self.respond(id, Ok(payload)),
            HostMessage::Error { id, error, .. } => self.respond(id, Err(error)),
            HostMessage::Event {
                plugin,
                topic,
                payload,
                ..
            } => {
                let context = lock(&self.0.contexts)
                    .get(&plugin)
                    .cloned()
                    .ok_or_else(closed)?;
                context.emit_json(&topic, payload)
            }
        }
    }
    fn respond(&self, id: String, result: PluginResult<Json>) -> PluginResult<()> {
        let pending = lock(&self.0.pending).remove(&id).ok_or_else(|| {
            PluginError::new(
                ErrorCode::InvalidRequest,
                "unknown or completed response ID",
            )
        })?;
        let _ = pending.sender.send(result);
        Ok(())
    }
    fn invoke(
        &self,
        plugin: String,
        operation: String,
        payload: Json,
    ) -> BoxFuture<'static, PluginResult<Json>> {
        let bridge = self.clone();
        Box::pin(async move {
            if bridge.0.closed.load(Ordering::Acquire) {
                return Err(closed());
            }
            let id = bridge
                .0
                .next
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |id| id.checked_add(1))
                .map_err(|_| closed())?
                .to_string();
            let message = serde_json::to_vec(&HostRequest::Call {
                api_version: API_VERSION,
                id: id.clone(),
                plugin: plugin.clone(),
                operation,
                payload,
            })
            .map_err(|_| closed())?;
            if message.len() > MAX_MESSAGE_BYTES {
                return Err(PluginError::new(
                    ErrorCode::InvalidRequest,
                    "bridge message exceeds limit",
                ));
            }
            let (sender, receiver) = oneshot::channel();
            {
                let mut pending = lock(&bridge.0.pending);
                if bridge.0.closed.load(Ordering::Acquire) {
                    return Err(closed());
                }
                if pending.len() >= 256 {
                    return Err(PluginError::new(
                        ErrorCode::Unavailable,
                        "too many pending bridge calls",
                    ));
                }
                pending.insert(id.clone(), Pending { plugin, sender });
            }
            let guard = PendingGuard {
                bridge: Arc::downgrade(&bridge.0),
                id,
            };
            bridge.0.transport.send(&message)?;
            let result = receiver.await.map_err(|_| closed())?;
            drop(guard);
            result
        })
    }
    /// Close all pending calls. Late host callbacks are rejected.
    pub fn close(&self) {
        self.0.closed.store(true, Ordering::Release);
        let ids = lock(&self.0.pending).keys().cloned().collect::<Vec<_>>();
        for id in ids {
            cancel_pending(&self.0, &id);
        }
        lock(&self.0.contexts).clear();
    }
    fn stop_plugin(&self, plugin: &str) {
        lock(&self.0.contexts).remove(plugin);
        let ids = lock(&self.0.pending)
            .iter()
            .filter(|(_, pending)| pending.plugin == plugin)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in ids {
            cancel_pending(&self.0, &id);
        }
    }
}
fn closed() -> PluginError {
    PluginError::new(
        ErrorCode::Unavailable,
        "native bridge is closed or detached",
    )
}
fn cancel_pending(bridge: &BridgeInner, id: &str) {
    let removed = lock(&bridge.pending).remove(id);
    if removed.is_some()
        && let Ok(message) = serde_json::to_vec(&HostRequest::Cancel {
            api_version: API_VERSION,
            id: id.into(),
        })
    {
        let _ = bridge.transport.send(&message);
    }
}
struct PendingGuard {
    bridge: Weak<BridgeInner>,
    id: String,
}
impl Drop for PendingGuard {
    fn drop(&mut self) {
        if let Some(bridge) = self.bridge.upgrade() {
            cancel_pending(&bridge, &self.id);
        }
    }
}
/// A registered plugin whose operations execute through a platform bridge.
pub struct BridgePlugin {
    descriptor: PluginDescriptor,
    bridge: NativeBridge,
}
impl Plugin for BridgePlugin {
    fn descriptor(&self) -> PluginDescriptor {
        self.descriptor.clone()
    }
    fn start(&self, context: PluginContext) -> BoxFuture<'static, PluginResult<()>> {
        let bridge = self.bridge.clone();
        let id = self.descriptor.id.clone();
        Box::pin(async move {
            if bridge.0.closed.load(Ordering::Acquire) {
                return Err(closed());
            }
            lock(&bridge.0.contexts).insert(id, context);
            Ok(())
        })
    }
    fn invoke(
        &self,
        operation: &str,
        payload: Json,
        _context: PluginContext,
    ) -> BoxFuture<'static, PluginResult<Json>> {
        self.bridge
            .invoke(self.descriptor.id.clone(), operation.into(), payload)
    }
    fn shutdown(&self) -> BoxFuture<'static, PluginResult<()>> {
        self.bridge.stop_plugin(&self.descriptor.id);
        Box::pin(async { Ok(()) })
    }
}
