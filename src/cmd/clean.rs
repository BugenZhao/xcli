use anyhow::Result;
use clap::Parser;

use crate::build;

use super::build::{ResolveArgs, resolve_and_cache};

#[derive(Parser)]
pub struct CleanArgs {
    /// Ignore cached selections and re-prompt for all options (selections are still saved)
    #[arg(long)]
    pub configure: bool,

    #[command(flatten)]
    pub resolve: ResolveArgs,

    /// Path to derived data
    #[arg(long)]
    pub derived_data: Option<String>,

    /// Pipe output through xcbeautify (auto-detected from PATH if not specified)
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub xcbeautify: Option<bool>,
}

pub fn cmd_clean(args: CleanArgs) -> Result<()> {
    let resolved = resolve_and_cache(&args.resolve, args.configure)?;

    let dest_raw = resolved.dest.xcodebuild_destination_string(false);

    build::clean(
        &resolved.ws,
        &resolved.scheme_name,
        &resolved.config,
        &dest_raw,
        args.derived_data.as_deref(),
        args.xcbeautify,
    )?;

    Ok(())
}
