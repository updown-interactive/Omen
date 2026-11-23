use super::*;
use crate::terminal::interactive::{InteractiveSession, UserAction};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct Desktop;
impl PlatformRunner for Desktop {
    fn detect(&self) -> PlatformStatus {
        let native_host = cfg!(any(
            target_os = "macos",
            target_os = "linux",
            target_os = "windows"
        ));
        let compiler = tools::find("rustc").is_some() && tools::find("cargo").is_some();
        let graphics = if cfg!(target_os = "macos") {
            tools::text("xcrun", &["--sdk", "macosx", "--show-sdk-path"]).is_ok()
        } else if cfg!(target_os = "linux") {
            tools::output(
                "pkg-config",
                &[
                    "--exists",
                    "wayland-client",
                    "xkbcommon",
                    "x11",
                    "fontconfig",
                    "freetype2",
                    "alsa",
                    "openssl",
                ],
            )
            .is_ok_and(|o| o.status.success())
        } else {
            tools::find("cl").is_some()
        };
        PlatformStatus::new(
            Platform::Desktop,
            "native GPUI · toolchain detected; launch not verified",
            vec![
                (native_host, "supported desktop OS"),
                (compiler, "Rust toolchain"),
                (graphics, "native SDK/development libraries"),
                (tools::python().is_some(), "Python 3.9+"),
            ],
        )
    }
    fn build(&self, p: &Project, o: &BuildOptions) -> Result<BuildArtifact> {
        package(p, Platform::Desktop, o)
    }
    fn run(&self, p: &Project, a: &BuildArtifact, o: &RunOptions) -> Result<()> {
        let child = Command::new(&a.path)
            .current_dir(&p.root)
            .spawn()
            .context("Could not start desktop application")?;

        let child_ref = Arc::new(Mutex::new(Some(child)));
        let child_ref_clone = Arc::clone(&child_ref);
        let mut guard = crate::cleanup::register(move || {
            if let Ok(mut opt) = child_ref_clone.lock() {
                if let Some(mut c) = opt.take() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
            }
        });

        if o.managed {
            loop {
                let status = child_ref
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Desktop process state could not be read"))?
                    .as_mut()
                    .context("Desktop process is no longer available")?
                    .try_wait()?;
                if let Some(status) = status {
                    guard.disarm();
                    if !status.success() {
                        bail!("Desktop application exited with {status}");
                    }
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        }

        InteractiveSession::print_banner();
        let session = InteractiveSession::start();

        loop {
            {
                let mut opt = child_ref.lock().unwrap();
                if let Some(ref mut c) = *opt {
                    match c.try_wait() {
                        Ok(Some(_status)) => break,
                        Ok(None) => {}
                        Err(_) => break,
                    }
                } else {
                    break;
                }
            }

            match session.recv_timeout(Duration::from_millis(500)) {
                Some(UserAction::HotReload | UserAction::HotRestart) => {
                    println!("\n🔄 Performing restart for desktop application...");
                    let start = Instant::now();
                    if let Ok(mut opt) = child_ref.lock() {
                        if let Some(mut c) = opt.take() {
                            let _ = c.kill();
                            let _ = c.wait();
                        }
                    }
                    match self.build(p, &BuildOptions::default()) {
                        Ok(new_artifact) => {
                            match Command::new(&new_artifact.path)
                                .current_dir(&p.root)
                                .spawn()
                            {
                                Ok(new_child) => {
                                    *child_ref.lock().unwrap() = Some(new_child);
                                    let elapsed = start.elapsed();
                                    println!(
                                        "✓ Hot restart completed in {:.1}s.",
                                        elapsed.as_secs_f32()
                                    );
                                }
                                Err(e) => eprintln!("Failed to spawn desktop application: {e}"),
                            }
                        }
                        Err(e) => eprintln!("Hot restart build failed:\n{e:#}"),
                    }
                }
                Some(UserAction::Help) => InteractiveSession::print_help(),
                Some(UserAction::Clear) => {
                    let _ = console::Term::stdout().clear_screen();
                    InteractiveSession::print_banner();
                }
                Some(UserAction::Quit) => {
                    println!("Quitting desktop application...");
                    break;
                }
                None => {}
            }
        }

        if let Ok(mut opt) = child_ref.lock() {
            if let Some(mut c) = opt.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
        guard.disarm();
        Ok(())
    }
}
