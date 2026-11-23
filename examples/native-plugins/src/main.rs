use futures::{StreamExt, executor::block_on};
use omen_native::{FileChanged, FilesystemPlugin, ReadText, WriteText};
use omen_plugin::{PermissionStatus, PermissionStore, PluginRuntime};
use std::sync::Arc;
struct DemoDirectory(std::path::PathBuf);
impl Drop for DemoDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("omen-native-demo-{}", std::process::id()));
    std::fs::create_dir(&root)?;
    let directory = DemoDirectory(root);
    let permissions = PermissionStore::default();
    // Trusted host policy authorizes only this temporary filesystem root.
    for permission in ["fs.read", "fs.write"] {
        permissions.set(permission, PermissionStatus::Granted);
    }
    let runtime = PluginRuntime::new(["fs.read", "fs.write"], Arc::new(permissions));
    runtime.register(FilesystemPlugin::new(&directory.0)?)?;
    block_on(async {
        runtime.start("omen.filesystem").await?;
        let mut changes = runtime.subscribe::<FileChanged>(8)?;
        runtime
            .call(WriteText {
                path: "hello.txt".into(),
                contents: "Hello from Omen's typed native plugin runtime.".into(),
            })
            .await?;
        println!(
            "{}",
            runtime
                .call(ReadText {
                    path: "hello.txt".into()
                })
                .await?
        );
        if let Some(event) = changes.next().await {
            println!("Native event: {}", event?.path);
        }
        println!("Discovered {} plugin(s)", runtime.discover().len());
        runtime.shutdown().await
    })?;
    Ok(())
}
