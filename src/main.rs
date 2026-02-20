mod build;
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
    /// Detect available workspaces (.xcworkspace / Package.swift)
    Detect,

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

    /// Build and run (equivalent to SweetPad's Build & Run Launch)
    Launch(LaunchArgs),
}

#[derive(Parser)]
struct LaunchArgs {
    /// Path to .xcworkspace or Package.swift
    #[arg(long)]
    workspace: Option<PathBuf>,

    /// Scheme name
    #[arg(long)]
    scheme: Option<String>,

    /// Build configuration (default: Debug)
    #[arg(long)]
    configuration: Option<String>,

    /// Destination spec: "simulator:<udid>", "device:<udid>", or "macos"
    #[arg(long)]
    destination: Option<String>,

    /// Path to derived data
    #[arg(long)]
    derived_data: Option<String>,

    /// Allow provisioning updates (default: true)
    #[arg(long, default_value_t = true)]
    allow_provisioning_updates: bool,

    /// Pipe build output through xcbeautify
    #[arg(long)]
    xcbeautify: bool,

    /// Use Rosetta destination for simulator (arch=x86_64)
    #[arg(long)]
    rosetta_destination: bool,

    /// Bring simulator to foreground (default: true)
    #[arg(long, default_value_t = true)]
    foreground_simulator: bool,

    /// Extra build arguments (repeatable)
    #[arg(long = "build-arg")]
    build_args: Vec<String>,

    /// Extra build environment KEY=VALUE (repeatable)
    #[arg(long = "build-env", value_parser = parse_key_val)]
    build_env: Vec<(String, String)>,

    /// Launch arguments passed to the app (repeatable)
    #[arg(long = "arg")]
    launch_args: Vec<String>,

    /// Launch environment KEY=VALUE (repeatable)
    #[arg(long = "env", value_parser = parse_key_val)]
    launch_env: Vec<(String, String)>,
}

fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=VALUE: no `=` found in `{s}`"))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Detect => cmd_detect(),
        Commands::Schemes { workspace } => cmd_schemes(workspace),
        Commands::Configs { workspace } => cmd_configs(workspace),
        Commands::Destinations => cmd_destinations(),
        Commands::Launch(args) => cmd_launch(args),
    }
}

fn cmd_detect() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let workspaces = workspace::detect_workspaces(&cwd);
    if workspaces.is_empty() {
        eprintln!("No workspaces found.");
    }
    for ws in &workspaces {
        let tag = match ws.ws_type {
            workspace::WorkspaceType::Xcode => "xcode",
            workspace::WorkspaceType::Spm => "spm",
        };
        println!("[{tag}] {}", ws.path.display());
    }
    Ok(())
}

fn cmd_schemes(ws_path: Option<PathBuf>) -> Result<()> {
    let ws = workspace::resolve_workspace(ws_path.as_deref())?;
    let schemes = scheme::list_schemes(&ws)?;
    for s in &schemes {
        println!("{s}");
    }
    Ok(())
}

fn cmd_configs(ws_path: Option<PathBuf>) -> Result<()> {
    let ws = workspace::resolve_workspace(ws_path.as_deref())?;
    let configs = scheme::list_configurations(&ws)?;
    for c in &configs {
        println!("{c}");
    }
    Ok(())
}

fn cmd_destinations() -> Result<()> {
    let dests = destination::list_destinations()?;
    for d in &dests {
        println!("{d}");
    }
    Ok(())
}

fn cmd_launch(args: LaunchArgs) -> Result<()> {
    // 1. Resolve inputs.
    let ws = workspace::resolve_workspace(args.workspace.as_deref())?;
    let scheme_name = scheme::resolve_scheme(&ws, args.scheme.as_deref())?;
    let config = scheme::resolve_configuration(&ws, args.configuration.as_deref())?;
    let dest = destination::resolve_destination(args.destination.as_deref())?;

    let dest_raw = dest.xcodebuild_destination_string(args.rosetta_destination);

    eprintln!("Workspace:     {}", ws.path.display());
    eprintln!("Scheme:        {scheme_name}");
    eprintln!("Configuration: {config}");
    eprintln!("Destination:   {dest}");
    eprintln!();

    // 2. Build.
    let build_opts = build::BuildOptions {
        ws: &ws,
        scheme: &scheme_name,
        configuration: &config,
        destination_raw: &dest_raw,
        derived_data: args.derived_data.as_deref(),
        allow_provisioning_updates: args.allow_provisioning_updates,
        xcbeautify: args.xcbeautify,
        extra_args: &args.build_args,
        extra_env: &args.build_env,
    };
    build::build(&build_opts)?;

    // 3. Get launch info from build settings.
    let info = build::get_launch_info(
        &ws,
        &scheme_name,
        &config,
        &dest,
        args.derived_data.as_deref(),
    )?;

    eprintln!();
    eprintln!("App path:  {}", info.app_path.display());
    eprintln!("Bundle ID: {}", info.bundle_id);

    // 4. Launch.
    let launch_opts = launch::LaunchOptions {
        dest: &dest,
        info: &info,
        args: &args.launch_args,
        env: &args.launch_env,
        foreground_simulator: args.foreground_simulator,
    };
    launch::launch(&launch_opts)?;

    Ok(())
}
