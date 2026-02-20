mod build;
mod cache;
mod cmd;
mod destination;
mod launch;
mod scheme;
mod util;
mod workspace;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "sweetpad", about = "CLI for building & running Xcode projects")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List available workspaces (.xcworkspace / Package.swift)
    Workspaces,

    /// List schemes for a workspace
    Schemes {
        /// Path to .xcworkspace or Package.swift
        #[arg(long)]
        workspace: Option<PathBuf>,
    },

    /// List build configurations for a workspace
    Configs {
        /// Path to .xcworkspace or Package.swift
        #[arg(long)]
        workspace: Option<PathBuf>,
    },

    /// List available destinations (simulators, devices, macOS)
    Destinations,

    /// Interactively select and cache workspace, scheme, configuration, and destination
    Configure(cmd::build::ResolveArgs),

    /// Build the project without launching
    Build(cmd::build::BuildArgs),

    /// Build and run (equivalent to SweetPad's Build & Run Launch)
    Launch(cmd::launch::LaunchArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Workspaces => cmd::cmd_workspaces(),
        Commands::Schemes { workspace } => cmd::cmd_schemes(workspace),
        Commands::Configs { workspace } => cmd::cmd_configs(workspace),
        Commands::Destinations => cmd::cmd_destinations(),
        Commands::Configure(args) => cmd::cmd_configure(args),
        Commands::Build(args) => cmd::cmd_build(args),
        Commands::Launch(args) => cmd::cmd_launch(args),
    }
}
