//! Runs a Codex chat turn away from the UI thread.
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use serde_json::Value;
use std::{
    env,
    fs::File,
    io::{self, BufRead, BufReader, Read, Seek, Write},
    path::{Path, PathBuf},
    process::{Child, Command},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

const OUTPUT_LIMIT: u64 = 8 * 1024 * 1024;

pub(crate) struct Reply {
    pub thread: Option<String>,
    pub text: String,
}

pub(crate) enum Outcome {
    Completed(Reply),
    Failed(String),
    Stopped,
}

pub(crate) struct Event {
    pub id: u64,
    pub outcome: Outcome,
}

#[derive(Clone)]
pub(crate) struct Client {
    sender: UnboundedSender<Event>,
    pub events: Rc<futures::lock::Mutex<UnboundedReceiver<Event>>>,
}

impl Default for Client {
    fn default() -> Self {
        let (sender, events) = unbounded();
        Self {
            sender,
            events: Rc::new(futures::lock::Mutex::new(events)),
        }
    }
}

impl Client {
    pub fn send(
        &self,
        id: u64,
        root: PathBuf,
        thread: Option<String>,
        prompt: String,
    ) -> io::Result<Task> {
        let process = Arc::new(Process::default());
        let worker_process = process.clone();
        let sender = self.sender.clone();
        std::thread::Builder::new()
            .name("omen-studio-agent".into())
            .spawn(move || {
                let result = run(&root, thread.as_deref(), &prompt, &worker_process);
                let outcome = if worker_process.stopped.load(Ordering::Acquire) {
                    Outcome::Stopped
                } else {
                    match result {
                        Ok(reply) => Outcome::Completed(reply),
                        Err(message) => Outcome::Failed(message),
                    }
                };
                let _ = sender.unbounded_send(Event { id, outcome });
            })?;
        Ok(Task(process))
    }
}

#[derive(Default)]
struct Process {
    stopped: AtomicBool,
    child: Mutex<Option<Child>>,
}

pub(crate) struct Task(Arc<Process>);

impl Task {
    pub fn stop(&self) {
        self.0.stopped.store(true, Ordering::Release);
        if let Ok(mut child) = self.0.child.lock()
            && let Some(child) = child.as_mut()
        {
            super::runner::kill_tree(child);
        }
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        self.stop();
    }
}

fn cli() -> Option<PathBuf> {
    if let Some(path) = env::var_os("OMEN_AGENT_CLI") {
        return omen_cli::tools::find(&path.to_string_lossy());
    }
    #[cfg(target_os = "macos")]
    {
        let relative = "ChatGPT.app/Contents/Resources/codex-cli/bin/codex";
        let bundled = Path::new("/Applications").join(relative);
        if bundled.is_file() {
            return Some(bundled);
        }
        if let Some(bundled) = env::var_os("HOME")
            .map(|home| PathBuf::from(home).join("Applications").join(relative))
            .filter(|path| path.is_file())
        {
            return Some(bundled);
        }
    }
    omen_cli::tools::find("codex")
}

fn run(
    root: &Path,
    thread: Option<&str>,
    prompt: &str,
    process: &Process,
) -> Result<Reply, String> {
    let binary = cli().ok_or(
        "Codex CLI could not be found. Install Codex or set OMEN_AGENT_CLI to its executable.",
    )?;
    run_cli(&binary, root, thread, prompt, process)
}

fn run_cli(
    binary: &Path,
    root: &Path,
    thread: Option<&str>,
    prompt: &str,
    process: &Process,
) -> Result<Reply, String> {
    let mut input = tempfile::tempfile().map_err(io_message)?;
    input.write_all(prompt.as_bytes()).map_err(io_message)?;
    input.rewind().map_err(io_message)?;
    let mut events = tempfile::tempfile().map_err(io_message)?;
    let mut errors = tempfile::tempfile().map_err(io_message)?;
    let mut command = Command::new(binary);
    command.current_dir(root).args([
        "exec",
        "--json",
        "--skip-git-repo-check",
        "--ignore-user-config",
        "-c",
        "approval_policy=\"never\"",
        "-c",
        "sandbox_mode=\"read-only\"",
    ]);
    if let Some(thread) = thread {
        command.args(["resume", thread]);
    }
    command
        .arg("-")
        .stdin(input)
        .stdout(events.try_clone().map_err(io_message)?)
        .stderr(errors.try_clone().map_err(io_message)?);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }

    {
        let mut child = process.child.lock().map_err(|_| "Could not start Codex")?;
        if process.stopped.load(Ordering::Acquire) {
            return Err("Stopped".into());
        }
        *child = Some(
            command
                .spawn()
                .map_err(|error| format!("Could not start Codex: {error}"))?,
        );
    }
    let status = wait(process, &events, &errors)?;
    let (reply, failure) = parse_events(&mut events)?;
    if status.success() && !reply.text.is_empty() {
        return Ok(reply);
    }
    let stderr = error_tail(&mut errors);
    Err(failure.or(stderr).unwrap_or_else(|| {
        if status.success() {
            "Codex completed without a reply.".into()
        } else {
            format!("Codex exited with {status}. Run `codex login` if you need to sign in.")
        }
    }))
}

