use android_activity::{
    AndroidApp, InputStatus, MainEvent, PollEvent,
    input::{InputEvent, KeyAction, Keycode, MotionAction},
};
use anyhow::{Result, anyhow, bail};
use futures::channel::oneshot;
use gpui::*;
use gpui_wgpu::{CosmicTextSystem, GpuContext, WgpuContext, WgpuRenderer, WgpuSurfaceConfig, wgpu};
use jni::{jni_sig, jni_str};
use ndk::native_window::NativeWindow;
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    ffi::OsString,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

// Callbacks are taken out of their slots before invocation: GPUI callbacks can
// reenter platform methods, and must never run under a RefCell borrow.
struct Slot<T>(RefCell<Option<T>>);
impl<T> Default for Slot<T> {
    fn default() -> Self {
        Self(RefCell::new(None))
    }
}
impl<T> Slot<T> {
    fn set(&self, value: T) {
        *self.0.borrow_mut() = Some(value);
    }
    fn call<R>(&self, invoke: impl FnOnce(&mut T) -> R) -> Option<R> {
        let mut callback = self.0.borrow_mut().take()?;
        let result = invoke(&mut callback);
        let mut slot = self.0.borrow_mut();
        if slot.is_none() {
            *slot = Some(callback);
        }
        Some(result)
    }
}
struct Dispatcher {
    thread: thread::ThreadId,
    queue: Mutex<mpsc::Sender<RunnableVariant>>,
    waker: android_activity::AndroidAppWaker,
}
impl PlatformDispatcher for Dispatcher {
    fn is_main_thread(&self) -> bool {
        thread::current().id() == self.thread
    }
    fn dispatch(&self, r: RunnableVariant, _: Priority) {
        thread::spawn(move || r.run());
    }
    fn dispatch_on_main_thread(&self, r: RunnableVariant, _: Priority) {
        if let Ok(queue) = self.queue.lock() {
            let _ = queue.send(r);
        }
        self.waker.wake();
    }
    fn dispatch_after(&self, d: Duration, r: RunnableVariant) {
        thread::spawn(move || {
            thread::sleep(d);
            r.run();
        });
    }
    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>) {
        thread::spawn(f);
    }
}
#[derive(Debug)]
struct Display {
    bounds: Cell<Bounds<Pixels>>,
}
impl PlatformDisplay for Display {
    fn id(&self) -> DisplayId {
        DisplayId::new(1)
    }
    fn uuid(&self) -> Result<uuid::Uuid> {
        Ok(uuid::Uuid::from_u128(1))
    }
    fn bounds(&self) -> Bounds<Pixels> {
        self.bounds.get()
    }
    fn refresh_interval(&self) -> Option<Duration> {
        Some(Duration::from_nanos(16_666_667))
    }
}
struct Keyboard;
impl PlatformKeyboardLayout for Keyboard {
    fn id(&self) -> &str {
        "android"
    }
    fn name(&self) -> &str {
        "Android"
    }
}
type CredentialTask = Task<Result<Option<(String, Vec<u8>)>>>;
type InputCallback = Slot<Box<dyn FnMut(PlatformInput) -> DispatchEventResult>>;
type ResizeCallback = Slot<Box<dyn FnMut(Size<Pixels>, f32)>>;

thread_local! {
    static TEXT_SCALE: Cell<f32> = const { Cell::new(1.) };
}

/// Preferred system font scale on the NativeActivity window thread.
pub fn text_scale() -> f32 {
    TEXT_SCALE.get()
}

fn query_device_settings(app: &AndroidApp) -> Option<[f32; 6]> {
    let vm_ptr = app.vm_as_ptr() as *mut jni::sys::JavaVM;
    if vm_ptr.is_null() {
        return None;
    }
    let vm = unsafe { jni::JavaVM::from_raw(vm_ptr) };
    vm.attach_current_thread(|env| {
        let result = (|| -> jni::errors::Result<_> {
            let class = env.find_class(jni_str!("dev/omen/app/OmenActivity"))?;
            let value = env
                .call_static_method(&class, jni_str!("getDeviceSettings"), jni_sig!("()[F"), &[])?
                .l()?;
            if value.is_null() {
                return Ok(None);
            }
            let values = unsafe { jni::objects::JFloatArray::from_raw(env, value.into_raw()) };
            let mut settings = [0.; 6];
            values.get_region(env, 0, &mut settings)?;
            Ok(Some(settings))
        })();
        if result.is_err() && env.exception_check() {
            let _ = env.exception_clear();
        }
        Ok::<_, jni::errors::Error>(result.ok().flatten())
    })
    .ok()
    .flatten()
}

