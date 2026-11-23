//! Shared schemas for the example filesystem, notification, and camera plugins.
use crate::*;
/// Read a UTF-8 file relative to the host-approved root.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadText {
    /// Relative path, with no parent traversal or symlinks.
    pub path: String,
}
impl Operation for ReadText {
    type Output = String;
    const PLUGIN: &str = "omen.filesystem";
    const NAME: &str = "read_text";
    const PERMISSIONS: &[&str] = &["fs.read"];
}
/// Write a small UTF-8 file relative to the host-approved root.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WriteText {
    /// Relative file path.
    pub path: String,
    /// UTF-8 contents, up to 256 KiB.
    pub contents: String,
}
impl Operation for WriteText {
    type Output = ();
    const PLUGIN: &str = "omen.filesystem";
    const NAME: &str = "write_text";
    const PERMISSIONS: &[&str] = &["fs.write"];
}
/// A successful write issued through this plugin (not an OS filesystem watcher).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileChanged {
    /// Relative path written.
    pub path: String,
}
impl PluginEvent for FileChanged {
    const PLUGIN: &str = "omen.filesystem";
    const NAME: &str = "changed";
    const PERMISSIONS: &[&str] = &["fs.write"];
}
/// A request to display one native notification.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShowNotification {
    /// Short title.
    pub title: String,
    /// Notification body.
    pub body: String,
}
impl Operation for ShowNotification {
    type Output = ();
    const PLUGIN: &str = "omen.notifications";
    const NAME: &str = "show";
    const PERMISSIONS: &[&str] = &["notifications"];
}
/// Capture a still JPEG image. No camera is opened during registration.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapturePhoto {
    /// Requested maximum width, 1 through 1920.
    pub width: u32,
    /// Requested maximum height, 1 through 1080.
    pub height: u32,
}
impl Operation for CapturePhoto {
    type Output = CameraImage;
    const PLUGIN: &str = "omen.camera";
    const NAME: &str = "capture";
    const PERMISSIONS: &[&str] = &["camera"];
}
/// A captured image. The bridge still applies its encoded-message size ceiling.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CameraImage {
    /// Media type, normally image/jpeg.
    pub mime: String,
    /// Encoded image bytes.
    pub data: Vec<u8>,
}
/// Filesystem descriptor shared by native bridge hosts.
pub fn filesystem_descriptor() -> PluginDescriptor {
    PluginDescriptor {
        id: ReadText::PLUGIN.into(),
        version: "0.1.0".into(),
        api_version: API_VERSION,
        operations: vec![
            Capability::operation::<ReadText>(),
            Capability::operation::<WriteText>(),
        ],
        events: vec![Capability::event::<FileChanged>()],
    }
}
/// Notification descriptor shared by native bridge hosts.
pub fn notifications_descriptor(available: bool) -> PluginDescriptor {
    let mut cap = Capability::operation::<ShowNotification>();
    cap.available = available;
    PluginDescriptor {
        id: ShowNotification::PLUGIN.into(),
        version: "0.1.0".into(),
        api_version: API_VERSION,
        operations: vec![cap],
        events: Vec::new(),
    }
}
/// Camera descriptor shared by native bridge hosts.
pub fn camera_descriptor(available: bool) -> PluginDescriptor {
    let mut cap = Capability::operation::<CapturePhoto>();
    cap.available = available;
    PluginDescriptor {
        id: CapturePhoto::PLUGIN.into(),
        version: "0.1.0".into(),
        api_version: API_VERSION,
        operations: vec![cap],
        events: Vec::new(),
    }
}
