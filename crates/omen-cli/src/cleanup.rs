use std::sync::{Arc, Mutex, OnceLock};

type CleanupAction = Box<dyn FnOnce() + Send + 'static>;

struct State {
    handlers: Vec<Arc<Mutex<Option<CleanupAction>>>>,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

fn state() -> &'static Mutex<State> {
    STATE.get_or_init(|| {
        if let Err(e) = ctrlc::set_handler(|| {
            run_all();
            std::process::exit(130);
        }) {
            eprintln!("Warning: could not register exit signal handler: {e}");
        }
        Mutex::new(State {
            handlers: Vec::new(),
        })
    })
}

/// Run all registered cleanup handlers in reverse order of registration.
pub fn run_all() {
    let handlers = {
        let Ok(mut state) = state().lock() else {
            return;
        };
        std::mem::take(&mut state.handlers)
    };
    for handler in handlers.into_iter().rev() {
        if let Ok(mut opt) = handler.lock() {
            if let Some(f) = opt.take() {
                f();
            }
        }
    }
}

pub struct CleanupGuard {
    handler: Arc<Mutex<Option<CleanupAction>>>,
    disarmed: bool,
}

impl CleanupGuard {
    /// Disarm the cleanup guard so the action does not run.
    pub fn disarm(&mut self) {
        self.disarmed = true;
        if let Ok(mut opt) = self.handler.lock() {
            opt.take();
        }
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        if !self.disarmed {
            if let Ok(mut opt) = self.handler.lock() {
                if let Some(f) = opt.take() {
                    f();
                }
            }
        }
    }
}

/// Register a cleanup action that runs on process exit, signal interruption, or when the returned guard is dropped.
pub fn register<F>(cleanup: F) -> CleanupGuard
where
    F: FnOnce() + Send + 'static,
{
    let handler = Arc::new(Mutex::new(Some(Box::new(cleanup) as CleanupAction)));
    if let Ok(mut state) = state().lock() {
        state.handlers.push(Arc::clone(&handler));
    }
    CleanupGuard {
        handler,
        disarmed: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn guard_runs_cleanup_on_drop() {
        let ran = Arc::new(AtomicBool::new(false));
        let ran_clone = Arc::clone(&ran);
        {
            let _guard = register(move || {
                ran_clone.store(true, Ordering::SeqCst);
            });
            assert!(!ran.load(Ordering::SeqCst));
        }
        assert!(ran.load(Ordering::SeqCst));
    }

    #[test]
    fn guard_does_not_run_when_disarmed() {
        let ran = Arc::new(AtomicBool::new(false));
        let ran_clone = Arc::clone(&ran);
        {
            let mut guard = register(move || {
                ran_clone.store(true, Ordering::SeqCst);
            });
            guard.disarm();
        }
        assert!(!ran.load(Ordering::SeqCst));
    }
}