fn query_system_insets(app: &AndroidApp) -> Option<(f32, f32, f32, f32)> {
    let vm_ptr = app.vm_as_ptr() as *mut jni::sys::JavaVM;
    let activity_ptr = app.activity_as_ptr() as jni::sys::jobject;
    if vm_ptr.is_null() || activity_ptr.is_null() {
        return None;
    }
    let jvm = unsafe { jni::JavaVM::from_raw(vm_ptr) };
    jvm.attach_current_thread(
        |env| -> Result<Option<(f32, f32, f32, f32)>, jni::errors::Error> {
            if let Ok(class) = env.find_class(jni_str!("dev/omen/app/OmenActivity")) {
                if let Ok(result) =
                    env.call_static_method(&class, jni_str!("getSafeInsets"), jni_sig!("()[I"), &[])
                    && let Ok(obj) = result.l()
                    && !obj.is_null()
                {
                    let j_array = unsafe { jni::objects::JIntArray::from_raw(env, obj.into_raw()) };
                    let mut elements = [0_i32; 4];
                    if j_array.get_region(env, 0, &mut elements).is_ok() {
                        let (top, right, bottom, left) = (
                            elements[0] as f32,
                            elements[1] as f32,
                            elements[2] as f32,
                            elements[3] as f32,
                        );
                        if top > 0. || bottom > 0. || left > 0. || right > 0. {
                            return Ok(Some((top, right, bottom, left)));
                        }
                    }
                } else if env.exception_check() {
                    let _ = env.exception_clear();
                }
            } else if env.exception_check() {
                let _ = env.exception_clear();
            }

            let activity = unsafe { jni::objects::JObject::from_raw(env, activity_ptr) };

            if let Ok(window_val) = env.call_method(
                &activity,
                jni_str!("getWindow"),
                jni_sig!("()Landroid/view/Window;"),
                &[],
            ) && let Ok(window_obj) = window_val.l()
                && !window_obj.is_null()
            {
                if let Ok(decor_val) = env.call_method(
                    &window_obj,
                    jni_str!("getDecorView"),
                    jni_sig!("()Landroid/view/View;"),
                    &[],
                ) && let Ok(decor_obj) = decor_val.l()
                    && !decor_obj.is_null()
                {
                    if let Ok(root_insets_val) = env.call_method(
                        &decor_obj,
                        jni_str!("getRootWindowInsets"),
                        jni_sig!("()Landroid/view/WindowInsets;"),
                        &[],
                    ) && let Ok(insets_obj) = root_insets_val.l()
                        && !insets_obj.is_null()
                    {
                        if let Ok(insets_res) = env.call_method(
                            &insets_obj,
                            jni_str!("getInsets"),
                            jni_sig!("(I)Landroid/graphics/Insets;"),
                            &[135_i32.into()],
                        ) && let Ok(insets_box) = insets_res.l()
                            && !insets_box.is_null()
                        {
                            let top = env
                                .get_field(&insets_box, jni_str!("top"), jni_sig!("I"))
                                .ok()
                                .and_then(|v| v.i().ok())
                                .unwrap_or(0);
                            let left = env
                                .get_field(&insets_box, jni_str!("left"), jni_sig!("I"))
                                .ok()
                                .and_then(|v| v.i().ok())
                                .unwrap_or(0);
                            let right = env
                                .get_field(&insets_box, jni_str!("right"), jni_sig!("I"))
                                .ok()
                                .and_then(|v| v.i().ok())
                                .unwrap_or(0);
                            let bottom = env
                                .get_field(&insets_box, jni_str!("bottom"), jni_sig!("I"))
                                .ok()
                                .and_then(|v| v.i().ok())
                                .unwrap_or(0);
                            if top > 0 || bottom > 0 || left > 0 || right > 0 {
                                return Ok(Some((
                                    top as f32,
                                    right as f32,
                                    bottom as f32,
                                    left as f32,
                                )));
                            }
                        } else if env.exception_check() {
                            let _ = env.exception_clear();
                        }

                        let top = env
                            .call_method(
                                &insets_obj,
                                jni_str!("getSystemWindowInsetTop"),
                                jni_sig!("()I"),
                                &[],
                            )
                            .ok()
                            .and_then(|v| v.i().ok())
                            .unwrap_or(0);
                        let left = env
                            .call_method(
                                &insets_obj,
                                jni_str!("getSystemWindowInsetLeft"),
                                jni_sig!("()I"),
                                &[],
                            )
                            .ok()
                            .and_then(|v| v.i().ok())
                            .unwrap_or(0);
                        let right = env
                            .call_method(
                                &insets_obj,
                                jni_str!("getSystemWindowInsetRight"),
                                jni_sig!("()I"),
                                &[],
                            )
                            .ok()
                            .and_then(|v| v.i().ok())
                            .unwrap_or(0);
                        let bottom = env
                            .call_method(
                                &insets_obj,
                                jni_str!("getSystemWindowInsetBottom"),
                                jni_sig!("()I"),
                                &[],
                            )
                            .ok()
                            .and_then(|v| v.i().ok())
                            .unwrap_or(0);
                        if env.exception_check() {
                            let _ = env.exception_clear();
                        }
                        if top > 0 || bottom > 0 || left > 0 || right > 0 {
                            return Ok(Some((
                                top as f32,
                                right as f32,
                                bottom as f32,
                                left as f32,
                            )));
                        }
                    }
                }
            }

            if let Ok(res_val) = env.call_method(
                &activity,
                jni_str!("getResources"),
                jni_sig!("()Landroid/content/res/Resources;"),
                &[],
            ) && let Ok(res_obj) = res_val.l()
                && !res_obj.is_null()
            {
                if let (Ok(name), Ok(def_type), Ok(def_pkg)) = (
                    env.new_string("status_bar_height"),
                    env.new_string("dimen"),
                    env.new_string("android"),
                ) {
                    if let Ok(id_val) = env.call_method(
                        &res_obj,
                        jni_str!("getIdentifier"),
                        jni_sig!("(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)I"),
                        &[(&name).into(), (&def_type).into(), (&def_pkg).into()],
                    ) && let Ok(id) = id_val.i()
                        && id > 0
                    {
                        if let Ok(dimen_val) = env.call_method(
                            &res_obj,
                            jni_str!("getDimensionPixelSize"),
                            jni_sig!("(I)I"),
                            &[id.into()],
                        ) && let Ok(top) = dimen_val.i()
                        {
                            return Ok(Some((top as f32, 0., 0., 0.)));
                        }
                    }
                }
            }

            Ok(None)
        },
    )
    .unwrap_or(None)
}

