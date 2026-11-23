//! Device discovery and managed run sessions for Omen Studio.
use crate::{
    cleanup, commands,
    platforms::{self, Device, Platform},
    project::manifest::{Manifest, TargetPlatform},
};
use anyhow::{Context, Result, bail};
use std::{
    io::Read,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunTarget {
    pub platform: Platform,
    pub name: String,
    pub detail: String,
    pub device: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct RunDevices {
    pub targets: Vec<RunTarget>,
    pub issues: Vec<String>,
}

pub fn discover(root: &Path) -> Result<RunDevices> {
    let file = root.join("Omen.toml");
    let manifest: Manifest = toml::from_str(
        &std::fs::read_to_string(&file)
            .with_context(|| format!("Cannot read {}", file.display()))?,
    )
    .context("Invalid Omen.toml")?;
    manifest.validate()?;
    let mut result = RunDevices::default();
    for platform in manifest.targets.platforms {
        match platform {
            TargetPlatform::Desktop => result.targets.push(RunTarget {
                platform: Platform::Desktop,
                name: "This computer".into(),
                detail: "Desktop".into(),
                device: None,
            }),
            TargetPlatform::Web => {
                if platforms::web::opener_available() {
                    result.targets.push(RunTarget {
                        platform: Platform::Web,
                        name: "Default browser".into(),
                        detail: "Web".into(),
                        device: None,
                    });
                } else {
                    result
                        .issues
                        .push("Web · No browser opener available".into());
                }
            }
            TargetPlatform::Ios => {
                add_devices(&mut result, "iOS Simulator", platforms::ios::devices())
            }
            TargetPlatform::Android => add_devices(
                &mut result,
                "Android Emulator",
                platforms::android::studio_devices(),
            ),
        }
    }
    Ok(result)
}

fn add_devices(result: &mut RunDevices, label: &str, devices: Result<Vec<Device>>) {
    match devices {
        Ok(devices) => {
            let start = result.targets.len();
            result.targets.extend(
                devices
                    .into_iter()
                    .filter(|device| device.runnable && device.kind != "physical")
                    .map(|device| {
                        let detail = match device.platform {
                            Platform::Ios => device
                                .kind
                                .rsplit('.')
                                .next()
                                .unwrap_or(&device.kind)
                                .replace('-', " "),
                            _ => device.state,
                        };
                        RunTarget {
                            platform: device.platform,
                            name: device.name,
                            detail: format!("{label} · {detail}"),
                            device: Some(device.id),
                        }
                    }),
            );
            if result.targets.len() == start {
                result
                    .issues
                    .push(format!("{label} · No devices available"));
            }
        }
        Err(error) => result.issues.push(format!("{label} · {error:#}")),
    }
}

/// Run in Studio's dedicated child process. Closing the control pipe ends the
/// session and runs the same app cleanup as an interrupted CLI run.
pub fn run(root: &Path, platform: &str, device: Option<String>, finish: fn()) -> Result<()> {
    use clap::ValueEnum;
    let Ok(platform) = Platform::from_str(platform, false) else {
        bail!("Invalid run platform: {platform}");
    };
    let stopping = Arc::new(AtomicBool::new(false));
    let control = Arc::clone(&stopping);
    let controller = std::thread::Builder::new()
        .name("omen-run-control".into())
        .spawn(move || {
            let _ = std::io::stdin().read(&mut [0]);
            control.store(true, Ordering::Release);
            cleanup::run_all();
            finish();
            std::process::exit(0);
        })?;
    let result = commands::run_managed(root, platform, device);
    if stopping.load(Ordering::Acquire) {
        // A runner can return as its app is terminated. Keep the worker alive
        // until the control thread has finished cleaning up build descendants.
        let _ = controller.join();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_menu_uses_only_manifest_platforms() -> Result<()> {
        let root =
            std::env::temp_dir().join(format!("omen-studio-discovery-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let mut manifest = crate::project::template::default_manifest("test-project", None);
        manifest.targets.platforms = vec![TargetPlatform::Desktop];
        std::fs::write(root.join("Omen.toml"), toml::to_string(&manifest)?)?;
        let devices = discover(&root);
        std::fs::remove_dir_all(root)?;
        assert_eq!(
            devices?
                .targets
                .iter()
                .map(|target| target.platform)
                .collect::<Vec<_>>(),
            [Platform::Desktop]
        );
        Ok(())
    }

    #[test]
    fn device_menu_excludes_physical_and_unavailable_devices() {
        let mut result = RunDevices::default();
        let devices = [
            ("USB", "physical", true),
            ("Offline", "emulator", false),
            ("Pixel", "avd", true),
        ]
        .map(|(name, kind, runnable)| Device {
            platform: Platform::Android,
            id: name.into(),
            name: name.into(),
            state: "Ready".into(),
            runnable,
            kind: kind.into(),
        });
        add_devices(&mut result, "Android Emulator", Ok(devices.into()));
        assert_eq!(
            result
                .targets
                .iter()
                .map(|target| target.name.as_str())
                .collect::<Vec<_>>(),
            ["Pixel"]
        );
    }
}