fn wait(
    process: &Process,
    events: &File,
    errors: &File,
) -> Result<std::process::ExitStatus, String> {
    loop {
        {
            let mut child = process
                .child
                .lock()
                .map_err(|_| "Could not monitor Codex")?;
            let running = child.as_mut().ok_or("Codex process could not be found")?;
            let result = running.try_wait();
            match result {
                Ok(Some(status)) => {
                    child.take();
                    return Ok(status);
                }
                Ok(None)
                    if [events, errors].into_iter().all(|file| {
                        file.metadata()
                            .is_ok_and(|metadata| metadata.len() <= OUTPUT_LIMIT)
                    }) => {}
                other => {
                    super::runner::kill_tree(running);
                    let _ = running.wait();
                    child.take();
                    return Err(other.err().map_or_else(
                        || "Codex session output exceeded the limit.".into(),
                        io_message,
                    ));
                }
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn parse_events(file: &mut File) -> Result<(Reply, Option<String>), String> {
    file.rewind().map_err(io_message)?;
    let mut reply = Reply {
        thread: None,
        text: String::new(),
    };
    let mut failure = None;
    for line in BufReader::new(file.take(OUTPUT_LIMIT)).split(b'\n') {
        let bytes = line.map_err(io_message)?;
        let Ok(event) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        match event["type"].as_str() {
            Some("thread.started") => {
                reply.thread = event["thread_id"]
                    .as_str()
                    .filter(|id| {
                        id.len() == 36
                            && id
                                .bytes()
                                .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
                    })
                    .map(str::to_owned);
            }
            Some("item.completed") if event["item"]["type"] == "agent_message" => {
                if let Some(text) = event["item"]["text"].as_str() {
                    reply.text = text.char_indices().nth(64_000).map_or_else(
                        || text.to_owned(),
                        |(end, _)| format!("{}\n…", &text[..end]),
                    );
                }
            }
            Some("turn.failed" | "error") => {
                failure = event["error"]["message"]
                    .as_str()
                    .or_else(|| event["message"].as_str())
                    .map(|message| message.chars().take(1500).collect());
            }
            _ => {}
        }
    }
    Ok((reply, failure))
}

fn error_tail(file: &mut File) -> Option<String> {
    let length = file.metadata().ok()?.len();
    file.seek(io::SeekFrom::Start(length.saturating_sub(4096)))
        .ok()?;
    let mut bytes = Vec::new();
    file.take(4096).read_to_end(&mut bytes).ok()?;
    String::from_utf8_lossy(&bytes)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().chars().take(700).collect())
}

fn io_message(error: io::Error) -> String {
    format!("Codex I/O error: {error}")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn codex_reply_carries_the_thread_for_followups() {
        let directory = tempfile::tempdir().unwrap();
        let binary = directory.path().join("codex-test");
        std::fs::write(&binary, r#"#!/bin/sh
set -eu
test "$PWD" = "$(dirname "$0")"
expected="exec --json --skip-git-repo-check --ignore-user-config -c approval_policy=\"never\" -c sandbox_mode=\"read-only\""
case "$(cat)" in
  Overview) test "$*" = "$expected -" ;;
  Follow-up) test "$*" = "$expected resume 12345678-1234-1234-1234-123456789abc -" ;;
  *) exit 3 ;;
esac
printf '%s\n' '{"type":"thread.started","thread_id":"12345678-1234-1234-1234-123456789abc"}'
printf '%s\n' '{"type":"item.completed","item":{"type":"command_execution","text":"do not display"}}'
printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"Project overview."}}'
"#).unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let root = directory.path().canonicalize().unwrap();
        let binary = root.join("codex-test");
        let initial = run_cli(&binary, &root, None, "Overview", &Process::default()).unwrap();
        let followup = run_cli(
            &binary,
            &root,
            initial.thread.as_deref(),
            "Follow-up",
            &Process::default(),
        )
        .unwrap();
        assert_eq!(followup.text, "Project overview.");
    }
}
