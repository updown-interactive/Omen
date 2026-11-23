use super::*;
use crate::terminal::{
    interactive::{InteractiveSession, UserAction},
    selection,
};
use std::{
    process::{Child, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

fn spawn_logcat_child(adb: &std::path::Path, serial: &str, pid: &str) -> Option<Child> {
    Command::new(adb)
        .args(["-s", serial, "logcat", &format!("--pid={pid}")])
        .spawn()
        .ok()
}

fn stop_logcat_child(logcat_ref: &Arc<Mutex<Option<Child>>>) {
    if let Ok(mut opt) = logcat_ref.lock() {
        if let Some(mut child) = opt.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
pub struct Android;
impl PlatformRunner for Android {
    fn detect(&self) -> PlatformStatus {
        let mut status = PlatformStatus::new(
            Platform::Android,
            "native GPUI + Vulkan · tools detected; device launch not verified",
            vec![
                (tools::find("cargo").is_some(), "Cargo"),
                (tools::python().is_some(), "Python 3.9+"),
                (
                    tools::installed_targets()
                        .iter()
                        .any(|s| s == "aarch64-linux-android"),
                    "Rust target aarch64-linux-android",
                ),
                (tools::sdk_home().is_some(), "Android SDK"),
                (
                    tools::ndk_compiler().is_some(),
                    "Android NDK 28.2.13676358 / ANDROID_NDK_HOME",
                ),
                (
                    tools::gradle().is_some(),
                    "Gradle (PATH, GRADLE or local wrapper cache)",
                ),
                (tools::java().is_some(), "JDK 17 or newer"),
                (tools::android_tool("adb").is_some(), "adb"),
            ],
        );
        if let Ok(devices) = devices() {
            status.connected_devices = devices
                .into_iter()
                .filter(|d| d.kind != "avd")
                .map(|d| {
                    if d.runnable {
                        d.name
                    } else {
                        format!("{} ({})", d.name, d.state)
                    }
                })
                .collect();
        }
        status
    }
    fn build(&self, p: &Project, o: &BuildOptions) -> Result<BuildArtifact> {
        package(p, Platform::Android, o)
    }
    fn run(&self, p: &Project, a: &BuildArtifact, o: &RunOptions) -> Result<()> {
        let chosen = selection::device(&devices()?, o.device.as_deref())?;
        let adb = tools::android_tool("adb").context("adb is unavailable")?;
        let serial = if chosen.kind == "avd" {
            start_emulator(&chosen, &adb, o.managed)?
        } else {
            chosen.id
        };
        let abis = tools::text(
            &adb,
            &["-s", &serial, "shell", "getprop", "ro.product.cpu.abilist"],
        )?;
        if !abis.split(',').any(|abi| abi == "arm64-v8a") {
            bail!("The native APK requires an ARM64 Android device; {serial} reports {abis}")
        }
        tools::run(
            Command::new(&adb)
                .args(["-s", &serial, "install", "-r"])
                .arg(&a.path),
        )?;
        let adb_cleanup = adb.clone();
        let serial_cleanup = serial.clone();
        let bundle_cleanup = a.bundle_id.clone();
        let mut app_guard = crate::cleanup::register(move || {
            let _ = tools::capture_timeout(
                Command::new(&adb_cleanup).args([
                    "-s",
                    &serial_cleanup,
                    "shell",
                    "am",
                    "force-stop",
                    &bundle_cleanup,
                ]),
                Duration::from_secs(5),
            );
        });

        let component = format!("{}/{}", a.bundle_id, a.activity);
        let result = tools::capture_timeout(
            Command::new(&adb).args([
                "-s", &serial, "shell", "am", "start", "-W", "-n", &component,
            ]),
            Duration::from_secs(90),
        )?;
        let message = String::from_utf8_lossy(&result.stdout);
        if !result.status.success() || message.contains("Error:") || !message.contains("Status: ok")
        {
            bail!(
                "Android launch failed: {message} {}",
                String::from_utf8_lossy(&result.stderr)
            )
        }
        let pid = tools::text(&adb, &["-s", &serial, "shell", "pidof", &a.bundle_id])?;
        if pid.is_empty() {
            bail!("Android Activity opened but the application process exited")
        }
        let target_pid = pid.split_whitespace().next().unwrap_or(&pid).to_string();

        let _managed_logs = o.managed.then(|| {
            let logcat = spawn_logcat_child(&adb, &serial, &target_pid);
            if logcat.is_none() {
                eprintln!("Could not attach to Android logcat.");
            }
            let logcat = Arc::new(Mutex::new(logcat));
            crate::cleanup::register(move || stop_logcat_child(&logcat))
        });

        println!(
            "Launched {} on {serial}. Waiting for native Vulkan presentation…",
            a.bundle_id
        );
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut presented = false;
        while Instant::now() < deadline {
            let logs = tools::text(
                &adb,
                &[
                    "-s",
                    &serial,
                    "logcat",
                    "-d",
                    &format!("--pid={target_pid}"),
                    "-s",
                    "gpui_android:I",
                ],
            )?;
            if logs.contains("GPUI Vulkan frame presented") {
                presented = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        if !presented {
            bail!(
                "Application started but GPUI did not confirm a Vulkan frame within 60 seconds. Inspect adb -s {serial} logcat --pid={target_pid}"
            )
        }
        println!("Native GPUI Vulkan frame verified on {serial}.");
        if o.managed {
            while probe(&adb, &["-s", &serial, "shell", "pidof", &a.bundle_id])
                .is_ok_and(|pid| !pid.is_empty())
            {
                std::thread::sleep(Duration::from_millis(500));
            }
            return Ok(());
        }
        if o.no_logs {
            app_guard.disarm();
            return Ok(());
        }

        InteractiveSession::print_banner();
        let session = InteractiveSession::start();

        let logcat_child = Arc::new(Mutex::new(None::<Child>));
        let logcat_child_clone = Arc::clone(&logcat_child);
        let mut logcat_guard = crate::cleanup::register(move || {
            if let Ok(mut opt) = logcat_child_clone.lock() {
                if let Some(mut child) = opt.take() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        });

        let mut current_pid = target_pid;
        if let Ok(mut opt) = logcat_child.lock() {
            *opt = spawn_logcat_child(&adb, &serial, &current_pid);
        }

        loop {
            match session.recv_timeout(Duration::from_millis(500)) {
                Some(UserAction::HotReload) => {
                    println!("\n⚡ Performing hot reload for {}...", a.bundle_id);
                    let start = Instant::now();

                    stop_logcat_child(&logcat_child);

                    match self.build(p, &BuildOptions::default()) {
                        Ok(new_artifact) => {
                            let install = Command::new(&adb)
                                .args(["-s", &serial, "install", "-r"])
                                .arg(&new_artifact.path)
                                .status();
                            match install {
                                Ok(s) if s.success() => {
                                    let launch = Command::new(&adb)
                                        .args([
                                            "-s", &serial, "shell", "am", "start", "-S", "-W",
                                            "-n", &component,
                                        ])
                                        .output();
                                    match launch {
                                        Ok(out) if out.status.success() => {
                                            if let Ok(new_pids) = tools::text(
                                                &adb,
                                                &["-s", &serial, "shell", "pidof", &a.bundle_id],
                                            ) {
                                                if let Some(p) = new_pids.split_whitespace().next()
                                                {
                                                    current_pid = p.to_string();
                                                }
                                            }
                                            let elapsed = start.elapsed();
                                            println!(
                                                "⚡ Hot reload completed in {:.1}s.",
                                                elapsed.as_secs_f32()
                                            );
                                        }
                                        Ok(out) => {
                                            eprintln!(
                                                "Hot reload launch failed: {}",
                                                String::from_utf8_lossy(&out.stderr)
                                            );
                                        }
                                        Err(e) => {
                                            eprintln!("Hot reload launch failed: {e}");
                                        }
                                    }
                                }
                                Ok(s) => eprintln!("Hot reload install failed ({s})"),
                                Err(e) => eprintln!("Hot reload install failed: {e}"),
                            }
                        }
                        Err(e) => {
                            eprintln!("Hot reload build failed:\n{e:#}");
                        }
                    }

                    if let Ok(mut opt) = logcat_child.lock() {
                        *opt = spawn_logcat_child(&adb, &serial, &current_pid);
                    }
                }
                Some(UserAction::HotRestart) => {
                    println!("\n🔄 Performing hot restart for {}...", a.bundle_id);
                    let start = Instant::now();

                    stop_logcat_child(&logcat_child);

                    match self.build(p, &BuildOptions::default()) {
                        Ok(new_artifact) => {
                            let _ = Command::new(&adb)
                                .args(["-s", &serial, "shell", "am", "force-stop", &a.bundle_id])
                                .output();

                            let install = Command::new(&adb)
                                .args(["-s", &serial, "install", "-r"])
                                .arg(&new_artifact.path)
                                .status();
                            match install {
                                Ok(s) if s.success() => {
                                    let result = tools::capture_timeout(
                                        Command::new(&adb).args([
                                            "-s", &serial, "shell", "am", "start", "-W", "-n",
                                            &component,
                                        ]),
                                        Duration::from_secs(90),
                                    );
                                    match result {
                                        Ok(res) if res.status.success() => {
                                            if let Ok(new_pids) = tools::text(
                                                &adb,
                                                &["-s", &serial, "shell", "pidof", &a.bundle_id],
                                            ) {
                                                if let Some(p) = new_pids.split_whitespace().next()
                                                {
                                                    current_pid = p.to_string();
                                                }
                                            }
                                            let deadline = Instant::now() + Duration::from_secs(30);
                                            while Instant::now() < deadline {
                                                let logs = tools::text(
                                                    &adb,
                                                    &[
                                                        "-s",
                                                        &serial,
                                                        "logcat",
                                                        "-d",
                                                        &format!("--pid={current_pid}"),
                                                        "-s",
                                                        "gpui_android:I",
                                                    ],
                                                )
                                                .unwrap_or_default();
                                                if logs.contains("GPUI Vulkan frame presented") {
                                                    break;
                                                }
                                                std::thread::sleep(Duration::from_millis(200));
                                            }
                                            let elapsed = start.elapsed();
                                            println!(
                                                "✓ Hot restart completed in {:.1}s.",
                                                elapsed.as_secs_f32()
                                            );
                                        }
                                        _ => eprintln!("Hot restart launch failed."),
                                    }
                                }
                                Ok(s) => eprintln!("Hot restart install failed ({s})"),
                                Err(e) => eprintln!("Hot restart install failed: {e}"),
                            }
                        }
                        Err(e) => {
                            eprintln!("Hot restart build failed:\n{e:#}");
                        }
                    }

                    if let Ok(mut opt) = logcat_child.lock() {
                        *opt = spawn_logcat_child(&adb, &serial, &current_pid);
                    }
                }
                Some(UserAction::Help) => {
                    InteractiveSession::print_help();
                }
                Some(UserAction::Clear) => {
                    let _ = console::Term::stdout().clear_screen();
                    InteractiveSession::print_banner();
                }
                Some(UserAction::Quit) => {
                    println!("Quitting Omen application on {serial}...");
                    break;
                }
                None => {
                    let alive = tools::text(&adb, &["-s", &serial, "shell", "pidof", &a.bundle_id])
                        .map(|out| out.split_whitespace().any(|p| p == current_pid))
                        .unwrap_or(false);

                    if !alive {
                        println!("Application process {current_pid} exited on {serial}.");
                        break;
                    }
                }
            }
        }

        stop_logcat_child(&logcat_child);
        logcat_guard.disarm();
        Ok(())
    }
}
pub fn parse_adb(text: &str) -> Vec<Device> {
    text.lines()
        .filter_map(|line| {
            let mut columns = line.split_whitespace();
            let id = columns.next()?;
            let state = columns.next()?;
            if line.starts_with("List ") || line.starts_with('*') {
                return None;
            }
            let model = columns
                .find_map(|s| s.strip_prefix("model:"))
                .unwrap_or(id)
                .replace('_', " ");
            Some(Device {
                platform: Platform::Android,
                id: id.into(),
                name: model,
                state: state.into(),
                runnable: state == "device",
                kind: if id.starts_with("emulator-") {
                    "emulator"
                } else {
                    "physical"
                }
                .into(),
            })
        })
        .collect()
}
fn probe(tool: &std::path::Path, args: &[&str]) -> Result<String> {
    let output = tools::capture_timeout(Command::new(tool).args(args), Duration::from_secs(5))?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

pub fn devices() -> Result<Vec<Device>> {
    let Some(adb) = tools::android_tool("adb") else {
        return Ok(Vec::new());
    };
    let mut result =
        parse_adb(&probe(&adb, &["devices", "-l"]).context("Android device discovery failed")?);
    if let Some(emulator) = tools::android_tool("emulator") {
        let avds = probe(&emulator, &["-list-avds"])?;
        for name in avds.lines().filter(|line| !line.is_empty()) {
            result.push(Device {
                platform: Platform::Android,
                id: format!("avd:{name}"),
                name: name.into(),
                state: "stopped / start on selection".into(),
                runnable: true,
                kind: "avd".into(),
            });
        }
    }
    Ok(result)
}

pub(crate) fn studio_devices() -> Result<Vec<Device>> {
    let mut devices = devices()?;
    let Some(adb) = tools::android_tool("adb") else {
        return Ok(devices);
    };
    let mut running_names = std::collections::HashSet::new();
    for device in devices
        .iter_mut()
        .filter(|device| device.kind == "emulator" && device.runnable)
    {
        if let Some(name) = emulator_name(&adb, &device.id) {
            device.name = name.to_owned();
            running_names.insert(name);
        }
    }
    devices.retain(|device| device.kind != "avd" || !running_names.contains(&device.name));
    Ok(devices)
}

fn emulator_name(adb: &std::path::Path, serial: &str) -> Option<String> {
    probe(adb, &["-s", serial, "emu", "avd", "name"])
        .ok()?
        .lines()
        .next()
        .map(str::to_owned)
}

fn running_emulator(adb: &std::path::Path, name: &str) -> Result<Option<String>> {
    Ok(parse_adb(&probe(adb, &["devices", "-l"])?)
        .into_iter()
        .filter(|device| device.runnable && device.kind == "emulator")
        .find(|device| emulator_name(adb, &device.id).as_deref() == Some(name))
        .map(|device| device.id))
}

fn start_emulator(device: &Device, adb: &std::path::Path, managed: bool) -> Result<String> {
    let name = device
        .id
        .strip_prefix("avd:")
        .context("Invalid AVD identifier")?;
    let mut serial = running_emulator(adb, name)?;
    let (child, mut boot_guard) = if serial.is_none() {
        let emulator =
            tools::android_tool("emulator").context("Android emulator tool is unavailable")?;
        let mut command = Command::new(emulator);
        command
            .args(["-avd", name])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(unix)]
        if managed {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        // Windows uses the originating process tree for forced cancellation.
        #[cfg(not(unix))]
        let _ = managed;
        let child = Arc::new(Mutex::new(command.spawn()?));
        let cleanup_child = Arc::clone(&child);
        let guard = crate::cleanup::register(move || {
            if let Ok(mut child) = cleanup_child.lock() {
                let _ = child.kill();
                let _ = child.wait();
            }
        });
        println!("Starting Android emulator {name}…");
        (Some(child), Some(guard))
    } else {
        (None, None)
    };

    let deadline = Instant::now() + Duration::from_secs(180);
    while Instant::now() < deadline {
        if let Some(child) = &child
            && let Some(status) = child
                .lock()
                .map_err(|_| anyhow::anyhow!("Could not monitor Android emulator"))?
                .try_wait()?
        {
            bail!("Android emulator exited: {status}");
        }
        if serial.is_none() {
            serial = running_emulator(adb, name)?;
        }
        if let Some(serial) = &serial
            && probe(
                adb,
                &["-s", serial, "shell", "getprop", "sys.boot_completed"],
            )
            .is_ok_and(|state| state == "1")
        {
            if let Some(guard) = &mut boot_guard {
                guard.disarm();
            }
            return Ok(serial.clone());
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    bail!("Emulator {name} did not boot within 180 seconds")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn device_states_are_not_misreported_as_runnable() {
        let devices = parse_adb(
            "List of devices attached\nemulator-5554 device product:sdk model:Pixel_7\nphone unauthorized\noff offline\n",
        );
        assert_eq!(devices.len(), 3);
        assert!(devices[0].runnable);
        assert!(!devices[1].runnable);
        assert!(!devices[2].runnable);
        assert_eq!(devices[0].name, "Pixel 7");
    }
}
