use anyhow::Result;

use crate::cache;

pub fn cmd_reset(profile: Option<String>) -> Result<()> {
    let root = cache::CachedState::root()?;
    if cache::CachedState::reset(&root, profile.as_deref())? {
        eprintln!("Cache cleared.");
    } else {
        eprintln!("No cache to clear.");
    }
    Ok(())
}
