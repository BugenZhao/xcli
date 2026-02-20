use anyhow::Result;

use super::build::{ResolveArgs, resolve_and_cache};

pub fn cmd_configure(args: ResolveArgs) -> Result<()> {
    resolve_and_cache(&args, true)?;
    Ok(())
}
