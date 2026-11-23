pub mod android;
pub mod desktop;
pub mod ios;
pub mod web;
use crate::{project::Project, tools};
use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::{fmt, path::PathBuf, process::Command};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, ValueEnum, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Desktop,
    Android,
    Ios,
    Web,
}
impl Platform {
    pub const ALL: [Self; 4] = [Self::Desktop, Self::Android, Self::Ios, Self::Web];
}
impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Desktop => "desktop",
            Self::Android => "android",
            Self::Ios => "ios",
            Self::Web => "web",
        })
    }
}
#[derive(Debug, Serialize)]
pub struct PlatformStatus {
    pub platform: Platform,
    pub can_build: bool,
    pub launch_verified: bool,
    pub detail: String,
    pub missing: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub connected_devices: Vec<String>,
}
impl PlatformStatus {
    pub fn new(platform: Platform, detail: &str, requirements: Vec<(bool, &str)>) -> Self {
        let missing: Vec<_> = requirements
            .into_iter()
            .filter(|(ok, _)| !ok)
            .map(|(_, s)| s.to_owned())
            .collect();
        Self {
            platform,
            can_build: missing.is_empty(),
            launch_verified: false,
            detail: detail.into(),
            missing,
            connected_devices: Vec::new(),
        }
    }
    pub fn label(&self) -> String {
        let devices = if self.connected_devices.is_empty() {
            String::new()
        } else {
            format!(" · {}", self.connected_devices.join(", "))
        };
        if !self.can_build {
            format!(
                "{}{devices} · unavailable: {}",
                self.platform,
                self.missing.join(", ")
            )
        } else {
            format!("{}{devices} · {}", self.platform, self.detail)
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Device {
    pub platform: Platform,
    pub id: String,
    pub name: String,
    pub state: String,
    pub runnable: bool,
    pub kind: String,
}
impl Device {
    pub fn label(&self) -> String {
        format!(
            "{} · {} · {} ({})",
            self.name, self.id, self.state, self.kind
        )
    }
}
#[derive(Debug, Deserialize)]
pub struct BuildArtifact {
    pub platform: Platform,
    pub path: PathBuf,
    #[serde(default)]
    pub bundle_id: String,
    #[serde(default)]
    pub activity: String,
}
#[derive(Clone, Default)]
pub struct BuildOptions {
    pub release: bool,
    pub ios_device: bool,
    pub sign: bool,
}
pub struct RunOptions {
    pub device: Option<String>,
    pub port: u16,
    pub no_open: bool,
    pub no_logs: bool,
    pub managed: bool,
}
pub trait PlatformRunner {
    fn detect(&self) -> PlatformStatus;
    fn build(&self, project: &Project, options: &BuildOptions) -> Result<BuildArtifact>;
    fn run(&self, project: &Project, artifact: &BuildArtifact, options: &RunOptions) -> Result<()>;
}
pub fn runner(platform: Platform) -> Box<dyn PlatformRunner> {
    match platform {
        Platform::Desktop => Box::new(desktop::Desktop),
        Platform::Android => Box::new(android::Android),
        Platform::Ios => Box::new(ios::Ios),
        Platform::Web => Box::new(web::Web),
    }
}
pub fn package(
    project: &Project,
    platform: Platform,
    options: &BuildOptions,
) -> Result<BuildArtifact> {
    project.manifest.require_target(&platform.to_string())?;
    let status = if platform == Platform::Ios && options.ios_device {
        PlatformStatus::new(
            platform,
            "native iOS device build",
            vec![
                (cfg!(target_os = "macos"), "macOS"),
                (tools::find("cargo").is_some(), "Cargo"),
                (tools::python().is_some(), "Python 3.9+"),
                (
                    tools::text("xcrun", &["--sdk", "iphoneos", "--show-sdk-path"]).is_ok(),
                    "Xcode iPhoneOS SDK",
                ),
                (
                    tools::installed_targets()
                        .iter()
                        .any(|t| t == "aarch64-apple-ios"),
                    "Rust target aarch64-apple-ios",
                ),
            ],
        )
    } else {
        runner(platform).detect()
    };
    if !status.can_build {
        bail!("{}", status.label())
    }
    let icons = crate::icons::load(&project.root, &project.manifest)?
        .map(|image| -> Result<_> {
            let stage = tempfile::tempdir()?;
            crate::icons::generate(&image, platform, stage.path())?;
            Ok(stage)
        })
        .transpose()?;
    let config = project.packaging_config()?;
    let out = project
        .target
        .join("omen/packages")
        .join(platform.to_string());
    let mut command = Command::new(tools::python().context("Python 3 was not found on PATH")?);
    command
        .arg(project.sdk.join("scripts/package.py"))
        .arg(platform.to_string())
        .arg("--project-root")
        .arg(&project.root)
        .arg("--config")
        .arg(config)
        .arg("--target-dir")
        .arg(&project.target)
        .arg("--out")
        .arg(&out);
    if let Some(icons) = &icons {
        command.arg("--icons").arg(icons.path());
    }
    if options.release {
        command.arg("--release");
    }
    if options.sign {
        command.arg("--sign");
    }
    if options.ios_device {
        command.arg("--device");
    }
    if let Some(sdk) = tools::sdk_home() {
        command.env("ANDROID_HOME", sdk);
    }
    if let Some(gradle) = tools::gradle() {
        command.env("GRADLE", gradle);
    }
    if let Some(java) = tools::java().and_then(|p| p.parent()?.parent().map(PathBuf::from)) {
        command.env("JAVA_HOME", java);
    }
    command.current_dir(&project.root);
    tools::run(&mut command)?;
    let artifact: BuildArtifact = serde_json::from_slice(
        &std::fs::read(out.join("omen-artifact.json"))
            .context("Packager did not produce an artifact manifest")?,
    )?;
    if artifact.platform != platform || !artifact.path.exists() {
        bail!("Packager returned a missing or incorrect artifact")
    }
    println!("Built {}", artifact.path.display());
    Ok(artifact)
}
pub fn discover_devices() -> (Vec<Device>, Vec<String>) {
    let mut devices = Vec::new();
    let mut issues = Vec::new();
    for result in [android::devices(), ios::devices()] {
        match result {
            Ok(found) => devices.extend(found),
            Err(error) => issues.push(format!("{error:#}")),
        }
    }
    (devices, issues)
}

#[cfg(test)]
mod status_tests {
    use super::*;
    #[test]
    fn connected_phone_stays_visible_when_a_build_requirement_is_missing() {
        let mut status = PlatformStatus::new(
            Platform::Android,
            "native GPUI",
            vec![(false, "Rust target aarch64-linux-android")],
        );
        status.connected_devices.push("V2413".into());
        assert_eq!(
            status.label(),
            "android · V2413 · unavailable: Rust target aarch64-linux-android"
        );
    }
}
