use omen_cli::{commands, project, toolchain};

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "omen",
    version,
    about = "Create, build and run native Omen applications"
)]
struct Cli {
    /// Project directory; searches parent directories for Omen.toml.
    #[arg(long, global = true, default_value = ".")]
    project: PathBuf,
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    /// Create a project using the local Omen SDK.
    New {
        name: String,
        #[arg(long)]
        sdk: Option<PathBuf>,
    },
    /// Select a platform, build the application and launch it.
    Run(commands::RunArgs),
    /// Build and package without launching.
    Build(commands::BuildArgs),
    /// Generate app icons from Omen.toml without building the app.
    #[command(aliases = ["icon", "icons"])]
    GenerateIcon {
        #[arg(short = 'p', long, value_enum)]
        platform: Option<commands::Platform>,
    },
    /// List connected devices and installed simulators.
    Devices {
        #[arg(long)]
        json: bool,
    },
    /// Inspect SDKs, tools and Rust targets without installing anything.
    Doctor {
        #[arg(long)]
        json: bool,
    },
}
fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    let bootstrap = if matches!(cli.command, Commands::GenerateIcon { .. }) {
        Ok(None)
    } else {
        toolchain::bootstrap(&cli.project)
    };
    report(bootstrap.and_then(|status| match status {
        Some(status) => Ok(status.code().unwrap_or(1) as u8),
        None => execute(cli).map(|()| 0),
    }))
}
fn report(result: anyhow::Result<u8>) -> std::process::ExitCode {
    match result {
        Ok(code) => std::process::ExitCode::from(code),
        Err(error) if commands::selection_cancelled(&error) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{} {error:#}", console::style("error:").red().bold());
            std::process::ExitCode::FAILURE
        }
    }
}
fn execute(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::New { name, sdk } => project::template::create(&name, sdk.as_deref()),
        Commands::Run(args) => commands::run(&cli.project, args),
        Commands::Build(args) => commands::build(&cli.project, args),
        Commands::GenerateIcon { platform } => commands::generate_icon(&cli.project, platform),
        Commands::Devices { json } => commands::devices(json),
        Commands::Doctor { json } => commands::doctor(json),
    }
}
