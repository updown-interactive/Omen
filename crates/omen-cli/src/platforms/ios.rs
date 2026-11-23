use super::*;
use crate::terminal::selection;
use std::time::Duration;
pub struct Ios;
impl PlatformRunner for Ios {
    fn detect(&self) -> PlatformStatus {
        let target = if tools::host_target()
            .unwrap_or_default()
            .starts_with("aarch64")
        {
            "aarch64-apple-ios-sim"
        } else {
            "x86_64-apple-ios"
        };
        PlatformStatus::new(
            Platform::Ios,
            "native UIKit + Metal · build available, launch unverified",
            vec![
                (cfg!(target_os = "macos"), "macOS"),
                (tools::find("cargo").is_some(), "Cargo"),
                (tools::python().is_some(), "Python 3.9+"),
                (
                    tools::text("xcrun", &["--sdk", "iphonesimulator", "--show-sdk-path"]).is_ok(),
                    "Xcode iOS simulator SDK",
                ),
                (
                    tools::installed_targets().iter().any(|s| s == target),
                    "Rust iOS simulator target",
                ),
            ],
        )
    }
    fn build(&self, p: &Project, o: &BuildOptions) -> Result<BuildArtifact> {
        if o.ios_device
            && !tools::installed_targets()
                .iter()
                .any(|s| s == "aarch64-apple-ios")
        {
            bail!("Install the Rust aarch64-apple-ios target for device builds")
        }
        package(p, Platform::Ios, o)
    }
    fn run(&self, p: &Project, a: &BuildArtifact, o: &RunOptions) -> Result<()> {
        eprintln!(
            "iOS simulator launch is unverified in this SDK. Launch errors will be reported; a successful build does not verify rendering."
        );
        let device = selection::device(&devices()?, o.device.as_deref())?;
        if device.state != "Booted" {
            checked(
                Command::new("xcrun").args(["simctl", "boot", &device.id]),
                Duration::from_secs(30),
            )?;
        }
        checked(
            Command::new("xcrun").args(["simctl", "bootstatus", &device.id, "-b"]),
            Duration::from_secs(180),
        )?;
        checked(
            Command::new("xcrun")
                .args(["simctl", "install", &device.id])
                .arg(&a.path),
            Duration::from_secs(90),
        )?;
        if o.managed {
            checked(
                Command::new("open").args([
                    "-a",
                    "Simulator",
                    "--args",
                    "-CurrentDeviceUDID",
                    &device.id,
                ]),
                Duration::from_secs(10),
            )?;
            let id = device.id.clone();
            let bundle = a.bundle_id.clone();
            let _app_guard = crate::cleanup::register(move || {
                let _ = checked(
                    Command::new("xcrun").args(["simctl", "terminate", &id, &bundle]),
                    Duration::from_secs(5),
                );
            });
            println!("Launching {} on {}…", a.bundle_id, device.name);
            let status = Command::new("xcrun")
                .args([
                    "simctl",
                    "launch",
                    "--console",
                    "--terminate-running-process",
                ])
                .args([&device.id, &a.bundle_id])
                .stdin(std::process::Stdio::null())
                .env("SIMCTL_CHILD_OMEN_NATIVE_TRACE", "1")
                .status()
                .context("Could not attach to the iOS simulator console")?;
            if !status.success() {
                bail!("iOS simulator session exited with {status}");
            }
            return Ok(());
        }
        let logs = p.root.join(".omen/logs");
        std::fs::create_dir_all(&logs)?;
        let result = checked(
            Command::new("xcrun")
                .args(["simctl", "launch", "--terminate-running-process"])
                .arg(format!(
                    "--stdout={}",
                    logs.join("ios-stdout.log").display()
                ))
                .arg(format!(
                    "--stderr={}",
                    logs.join("ios-stderr.log").display()
                ))
                .args([&device.id, &a.bundle_id])
                .env("SIMCTL_CHILD_OMEN_NATIVE_TRACE", "1"),
            Duration::from_secs(90),
        )?;
        let text = String::from_utf8_lossy(&result.stdout);
        let pid = text
            .rsplit(':')
            .next()
            .context("No iOS process ID returned")?
            .trim()
            .parse::<u32>()
            .context("iOS did not report a launched process ID")?;
        checked(
            Command::new("kill").args(["-0", &pid.to_string()]),
            Duration::from_secs(5),
        )?;
        println!(
            "Launched process {pid} on {}. Rendering remains unverified. Logs: {}",
            device.name,
            logs.display()
        );
        Ok(())
    }
}
fn checked(command: &mut Command, timeout: Duration) -> Result<std::process::Output> {
    let result = tools::capture_timeout(command, timeout)?;
    if !result.status.success() {
        bail!(
            "iOS command failed: {} {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        )
    }
    Ok(result)
}
pub fn parse_devices(text: &str) -> Result<Vec<Device>> {
    let value: serde_json::Value = serde_json::from_str(text)?;
    let runtimes = value["devices"]
        .as_object()
        .context("simctl did not return a device map")?;
    let mut devices = Vec::new();
    for (runtime, entries) in runtimes {
        if !runtime.contains("iOS") {
            continue;
        }
        for item in entries.as_array().into_iter().flatten() {
            let available = item["isAvailable"].as_bool().unwrap_or(false);
            devices.push(Device {
                platform: Platform::Ios,
                id: item["udid"]
                    .as_str()
                    .context("Simulator missing UDID")?
                    .into(),
                name: item["name"]
                    .as_str()
                    .context("Simulator missing name")?
                    .into(),
                state: item["state"].as_str().unwrap_or("Unknown").into(),
                runnable: available,
                kind: runtime.clone(),
            });
        }
    }
    Ok(devices)
}
pub fn devices() -> Result<Vec<Device>> {
    if !cfg!(target_os = "macos") || tools::find("xcrun").is_none() {
        return Ok(Vec::new());
    }
    let result = checked(
        Command::new("xcrun").args(["simctl", "list", "devices", "available", "--json"]),
        Duration::from_secs(10),
    )
    .context("iOS simulator discovery failed")?;
    parse_devices(std::str::from_utf8(&result.stdout)?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_preserves_availability_and_ignores_other_runtimes() {
        let devices=parse_devices(r#"{"devices":{"com.apple.CoreSimulator.SimRuntime.iOS-26-0":[{"udid":"one","name":"iPhone 17 Pro","state":"Shutdown","isAvailable":true},{"udid":"old","name":"Old","isAvailable":false}],"watchOS":[]}}"#).unwrap();
        assert_eq!(devices.len(), 2);
        assert!(devices[0].runnable);
        assert!(!devices[1].runnable);
    }
}
