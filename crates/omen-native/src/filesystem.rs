use crate::*;
use std::{
    io::{Read, Write},
    path::{Component, Path},
    sync::{Arc, Mutex},
};
/// Maximum file size exposed by the example filesystem plugin.
pub const MAX_FILE_BYTES: usize = 262_144;
fn native_error() -> PluginError {
    PluginError::new(ErrorCode::Native, "filesystem operation failed")
}
fn validate(path: &str) -> PluginResult<Vec<std::ffi::OsString>> {
    let path = Path::new(path);
    let parts = path
        .components()
        .map(|component| match component {
            Component::Normal(value) => Ok(value.to_os_string()),
            _ => Err(PluginError::new(
                ErrorCode::PermissionDenied,
                "path must stay inside approved root",
            )),
        })
        .collect::<PluginResult<Vec<_>>>()?;
    if parts.is_empty() {
        return Err(PluginError::new(
            ErrorCode::InvalidRequest,
            "empty file path",
        ));
    }
    Ok(parts)
}
/// Rooted desktop filesystem plugin. Unix uses descriptor-relative no-follow opens.
pub struct FilesystemPlugin {
    root: Arc<Mutex<Option<Arc<std::fs::File>>>>,
}
impl FilesystemPlugin {
    /// Pin a trusted existing root directory. Symlink roots are rejected on Unix.
    pub fn new(root: impl AsRef<Path>) -> PluginResult<Self> {
        #[cfg(unix)]
        {
            use rustix::fs::{Mode, OFlags, open};
            let file = std::fs::File::from(
                open(
                    root.as_ref(),
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| native_error())?,
            );
            Ok(Self {
                root: Arc::new(Mutex::new(Some(Arc::new(file)))),
            })
        }
        #[cfg(not(unix))]
        {
            let _ = root;
            Err(PluginError::new(
                ErrorCode::Unsupported,
                "rooted filesystem adapter requires a Unix host",
            ))
        }
    }
}
#[cfg(unix)]
fn open_file(root: &std::fs::File, path: &str, write: bool) -> PluginResult<std::fs::File> {
    use rustix::fs::{Mode, OFlags, openat};
    let parts = validate(path)?;
    let mut parent = root.try_clone().map_err(|_| native_error())?;
    for part in &parts[..parts.len() - 1] {
        parent = std::fs::File::from(
            openat(
                &parent,
                part,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| native_error())?,
        );
    }
    let flags = if write {
        OFlags::WRONLY | OFlags::CREATE
    } else {
        OFlags::RDONLY
    };
    let file = std::fs::File::from(
        openat(
            &parent,
            &parts[parts.len() - 1],
            flags | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|_| native_error())?,
    );
    if !file.metadata().map_err(|_| native_error())?.is_file() {
        return Err(PluginError::new(
            ErrorCode::PermissionDenied,
            "only regular files are supported",
        ));
    }
    Ok(file)
}
impl Plugin for FilesystemPlugin {
    fn descriptor(&self) -> PluginDescriptor {
        filesystem_descriptor()
    }
    fn shutdown(&self) -> futures::future::BoxFuture<'static, PluginResult<()>> {
        self.root
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        Box::pin(async { Ok(()) })
    }
    fn invoke(
        &self,
        operation: &str,
        payload: Json,
        context: PluginContext,
    ) -> futures::future::BoxFuture<'static, PluginResult<Json>> {
        let root = self
            .root
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let operation = operation.to_string();
        Box::pin(async move {
            let root = root.ok_or_else(|| {
                PluginError::new(ErrorCode::Unavailable, "filesystem plugin stopped")
            })?;
            blocking::unblock(move || {
                #[cfg(unix)]
                {
                    match operation.as_str() {
                        "read_text" => {
                            let request: ReadText = serde_json_payload(payload)?;
                            context.require("fs.read")?;
                            let file = open_file(&root, &request.path, false)?;
                            context.check_cancelled()?;
                            let mut bytes = Vec::new();
                            file.take((MAX_FILE_BYTES + 1) as u64)
                                .read_to_end(&mut bytes)
                                .map_err(|_| native_error())?;
                            if bytes.len() > MAX_FILE_BYTES {
                                return Err(PluginError::new(
                                    ErrorCode::InvalidRequest,
                                    "file exceeds size limit",
                                ));
                            }
                            let text = String::from_utf8(bytes).map_err(|_| {
                                PluginError::new(ErrorCode::InvalidRequest, "file is not UTF-8")
                            })?;
                            Ok(Json::String(text))
                        }
                        "write_text" => {
                            let request: WriteText = serde_json_payload(payload)?;
                            if request.contents.len() > MAX_FILE_BYTES {
                                return Err(PluginError::new(
                                    ErrorCode::InvalidRequest,
                                    "file exceeds size limit",
                                ));
                            }
                            context.require("fs.write")?;
                            let mut file = open_file(&root, &request.path, true)?;
                            context.require("fs.write")?;
                            file.set_len(0).map_err(|_| native_error())?;
                            file.write_all(request.contents.as_bytes())
                                .map_err(|_| native_error())?;
                            context.emit(&FileChanged { path: request.path })?;
                            Ok(Json::Null)
                        }
                        _ => Err(PluginError::new(
                            ErrorCode::NotFound,
                            "unknown filesystem operation",
                        )),
                    }
                }
                #[cfg(not(unix))]
                {
                    let _ = (root, operation, payload, context);
                    Err(PluginError::new(
                        ErrorCode::Unsupported,
                        "filesystem adapter unavailable",
                    ))
                }
            })
            .await
        })
    }
}
fn serde_json_payload<T: serde::de::DeserializeOwned>(value: Json) -> PluginResult<T> {
    serde_json::from_value(value)
        .map_err(|_| PluginError::new(ErrorCode::InvalidRequest, "invalid native request"))
}
