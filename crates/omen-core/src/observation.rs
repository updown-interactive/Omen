use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

thread_local! {
    // Dynamic read scopes only; application values and runtimes remain explicitly owned.
    static READS: RefCell<Vec<Vec<Signal>>> = const { RefCell::new(Vec::new()) };
    static BATCH: RefCell<Batch> = const { RefCell::new(Batch { depth: 0, pending: Vec::new() }) };
}
struct Batch {
    depth: usize,
    pending: Vec<Signal>,
}

pub(crate) fn batch<R>(update: impl FnOnce() -> R) -> R {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            let pending = BATCH.with(|batch| {
                let mut batch = batch.borrow_mut();
                batch.depth -= 1;
                if batch.depth == 0 {
                    std::mem::take(&mut batch.pending)
                } else {
                    Vec::new()
                }
            });
            for signal in pending {
                signal.notify();
            }
        }
    }
    BATCH.with(|batch| batch.borrow_mut().depth += 1);
    let guard = Guard;
    let result = update();
    drop(guard);
    result
}

type Listener = (Weak<dyn Fn()>, Weak<()>);
#[derive(Clone)]
pub(crate) struct Signal(Rc<RefCell<Vec<Listener>>>);
pub(crate) struct Subscription {
    _active: Rc<()>,
}
impl Signal {
    pub(crate) fn new() -> Self {
        Self(Rc::default())
    }
    pub(crate) fn track(&self) {
        READS.with(|reads| {
            if let Some(current) = reads.borrow_mut().last_mut()
                && !current.iter().any(|signal| Rc::ptr_eq(&signal.0, &self.0))
            {
                current.push(self.clone());
            }
        });
    }
    pub(crate) fn subscribe(&self, callback: &Rc<dyn Fn()>) -> Subscription {
        let active = Rc::new(());
        let mut listeners = self.0.borrow_mut();
        listeners
            .retain(|(callback, active)| callback.strong_count() > 0 && active.strong_count() > 0);
        listeners.push((Rc::downgrade(callback), Rc::downgrade(&active)));
        Subscription { _active: active }
    }
    pub(crate) fn notify(&self) {
        if BATCH.with(|batch| {
            let mut batch = batch.borrow_mut();
            if batch.depth == 0 {
                return false;
            }
            if !batch
                .pending
                .iter()
                .any(|pending| Rc::ptr_eq(&pending.0, &self.0))
            {
                batch.pending.push(self.clone());
            }
            true
        }) {
            return;
        }
        let callbacks = {
            let mut listeners = self.0.borrow_mut();
            listeners.retain(|(callback, active)| {
                callback.strong_count() > 0 && active.strong_count() > 0
            });
            listeners
                .iter()
                .filter_map(|(callback, _)| callback.upgrade())
                .collect::<Vec<_>>()
        };
        for callback in callbacks {
            callback();
        }
    }
}
pub(crate) fn track<R>(build: impl FnOnce() -> R) -> (R, Vec<Signal>) {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            READS.with(|reads| {
                reads.borrow_mut().pop();
            });
        }
    }
    READS.with(|reads| reads.borrow_mut().push(Vec::new()));
    let guard = Guard;
    let result = build();
    let signals = READS.with(|reads| reads.borrow().last().cloned().unwrap_or_default());
    drop(guard);
    (result, signals)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn batches_are_nested_and_callbacks_can_reenter_after_borrows_end() {
        let value = crate::Value::new(0);
        let (_, signals) = track(|| value.get());
        let calls = Rc::new(Cell::new(0));
        let observed = calls.clone();
        let changed = value.clone();
        let callback: Rc<dyn Fn()> = Rc::new(move || {
            observed.set(observed.get() + 1);
            if changed.get() == 3 {
                changed.set(4);
            }
        });
        let _subscription = signals[0].subscribe(&callback);
        batch(|| {
            value.update(|current| *current += 1);
            batch(|| value.set(2));
            value.set(3);
            assert_eq!(calls.get(), 0);
        });
        assert_eq!(value.get(), 4);
        assert_eq!(calls.get(), 2);
    }
}
