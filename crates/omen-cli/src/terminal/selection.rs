use crate::platforms::{Device, Platform, PlatformStatus};
use anyhow::{Result, bail};
use dialoguer::{Select, theme::ColorfulTheme};
use std::io::IsTerminal;

#[derive(Debug)]
pub struct Cancelled;
impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Selection cancelled")
    }
}
impl std::error::Error for Cancelled {}

pub fn require_interactive() -> Result<()> {
    if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
        bail!(
            "Interactive selection needs a terminal; supply --platform and, for multiple devices, --device"
        );
    }
    Ok(())
}

fn select(prompt: &str, items: &[String]) -> Result<usize> {
    if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
        bail!(
            "Interactive selection needs a terminal; supply --platform and, for multiple devices, --device"
        )
    }
    Select::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .items(items)
        .default(0)
        .interact_opt()?
        .ok_or_else(|| anyhow::Error::new(Cancelled))
}
pub fn platform(name: &str, statuses: &[PlatformStatus]) -> Result<Platform> {
    for status in statuses.iter().filter(|s| !s.can_build) {
        eprintln!("{}", console::style(status.label()).dim());
    }
    let available: Vec<_> = statuses.iter().filter(|s| s.can_build).collect();
    if available.is_empty() {
        bail!("No build targets are available; run omen doctor")
    }
    let labels = available.iter().map(|s| s.label()).collect::<Vec<_>>();
    Ok(available[select(&format!("{name} · Select a run target"), &labels)?].platform)
}
pub fn device(devices: &[Device], requested: Option<&str>) -> Result<Device> {
    let available: Vec<_> = devices.iter().filter(|d| d.runnable).collect();
    if let Some(requested) = requested {
        let matching: Vec<_> = available
            .iter()
            .filter(|d| d.id == requested || d.name == requested)
            .collect();
        return match matching.as_slice() {
            [device] => Ok((***device).clone()),
            [] => bail!("Device '{requested}' is unavailable; run omen devices"),
            _ => bail!("Device name '{requested}' is ambiguous; use its ID"),
        };
    }
    match available.as_slice() {
        [] => bail!("No runnable devices found; run omen devices"),
        [device] => Ok((*device).clone()),
        _ => {
            let labels = available.iter().map(|d| d.label()).collect::<Vec<_>>();
            Ok(available[select("Select a device", &labels)?].clone())
        }
    }
}
