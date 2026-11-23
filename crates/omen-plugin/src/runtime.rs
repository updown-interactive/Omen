use crate::*;
use futures::{
    Stream, StreamExt,
    channel::mpsc,
    future::{AbortHandle, Abortable},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    pin::Pin,
    sync::{
        Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    task::{Context, Poll},
};
/// Observable plugin lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginState {
    /// Registered, not initialized.
    Registered,
    /// Initialization is in progress.
    Starting,
    /// Accepts operations/subscriptions.
    Ready,
    /// Shutdown in progress.
    Stopping,
    /// Shutdown completed.
    Stopped,
    /// Initialization failed or was cancelled.
    Failed,
}
struct Entry {
    plugin: Arc<dyn Plugin>,
    descriptor: PluginDescriptor,
    state: PluginState,
    registration: u64,
}
struct Active {
    plugin: String,
    abort: AbortHandle,
    cancelled: Arc<AtomicBool>,
}
struct Listener {
    plugin: String,
    topic: String,
    sender: mpsc::Sender<PluginResult<Json>>,
    lagged: Arc<AtomicBool>,
}
struct Inner {
    plugins: Mutex<BTreeMap<String, Entry>>,
    broker: Arc<dyn PermissionBroker>,
    allowlist: BTreeSet<String>,
    active: Mutex<BTreeMap<u64, Active>>,
    listeners: Mutex<BTreeMap<u64, Listener>>,
    next: AtomicU64,
    stopped: AtomicBool,
}
impl Drop for Inner {
    fn drop(&mut self) {
        for active in lock(&self.active).values() {
            active.cancelled.store(true, Ordering::Release);
            active.abort.abort();
        }
    }
}
/// An isolated plugin registry and host security policy. Clones share one runtime.
#[derive(Clone)]
pub struct PluginRuntime(Arc<Inner>);
impl PluginRuntime {
    /// Construct a deny-by-default registry with an application permission ceiling.
    pub fn new(
        allowlist: impl IntoIterator<Item = impl Into<String>>,
        broker: Arc<dyn PermissionBroker>,
    ) -> Self {
        Self(Arc::new(Inner {
            plugins: Mutex::default(),
            broker,
            allowlist: allowlist.into_iter().map(Into::into).collect(),
            active: Mutex::default(),
            listeners: Mutex::default(),
            next: AtomicU64::new(1),
            stopped: AtomicBool::new(false),
        }))
    }
    /// Register a trusted implementation; no lifecycle hooks or operations run yet.
    pub fn register(&self, plugin: impl Plugin) -> PluginResult<()> {
        if self.0.stopped.load(Ordering::Acquire) {
            return Err(unavailable());
        }
        let descriptor = plugin.descriptor();
        validate_descriptor(&descriptor)?;
        let mut plugins = lock(&self.0.plugins);
        if self.0.stopped.load(Ordering::Acquire) {
            return Err(unavailable());
        }
        if plugins.contains_key(&descriptor.id) {
            return Err(PluginError::new(
                ErrorCode::InvalidRequest,
                "duplicate plugin ID",
            ));
        }
        plugins.insert(
            descriptor.id.clone(),
            Entry {
                plugin: Arc::new(plugin),
                descriptor,
                state: PluginState::Registered,
                registration: self.next_id()?,
            },
        );
        Ok(())
    }
    /// Enumerate capabilities independently of the current permission grant.
    pub fn discover(&self) -> Vec<(PluginDescriptor, PluginState)> {
        lock(&self.0.plugins)
            .values()
            .map(|entry| (entry.descriptor.clone(), entry.state))
            .collect()
    }
    /// Ask the platform broker for permission, bounded by the application allowlist.
    pub async fn request_permission(&self, permission: &str) -> PluginResult<PermissionStatus> {
        if self.0.stopped.load(Ordering::Acquire) {
            return Err(unavailable());
        }
        if !self.0.allowlist.contains(permission) {
            return Err(denied());
        }
        self.0.broker.request(permission.to_string()).await
    }
    /// Initialize one registered plugin. Cancellation leaves it failed, never ready.
    pub async fn start(&self, id: &str) -> PluginResult<()> {
        if self.0.stopped.load(Ordering::Acquire) {
            return Err(unavailable());
        }
        let plugin = {
            let mut plugins = lock(&self.0.plugins);
            let entry = plugins.get_mut(id).ok_or_else(not_found)?;
            if entry.state != PluginState::Registered {
                return Err(unavailable());
            }
            entry.state = PluginState::Starting;
            entry.plugin.clone()
        };
        let mut guard = LifecycleGuard {
            runtime: self.clone(),
            id: id.into(),
            completed: false,
        };
        let result = plugin
            .start(self.context(id, Arc::new(AtomicBool::new(false))))
            .await;
        if result.is_err() {
            let _ = plugin.shutdown().await;
        }
        if self.0.stopped.load(Ordering::Acquire) {
            if result.is_ok() {
                let _ = plugin.shutdown().await;
            }
            if let Some(entry) = lock(&self.0.plugins).get_mut(id) {
                entry.state = PluginState::Stopped;
            }
            guard.completed = true;
            return Err(unavailable());
        }
        if let Some(entry) = lock(&self.0.plugins).get_mut(id)
            && entry.state == PluginState::Starting
        {
            entry.state = if result.is_ok() {
                PluginState::Ready
            } else {
                PluginState::Failed
            };
        }
        guard.completed = true;
        result
    }
    fn context(&self, id: &str, cancelled: Arc<AtomicBool>) -> PluginContext {
        PluginContext {
            runtime: Arc::downgrade(&self.0),
            plugin: id.into(),
            cancelled,
            permissions: lock(&self.0.plugins)
                .get(id)
                .map(|entry| {
                    entry
                        .descriptor
                        .operations
                        .iter()
                        .flat_map(|cap| cap.permissions.iter().cloned())
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
    /// Create a typed cancellable operation. Native work starts when the future is polled.
    pub fn call<O: Operation>(&self, request: O) -> PluginCall<O::Output> {
        let payload = serde_json::to_value(request)
            .map_err(|_| PluginError::new(ErrorCode::InvalidRequest, "request encoding failed"));
        let runtime = self.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let token = cancelled.clone();
        let (abort, registration) = AbortHandle::new_pair();
        let active_abort = abort.clone();
        let future = async move {
            let payload = payload?;
            let (plugin, capability) = runtime.authorize_operation(O::PLUGIN, O::NAME)?;
            if serde_json::to_vec(&payload).map_or(true, |bytes| bytes.len() > MAX_MESSAGE_BYTES) {
                return Err(PluginError::new(
                    ErrorCode::InvalidRequest,
                    "request exceeds message limit",
                ));
            }
            let id = runtime.next_id()?;
            {
                let plugins = lock(&runtime.0.plugins);
                if runtime.0.stopped.load(Ordering::Acquire)
                    || !plugins
                        .get(O::PLUGIN)
                        .is_some_and(|entry| entry.state == PluginState::Ready)
                {
                    return Err(unavailable());
                }
                let mut active = lock(&runtime.0.active);
                if active.len() >= 256 {
                    return Err(PluginError::new(
                        ErrorCode::Unavailable,
                        "too many in-flight calls",
                    ));
                }
                active.insert(
                    id,
                    Active {
                        plugin: O::PLUGIN.into(),
                        abort: active_abort,
                        cancelled: token.clone(),
                    },
                );
            }
            let _guard = CallGuard {
                runtime: Arc::downgrade(&runtime.0),
                id,
            };
            let mut context = runtime.context(O::PLUGIN, token);
            context.permissions = capability.permissions.into_iter().collect();
            let result = plugin.invoke(O::NAME, payload, context).await?;
            if serde_json::to_vec(&result).map_or(true, |bytes| bytes.len() > MAX_MESSAGE_BYTES) {
                return Err(PluginError::new(
                    ErrorCode::Native,
                    "response exceeds message limit",
                ));
            }
            serde_json::from_value(result).map_err(|_| {
                PluginError::new(
                    ErrorCode::Native,
                    "plugin returned an incompatible response",
                )
            })
        };
        PluginCall {
            future: Box::pin(async move {
                Abortable::new(future, registration)
                    .await
                    .unwrap_or_else(|_| {
                        Err(PluginError::new(
                            ErrorCode::Cancelled,
                            "operation cancelled",
                        ))
                    })
            }),
            abort,
            cancelled,
        }
    }
    fn next_id(&self) -> PluginResult<u64> {
        self.0
            .next
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |id| id.checked_add(1))
            .map_err(|_| PluginError::new(ErrorCode::Unavailable, "request ID space exhausted"))
    }
    fn authorize_operation(
        &self,
        plugin: &str,
        name: &str,
    ) -> PluginResult<(Arc<dyn Plugin>, Capability)> {
        if self.0.stopped.load(Ordering::Acquire) {
            return Err(unavailable());
        }
        let (native, capability) = {
            let plugins = lock(&self.0.plugins);
            let entry = plugins.get(plugin).ok_or_else(not_found)?;
            if entry.state != PluginState::Ready {
                return Err(unavailable());
            }
            (
                entry.plugin.clone(),
                entry
                    .descriptor
                    .operations
                    .iter()
                    .find(|cap| cap.name == name)
                    .cloned()
                    .ok_or_else(not_found)?,
            )
        };
        authorize(&self.0, &capability)?;
        Ok((native, capability))
    }
    /// Subscribe to a declared event with bounded backpressure. Dropping unsubscribes.
    pub fn subscribe<E: PluginEvent>(&self, capacity: usize) -> PluginResult<EventSubscription<E>> {
        if self.0.stopped.load(Ordering::Acquire) {
            return Err(unavailable());
        }
        if capacity == 0 || capacity > 4096 {
            return Err(PluginError::new(
                ErrorCode::InvalidRequest,
                "event capacity must be 1 through 4096",
            ));
        }
        let capability = {
            let plugins = lock(&self.0.plugins);
            let entry = plugins.get(E::PLUGIN).ok_or_else(not_found)?;
            if entry.state != PluginState::Ready {
                return Err(unavailable());
            }
            entry
                .descriptor
                .events
                .iter()
                .find(|cap| cap.name == E::NAME)
                .cloned()
                .ok_or_else(not_found)?
        };
        authorize(&self.0, &capability)?;
        let id = self.next_id()?;
        let (sender, receiver) = mpsc::channel(capacity);
        let lagged = Arc::new(AtomicBool::new(false));
        let plugins = lock(&self.0.plugins);
        if self.0.stopped.load(Ordering::Acquire)
            || !plugins
                .get(E::PLUGIN)
                .is_some_and(|entry| entry.state == PluginState::Ready)
        {
            return Err(unavailable());
        }
        lock(&self.0.listeners).insert(
            id,
            Listener {
                plugin: E::PLUGIN.into(),
                topic: E::NAME.into(),
                sender,
                lagged: lagged.clone(),
            },
        );
        Ok(EventSubscription {
            receiver,
            runtime: Arc::downgrade(&self.0),
            id,
            lagged,
            marker: std::marker::PhantomData,
            terminated: false,
        })
    }
    /// Stop one plugin, cancel its calls, close its subscriptions, and release resources.
    pub async fn stop(&self, id: &str) -> PluginResult<()> {
        let plugin = {
            let mut plugins = lock(&self.0.plugins);
            let entry = plugins.get_mut(id).ok_or_else(not_found)?;
            if entry.state == PluginState::Starting {
                return Err(unavailable());
            }
            if matches!(entry.state, PluginState::Stopped | PluginState::Stopping) {
                return Ok(());
            }
            entry.state = PluginState::Stopping;
            entry.plugin.clone()
        };
        for active in lock(&self.0.active)
            .values()
            .filter(|active| active.plugin == id)
        {
            active.cancelled.store(true, Ordering::Release);
            active.abort.abort();
        }
        lock(&self.0.listeners).retain(|_, listener| listener.plugin != id);
        let result = plugin.shutdown().await;
        if let Some(entry) = lock(&self.0.plugins).get_mut(id) {
            entry.state = PluginState::Stopped;
        }
        result
    }
    /// Shut down all plugins in reverse registration order; attempts all cleanups.
    pub async fn shutdown(&self) -> PluginResult<()> {
        self.0.stopped.store(true, Ordering::Release);
        let mut entries = lock(&self.0.plugins)
            .values()
            .map(|entry| (entry.registration, entry.descriptor.id.clone()))
            .collect::<Vec<_>>();
        entries.sort_by_key(|(order, _)| std::cmp::Reverse(*order));
        let ids = entries.into_iter().map(|(_, id)| id);
        let mut failure = None;
        for id in ids {
            if let Err(error) = self.stop(&id).await {
                failure.get_or_insert(error);
            }
        }
        failure.map_or(Ok(()), Err)
    }
}
fn authorize(inner: &Inner, cap: &Capability) -> PluginResult<()> {
    if !cap.available {
        return Err(PluginError::new(
            ErrorCode::Unsupported,
            "capability unavailable on this host",
        ));
    }
    for permission in &cap.permissions {
        if !inner.allowlist.contains(permission)
            || inner.broker.status(permission) != PermissionStatus::Granted
        {
            return Err(denied());
        }
    }
    Ok(())
}
fn validate_descriptor(descriptor: &PluginDescriptor) -> PluginResult<()> {
    if descriptor.api_version != API_VERSION {
        return Err(PluginError::new(
            ErrorCode::IncompatibleVersion,
            "plugin API major version mismatch",
        ));
    }
    fn identifier(id: &str) -> bool {
        !id.is_empty()
            && id.len() <= 128
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    }
    if !identifier(&descriptor.id) || descriptor.version.is_empty() {
        return Err(PluginError::new(
            ErrorCode::InvalidRequest,
            "invalid plugin identity",
        ));
    }
    for capabilities in [&descriptor.operations, &descriptor.events] {
        let mut names = BTreeSet::new();
        for cap in capabilities {
            if !identifier(&cap.name)
                || !names.insert(&cap.name)
                || cap.permissions.iter().any(|p| !identifier(p))
            {
                return Err(PluginError::new(
                    ErrorCode::InvalidRequest,
                    "invalid or duplicate capability",
                ));
            }
        }
    }
    Ok(())
}
fn unavailable() -> PluginError {
    PluginError::new(ErrorCode::Unavailable, "plugin is not ready")
}
fn not_found() -> PluginError {
    PluginError::new(ErrorCode::NotFound, "plugin or capability not found")
}
fn denied() -> PluginError {
    PluginError::new(
        ErrorCode::PermissionDenied,
        "permission not granted by host policy",
    )
}
struct LifecycleGuard {
    runtime: PluginRuntime,
    id: String,
    completed: bool,
}
impl Drop for LifecycleGuard {
    fn drop(&mut self) {
        if !self.completed
            && let Some(entry) = lock(&self.runtime.0.plugins).get_mut(&self.id)
            && entry.state == PluginState::Starting
        {
            entry.state = PluginState::Failed;
        }
    }
}
struct CallGuard {
    runtime: Weak<Inner>,
    id: u64,
}
impl Drop for CallGuard {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.upgrade() {
            lock(&runtime.active).remove(&self.id);
        }
    }
}
/// An owned operation future. Drop or cancel stops further polling.
pub struct PluginCall<T> {
    future: BoxFuture<'static, PluginResult<T>>,
    abort: AbortHandle,
    cancelled: Arc<AtomicBool>,
}
impl<T> PluginCall<T> {
    /// Cancel this invocation. Native side effects already performed are not rolled back.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.abort.abort();
    }
}
impl<T> Future for PluginCall<T> {
    type Output = PluginResult<T>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.future.as_mut().poll(cx)
    }
}
impl<T> Drop for PluginCall<T> {
    fn drop(&mut self) {
        self.cancel();
    }
}
/// Scoped plugin context. It cannot emit events on another plugin's topics.
#[derive(Clone)]
pub struct PluginContext {
    runtime: Weak<Inner>,
    plugin: String,
    cancelled: Arc<AtomicBool>,
    permissions: BTreeSet<String>,
}
impl PluginContext {
    /// Check cooperative cancellation before a native side effect.
    pub fn check_cancelled(&self) -> PluginResult<()> {
        if self.cancelled.load(Ordering::Acquire) {
            Err(PluginError::new(
                ErrorCode::Cancelled,
                "operation cancelled",
            ))
        } else {
            Ok(())
        }
    }
    /// Recheck a declared permission immediately before a native side effect.
    pub fn require(&self, permission: &str) -> PluginResult<()> {
        self.check_cancelled()?;
        let runtime = self.runtime.upgrade().ok_or_else(unavailable)?;
        if runtime.stopped.load(Ordering::Acquire)
            || !lock(&runtime.plugins)
                .get(&self.plugin)
                .is_some_and(|entry| {
                    matches!(entry.state, PluginState::Ready | PluginState::Starting)
                })
        {
            return Err(unavailable());
        }
        let declared = self.permissions.contains(permission);
        if !declared
            || !runtime.allowlist.contains(permission)
            || runtime.broker.status(permission) != PermissionStatus::Granted
        {
            return Err(denied());
        }
        Ok(())
    }
    /// Publish a typed event; unauthorized or lagging listeners are closed.
    pub fn emit<E: PluginEvent>(&self, event: &E) -> PluginResult<()> {
        if E::PLUGIN != self.plugin {
            return Err(denied());
        }
        self.emit_json(
            E::NAME,
            serde_json::to_value(event).map_err(|_| {
                PluginError::new(ErrorCode::InvalidRequest, "event encoding failed")
            })?,
        )
    }
    pub(crate) fn emit_json(&self, topic: &str, payload: Json) -> PluginResult<()> {
        self.check_cancelled()?;
        let runtime = self.runtime.upgrade().ok_or_else(unavailable)?;
        let cap = {
            let plugins = lock(&runtime.plugins);
            let entry = plugins.get(&self.plugin).ok_or_else(not_found)?;
            if entry.state != PluginState::Ready {
                return Err(unavailable());
            }
            entry
                .descriptor
                .events
                .iter()
                .find(|cap| cap.name == topic)
                .cloned()
                .ok_or_else(not_found)?
        };
        authorize(&runtime, &cap)?;
        if serde_json::to_vec(&payload).map_or(true, |bytes| bytes.len() > MAX_MESSAGE_BYTES) {
            return Err(PluginError::new(
                ErrorCode::InvalidRequest,
                "event exceeds message limit",
            ));
        }
        lock(&runtime.listeners).retain(|_, listener| {
            if listener.plugin != self.plugin || listener.topic != topic {
                return true;
            }
            if listener.sender.try_send(Ok(payload.clone())).is_err() {
                listener.lagged.store(true, Ordering::Release);
                false
            } else {
                true
            }
        });
        Ok(())
    }
}
/// A typed event stream that unsubscribes on drop and reports overflow explicitly.
pub struct EventSubscription<E> {
    receiver: mpsc::Receiver<PluginResult<Json>>,
    runtime: Weak<Inner>,
    id: u64,
    lagged: Arc<AtomicBool>,
    marker: std::marker::PhantomData<fn() -> E>,
    terminated: bool,
}
impl<E: PluginEvent> Stream for EventSubscription<E> {
    type Item = PluginResult<E>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.terminated {
            return Poll::Ready(None);
        }
        if self.lagged.swap(false, Ordering::AcqRel) {
            self.terminated = true;
            self.receiver.close();
            return Poll::Ready(Some(Err(PluginError::new(
                ErrorCode::Lagged,
                "event subscription overflowed",
            ))));
        }
        let Some(runtime) = self.runtime.upgrade() else {
            self.terminated = true;
            return Poll::Ready(None);
        };
        let capability = {
            let plugins = lock(&runtime.plugins);
            plugins
                .get(E::PLUGIN)
                .filter(|entry| entry.state == PluginState::Ready)
                .and_then(|entry| {
                    entry
                        .descriptor
                        .events
                        .iter()
                        .find(|cap| cap.name == E::NAME)
                })
                .cloned()
        };
        let Some(capability) = capability else {
            self.terminated = true;
            self.receiver.close();
            return Poll::Ready(None);
        };
        if runtime.stopped.load(Ordering::Acquire) {
            self.terminated = true;
            self.receiver.close();
            return Poll::Ready(None);
        }
        let denied = authorize(&runtime, &capability).err();
        if let Some(error) = denied {
            self.terminated = true;
            self.receiver.close();
            if let Some(runtime) = self.runtime.upgrade() {
                lock(&runtime.listeners).remove(&self.id);
            }
            return Poll::Ready(Some(Err(error)));
        }
        match self.receiver.poll_next_unpin(cx) {
            Poll::Ready(Some(Ok(payload))) => {
                Poll::Ready(Some(serde_json::from_value(payload).map_err(|_| {
                    PluginError::new(ErrorCode::Native, "incompatible event payload")
                })))
            }
            Poll::Ready(Some(Err(error))) => Poll::Ready(Some(Err(error))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
impl<E> Drop for EventSubscription<E> {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.upgrade() {
            lock(&runtime.listeners).remove(&self.id);
        }
    }
}
