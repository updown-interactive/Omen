pub use crate::platforms::Platform;
use crate::{
    platforms::{self, BuildOptions, RunOptions},
    project::Project,
    terminal::selection,
    tools,
};
use anyhow::{Result, bail};
use clap::Args;
use std::path::Path;
#[derive(Args)]
pub struct BuildArgs {
    #[arg(short = 'p', long, value_enum)]
    pub platform: Platform,
    #[arg(long)]
    pub release: bool,
    /// Use configured signing credentials; never uploads or publishes.
    #[arg(long)]
    pub sign: bool,
    /// Package for a physical iOS device (requires signing).
    #[arg(long, requires = "sign")]
    pub ios_device: bool,
}
#[derive(Args, Default)]
pub struct RunArgs {
    #[arg(short = 'p', long, value_enum)]
    platform: Option<Platform>,
    #[arg(long)]
    device: Option<String>,
    #[arg(long)]
    release: bool,
    #[arg(long)]
    sign: bool,
    /// Serve web output without opening a browser.
    #[arg(long)]
    no_open: bool,
    /// Return after Android native frame verification instead of streaming logcat.
    #[arg(long)]
    no_logs: bool,
    /// Browser development server port; zero selects an unused port.
    #[arg(long, default_value = "0")]
    port: u16,
}
pub fn generate_icon(start: &Path, platform: Option<Platform>) -> Result<()> {
    crate::icons::generate_project(start, platform)
}

pub fn build(root: &Path, args: BuildArgs) -> Result<()> {
    if args.ios_device && args.platform != Platform::Ios {
        bail!("--ios-device applies only to iOS")
    }
    if args.platform == Platform::Android && args.sign && !args.release {
        bail!("Android signing requires --release");
    }
    let p = Project::load(root)?;
    p.manifest.require_target(&args.platform.to_string())?;
    platforms::runner(args.platform).build(
        &p,
        &BuildOptions {
            release: args.release,
            ios_device: args.ios_device,
            sign: args.sign,
        },
    )?;
    Ok(())
}
pub fn run(root: &Path, args: RunArgs) -> Result<()> {
    run_with_mode(root, args, false)
}

pub(crate) fn run_managed(root: &Path, platform: Platform, device: Option<String>) -> Result<()> {
    run_with_mode(
        root,
        RunArgs {
            platform: Some(platform),
            device,
            ..Default::default()
        },
        true,
    )
}

fn run_with_mode(root: &Path, args: RunArgs, managed: bool) -> Result<()> {
    if args.platform.is_none() {
        selection::require_interactive()?;
    }
    let p = Project::load(root)?;
    let platform = match args.platform {
        Some(platform) => platform,
        None => {
            let statuses = Platform::ALL
                .into_iter()
                .filter(|platform| p.manifest.supports_target(&platform.to_string()))
                .map(|platform| platforms::runner(platform).detect())
                .collect::<Vec<_>>();
            selection::platform(&p.manifest.app.name, &statuses)?
        }
    };
    p.manifest.require_target(&platform.to_string())?;
    if args.device.is_some() && matches!(platform, Platform::Desktop | Platform::Web) {
        bail!("--device applies to Android and iOS")
    }
    if platform == Platform::Android && args.sign && !args.release {
        bail!("Android signing requires --release");
    }
    if platform == Platform::Web && !args.no_open && !platforms::web::opener_available() {
        bail!("No browser opener is available; use --no-open");
    }
    // Resolve devices before an expensive build; a missing/ambiguous selection is actionable immediately.
    let device = match platform {
        Platform::Android => {
            Some(selection::device(&platforms::android::devices()?, args.device.as_deref())?.id)
        }
        Platform::Ios => {
            Some(selection::device(&platforms::ios::devices()?, args.device.as_deref())?.id)
        }
        _ => None,
    };
    let runner = platforms::runner(platform);
    let artifact = runner.build(
        &p,
        &BuildOptions {
            release: args.release,
            ios_device: false,
            sign: args.sign,
        },
    )?;
    runner.run(
        &p,
        &artifact,
        &RunOptions {
            device,
            port: args.port,
            no_open: args.no_open,
            no_logs: args.no_logs,
            managed,
        },
    )
}
pub fn devices(json: bool) -> Result<()> {
    let (devices, issues) = platforms::discover_devices();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({"devices":devices,"issues":issues}))?
        )
    } else {
        if devices.is_empty() {
            println!("No connected devices or installed simulators detected.");
        }
        for device in devices {
            println!("{} · {}", device.platform, device.label());
        }
        for issue in issues {
            eprintln!("Discovery: {issue}");
        }
    }
    Ok(())
}
pub fn doctor(json: bool) -> Result<()> {
    let statuses = Platform::ALL
        .into_iter()
        .map(|p| platforms::runner(p).detect())
        .collect::<Vec<_>>();
    let sdk = crate::project::manifest::sdk_path(None)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|e| e.to_string());
    let host = tools::host_target().unwrap_or_else(|e| e.to_string());
    let detected_tools = std::collections::BTreeMap::from([
        ("cargo", tools::find("cargo")),
        ("rustc", tools::find("rustc")),
        ("python", tools::python()),
        ("android_sdk", tools::sdk_home()),
        ("ndk_compiler", tools::ndk_compiler()),
        ("java", tools::java()),
        ("gradle", tools::gradle()),
        ("adb", tools::android_tool("adb")),
    ]);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({"host":host,"sdk":sdk,"rust_targets":tools::installed_targets(),"tools":detected_tools,"platforms":statuses})
            )?
        )
    } else {
        println!("Omen development environment\nHost: {host}\nSDK: {sdk}\n");
        for (name, path) in &detected_tools {
            println!(
                "{name}: {}",
                path.as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "not detected".into())
            );
        }
        println!();
        for status in statuses {
            println!("{}", status.label());
        }
        println!(
            "\nAvailability reports tools only. iOS launch remains unverified; no tools are installed automatically."
        );
    }
    Ok(())
}

pub fn selection_cancelled(error: &anyhow::Error) -> bool {
    error.is::<selection::Cancelled>()
}
