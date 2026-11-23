use anyhow::{Context, Result, bail};
use std::{
    env,
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{Command, Output},
};

pub fn find(name: &str) -> Option<PathBuf> {
    find_in(name, &env::var_os("PATH")?)
}

pub(crate) fn find_in(name: &str, search_path: &OsStr) -> Option<PathBuf> {
    let path = Path::new(name);
    if path.is_absolute() && path.is_file() {
        return Some(path.into());
    }
    env::split_paths(search_path).find_map(|dir| {
        let candidates = if cfg!(windows) {
            vec![
                dir.join(name),
                dir.join(format!("{name}.exe")),
                dir.join(format!("{name}.cmd")),
                dir.join(format!("{name}.bat")),
            ]
        } else {
            vec![dir.join(name)]
        };
        candidates.into_iter().find(|p| p.is_file())
    })
}
pub fn python() -> Option<PathBuf> {
    ["python3", "python"]
        .into_iter()
        .filter_map(find)
        .find(|path| {
            text(path, &["--version"]).is_ok_and(|version| {
                version
                    .strip_prefix("Python 3.")
                    .and_then(|v| v.split('.').next())
                    .and_then(|v| v.parse::<u32>().ok())
                    .is_some_and(|minor| minor >= 9)
            })
        })
}
pub fn required(name: &str) -> Result<PathBuf> {
    find(name).with_context(|| format!("{name} was not found on PATH"))
}
pub fn output(tool: impl AsRef<OsStr>, args: &[&str]) -> Result<Output> {
    Command::new(tool)
        .args(args)
        .output()
        .context("Could not inspect development tool")
}
pub fn text(tool: impl AsRef<OsStr>, args: &[&str]) -> Result<String> {
    let result = output(tool, args)?;
    if !result.status.success() {
        bail!(
            "Tool probe failed: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        )
    }
    Ok(String::from_utf8(result.stdout)?.trim().to_owned())
}
pub fn run(command: &mut Command) -> Result<()> {
    let description = format!("{command:?}");
    let status = command
        .status()
        .with_context(|| format!("Could not start {description}"))?;
    if !status.success() {
        bail!("Command failed ({status}): {description}")
    }
    Ok(())
}
pub fn sdk_home() -> Option<PathBuf> {
    ["ANDROID_HOME", "ANDROID_SDK_ROOT"]
        .into_iter()
        .find_map(env::var_os)
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME").map(|home| {
                PathBuf::from(home).join(if cfg!(target_os = "macos") {
                    "Library/Android/sdk"
                } else {
                    "Android/Sdk"
                })
            })
        })
        .or_else(|| env::var_os("LOCALAPPDATA").map(|home| PathBuf::from(home).join("Android/Sdk")))
        .filter(|path| path.is_dir())
}
pub fn android_tool(name: &str) -> Option<PathBuf> {
    find(name).or_else(|| {
        sdk_home()
            .map(|root| {
                root.join(if name == "adb" {
                    "platform-tools"
                } else {
                    "emulator"
                })
                .join(format!("{name}{}", if cfg!(windows) { ".exe" } else { "" }))
            })
            .filter(|p| p.is_file())
    })
}
pub fn java() -> Option<PathBuf> {
    let candidate = env::var_os("JAVA_HOME").map(PathBuf::from).map(|p| {
        p.join("bin")
            .join(if cfg!(windows) { "java.exe" } else { "java" })
    });
    candidate
        .into_iter()
        .chain([PathBuf::from(
            "/Applications/Android Studio.app/Contents/jbr/Contents/Home/bin/java",
        )])
        .chain(find("java"))
        .find(|p| {
            p.is_file()
                && output(p, ["-version"].as_slice()).is_ok_and(|o| {
                    let text = format!(
                        "{}{}",
                        String::from_utf8_lossy(&o.stdout),
                        String::from_utf8_lossy(&o.stderr)
                    );
                    let version = text.split('"').nth(1).unwrap_or("");
                    let mut numbers = version.split('.');
                    let first = numbers
                        .next()
                        .and_then(|s| s.parse::<u32>().ok())
                        .unwrap_or(0);
                    let major = if first == 1 {
                        numbers
                            .next()
                            .and_then(|s| s.parse::<u32>().ok())
                            .unwrap_or(0)
                    } else {
                        first
                    };
                    o.status.success() && major >= 17
                })
        })
}
pub fn gradle() -> Option<PathBuf> {
    if let Some(path) = env::var_os("GRADLE")
        .and_then(|p| find(&p.to_string_lossy()))
        .or_else(|| find("gradle"))
    {
        return Some(path);
    }
    fn scan(root: &Path, depth: usize, result: &mut Vec<PathBuf>) {
        if depth == 0 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                scan(&p, depth - 1, result)
            } else if p.file_name().is_some_and(|s| {
                s == if cfg!(windows) {
                    "gradle.bat"
                } else {
                    "gradle"
                }
            }) && p.parent().is_some_and(|p| p.ends_with("bin"))
            {
                result.push(p)
            }
        }
    }
    let mut found = Vec::new();
    scan(
        &PathBuf::from(env::var_os("HOME")?).join(".gradle/wrapper/dists"),
        5,
        &mut found,
    );
    found.sort();
    found.pop()
}
pub fn installed_targets() -> Vec<String> {
    if let Ok(sysroot) = required("rustc").and_then(|p| text(p, &["--print", "sysroot"]))
        && let Ok(entries) = std::fs::read_dir(Path::new(&sysroot).join("lib/rustlib"))
    {
        return entries
            .flatten()
            .filter_map(|entry| {
                let lib = entry.path().join("lib");
                let installed = std::fs::read_dir(lib)
                    .ok()?
                    .flatten()
                    .any(|f| f.file_name().to_string_lossy().starts_with("libstd-"));
                installed.then(|| entry.file_name().to_string_lossy().into_owned())
            })
            .collect();
    }
    Vec::new()
}
pub fn host_target() -> Result<String> {
    let version = text(required("rustc")?, &["-vV"])?;
    version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_owned)
        .context("rustc did not report its host target")
}
pub fn ndk_compiler() -> Option<PathBuf> {
    let root = env::var_os("ANDROID_NDK_HOME")
        .map(PathBuf::from)
        .or_else(|| sdk_home().map(|p| p.join("ndk/28.2.13676358")))?;
    let host = if cfg!(target_os = "macos") {
        "darwin-x86_64"
    } else if cfg!(windows) {
        "windows-x86_64"
    } else {
        "linux-x86_64"
    };
    let file = root
        .join("toolchains/llvm/prebuilt")
        .join(host)
        .join("bin")
        .join(if cfg!(windows) {
            "aarch64-linux-android26-clang.cmd"
        } else {
            "aarch64-linux-android26-clang"
        });
    file.is_file().then_some(file)
}

pub fn capture_timeout(command: &mut Command, timeout: std::time::Duration) -> Result<Output> {
    use std::{io::Read, process::Stdio, time::Instant};
    let description = format!("{command:?}");
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Could not start {description}"))?;
    let stdout = child.stdout.take().context("Missing stdout pipe")?;
    let stderr = child.stderr.take().context("Missing stderr pipe")?;
    let read = |mut stream: Box<dyn Read + Send>| {
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).map(|_| bytes)
    };
    let out = std::thread::spawn(move || read(Box::new(stdout)));
    let err = std::thread::spawn(move || read(Box::new(stderr)));
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let stdout = out
        .join()
        .map_err(|_| anyhow::anyhow!("Tool output reader failed"))??;
    let stderr = err
        .join()
        .map_err(|_| anyhow::anyhow!("Tool error reader failed"))??;
    let status = status.with_context(|| {
        format!(
            "Timed out after {} seconds: {description}\n{}",
            timeout.as_secs(),
            String::from_utf8_lossy(&stderr)
        )
    })?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}
