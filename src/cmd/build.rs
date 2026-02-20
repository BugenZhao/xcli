use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use crate::destination::Destination;
use crate::workspace::Workspace;
use crate::{build, cache, destination, scheme, workspace};

#[derive(Parser)]
pub struct BuildArgs {
    /// Ignore cached selections and re-prompt for all options (selections are still saved)
    #[arg(long)]
    pub configure: bool,

    /// Path to .xcworkspace or Package.swift; if omitted, uses cached value or prompts for selection
    #[arg(long)]
    pub workspace: Option<PathBuf>,

    /// Scheme name; if omitted, uses cached value or prompts for selection
    #[arg(long)]
    pub scheme: Option<String>,

    /// Build configuration (default: Debug); if omitted, uses cached value or prompts for selection
    #[arg(long)]
    pub configuration: Option<String>,

    /// Destination spec: "simulator:<udid>", "device:<udid>", or "macos"; if omitted, uses cached value or prompts for selection
    #[arg(long)]
    pub destination: Option<String>,

    /// Path to derived data
    #[arg(long)]
    pub derived_data: Option<String>,

    /// Allow provisioning updates (default: true)
    #[arg(long, default_value_t = true)]
    pub allow_provisioning_updates: bool,

    /// Pipe build output through xcbeautify (auto-detected from PATH if not specified)
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub xcbeautify: Option<bool>,

    /// Use Rosetta destination for simulator (arch=x86_64)
    #[arg(long)]
    pub rosetta_destination: bool,

    /// Extra build arguments (repeatable)
    #[arg(long = "build-arg")]
    pub build_args: Vec<String>,

    /// Skip code signing (sets CODE_SIGN_IDENTITY='', CODE_SIGNING_REQUIRED=NO, CODE_SIGNING_ALLOWED=NO)
    #[arg(long)]
    pub skip_codesigning: bool,

    /// Extra build environment KEY=VALUE (repeatable)
    #[arg(long = "build-env", value_parser = parse_key_val)]
    pub build_env: Vec<(String, String)>,
}

fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=VALUE: no `=` found in `{s}`"))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

pub struct ResolvedBuild {
    pub ws: Workspace,
    pub scheme_name: String,
    pub config: String,
    pub dest: Destination,
}

/// Resolve inputs, build, and return the resolved state.
pub fn resolve_and_build(args: &BuildArgs) -> Result<ResolvedBuild> {
    let cache_root = cache::CachedState::root()?;
    let mut state = if args.configure {
        cache::CachedState::default()
    } else {
        cache::CachedState::load(&cache_root)
    };

    // 1. Resolve inputs, falling back to cached values.
    let cached_ws_path = state.workspace.as_ref().map(|p| cache_root.join(p));
    let ws_explicit = args.workspace.as_deref().or(cached_ws_path.as_deref());
    let ws = workspace::resolve_workspace(ws_explicit)?;

    let scheme_explicit = args.scheme.as_deref().or(state.scheme.as_deref());
    let scheme_name = scheme::resolve_scheme(&ws, scheme_explicit)?;

    let config_explicit = args
        .configuration
        .as_deref()
        .or(state.configuration.as_deref());
    let config = scheme::resolve_configuration(&ws, config_explicit)?;

    let dest_explicit = args.destination.as_deref();
    let dest = if let Some(spec) = dest_explicit {
        destination::resolve_destination(Some(spec))?
    } else if let Some(cached) = state.destination.clone() {
        cached
    } else {
        destination::resolve_destination(None)?
    };

    let dest_raw = dest.xcodebuild_destination_string(args.rosetta_destination);

    eprintln!("Workspace:     {}", ws.path.display());
    eprintln!("Scheme:        {scheme_name}");
    eprintln!("Configuration: {config}");
    eprintln!("Destination:   {dest}");
    eprintln!();

    // Save resolved values to cache.
    state.workspace = Some(
        ws.path
            .strip_prefix(&cache_root)
            .unwrap_or(&ws.path)
            .display()
            .to_string(),
    );
    state.scheme = Some(scheme_name.clone());
    state.configuration = Some(config.clone());
    state.destination = Some(dest.clone());
    if let Err(e) = state.save(&cache_root) {
        eprintln!("Warning: failed to save cache: {e}");
    }

    // 2. Build.
    let build_opts = build::BuildOptions {
        ws: &ws,
        scheme: &scheme_name,
        configuration: &config,
        destination_raw: &dest_raw,
        derived_data: args.derived_data.as_deref(),
        allow_provisioning_updates: args.allow_provisioning_updates,
        skip_codesigning: args.skip_codesigning,
        xcbeautify: args.xcbeautify,
        extra_args: &args.build_args,
        extra_env: &args.build_env,
    };
    build::build(&build_opts)?;

    Ok(ResolvedBuild {
        ws,
        scheme_name,
        config,
        dest,
    })
}

pub fn cmd_build(args: BuildArgs) -> Result<()> {
    resolve_and_build(&args)?;
    Ok(())
}
