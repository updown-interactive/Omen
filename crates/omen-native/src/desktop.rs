use crate::*;
use futures::future::BoxFuture;
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
};
/// Desktop notification adapter using the host's notification service.
pub struct NotificationsPlugin;
impl NotificationsPlugin {
    fn available() -> bool {
        (cfg!(target_os = "macos") && Path::new("/usr/bin/osascript").is_file())
            || (cfg!(target_os = "linux") && Path::new("/usr/bin/notify-send").is_file())
    }
}
impl Plugin for NotificationsPlugin {
    fn descriptor(&self) -> PluginDescriptor {
        notifications_descriptor(Self::available())
    }
    fn invoke(
        &self,
        operation: &str,
        payload: Json,
        context: PluginContext,
    ) -> BoxFuture<'static, PluginResult<Json>> {
        if operation != "show" {
            return Box::pin(async {
                Err(PluginError::new(
                    ErrorCode::NotFound,
                    "unknown notification operation",
                ))
            });
        }
        Box::pin(async move {
            let request: ShowNotification = serde_json::from_value(payload)
                .map_err(|_| PluginError::new(ErrorCode::InvalidRequest, "invalid notification"))?;
            if request.title.len() > 256 || request.body.len() > 4096 {
                return Err(PluginError::new(
                    ErrorCode::InvalidRequest,
                    "notification exceeds size limit",
                ));
            }
            blocking::unblock(move || send_notification(request, context)).await
        })
    }
}
fn send_notification(request: ShowNotification, context: PluginContext) -> PluginResult<Json> {
    context.require("notifications")?;
    let mut command = Command::new(if cfg!(target_os = "macos") {
        "/usr/bin/osascript"
    } else {
        "/usr/bin/notify-send"
    });
    if cfg!(target_os = "macos") {
        command.args(["-e","on run argv\ndisplay notification (item 2 of argv) with title (item 1 of argv)\nend run","--",&request.title,&request.body]);
    } else if cfg!(target_os = "linux") {
        command.args(["--", &request.title, &request.body]);
    } else {
        return Err(PluginError::new(
            ErrorCode::Unsupported,
            "native notifications unavailable",
        ));
    }
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| PluginError::new(ErrorCode::Native, "notification service unavailable"))?;
    let mut child = ChildCleanup(child);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        context.require("notifications")?;
        if std::time::Instant::now() >= deadline {
            return Err(PluginError::new(
                ErrorCode::Native,
                "notification request timed out",
            ));
        }
        match child.0.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(Json::Null),
            Ok(Some(_)) | Err(_) => {
                return Err(PluginError::new(
                    ErrorCode::Native,
                    "notification service failed",
                ));
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
        }
    }
}
/// Pluggable host camera backend. Tests can use a deterministic backend without hardware.
pub trait CameraBackend: Send + Sync + 'static {
    /// Release retained camera resources during plugin shutdown.
    fn shutdown(&self) -> BoxFuture<'static, PluginResult<()>> {
        Box::pin(async { Ok(()) })
    }
    /// Whether camera capture can be attempted on this host.
    fn available(&self) -> bool;
    /// Capture a JPEG, respecting permission and cancellation immediately before access.
    fn capture(
        &self,
        request: CapturePhoto,
        context: PluginContext,
    ) -> BoxFuture<'static, PluginResult<CameraImage>>;
}
/// Typed camera plugin delegating to a platform implementation.
pub struct CameraPlugin(Arc<dyn CameraBackend>);
impl CameraPlugin {
    /// Install an explicit camera backend. Registration does not activate hardware.
    pub fn new(backend: impl CameraBackend) -> Self {
        Self(Arc::new(backend))
    }
}
impl Plugin for CameraPlugin {
    fn descriptor(&self) -> PluginDescriptor {
        camera_descriptor(self.0.available())
    }
    fn shutdown(&self) -> BoxFuture<'static, PluginResult<()>> {
        self.0.shutdown()
    }
    fn invoke(
        &self,
        operation: &str,
        payload: Json,
        context: PluginContext,
    ) -> BoxFuture<'static, PluginResult<Json>> {
        if operation != "capture" {
            return Box::pin(async {
                Err(PluginError::new(
                    ErrorCode::NotFound,
                    "unknown camera operation",
                ))
            });
        }
        let backend = self.0.clone();
        Box::pin(async move {
            let request: CapturePhoto = serde_json::from_value(payload).map_err(|_| {
                PluginError::new(ErrorCode::InvalidRequest, "invalid camera request")
            })?;
            if request.width == 0
                || request.width > 1920
                || request.height == 0
                || request.height > 1080
            {
                return Err(PluginError::new(
                    ErrorCode::InvalidRequest,
                    "invalid camera dimensions",
                ));
            }
            context.require("camera")?;
            let image = backend.capture(request, context).await?;
            serde_json::to_value(image)
                .map_err(|_| PluginError::new(ErrorCode::Native, "camera response encoding failed"))
        })
    }
}
/// Optional FFmpeg still-capture adapter for macOS AVFoundation or Linux video4linux2.
/// The trusted host supplies a fixed executable path and device; requests cannot change them.
#[derive(Clone)]
pub struct FfmpegCamera {
    executable: PathBuf,
    device: String,
}
impl FfmpegCamera {
    /// Select an already installed executable and a platform device identifier.
    pub fn new(executable: impl AsRef<Path>, device: impl Into<String>) -> Self {
        Self {
            executable: executable.as_ref().into(),
            device: device.into(),
        }
    }
}
impl CameraBackend for FfmpegCamera {
    fn available(&self) -> bool {
        self.executable.is_file() && cfg!(any(target_os = "macos", target_os = "linux"))
    }
    fn capture(
        &self,
        request: CapturePhoto,
        context: PluginContext,
    ) -> BoxFuture<'static, PluginResult<CameraImage>> {
        let backend = self.clone();
        Box::pin(async move {
            blocking::unblock(move || {
                context.require("camera")?;
                let format = if cfg!(target_os = "macos") {
                    "avfoundation"
                } else if cfg!(target_os = "linux") {
                    "video4linux2"
                } else {
                    return Err(PluginError::new(
                        ErrorCode::Unsupported,
                        "camera backend unavailable",
                    ));
                };
                let child = Command::new(&backend.executable)
                    .args([
                        "-nostdin",
                        "-hide_banner",
                        "-loglevel",
                        "error",
                        "-f",
                        format,
                        "-i",
                        &backend.device,
                        "-frames:v",
                        "1",
                        "-vf",
                        &format!("scale={}:{}", request.width, request.height),
                        "-f",
                        "image2pipe",
                        "-vcodec",
                        "mjpeg",
                        "pipe:1",
                    ])
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|_| {
                        PluginError::new(ErrorCode::Native, "camera capture could not start")
                    })?;
                let mut child = ChildCleanup(child);
                let stdout = child.0.stdout.take().ok_or_else(|| {
                    PluginError::new(ErrorCode::Native, "camera output unavailable")
                })?;
                let (sender, receiver) = std::sync::mpsc::sync_channel(1);
                std::thread::spawn(move || {
                    let mut data = Vec::new();
                    let result = stdout.take(262_145).read_to_end(&mut data).map(|_| data);
                    let _ = sender.send(result);
                });
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
                let data = loop {
                    context.require("camera")?;
                    if std::time::Instant::now() >= deadline {
                        return Err(PluginError::new(
                            ErrorCode::Native,
                            "camera capture timed out",
                        ));
                    }
                    match receiver.recv_timeout(std::time::Duration::from_millis(50)) {
                        Ok(Ok(data)) => break data,
                        Ok(Err(_)) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            return Err(PluginError::new(
                                ErrorCode::Native,
                                "camera output failed",
                            ));
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    }
                };
                if data.len() > 262_144 || !data.starts_with(&[0xff, 0xd8]) {
                    return Err(PluginError::new(
                        ErrorCode::Native,
                        "camera returned no bounded JPEG",
                    ));
                }
                loop {
                    context.require("camera")?;
                    if std::time::Instant::now() >= deadline {
                        return Err(PluginError::new(
                            ErrorCode::Native,
                            "camera capture timed out",
                        ));
                    }
                    match child.0.try_wait() {
                        Ok(Some(status)) if status.success() => break,
                        Ok(Some(_)) | Err(_) => {
                            return Err(PluginError::new(
                                ErrorCode::Native,
                                "camera capture failed",
                            ));
                        }
                        Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
                    }
                }
                Ok(CameraImage {
                    mime: "image/jpeg".into(),
                    data,
                })
            })
            .await
        })
    }
}
/// Register the desktop adapters; the host still controls startup and permission grants.
pub fn register_desktop(
    runtime: &PluginRuntime,
    root: impl AsRef<Path>,
    camera: impl CameraBackend,
) -> PluginResult<()> {
    runtime.register(FilesystemPlugin::new(root)?)?;
    runtime.register(NotificationsPlugin)?;
    runtime.register(CameraPlugin::new(camera))
}

struct ChildCleanup(std::process::Child);
impl Drop for ChildCleanup {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
