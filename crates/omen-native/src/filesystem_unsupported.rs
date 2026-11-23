use crate::*;
/// Maximum file size defined by the cross-platform filesystem schema.
pub const MAX_FILE_BYTES: usize = 262_144;
/// Explicit unsupported adapter; no unsafe path-based fallback is used on this host.
pub struct FilesystemPlugin;
impl FilesystemPlugin {
    /// Return Unsupported until a rooted handle-based implementation exists for this OS.
    pub fn new(_root: impl AsRef<std::path::Path>) -> PluginResult<Self> {
        Err(PluginError::new(
            ErrorCode::Unsupported,
            "rooted filesystem is unavailable on this host",
        ))
    }
}
impl Plugin for FilesystemPlugin {
    fn descriptor(&self) -> PluginDescriptor {
        let mut descriptor = filesystem_descriptor();
        for cap in &mut descriptor.operations {
            cap.available = false;
        }
        descriptor
    }
    fn invoke(
        &self,
        _: &str,
        _: Json,
        _: PluginContext,
    ) -> futures::future::BoxFuture<'static, PluginResult<Json>> {
        Box::pin(async {
            Err(PluginError::new(
                ErrorCode::Unsupported,
                "rooted filesystem is unavailable on this host",
            ))
        })
    }
}
