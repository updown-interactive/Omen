//! Cancellable local futures, independent of a rendering backend.
use crate::{Action, Value};
use futures::future::{AbortHandle, AbortRegistration, Abortable, LocalBoxFuture};
use std::{cell::RefCell, fmt, future::Future, rc::Rc};

/// Result of an asynchronous task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskStatus {
    /// Waiting for the host executor.
    Pending,
    /// Currently being polled.
    Running,
    /// Completed successfully.
    Succeeded,
    /// Cancelled by its owner or a newer invocation.
    Cancelled,
    /// Failed with a displayable error.
    Failed(String),
}
/// A handle that can cancel work and observe its status.
#[derive(Clone)]
pub struct TaskHandle {
    abort: AbortHandle,
    status: Value<TaskStatus>,
}
impl TaskHandle {
    /// Cancel pending/running work. Completed work remains completed.
    pub fn cancel(&self) {
        if matches!(self.status.get(), TaskStatus::Pending | TaskStatus::Running) {
            self.abort.abort();
            self.status.set(TaskStatus::Cancelled);
        }
    }
    /// Current task status.
    pub fn status(&self) -> TaskStatus {
        self.status.get()
    }
}
impl fmt::Debug for TaskHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TaskHandle")
            .field("status", &self.status())
            .finish()
    }
}
/// A supported async action return value.
pub trait TaskOutput {
    /// Convert unit or a Result into a standard task result.
    fn into_task_result(self) -> Result<(), String>;
}
impl TaskOutput for () {
    fn into_task_result(self) -> Result<(), String> {
        Ok(())
    }
}
impl<E: fmt::Display> TaskOutput for Result<(), E> {
    fn into_task_result(self) -> Result<(), String> {
        self.map_err(|error| error.to_string())
    }
}
/// A repeatable factory for a local asynchronous action.
#[derive(Clone)]
pub struct AsyncAction(Rc<dyn Fn() -> LocalBoxFuture<'static, Result<(), String>>>);
impl AsyncAction {
    /// Create a factory. Each invocation gets a fresh future.
    pub fn new<F, Fut>(factory: F) -> Self
    where
        F: Fn() -> Fut + 'static,
        Fut: Future + 'static,
        Fut::Output: TaskOutput,
    {
        Self(Rc::new(move || {
            let future = factory();
            Box::pin(async move { future.await.into_task_result() })
        }))
    }
}
impl PartialEq for AsyncAction {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl fmt::Debug for AsyncAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AsyncAction(..)")
    }
}
/// An asynchronous error callback.
#[derive(Clone)]
pub struct ErrorAction(Rc<dyn Fn(String)>);
impl ErrorAction {
    /// Create an error handler.
    pub fn new(handler: impl Fn(String) + 'static) -> Self {
        Self(Rc::new(handler))
    }
}
impl PartialEq for ErrorAction {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl fmt::Debug for ErrorAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ErrorAction(..)")
    }
}
/// Queued local work. A renderer supplies the executor; tests can poll it directly.
pub struct TaskJob {
    future: LocalBoxFuture<'static, Result<(), String>>,
    registration: AbortRegistration,
    handle: TaskHandle,
    error: Option<ErrorAction>,
}
impl TaskJob {
    /// Poll until completion or cancellation, handling failures without panicking.
    pub async fn run(self) {
        if self.handle.status() == TaskStatus::Cancelled {
            return;
        }
        self.handle.status.set(TaskStatus::Running);
        match Abortable::new(self.future, self.registration).await {
            Ok(Ok(())) if self.handle.status() != TaskStatus::Cancelled => {
                self.handle.status.set(TaskStatus::Succeeded)
            }
            Ok(Err(error)) if self.handle.status() != TaskStatus::Cancelled => {
                self.handle.status.set(TaskStatus::Failed(error.clone()));
                if let Some(handler) = self.error {
                    (handler.0)(error);
                }
            }
            _ => self.handle.cancel(),
        }
    }
}
#[derive(Default)]
pub(crate) struct TaskQueue(pub RefCell<Vec<TaskJob>>);
/// Runtime-owned task dispatch for a mounted view node.
#[derive(Clone)]
pub struct TaskContext {
    pub(crate) queue: Rc<TaskQueue>,
    pub(crate) owner: std::rc::Weak<crate::reactive::ScopeInner>,
    pub(crate) key: String,
    pub(crate) wake: Rc<dyn Fn()>,
}
impl TaskContext {
    /// Start work, cancelling the previous invocation at this node (latest wins).
    pub fn spawn(&self, action: &AsyncAction, error: Option<ErrorAction>) -> TaskHandle {
        let (abort, registration) = AbortHandle::new_pair();
        let handle = TaskHandle {
            abort,
            status: Value::new(TaskStatus::Pending),
        };
        if let Some(owner) = self.owner.upgrade() {
            if let Some(previous) = owner
                .tasks
                .borrow_mut()
                .insert(self.key.clone(), handle.clone())
            {
                previous.cancel();
            }
            self.queue.0.borrow_mut().push(TaskJob {
                future: (action.0)(),
                registration,
                handle: handle.clone(),
                error,
            });
            (self.wake)();
        } else {
            handle.cancel();
        }
        handle
    }
    pub(crate) fn cancel(&self) {
        if let Some(owner) = self.owner.upgrade()
            && let Some(task) = owner.tasks.borrow_mut().remove(&self.key)
        {
            task.cancel();
        }
    }
}
impl PartialEq for TaskContext {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.owner.ptr_eq(&other.owner)
    }
}
impl fmt::Debug for TaskContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TaskContext").field(&self.key).finish()
    }
}
pub(crate) trait EffectKey: fmt::Debug {
    fn as_any(&self) -> &dyn std::any::Any;
    fn same(&self, other: &dyn EffectKey) -> bool;
}
impl<T: fmt::Debug + PartialEq + 'static> EffectKey for T {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn same(&self, other: &dyn EffectKey) -> bool {
        other
            .as_any()
            .downcast_ref::<T>()
            .is_some_and(|value| self == value)
    }
}
/// Cleanup-producing effect keyed by a dependency snapshot.
#[derive(Clone)]
pub struct Effect {
    pub(crate) key: Rc<dyn EffectKey>,
    pub(crate) setup: Rc<dyn Fn() -> Action>,
}
impl Effect {
    /// Create an effect. Setup returns an action run before replacement or unmount.
    pub fn new(
        key: impl fmt::Debug + PartialEq + 'static,
        setup: impl Fn() -> Action + 'static,
    ) -> Self {
        Self {
            key: Rc::new(key),
            setup: Rc::new(setup),
        }
    }
}
impl PartialEq for Effect {
    fn eq(&self, other: &Self) -> bool {
        self.key.same(other.key.as_ref()) && Rc::ptr_eq(&self.setup, &other.setup)
    }
}
impl fmt::Debug for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Effect").field(&self.key).finish()
    }
}
