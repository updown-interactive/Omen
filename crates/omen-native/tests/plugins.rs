#[cfg(unix)]
use futures::StreamExt;
use futures::executor::block_on;
use omen_native::*;
use omen_plugin::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
#[cfg(unix)]
struct Temp(std::path::PathBuf);
#[cfg(unix)]
impl Temp {
    fn new() -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "omen-plugin-test-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
#[cfg(unix)]
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[cfg(unix)]
#[test]
fn rooted_filesystem_round_trips_and_emits_only_authorized_writes() {
    block_on(async {
        let temp = Temp::new();
        let permissions = PermissionStore::default();
        for permission in ["fs.read", "fs.write"] {
            permissions.set(permission, PermissionStatus::Granted);
        }
        let runtime = PluginRuntime::new(["fs.read", "fs.write"], Arc::new(permissions));
        runtime
            .register(FilesystemPlugin::new(&temp.0).unwrap())
            .unwrap();
        runtime.start("omen.filesystem").await.unwrap();
        let mut events = runtime.subscribe::<FileChanged>(2).unwrap();
        runtime
            .call(WriteText {
                path: "notes.txt".into(),
                contents: "Hello native ✓".into(),
            })
            .await
            .unwrap();
        assert_eq!(
            runtime
                .call(ReadText {
                    path: "notes.txt".into()
                })
                .await
                .unwrap(),
            "Hello native ✓"
        );
        assert_eq!(events.next().await.unwrap().unwrap().path, "notes.txt");
        assert!(
            runtime
                .call(ReadText {
                    path: "../outside".into()
                })
                .await
                .is_err()
        );
        assert!(
            runtime
                .call(ReadText {
                    path: temp.0.display().to_string()
                })
                .await
                .is_err()
        );
        assert!(
            runtime
                .call(WriteText {
                    path: "large".into(),
                    contents: "x".repeat(MAX_FILE_BYTES + 1)
                })
                .await
                .is_err()
        );
        runtime.shutdown().await.unwrap();
    });
}
#[cfg(unix)]
#[test]
fn symlinked_files_and_intermediate_directories_cannot_escape_root() {
    block_on(async {
        use std::os::unix::fs::symlink;
        let root = Temp::new();
        let outside = Temp::new();
        std::fs::write(outside.0.join("secret"), "unchanged").unwrap();
        symlink(outside.0.join("secret"), root.0.join("link")).unwrap();
        symlink(&outside.0, root.0.join("directory")).unwrap();
        let permissions = PermissionStore::default();
        for p in ["fs.read", "fs.write"] {
            permissions.set(p, PermissionStatus::Granted);
        }
        let runtime = PluginRuntime::new(["fs.read", "fs.write"], Arc::new(permissions));
        runtime
            .register(FilesystemPlugin::new(&root.0).unwrap())
            .unwrap();
        runtime.start("omen.filesystem").await.unwrap();
        for path in ["link", "directory/secret"] {
            assert!(runtime.call(ReadText { path: path.into() }).await.is_err());
            assert!(
                runtime
                    .call(WriteText {
                        path: path.into(),
                        contents: "changed".into()
                    })
                    .await
                    .is_err()
            );
        }
        assert_eq!(
            std::fs::read_to_string(outside.0.join("secret")).unwrap(),
            "unchanged"
        );
    });
}
struct FakeCamera(Arc<AtomicUsize>);
impl CameraBackend for FakeCamera {
    fn available(&self) -> bool {
        true
    }
    fn capture(
        &self,
        _: CapturePhoto,
        context: PluginContext,
    ) -> futures::future::BoxFuture<'static, PluginResult<CameraImage>> {
        let calls = self.0.clone();
        Box::pin(async move {
            context.require("camera")?;
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(CameraImage {
                mime: "image/jpeg".into(),
                data: vec![0xff, 0xd8, 0xff, 0xd9],
            })
        })
    }
}
#[test]
fn camera_never_accesses_backend_without_permission_and_valid_dimensions() {
    block_on(async {
        let permissions = PermissionStore::default();
        let runtime = PluginRuntime::new(["camera"], Arc::new(permissions.clone()));
        let calls = Arc::new(AtomicUsize::new(0));
        runtime
            .register(CameraPlugin::new(FakeCamera(calls.clone())))
            .unwrap();
        runtime.start("omen.camera").await.unwrap();
        assert_eq!(
            runtime
                .call(CapturePhoto {
                    width: 320,
                    height: 240
                })
                .await
                .unwrap_err()
                .code,
            ErrorCode::PermissionDenied
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        permissions.set("camera", PermissionStatus::Granted);
        assert_eq!(
            runtime
                .call(CapturePhoto {
                    width: 0,
                    height: 240
                })
                .await
                .unwrap_err()
                .code,
            ErrorCode::InvalidRequest
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            runtime
                .call(CapturePhoto {
                    width: 320,
                    height: 240
                })
                .await
                .unwrap()
                .mime,
            "image/jpeg"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn notifications_are_denied_without_ever_invoking_the_os_service() {
    block_on(async {
        let runtime = PluginRuntime::new(["notifications"], Arc::new(PermissionStore::default()));
        runtime.register(NotificationsPlugin).unwrap();
        runtime.start("omen.notifications").await.unwrap();
        let error = runtime
            .call(ShowNotification {
                title: "Test".into(),
                body: "Not sent".into(),
            })
            .await
            .unwrap_err();
        assert!(matches!(
            error.code,
            ErrorCode::PermissionDenied | ErrorCode::Unsupported
        ));
    });
}
