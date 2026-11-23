//! Latest-selection worker: parsing never runs on the GPUI thread.
use omen::TextSnapshot;
use omen_macro_core::inspector::{self, InspectedNode};
use std::sync::{Arc, Condvar, Mutex};
type Reply = futures::channel::oneshot::Sender<Result<Option<InspectedNode>, String>>;
struct Request {
    text: TextSnapshot,
    cursor: usize,
    reply: Reply,
}
#[derive(Default)]
struct Queue {
    latest: Option<Request>,
    stopped: bool,
}
struct Worker {
    queue: Arc<(Mutex<Queue>, Condvar)>,
    thread: Option<std::thread::JoinHandle<()>>,
    error: Option<String>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        let (lock, wake) = &*self.queue;
        if let Ok(mut queue) = lock.lock() {
            queue.stopped = true;
            queue.latest = None;
            wake.notify_one();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
#[derive(Clone)]
pub struct InspectorService(Arc<Worker>);
impl Default for InspectorService {
    fn default() -> Self {
        Self::new()
    }
}
impl InspectorService {
    pub fn new() -> Self {
        let queue = Arc::new((Mutex::new(Queue::default()), Condvar::new()));
        let input = queue.clone();
        let thread = std::thread::Builder::new()
            .name("omen-code-inspector".into())
            .spawn(move || {
                let (lock, wake) = &*input;
                loop {
                    let request = {
                        let Ok(mut queue) = lock.lock() else {
                            return;
                        };
                        while queue.latest.is_none() && !queue.stopped {
                            let Ok(next) = wake.wait(queue) else {
                                return;
                            };
                            queue = next;
                        }
                        if queue.stopped {
                            return;
                        }
                        queue.latest.take()
                    };
                    if let Some(request) = request
                        && !request.reply.is_canceled()
                    {
                        let result = inspector::inspect(&request.text.to_string(), request.cursor);
                        let _ = request.reply.send(result);
                    }
                }
            });
        let (thread, error) = match thread {
            Ok(thread) => (Some(thread), None),
            Err(error) => (None, Some(error.to_string())),
        };
        Self(Arc::new(Worker {
            queue,
            thread,
            error,
        }))
    }
    pub async fn inspect(
        &self,
        text: TextSnapshot,
        cursor: usize,
    ) -> Result<Option<InspectedNode>, String> {
        if let Some(error) = &self.0.error {
            return Err(error.clone());
        }
        let (reply, result) = futures::channel::oneshot::channel();
        let (lock, wake) = &*self.0.queue;
        {
            let mut queue = lock.lock().map_err(|_| "Inspector stopped")?;
            queue.latest = Some(Request {
                text,
                cursor,
                reply,
            });
            wake.notify_one();
        }
        result
            .await
            .map_err(|_| "Inspector selection changed".to_string())?
    }
}
