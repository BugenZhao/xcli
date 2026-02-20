use anyhow::Result;
use clap::Parser;

use crate::build;

use super::build::{XcodeActionArgs, resolve_and_cache};

#[derive(Parser)]
pub struct CleanArgs {
    #[command(flatten)]
    pub action: XcodeActionArgs,
}

pub fn cmd_clean(args: CleanArgs) -> Result<()> {
    let resolved = resolve_and_cache(&args.action.resolve, args.action.configure)?;

    let dest_raw = resolved.dest.xcodebuild_destination_string();

    build::clean(
        &resolved.ws,
        &resolved.scheme_name,
        &resolved.config,
        &dest_raw,
        args.action.derived_data.as_deref(),
        args.action.xcbeautify,
    )?;

    Ok(())
}
