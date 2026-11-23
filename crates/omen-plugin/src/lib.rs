//! Typed, permission-gated native plugins independent of a UI renderer.
#![deny(missing_docs)]
#![forbid(unsafe_code)]
mod bridge;
mod runtime;
pub mod standard;
pub use bridge::*;
use futures::future::BoxFuture;
pub use runtime::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
pub use serde_json::Value as Json;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, MutexGuard},
};

pub(crate) fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
/// Protocol/API major version supported by this runtime.
pub const API_VERSION: u32 = 1;
/// Maximum encoded message size across a native boundary (one MiB).
pub const MAX_MESSAGE_BYTES: usize = 1_048_576;
/// A stable machine-readable plugin failure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct PluginError {
    /// Error category; callers should branch on this rather than the message.
    pub code: ErrorCode,
    /// Safe diagnostic. Avoid including secrets or absolute filesystem paths.
    pub message: String,
}
/// Stable wire error categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// No registered plugin.
    NotFound,
    /// Invalid payload or duplicate registration.
    InvalidRequest,
    /// Permission not granted or not in the application's allowlist.
    PermissionDenied,
    /// Host does not implement the capability.
    Unsupported,
    /// Incompatible protocol major version.
    IncompatibleVersion,
    /// Plugin is not ready, or runtime is stopped.
    Unavailable,
    /// Caller cancelled the operation.
    Cancelled,
    /// Native I/O or other host operation failed.
    Native,
    /// Event consumer could not keep up.
    Lagged,
}
impl PluginError {
    /// Construct an error without exposing a host-specific error type.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
/// Result shared by all native interfaces.
pub type PluginResult<T> = Result<T, PluginError>;
/// A typed operation. Implementations define their request and response schemas.
pub trait Operation: Serialize + DeserializeOwned + Send + 'static {
    /// Typed response schema.
    type Output: Serialize + DeserializeOwned + Send + 'static;
    /// Registered plugin identifier.
    const PLUGIN: &'static str;
    /// Operation name within that plugin.
    const NAME: &'static str;
    /// Permission names required by this operation.
    const PERMISSIONS: &'static [&'static str];
}
/// A typed event emitted by a plugin.
pub trait PluginEvent: Serialize + DeserializeOwned + Send + 'static {
    /// Owning plugin.
    const PLUGIN: &'static str;
    /// Event topic.
    const NAME: &'static str;
    /// Required subscription permissions.
    const PERMISSIONS: &'static [&'static str];
}
/// Host grant status. A prompt is requested explicitly, never during invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionStatus {
    /// Not yet decided.
    Prompt,
    /// Host has granted this capability.
    Granted,
    /// Host denied access.
    Denied,
}
/// Platform permission broker. The application allowlist remains an additional ceiling.
pub trait PermissionBroker: Send + Sync + 'static {
    /// Current status, without presenting UI.
    fn status(&self, permission: &str) -> PermissionStatus;
    /// Explicit user-initiated permission request. Host adapters own any native prompt.
    fn request(&self, permission: String) -> BoxFuture<'static, PluginResult<PermissionStatus>>;
}
/// Host-controlled permissions, useful for desktop app-sandbox policy and tests.
#[derive(Clone, Default)]
pub struct PermissionStore(Arc<Mutex<BTreeMap<String, PermissionStatus>>>);
impl PermissionStore {
    /// Set a grant as the trusted host. Plugin operation payloads cannot modify grants.
    pub fn set(&self, permission: impl Into<String>, status: PermissionStatus) {
        lock(&self.0).insert(permission.into(), status);
    }
}
impl PermissionBroker for PermissionStore {
    fn status(&self, permission: &str) -> PermissionStatus {
        lock(&self.0)
            .get(permission)
            .copied()
            .unwrap_or(PermissionStatus::Denied)
    }
    fn request(&self, permission: String) -> BoxFuture<'static, PluginResult<PermissionStatus>> {
        let status = self.status(&permission);
        Box::pin(async move { Ok(status) })
    }
}
/// Capability metadata supplied by a trusted plugin implementation.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Capability {
    /// Stable operation/event name.
    pub name: String,
    /// Required permissions.
    pub permissions: Vec<String>,
    /// Whether this host can implement the capability.
    pub available: bool,
}
impl Capability {
    /// Metadata for a typed operation.
    pub fn operation<O: Operation>() -> Self {
        Self {
            name: O::NAME.into(),
            permissions: O::PERMISSIONS.iter().map(|p| (*p).into()).collect(),
            available: true,
        }
    }
    /// Metadata for a typed event.
    pub fn event<E: PluginEvent>() -> Self {
        Self {
            name: E::NAME.into(),
            permissions: E::PERMISSIONS.iter().map(|p| (*p).into()).collect(),
            available: true,
        }
    }
}
/// Discoverable plugin identity and API compatibility.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginDescriptor {
    /// Unique reverse-domain or application-defined plugin ID.
    pub id: String,
    /// Implementation version, informational; schema compatibility uses api_version.
    pub version: String,
    /// Required protocol major version.
    pub api_version: u32,
    /// Callable operations.
    pub operations: Vec<Capability>,
    /// Subscribable event topics.
    pub events: Vec<Capability>,
}
/// Object-safe native plugin lifecycle and operation interface.
pub trait Plugin: Send + Sync + 'static {
    /// Immutable declaration of supported operations, topics, and permissions.
    fn descriptor(&self) -> PluginDescriptor;
    /// Acquire native resources. Called explicitly once before use.
    fn start(&self, _context: PluginContext) -> BoxFuture<'static, PluginResult<()>> {
        Box::pin(async { Ok(()) })
    }
    /// Dispatch one validated, authorized operation.
    fn invoke(
        &self,
        operation: &str,
        payload: Json,
        context: PluginContext,
    ) -> BoxFuture<'static, PluginResult<Json>>;
    /// Release resources after in-flight calls are cancelled. Must be idempotent;
    /// cleanup may be retried after failed/partial initialization.
    fn shutdown(&self) -> BoxFuture<'static, PluginResult<()>> {
        Box::pin(async { Ok(()) })
    }
}
type Handler =
    Arc<dyn Fn(Json, PluginContext) -> BoxFuture<'static, PluginResult<Json>> + Send + Sync>;
