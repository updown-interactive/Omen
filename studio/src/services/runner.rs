//! Runs builds and apps in a dedicated child of the Studio executable.
use crate::workspace::service::WorkspaceService;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use omen_cli::studio::{RunDevices, RunTarget};
use std::{
    env,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const LOG_WINDOW: u64 = 64 * 1024;
const RUN_CHILD: &str = "--omen-studio-run";

pub(crate) enum Event {
    Devices(u64, Result<RunDevices, String>),
    Output(u64, String),
    Finished(u64, Result<(), String>),
}

#[derive(Clone)]
pub(crate) struct Runner {
    sender: UnboundedSender<Event>,
    pub events: Rc<futures::lock::Mutex<UnboundedReceiver<Event>>>,
}

impl Default for Runner {
    fn default() -> Self {
        let (sender, events) = unbounded();
        Self {
            sender,
            events: Rc::new(futures::lock::Mutex::new(events)),
        }
    }
}

impl Runner {
    pub fn discover(&self, id: u64, root: PathBuf) -> io::Result<()> {
        let sender = self.sender.clone();
        std::thread::Builder::new()
            .name("omen-devices".into())
            .spawn(move || {
                let result =
                    omen_cli::studio::discover(&root).map_err(|error| format!("{error:#}"));
                let _ = sender.unbounded_send(Event::Devices(id, result));
            })?;
        Ok(())
    }

    pub fn start(
        &self,
        id: u64,
        root: PathBuf,
        target: RunTarget,
        workspace: WorkspaceService,
    ) -> io::Result<RunSession> {
        let cancel = Arc::new(AtomicBool::new(false));
        let control = RunSession(Arc::clone(&cancel));
        let sender = self.sender.clone();
        std::thread::Builder::new()
            .name("omen-run".into())
            .spawn(move || {
                let result = (|| {
                    workspace
                        .save_for_run(root.clone())
                        .map_err(|error| format!("Could not save project for Run: {error}"))?;
                    if cancel.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    run(&root, &target, &cancel, |output| {
                        let _ = sender.unbounded_send(Event::Output(id, output));
                    })
                })();
                let _ = sender.unbounded_send(Event::Finished(id, result));
            })?;
        Ok(control)
    }
}

pub(crate) struct RunSession(Arc<AtomicBool>);

impl RunSession {
    pub fn stop(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

impl Drop for RunSession {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run(
    root: &Path,
    target: &RunTarget,
    cancel: &AtomicBool,
    mut publish: impl FnMut(String),
) -> Result<(), String> {
    let path = run_path(root)?;
    if cancel.load(Ordering::Relaxed) {
        return Ok(());
    }
    let log_path = root.join(format!(".omen/logs/studio-run-{}.log", std::process::id()));
    let (mut child, mut output) = spawn(root, target, &log_path, path)
        .map_err(|error| format!("Could not start Omen runner: {error}"))?;
    let mut cancel_deadline = None;
    loop {
        let status = child.try_wait();
        if let Ok(Some(text)) = output.read() {
            publish(text);
        }
        match status {
            Ok(Some(status)) => {
                return if status.success() || cancel.load(Ordering::Relaxed) {
                    Ok(())
                } else {
                    Err(run_error(&log_path, status))
                };
            }
            Ok(None) => {}
            Err(error) => {
                kill_tree(&mut child);
                let _ = child.wait();
                return Err(format!("Could not monitor Omen runner: {error}"));
            }
        }
        if cancel.load(Ordering::Relaxed) {
            let deadline = *cancel_deadline.get_or_insert_with(|| {
                drop(child.stdin.take());
                Instant::now() + Duration::from_secs(8)
            });
            if Instant::now() >= deadline {
                kill_tree(&mut child);
                child.wait().map_err(|error| error.to_string())?;
                if let Ok(Some(text)) = output.read() {
                    publish(text);
                }
                return Ok(());
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn run_path(root: &Path) -> Result<std::ffi::OsString, String> {
    let inherited = env::var_os("PATH").unwrap_or_default();
    let cargo_bin = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            let home = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
            env::var_os(home).map(|home| PathBuf::from(home).join(".cargo"))
        })
        .map(|cargo| cargo.join("bin"));
    let path = env::join_paths(
        env::split_paths(&inherited)
            .filter(|entry| !entry.as_os_str().is_empty())
            .chain(cargo_bin),
    )
    .map_err(|error| error.to_string())?;
    omen_cli::toolchain::path_for(root, &path)
        .map(|toolchain| toolchain.unwrap_or(path))
        .map_err(|error| format!("Could not select the project toolchain: {error:#}"))
}

fn spawn(
    root: &Path,
    target: &RunTarget,
    log_path: &Path,
    path: std::ffi::OsString,
) -> io::Result<(Child, LogOutput)> {
    fs::create_dir_all(root.join(".omen/logs"))?;
    let log = File::create(log_path)?;
    let output = LogOutput {
        file: File::open(log_path)?,
        length: 0,
    };
    let mut command = Command::new(env::current_exe()?);
    command
        .arg(RUN_CHILD)
        .arg(root)
        .arg(target.platform.to_string())
        .args(target.device.iter())
        .current_dir(root)
        .env("PATH", path)
        .env("PYTHONUNBUFFERED", "1")
        .stdin(Stdio::piped())
        .stdout(log.try_clone()?)
        .stderr(log);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    Ok((command.spawn()?, output))
}

struct LogOutput {
    file: File,
    length: u64,
}

impl LogOutput {
    fn read(&mut self) -> io::Result<Option<String>> {
        let length = self.file.metadata()?.len();
        if length == self.length {
            return Ok(None);
        }
        let start = length.saturating_sub(LOG_WINDOW);
        self.file.seek(SeekFrom::Start(start))?;
        let mut bytes = Vec::new();
        (&mut self.file)
            .take(length - start)
            .read_to_end(&mut bytes)?;
        self.length = start + bytes.len() as u64;
        let mut text = String::from_utf8_lossy(&bytes).into_owned();
        if start != 0 {
            if let Some(line) = text.find('\n') {
                text.drain(..=line);
            }
            text.insert_str(0, "… Earlier output is in the session log file …\n");
        }
        Ok(Some(text))
    }
}

fn run_error(log_path: &Path, status: std::process::ExitStatus) -> String {
    let tail = (|| -> io::Result<Vec<u8>> {
        let mut log = File::open(log_path)?;
        let offset = log.metadata()?.len().saturating_sub(4096);
        log.seek(SeekFrom::Start(offset))?;
        let mut bytes = Vec::new();
        log.take(4096).read_to_end(&mut bytes)?;
        Ok(bytes)
    })()
    .unwrap_or_default();
    let text = String::from_utf8_lossy(&tail);
    let summary = text
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .unwrap_or("Omen runner exited unexpectedly");
    format!("{summary} ({status}). Log: {}", log_path.display())
}

#[cfg(unix)]
fn signal_group(pid: u32) {
    // The runner is the leader of a dedicated process group.
    unsafe { libc::kill(-(pid as libc::pid_t), libc::SIGKILL) };
}

#[cfg(windows)]
fn signal_group(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

pub(super) fn kill_tree(child: &mut Child) {
    #[cfg(any(unix, windows))]
    signal_group(child.id());
    let _ = child.kill();
}

/// Intercept the private run-control invocation before initializing the UI.
pub fn run_child_if_requested() {
    let mut args = env::args_os().skip(1);
    if !args.next().is_some_and(|arg| arg == RUN_CHILD) {
        return;
    }
    let (Some(root), Some(platform)) = (
        args.next(),
        args.next().and_then(|arg| arg.into_string().ok()),
    ) else {
        eprintln!("Invalid Omen Studio runner arguments");
        std::process::exit(2);
    };
    let device = args.next().and_then(|arg| arg.into_string().ok());
    match omen_cli::studio::run(Path::new(&root), &platform, device, || {
        #[cfg(unix)]
        if unsafe { libc::getpgrp() } == std::process::id() as libc::pid_t {
            signal_group(std::process::id());
        }
        #[cfg(windows)]
        signal_group(std::process::id());
    }) {
        Ok(()) => std::process::exit(0),
        Err(error) => {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn console_follows_recent_output_without_truncating_the_session_file() -> io::Result<()> {
        let path = env::temp_dir().join(format!("omen-session-log-{}", std::process::id()));
        let mut writer = File::create(&path)?;
        let mut output = LogOutput {
            file: File::open(&path)?,
            length: 0,
        };
        assert_eq!(output.read()?, None);
        writer.write_all(b"Building\n")?;
        assert_eq!(output.read()?.as_deref(), Some("Building\n"));
        writer.write_all("x\n".repeat(LOG_WINDOW as usize).as_bytes())?;
        writer.write_all("Finished ✓\n".as_bytes())?;
        let retained = output.read()?.unwrap();
        assert!(retained.ends_with("Finished ✓\n"));
        assert!(!retained.contains("Building"));
        assert!(writer.metadata()?.len() > LOG_WINDOW);
        drop(writer);
        drop(output);
        fs::remove_file(path)?;
        Ok(())
    }
}
