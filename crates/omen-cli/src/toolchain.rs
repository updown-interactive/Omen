//! Keep discovery and builds on the same project-selected Rust toolchain.
use crate::tools;
use anyhow::{Context, Result};
use std::{
    env,
    path::Path,
    process::{Command, ExitStatus},
};

pub fn path_for(
    project: &Path,
    search_path: &std::ffi::OsStr,
) -> Result<Option<std::ffi::OsString>> {
    // Explicit compiler overrides belong to the caller. Without rustup, keep
    // supporting ordinary system/package-manager Rust installations.
    if env::var_os("RUSTC").is_some() {
        return Ok(None);
    }
    let Some(rustup) = tools::find_in("rustup", search_path) else {
        return Ok(None);
    };
    let directory = project
        .canonicalize()
        .ok()
        .filter(|p| p.is_dir())
        .or_else(|| env::current_dir().ok())
        .context("Cannot locate the project directory")?;
    let probe = tools::capture_timeout(
        Command::new(rustup)
            .current_dir(directory)
            .env("PATH", search_path)
            .args(["which", "rustc"]),
        std::time::Duration::from_secs(5),
    )
    .context("Cannot inspect the active rustup toolchain")?;
    if !probe.status.success() {
        return Ok(None);
    }
    let compiler = std::path::PathBuf::from(String::from_utf8(probe.stdout)?.trim());
    let Some(bin) = compiler.parent() else {
        return Ok(None);
    };
    let cargo = bin.join(if cfg!(windows) { "cargo.exe" } else { "cargo" });
    if !compiler.is_file() || !cargo.is_file() {
        return Ok(None);
    }
    let same_file = |name: &str, expected: &Path| {
        tools::find_in(name, search_path).and_then(|p| p.canonicalize().ok())
            == expected.canonicalize().ok()
    };
    if same_file("rustc", &compiler) && same_file("cargo", &cargo) {
        return Ok(None);
    }
    Ok(Some(env::join_paths(
        std::iter::once(bin.to_path_buf()).chain(env::split_paths(search_path)),
    )?))
}

pub fn bootstrap(project: &Path) -> Result<Option<ExitStatus>> {
    let previous = env::var_os("PATH").unwrap_or_default();
    let Some(path) = path_for(project, &previous)? else {
        return Ok(None);
    };
    // Re-exec rather than changing global process environment. All probes,
    // Cargo metadata and Python/Gradle build children inherit this exact PATH.
    // The concrete bin paths match on the next invocation, preventing recursion.
    let status = Command::new(env::current_exe()?)
        .args(env::args_os().skip(1))
        .env("PATH", path)
        .status()
        .context("Cannot start Omen with the active Rust toolchain")?;
    Ok(Some(status))
}
