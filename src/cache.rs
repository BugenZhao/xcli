use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::destination::Destination;

const CACHE_DIR: &str = ".xcli";
const CACHE_FILE: &str = "state.toml";

/// Persisted state from the last `launch` invocation.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CachedState {
    pub workspace: Option<String>,
    pub scheme: Option<String>,
    pub configuration: Option<String>,
    pub destination: Option<Destination>,
}

impl CachedState {
    /// Load cached state from `.xcli/state.toml` relative to `root`.
    pub fn load(root: &Path) -> Self {
        let path = root.join(CACHE_DIR).join(CACHE_FILE);
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Save cached state to `.xcli/state.toml` relative to `root`.
    pub fn save(&self, root: &Path) -> Result<()> {
        let dir = root.join(CACHE_DIR);
        std::fs::create_dir_all(&dir)?;
        let content = toml::to_string_pretty(self)?;
        std::fs::write(dir.join(CACHE_FILE), content)?;
        Ok(())
    }

    /// Root directory for the cache (current working directory).
    pub fn root() -> Result<PathBuf> {
        Ok(std::env::current_dir()?)
    }

    /// Remove the cache file. Returns `Ok(true)` if the file was removed,
    /// `Ok(false)` if it didn't exist.
    pub fn reset(root: &Path) -> Result<bool> {
        let path = root.join(CACHE_DIR).join(CACHE_FILE);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
}