struct WindowState {
    touches: RefCell<crate::touch::TouchIds>,
    presented: Cell<bool>,
    native: RefCell<Option<NativeWindow>>,
    renderer: RefCell<WgpuRenderer>,
    display: Rc<Display>,
    app: AndroidApp,
    bounds: Cell<Bounds<Pixels>>,
    scale: Cell<f32>,
    visible: Cell<bool>,
    active: Cell<bool>,
    handle: AnyWindowHandle,
    input: InputCallback,
    frame: Slot<Box<dyn FnMut(RequestFrameOptions)>>,
    resized: ResizeCallback,
    visibility: Slot<Box<dyn FnMut(WindowVisibility)>>,
    activated: Slot<Box<dyn FnMut(bool)>>,
    inset_changed: Slot<Box<dyn FnMut(WindowInsets)>>,
    input_handler: RefCell<Option<PlatformInputHandler>>,
    title: RefCell<String>,
    back: Slot<Box<dyn FnMut()>>,
    back_enabled: Cell<bool>,
    safe_insets: Cell<Edges<Pixels>>,
    ime_insets: Cell<Edges<Pixels>>,
    dark: Cell<bool>,
    appearance_changed: Slot<Box<dyn FnMut()>>,
}
impl WindowState {
    fn update_insets(&self) {
        let scale = self.scale.get();
        let (top, right, bottom, left) = if let Some(insets) = query_system_insets(&self.app) {
            insets
        } else {
            let rect = self.app.content_rect();
            let size = self.bounds.get().size;
            let right = (f32::from(size.width) * scale - rect.right as f32).max(0.);
            let bottom = (f32::from(size.height) * scale - rect.bottom as f32).max(0.);
            (rect.top as f32, right, bottom, rect.left as f32)
        };
        self.safe_insets.set(Edges {
            top: px((top / scale).max(0.)),
            right: px((right / scale).max(0.)),
            bottom: px((bottom / scale).max(0.)),
            left: px((left / scale).max(0.)),
        });
    }
    fn geometry(&self) {
        let Some(window) = self.native.borrow().clone() else {
            return;
        };
        let scale = self
            .app
            .config()
            .density()
            .map(|d| d as f32 / 160.)
            .unwrap_or(1.)
            .max(0.1);
        let bounds = Bounds::new(
            Point::default(),
            size(
                px(window.width() as f32 / scale),
                px(window.height() as f32 / scale),
            ),
        );
        self.scale.set(scale);
        self.bounds.set(bounds);
        self.display.bounds.set(bounds);
        self.renderer.borrow_mut().update_drawable_size(size(
            DevicePixels(window.width()),
            DevicePixels(window.height()),
        ));
        self.update_insets();
        if let Some([text_scale, dark, top, right, bottom, left]) = query_device_settings(&self.app)
        {
            TEXT_SCALE.set(text_scale.max(0.01));
            self.ime_insets.set(Edges {
                top: px(top / scale),
                right: px(right / scale),
                bottom: px(bottom / scale),
                left: px(left / scale),
            });
            let dark = dark != 0.;
            if self.dark.replace(dark) != dark {
                self.appearance_changed.call(|callback| callback());
            }
        }
        self.resized.call(|f| f(bounds.size, scale));
        self.inset_changed.call(|f| f(self.insets()));
    }
    fn insets(&self) -> WindowInsets {
        let insets = self.safe_insets.get();
        if insets.top == px(0.)
            && insets.bottom == px(0.)
            && insets.left == px(0.)
            && insets.right == px(0.)
        {
            self.update_insets();
        }
        WindowInsets {
            safe_area: self.safe_insets.get(),
            ime: self.ime_insets.get(),
            ..Default::default()
        }
    }
    fn visibility(&self, visible: bool) {
        if self.visible.replace(visible) != visible {
            if !visible {
                let contacts = self.touches.borrow_mut().drain();
                for id in contacts {
                    self.input.call(|f| {
                        f(PlatformInput::Touch(TouchEvent {
                            id: TouchId(id),
                            phase: TouchPhase::Cancelled,
                            position: Point::default(),
                            predicted_position: None,
                            force: None,
                        }))
                    });
                }
            }
            self.visibility.call(|f| {
                f(if visible {
                    WindowVisibility::Visible
                } else {
                    WindowVisibility::Hidden
                })
            });
        }
    }
    fn tick(&self) {
        if self.visible.get() && self.native.borrow().is_some() {
            self.frame.call(|f| {
                f(RequestFrameOptions {
                    require_presentation: true,
                    ..Default::default()
                })
            });
        }
    }
}
#[derive(Clone, Debug)]
struct AndroidDisplay;
impl HasDisplayHandle for AndroidDisplay {
    fn display_handle(&self) -> std::result::Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::android())
    }
}
#[derive(Clone, Debug)]
struct SurfaceWindow(NativeWindow);
impl HasWindowHandle for SurfaceWindow {
    fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, HandleError> {
        self.0.window_handle()
    }
}
impl HasDisplayHandle for SurfaceWindow {
    fn display_handle(&self) -> std::result::Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::android())
    }
}
struct AndroidWindow(Rc<WindowState>);
impl HasWindowHandle for AndroidWindow {
    fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, HandleError> {
        let native = self.0.native.borrow();
        let window = native.as_ref().ok_or(HandleError::Unavailable)?;
        // WindowState retains ANativeWindow until TerminateWindow, when rendering
        // is synchronously unconfigured before releasing the last window handle.
        Ok(unsafe { WindowHandle::borrow_raw(window.window_handle()?.as_raw()) })
    }
}
impl HasDisplayHandle for AndroidWindow {
    fn display_handle(&self) -> std::result::Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::android())
    }
}
impl PlatformWindow for AndroidWindow {
    fn bounds(&self) -> Bounds<Pixels> {
        self.0.bounds.get()
    }
    fn is_maximized(&self) -> bool {
        false
    }
    fn window_bounds(&self) -> WindowBounds {
        WindowBounds::Fullscreen(self.bounds())
    }
    fn content_size(&self) -> Size<Pixels> {
        self.bounds().size
    }
    fn resize(&mut self, _: Size<Pixels>) {}
    fn scale_factor(&self) -> f32 {
        self.0.scale.get()
    }
    fn appearance(&self) -> WindowAppearance {
        if self.0.dark.get() {
            WindowAppearance::Dark
        } else {
            WindowAppearance::Light
        }
    }
    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(self.0.display.clone())
    }
    fn mouse_position(&self) -> Point<Pixels> {
        Point::default()
    }
    fn modifiers(&self) -> Modifiers {
        Modifiers::default()
    }
    fn capslock(&self) -> Capslock {
        Capslock::default()
    }
    fn set_input_handler(&mut self, h: PlatformInputHandler) {
        *self.0.input_handler.borrow_mut() = Some(h);
    }
    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.0.input_handler.borrow_mut().take()
    }
    fn prompt(
        &self,
        _: PromptLevel,
        _: &str,
        _: Option<&str>,
        _: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>> {
        None
    }
    fn activate(&self) {}
    fn is_active(&self) -> bool {
        self.0.active.get()
    }
    fn visibility(&self) -> WindowVisibility {
        if self.0.visible.get() {
            WindowVisibility::Visible
        } else {
            WindowVisibility::Hidden
        }
    }
    fn is_hovered(&self) -> bool {
        false
    }
    fn background_appearance(&self) -> WindowBackgroundAppearance {
        WindowBackgroundAppearance::Opaque
    }
    fn set_title(&mut self, s: &str) {
        *self.0.title.borrow_mut() = s.into();
    }
    fn set_background_appearance(&self, _: WindowBackgroundAppearance) {}
    fn minimize(&self) {}
    fn zoom(&self) {}
    fn toggle_fullscreen(&self) {}
    fn is_fullscreen(&self) -> bool {
        true
    }
    fn on_request_frame(&self, f: Box<dyn FnMut(RequestFrameOptions)>) {
        self.0.frame.set(f);
    }
    fn on_input(&self, f: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {
        self.0.input.set(f);
    }
    fn on_active_status_change(&self, f: Box<dyn FnMut(bool)>) {
        self.0.activated.set(f);
    }
    fn on_visibility_change(&self, f: Box<dyn FnMut(WindowVisibility)>) {
        self.0.visibility.set(f);
    }
    fn on_hover_status_change(&self, _: Box<dyn FnMut(bool)>) {}
    fn on_resize(&self, f: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.0.resized.set(f);
    }
    fn on_moved(&self, _: Box<dyn FnMut()>) {}
    fn on_display_changed(&self, _: Box<dyn FnMut()>) {}
    fn on_should_close(&self, _: Box<dyn FnMut() -> bool>) {}
    fn on_hit_test_window_control(&self, _: Box<dyn FnMut() -> Option<WindowControlArea>>) {}
    fn on_close(&self, _: Box<dyn FnOnce()>) {}
    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        self.0.appearance_changed.set(callback);
    }
    fn draw(&self, scene: &Scene) {
        if self.0.native.borrow().is_some() {
            let presented = self.0.renderer.borrow_mut().draw(scene);
            if presented && !self.0.presented.replace(true) {
                android_log(c"GPUI Vulkan frame presented");
            }
        }
    }
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.0.renderer.borrow().sprite_atlas().clone()
    }
    fn is_subpixel_rendering_supported(&self) -> bool {
        false
    }
    fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.0.renderer.borrow().gpu_specs()
    }
    fn update_ime_position(&self, _: Bounds<Pixels>) {}
    fn insets(&self) -> WindowInsets {
        self.0.insets()
    }
    fn on_insets_changed(&self, f: Box<dyn FnMut(WindowInsets)>) {
        self.0.inset_changed.set(f);
    }
    fn set_back_handler(&self, f: Box<dyn FnMut()>) {
        self.0.back.set(f);
    }
    fn set_back_enabled(&self, enabled: bool) {
        self.0.back_enabled.set(enabled);
    }
    fn show_soft_keyboard(&self) {
        self.0.app.show_soft_input(true);
    }
    fn hide_soft_keyboard(&self) {
        self.0.app.hide_soft_input(false);
    }
}

