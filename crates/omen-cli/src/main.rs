use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "omen", version, about = "Omen UI framework CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Verify that the Omen CLI is available.
    Doctor,
}

fn main() {
    match Cli::parse().command {
        Commands::Doctor => {
            println!("Omen development environment");
            println!("CLI is working.");
        }
    }
}
