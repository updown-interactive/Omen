use crate::*;
use futures::{Future, channel::oneshot};
use omen_core::{
    AppLifecycle, DeviceEnvironment, DevicePlatform, Insets, KeyboardViewport, PointerCapabilities,
    ThemeMode, View, Viewport,
};
use omen_plugin::{ErrorCode, NativeBridge, PluginError};
use serde::Deserialize;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Wake},
    time::Duration,
};
const MAX_INPUT: usize = 1_048_576;
#[link(wasm_import_module = "omen")]
unsafe extern "C" {
    fn scene(data: *const u8, len: u32);
    fn schedule();
    fn timer_start(id: u32, milliseconds: u32);
    fn timer_cancel(id: u32);
    fn native_send(data: *const u8, len: u32) -> u32;
    fn permission_start(id: u32, data: *const u8, len: u32);
}
struct App {
    engine: WebEngine,
    tasks: Vec<Pin<Box<dyn Future<Output = ()>>>>,
    device: DeviceEnvironment,
    reduced: bool,
    foreground: bool,
}
thread_local! {
    static APP:RefCell<Option<App>>=const{RefCell::new(None)};
    static BUFFERS:RefCell<BTreeMap<u32,Vec<u8>>>=const{RefCell::new(BTreeMap::new())};
    static TIMERS:RefCell<BTreeMap<u32,oneshot::Sender<()>>>=const{RefCell::new(BTreeMap::new())};
    static NEXT_TIMER:RefCell<u32>=const{RefCell::new(1)};
    static GRANTS:omen_plugin::PermissionStore=omen_plugin::PermissionStore::default();
    static PERMISSION_REQUESTS:RefCell<BTreeMap<u32,oneshot::Sender<omen_plugin::PermissionStatus>>>=const{RefCell::new(BTreeMap::new())};
    static BRIDGE:NativeBridge=NativeBridge::new(|bytes:&[u8]| {
        // The packaged host copies borrowed bytes synchronously and queues callbacks.
        if unsafe{native_send(bytes.as_ptr(),bytes.len() as u32)}!=0{Ok(())}else{Err(PluginError::new(ErrorCode::Unavailable,"native host is detached"))}
    });
}
struct BrowserWake;
impl Wake for BrowserWake {
    fn wake(self: Arc<Self>) {
        request_frame();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        request_frame();
    }
}
fn request_frame() {
    // Host scheduling never synchronously calls back into Rust.
    unsafe {
        schedule();
    }
}
/// Run a constructed view in the packaged browser/WebView host.
pub fn run_view(view: impl View) -> Result<(), WindowError> {
    APP.with(|app| {
        let mut app = app.borrow_mut();
        if app.is_some() {
            return Err(WindowError::AlreadyRunning);
        }
        let mut engine = WebEngine::new(view);
        engine.set_waker(request_frame);
        *app = Some(App {
            engine,
            tasks: Vec::new(),
            device: DeviceEnvironment {
                platform: DevicePlatform::Web,
                ..DeviceEnvironment::default()
            },
            reduced: false,
            foreground: true,
        });
        Ok(())
    })?;
    request_frame();
    Ok(())
}
/// Shared native-plugin transport for this WASM application instance.
/// Register reviewed descriptors in a PluginRuntime and use a host permission broker.
pub fn native_bridge() -> NativeBridge {
    BRIDGE.with(Clone::clone)
}
struct Sleep {
    id: u32,
    receiver: oneshot::Receiver<()>,
}
impl Future for Sleep {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        Pin::new(&mut self.receiver).poll(cx).map(|_| ())
    }
}
impl Drop for Sleep {
    fn drop(&mut self) {
        TIMERS.with(|timers| {
            timers.borrow_mut().remove(&self.id);
        });
        unsafe {
            timer_cancel(self.id);
        }
    }
}
/// Wait without blocking the browser event loop. Dropping cancels the host timer.
pub async fn sleep(duration: Duration) {
    let id = NEXT_TIMER.with(|next| {
        let mut next = next.borrow_mut();
        let id = *next;
        *next = next.wrapping_add(1).max(1);
        id
    });
    let (sender, receiver) = oneshot::channel();
    TIMERS.with(|timers| {
        timers.borrow_mut().insert(id, sender);
    });
    unsafe {
        timer_start(id, duration.as_millis().min(u32::MAX as u128) as u32);
    }
    Sleep { id, receiver }.await;
}
/// Reserve a bounded input buffer. The trusted JS host writes exactly len bytes, then consumes it.
#[unsafe(no_mangle)]
pub extern "C" fn omen_web_alloc(len: u32) -> u32 {
    let len = len as usize;
    if len == 0 || len > MAX_INPUT {
        return 0;
    }
    BUFFERS.with(|buffers| {
        let mut buffers = buffers.borrow_mut();
        if buffers.values().map(Vec::len).sum::<usize>() + len > MAX_INPUT * 4 {
            return 0;
        }
        let mut bytes = vec![0; len];
        let pointer = bytes.as_mut_ptr() as u32;
        buffers.insert(pointer, bytes);
        pointer
    })
}
/// Consume one reserved event buffer. Returns zero on success, one for invalid input.
#[unsafe(no_mangle)]
pub extern "C" fn omen_web_event(pointer: u32, len: u32) -> u32 {
    let bytes = BUFFERS.with(|buffers| buffers.borrow_mut().remove(&pointer));
    let Some(bytes) = bytes.filter(|bytes| bytes.len() == len as usize) else {
        return 1;
    };
    let Ok(event) = serde_json::from_slice::<Envelope>(&bytes) else {
        return 1;
    };
    match event {
        Envelope::Control(event) => {
            APP.with(|app| {
                if let Some(app) = app.borrow().as_ref() {
                    app.engine.dispatch(event);
                }
            });
        }
        Envelope::Host(HostEvent::Viewport {
            width,
            height,
            insets,
            system_dark,
            reduced_motion,
            pixel_ratio,
            touch,
            mouse,
            keyboard_bottom,
            text_scale,
        }) => {
            if !width.is_finite() || !height.is_finite() || width < 0. || height < 0. {
                return 1;
            }
            APP.with(|app| {
                if let Some(app) = app.borrow_mut().as_mut() {
                    app.device = DeviceEnvironment {
                        appearance: if system_dark {
                            ThemeMode::Dark
                        } else {
                            ThemeMode::Light
                        },
                        platform: DevicePlatform::Web,
                        pixel_ratio: pixel_ratio.unwrap_or(1.).max(0.01),
                        pointer: PointerCapabilities { touch, mouse },
                        keyboard: KeyboardViewport::from_insets(Insets {
                            bottom: keyboard_bottom.max(0.),
                            ..Insets::default()
                        }),
                        text_scale: text_scale.unwrap_or(1.).max(0.01),
                        ..DeviceEnvironment::default()
                    }
                    .with_viewport(Viewport {
                        width,
                        height,
                        safe_area: Insets {
                            top: insets[0].max(0.),
                            right: insets[1].max(0.),
                            bottom: insets[2].max(0.),
                            left: insets[3].max(0.),
                        },
                    });
                    app.reduced = reduced_motion;
                }
            });
        }
        Envelope::Host(HostEvent::Lifecycle { state }) => {
            let lifecycle = match state.as_str() {
                "foreground" => AppLifecycle::Foreground,
                "background" => AppLifecycle::Background,
                _ => return 1,
            };
            APP.with(|app| {
                if let Some(app) = app.borrow_mut().as_mut() {
                    app.foreground = lifecycle == AppLifecycle::Foreground;
                    let _ = app.engine.set_lifecycle(lifecycle);
                    if !app.foreground {
                        app.tasks.clear();
                        app.engine.take_tasks();
                    }
                }
            });
        }
        Envelope::Host(HostEvent::Permission { id, status }) => {
            if let Some(sender) =
                PERMISSION_REQUESTS.with(|requests| requests.borrow_mut().remove(&id))
            {
                let _ = sender.send(status);
            }
        }
        Envelope::Host(HostEvent::Timer { id }) => {
            if let Some(sender) = TIMERS.with(|timers| timers.borrow_mut().remove(&id)) {
                let _ = sender.send(());
            }
        }
        Envelope::Host(HostEvent::Native { message }) => {
            let Ok(bytes) = serde_json::to_vec(&message) else {
                return 1;
            };
            if BRIDGE.with(|bridge| bridge.receive(&bytes)).is_err() {
                return 1;
            }
        }
    }
    APP.with(|app| {
        if let Some(app) = app.borrow_mut().as_mut()
            && app.foreground
        {
            poll_tasks(app);
        }
    });
    request_frame();
    0
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Envelope {
    Control(BrowserEvent),
    Host(HostEvent),
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum HostEvent {
    Viewport {
        width: f32,
        height: f32,
        insets: [f32; 4],
        system_dark: bool,
        reduced_motion: bool,
        #[serde(default)]
        pixel_ratio: Option<f32>,
        #[serde(default)]
        touch: bool,
        #[serde(default)]
        mouse: bool,
        #[serde(default)]
        keyboard_bottom: f32,
        #[serde(default)]
        text_scale: Option<f32>,
    },
    Lifecycle {
        state: String,
    },
    Timer {
        id: u32,
    },
    Permission {
        id: u32,
        status: omen_plugin::PermissionStatus,
    },
    Native {
        message: serde_json::Value,
    },
}
/// Reconcile and poll ready local work once. The JS host schedules subsequent frames via wakeups.
#[unsafe(no_mangle)]
pub extern "C" fn omen_web_render() -> u32 {
    APP.with(|app| {
        let mut app = app.borrow_mut();
        let Some(app) = app.as_mut() else {
            return 1;
        };
        if !app.foreground {
            return 0;
        }
        let frame = match app.engine.render_device(app.device, app.reduced) {
            Ok(frame) => frame,
            Err(_) => return 1,
        };
        poll_tasks(app);
        let Ok(bytes) = serde_json::to_vec(&frame) else {
            return 1;
        };
        if bytes.len() > 8_388_608 {
            return 1;
        }
        // The renderer copies the scene bytes and patches DOM without reentering Rust.
        unsafe {
            scene(bytes.as_ptr(), bytes.len() as u32);
        }
        0
    })
}
fn poll_tasks(app: &mut App) {
    app.tasks.extend(
        app.engine
            .take_tasks()
            .into_iter()
            .map(|task| Box::pin(task.run()) as Pin<Box<dyn Future<Output = ()>>>),
    );
    let waker = Arc::new(BrowserWake).into();
    let mut context = Context::from_waker(&waker);
    app.tasks
        .retain_mut(|task| task.as_mut().poll(&mut context).is_pending());
}
struct BrowserPermissions {
    store: omen_plugin::PermissionStore,
}
impl omen_plugin::PermissionBroker for BrowserPermissions {
    fn status(&self, permission: &str) -> omen_plugin::PermissionStatus {
        omen_plugin::PermissionBroker::status(&self.store, permission)
    }
    fn request(
        &self,
        permission: String,
    ) -> futures::future::BoxFuture<'static, omen_plugin::PluginResult<omen_plugin::PermissionStatus>>
    {
        let store = self.store.clone();
        Box::pin(async move {
            let id = NEXT_TIMER.with(|next| {
                let mut next = next.borrow_mut();
                let id = *next;
                *next = next.wrapping_add(1).max(1);
                id
            });
            let (sender, receiver) = oneshot::channel();
            PERMISSION_REQUESTS.with(|requests| {
                requests.borrow_mut().insert(id, sender);
            });
            let _guard = PermissionGuard(id);
            // The reviewed host copies the permission name synchronously and requests from a user action.
            unsafe {
                permission_start(id, permission.as_ptr(), permission.len() as u32);
            }
            let status = receiver.await.map_err(|_| {
                PluginError::new(ErrorCode::Unavailable, "permission host detached")
            })?;
            store.set(permission, status);
            Ok(status)
        })
    }
}
struct PermissionGuard(u32);
impl Drop for PermissionGuard {
    fn drop(&mut self) {
        PERMISSION_REQUESTS.with(|requests| {
            requests.borrow_mut().remove(&self.0);
        });
    }
}
/// Permission broker backed by explicit, user-initiated browser/native host requests.
pub fn permission_broker() -> Arc<dyn omen_plugin::PermissionBroker> {
    Arc::new(BrowserPermissions {
        store: GRANTS.with(Clone::clone),
    })
}
/// Dispose state, tasks, timers, handlers, and native transports when the host unloads.
#[unsafe(no_mangle)]
pub extern "C" fn omen_web_dispose() {
    APP.with(|app| {
        app.borrow_mut().take();
    });
    TIMERS.with(|timers| timers.borrow_mut().clear());
    BUFFERS.with(|buffers| buffers.borrow_mut().clear());
    PERMISSION_REQUESTS.with(|requests| requests.borrow_mut().clear());
    BRIDGE.with(NativeBridge::close);
}