/// NativeActivity-backed GPUI platform. One Android activity owns one GPUI window.
/// Android IME composition and accessibility adapters are not implemented yet.
pub struct AndroidPlatform {
    app: AndroidApp,
    dispatcher: Arc<Dispatcher>,
    queue: RefCell<mpsc::Receiver<RunnableVariant>>,
    text: Arc<CosmicTextSystem>,
    display: Rc<Display>,
    window: RefCell<Option<Rc<WindowState>>>,
    gpu: GpuContext,
    exiting: Cell<bool>,
    lifecycle: Slot<Box<dyn FnMut(AppLifecyclePhase)>>,
    memory: Slot<Box<dyn FnMut()>>,
    quit: Slot<Box<dyn FnMut() -> bool>>,
}
impl AndroidPlatform {
    /// Construct on the NativeActivity app thread, before creating GPUI's application.
    pub fn new(app: AndroidApp) -> Result<Rc<Self>> {
        let (tx, rx) = mpsc::channel();
        let dispatcher = Arc::new(Dispatcher {
            thread: thread::current().id(),
            queue: Mutex::new(tx),
            waker: app.create_waker(),
        });
        let text = Arc::new(CosmicTextSystem::new_without_system_fonts("Roboto"));
        let mut fonts = Vec::new();
        for path in [
            "/system/fonts/Roboto-Regular.ttf",
            "/system/fonts/RobotoStatic-Regular.ttf",
            "/system/fonts/NotoSans-Regular.ttf",
        ] {
            if let Ok(bytes) = std::fs::read(path) {
                fonts.push(Cow::Owned(bytes));
            }
        }
        if fonts.is_empty() {
            bail!("No supported Android system font found");
        }
        text.add_fonts(fonts)?;
        Ok(Rc::new(Self {
            app,
            dispatcher,
            queue: RefCell::new(rx),
            text,
            display: Rc::new(Display {
                bounds: Cell::new(Bounds::new(Point::default(), size(px(360.), px(640.)))),
            }),
            window: RefCell::new(None),
            gpu: Rc::new(RefCell::new(None)),
            exiting: Cell::new(false),
            lifecycle: Slot::default(),
            memory: Slot::default(),
            quit: Slot::default(),
        }))
    }
    fn window(&self) -> Option<Rc<WindowState>> {
        self.window.borrow().clone()
    }
    fn input(&self) {
        let Some(window) = self.window() else { return };
        let Ok(mut events) = self.app.input_events_iter() else {
            return;
        };
        while events.next(|event| {
            if let InputEvent::MotionEvent(event) = event {
                let phase = match event.action() {
                    MotionAction::Down | MotionAction::PointerDown => TouchPhase::Started,
                    MotionAction::Up | MotionAction::PointerUp => TouchPhase::Ended,
                    MotionAction::Move => TouchPhase::Moved,
                    MotionAction::Cancel => TouchPhase::Cancelled,
                    _ => return InputStatus::Unhandled,
                };
                for (index, pointer) in event.pointers().enumerate() {
                    if matches!(
                        event.action(),
                        MotionAction::PointerDown | MotionAction::PointerUp
                    ) && index != event.pointer_index()
                    {
                        continue;
                    }
                    let pointer_id = pointer.pointer_id();
                    let id = if phase == TouchPhase::Started {
                        window.touches.borrow_mut().start(pointer_id)
                    } else if let Some(id) = window.touches.borrow().get(pointer_id) {
                        id
                    } else {
                        continue;
                    };
                    if matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                        window.touches.borrow_mut().end(pointer_id);
                    }
                    window.input.call(|f| {
                        f(PlatformInput::Touch(TouchEvent {
                            id: TouchId(id),
                            phase,
                            position: point(
                                px(pointer.x() / window.scale.get()),
                                px(pointer.y() / window.scale.get()),
                            ),
                            predicted_position: None,
                            force: None,
                        }))
                    });
                }
                if phase == TouchPhase::Cancelled {
                    window.touches.borrow_mut().clear();
                }
                InputStatus::Handled
            } else if let InputEvent::KeyEvent(event) = event {
                if event.key_code() == Keycode::Back && window.back_enabled.get() {
                    if event.action() == KeyAction::Up {
                        window.back.call(|f| f());
                    }
                    return InputStatus::Handled;
                }
                InputStatus::Unhandled
            } else {
                InputStatus::Unhandled
            }
        }) {}
    }
    fn event(&self, event: MainEvent<'_>, launch: &mut Option<Box<dyn FnOnce()>>) {
        match event {
            MainEvent::InitWindow { .. } => {
                if let Some(window) = self.window() {
                    if let Some(native) = self.app.native_window() {
                        let config = surface_config(&native);
                        let result = self
                            .gpu
                            .borrow()
                            .as_ref()
                            .ok_or_else(|| anyhow!("Missing GPU context"))
                            .and_then(|gpu| {
                                window.renderer.borrow_mut().replace_surface(
                                    &native,
                                    config,
                                    &gpu.instance,
                                )
                            });
                        match result {
                            Ok(()) => {
                                *window.native.borrow_mut() = Some(native);
                                window.geometry();
                                window.visibility(true);
                            }
                            Err(error) => {
                                log::error!("Android surface recreation failed: {error}");
                                self.exiting.set(true);
                            }
                        }
                    }
                } else if let Some(launch) = launch.take() {
                    launch();
                }
            }
            MainEvent::TerminateWindow { .. } => {
                if let Some(window) = self.window() {
                    window.visibility(false);
                    window.touches.borrow_mut().clear();
                    window.renderer.borrow_mut().unconfigure_surface();
                    window.native.borrow_mut().take();
                }
            }
            MainEvent::WindowResized { .. }
            | MainEvent::ContentRectChanged { .. }
            | MainEvent::ConfigChanged { .. }
            | MainEvent::InsetsChanged { .. } => {
                if let Some(window) = self.window() {
                    window.geometry();
                }
            }
            MainEvent::RedrawNeeded { .. } => {
                if let Some(window) = self.window() {
                    window.tick();
                }
            }
            MainEvent::InputAvailable => self.input(),
            MainEvent::Resume { .. } => {
                self.lifecycle.call(|f| f(AppLifecyclePhase::Active));
                if let Some(window) = self.window() {
                    window.visibility(true);
                }
            }
            MainEvent::Pause | MainEvent::Stop => {
                self.lifecycle.call(|f| f(AppLifecyclePhase::Background));
                if let Some(window) = self.window() {
                    window.visibility(false);
                }
            }
            MainEvent::GainedFocus | MainEvent::LostFocus => {
                if let Some(window) = self.window() {
                    let active = matches!(event, MainEvent::GainedFocus);
                    window.active.set(active);
                    window.activated.call(|f| f(active));
                }
            }
            MainEvent::LowMemory => {
                self.memory.call(|f| f());
            }
            MainEvent::Destroy => {
                self.exiting.set(true);
            }
            _ => {}
        }
    }
}
fn surface_config(window: &NativeWindow) -> WgpuSurfaceConfig {
    WgpuSurfaceConfig {
        size: size(
            DevicePixels(window.width().max(1)),
            DevicePixels(window.height().max(1)),
        ),
        transparent: false,
        preferred_present_mode: None,
    }
}
impl Platform for AndroidPlatform {
    fn background_executor(&self) -> BackgroundExecutor {
        BackgroundExecutor::new(self.dispatcher.clone())
    }
    fn foreground_executor(&self) -> ForegroundExecutor {
        ForegroundExecutor::new(self.dispatcher.clone())
    }
    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.text.clone()
    }
    fn run(&self, launch: Box<dyn FnOnce()>) {
        let mut launch = Some(launch);
        while !self.exiting.get() {
            let timeout = self
                .window()
                .filter(|window| window.visible.get())
                .map(|_| Duration::from_millis(16));
            self.app.poll_events(timeout, |event| {
                if let PollEvent::Main(event) = event {
                    self.event(event, &mut launch)
                }
            });
            let pending: Vec<_> = self.queue.borrow().try_iter().collect();
            for runnable in pending {
                runnable.run();
            }
            if let Some(window) = self.window() {
                window.tick();
            }
        }
        if let Some(window) = self.window() {
            window.visibility(false);
            window.renderer.borrow_mut().unconfigure_surface();
            window.native.borrow_mut().take();
        }
    }
    fn quit(&self) {
        self.exiting.set(true);
        self.dispatcher.waker.wake();
    }
    fn restart(&self, _: Option<PathBuf>, _: Vec<OsString>) {}
    fn activate(&self, _: bool) {}
    fn hide(&self) {}
    fn hide_other_apps(&self) {}
    fn unhide_other_apps(&self) {}
    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        vec![self.display.clone()]
    }
    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(self.display.clone())
    }
    fn on_displays_changed(&self, _: Box<dyn FnMut()>) {}
    fn active_window(&self) -> Option<AnyWindowHandle> {
        self.window().map(|w| w.handle)
    }
    fn open_window(
        &self,
        handle: AnyWindowHandle,
        _: WindowParams,
    ) -> Result<Box<dyn PlatformWindow>> {
        if self.window().is_some() {
            bail!("Only one Android GPUI window is supported");
        }
        let native = self
            .app
            .native_window()
            .ok_or_else(|| anyhow!("Android surface is not ready"))?;
        // Restrict this platform to Vulkan; do not silently select a browser or GL backend.
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            // Some Android emulator Vulkan drivers crash in debug-object naming.
            // Keep debug utils opt-in; the normal native path needs no labels.
            flags: if cfg!(feature = "gpu-validation") {
                wgpu::InstanceFlags::default()
            } else {
                wgpu::InstanceFlags::empty()
            },
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: Some(Box::new(AndroidDisplay)),
        });
        let surface = instance.create_surface(SurfaceWindow(native.clone()))?;
        let context = WgpuContext::new(instance, &surface, None)?;
        drop(surface);
        *self.gpu.borrow_mut() = Some(context);
        let renderer = WgpuRenderer::new(
            self.gpu.clone(),
            &SurfaceWindow(native.clone()),
            surface_config(&native),
            None,
        )?;
        let window = Rc::new(WindowState {
            touches: RefCell::new(crate::touch::TouchIds::default()),
            presented: Cell::new(false),
            native: RefCell::new(Some(native)),
            renderer: RefCell::new(renderer),
            display: self.display.clone(),
            app: self.app.clone(),
            bounds: Cell::new(self.display.bounds()),
            scale: Cell::new(1.),
            visible: Cell::new(true),
            active: Cell::new(true),
            handle,
            input: Slot::default(),
            frame: Slot::default(),
            resized: Slot::default(),
            visibility: Slot::default(),
            activated: Slot::default(),
            inset_changed: Slot::default(),
            input_handler: RefCell::new(None),
            title: RefCell::new(String::new()),
            back: Slot::default(),
            back_enabled: Cell::new(false),
            safe_insets: Cell::new(Edges::default()),
            ime_insets: Cell::new(Edges::default()),
            dark: Cell::default(),
            appearance_changed: Slot::default(),
        });
        window.geometry();
        *self.window.borrow_mut() = Some(window.clone());
        Ok(Box::new(AndroidWindow(window)))
    }
    fn window_appearance(&self) -> WindowAppearance {
        WindowAppearance::Light
    }
    fn open_url(&self, _: &str) {}
    fn on_open_urls(&self, _: Box<dyn FnMut(Vec<String>)>) {}
    fn register_url_scheme(&self, _: &str) -> Task<Result<()>> {
        Task::ready(Err(anyhow!("URL registration is unavailable")))
    }
    fn prompt_for_paths(
        &self,
        _: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        let (tx, rx) = oneshot::channel();
        let _ = tx.send(Err(anyhow!("File picker is unavailable")));
        rx
    }
    fn prompt_for_new_path(
        &self,
        _: &Path,
        _: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        let (tx, rx) = oneshot::channel();
        let _ = tx.send(Err(anyhow!("File picker is unavailable")));
        rx
    }
    fn can_select_mixed_files_and_dirs(&self) -> bool {
        false
    }
    fn reveal_path(&self, _: &Path) {}
    fn open_with_system(&self, _: &Path) {}
    fn on_quit(&self, f: Box<dyn FnMut() -> bool>) {
        self.quit.set(f);
    }
    fn on_reopen(&self, _: Box<dyn FnMut()>) {}
    fn on_system_sleep(&self, _: Box<dyn FnMut()>) {}
    fn on_system_wake(&self, _: Box<dyn FnMut()>) {}
    fn on_app_lifecycle(&self, f: Box<dyn FnMut(AppLifecyclePhase)>) {
        self.lifecycle.set(f);
    }
    fn on_memory_warning(&self, f: Box<dyn FnMut()>) {
        self.memory.set(f);
    }
    fn set_menus(&self, _: Vec<Menu>, _: &Keymap) {}
    fn set_dock_menu(&self, _: Vec<MenuItem>, _: &Keymap) {}
    fn on_app_menu_action(&self, _: Box<dyn FnMut(&dyn Action)>) {}
    fn on_will_open_app_menu(&self, _: Box<dyn FnMut()>) {}
    fn on_validate_app_menu_command(&self, _: Box<dyn FnMut(&dyn Action) -> bool>) {}
    fn thermal_state(&self) -> ThermalState {
        ThermalState::Nominal
    }
    fn on_thermal_state_change(&self, _: Box<dyn FnMut()>) {}
    fn prevent_idle_sleep(&self, _: &str) -> Task<Result<ActivityGuard>> {
        Task::ready(Err(anyhow!("Idle-sleep control is unavailable")))
    }
    fn app_path(&self) -> Result<PathBuf> {
        self.app
            .internal_data_path()
            .ok_or_else(|| anyhow!("App storage unavailable"))
    }
    fn path_for_auxiliary_executable(&self, _: &str) -> Result<PathBuf> {
        bail!("Auxiliary executables are unavailable")
    }
    fn set_cursor_style(&self, _: CursorStyle) {}
    fn hide_cursor_until_mouse_moves(&self) {}
    fn is_cursor_visible(&self) -> bool {
        false
    }
    fn should_auto_hide_scrollbars(&self) -> bool {
        true
    }
    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        None
    }
    fn write_to_clipboard(&self, _: ClipboardItem) {}
    fn write_credentials(&self, _: &str, _: &str, _: &[u8]) -> Task<Result<()>> {
        Task::ready(Err(anyhow!("Credential storage unavailable")))
    }
    fn read_credentials(&self, _: &str) -> CredentialTask {
        Task::ready(Err(anyhow!("Credential storage unavailable")))
    }
    fn delete_credentials(&self, _: &str) -> Task<Result<()>> {
        Task::ready(Err(anyhow!("Credential storage unavailable")))
    }
    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        Box::new(Keyboard)
    }
    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        Rc::new(DummyKeyboardMapper)
    }
    fn on_keyboard_layout_change(&self, _: Box<dyn FnMut()>) {}
}

#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(
        priority: i32,
        tag: *const std::ffi::c_char,
        text: *const std::ffi::c_char,
    ) -> i32;
}
fn android_log(message: &std::ffi::CStr) {
    // Both C strings remain live for the synchronous Android logging call.
    unsafe {
        __android_log_write(4, c"gpui_android".as_ptr(), message.as_ptr());
    }
}