/// Register typed Rust handlers without writing JSON dispatch by hand.
pub struct TypedPlugin {
    descriptor: PluginDescriptor,
    handlers: BTreeMap<String, Handler>,
}
impl TypedPlugin {
    /// Construct an empty implementation at the current API version.
    pub fn new(id: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            descriptor: PluginDescriptor {
                id: id.into(),
                version: version.into(),
                api_version: API_VERSION,
                operations: Vec::new(),
                events: Vec::new(),
            },
            handlers: BTreeMap::new(),
        }
    }
    /// Add a typed handler, rejecting duplicate or mismatched plugin IDs.
    pub fn operation<O, F, Fut>(mut self, handler: F) -> PluginResult<Self>
    where
        O: Operation,
        F: Fn(O, PluginContext) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = PluginResult<O::Output>> + Send + 'static,
    {
        if O::PLUGIN != self.descriptor.id || self.handlers.contains_key(O::NAME) {
            return Err(PluginError::new(
                ErrorCode::InvalidRequest,
                "duplicate operation or mismatched plugin ID",
            ));
        }
        self.descriptor
            .operations
            .push(Capability::operation::<O>());
        self.handlers.insert(
            O::NAME.into(),
            Arc::new(move |payload, context| {
                let parsed = serde_json::from_value(payload).map_err(|_| {
                    PluginError::new(ErrorCode::InvalidRequest, "invalid operation payload")
                });
                match parsed {
                    Ok(request) => {
                        let future = handler(request, context);
                        Box::pin(async move {
                            serde_json::to_value(future.await?).map_err(|_| {
                                PluginError::new(ErrorCode::Native, "response encoding failed")
                            })
                        })
                    }
                    Err(error) => Box::pin(async move { Err(error) }),
                }
            }),
        );
        Ok(self)
    }
    /// Declare a typed event topic emitted by a handler's context.
    pub fn event<E: PluginEvent>(mut self) -> PluginResult<Self> {
        if E::PLUGIN != self.descriptor.id
            || self
                .descriptor
                .events
                .iter()
                .any(|event| event.name == E::NAME)
        {
            return Err(PluginError::new(
                ErrorCode::InvalidRequest,
                "duplicate event or mismatched plugin ID",
            ));
        }
        self.descriptor.events.push(Capability::event::<E>());
        Ok(self)
    }
}
impl Plugin for TypedPlugin {
    fn descriptor(&self) -> PluginDescriptor {
        self.descriptor.clone()
    }
    fn invoke(
        &self,
        operation: &str,
        payload: Json,
        context: PluginContext,
    ) -> BoxFuture<'static, PluginResult<Json>> {
        if let Some(handler) = self.handlers.get(operation) {
            handler(payload, context)
        } else {
            Box::pin(async { Err(PluginError::new(ErrorCode::NotFound, "unknown operation")) })
        }
    }
}
