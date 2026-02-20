use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use walkdir::WalkDir;

/// The type of Xcode workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceType {
    Xcode,
    Spm,
}

/// A detected workspace path with its type.
#[derive(Debug, Clone)]
pub struct Workspace {
    pub path: PathBuf,
    pub ws_type: WorkspaceType,
}

impl Workspace {
    pub fn new(path: PathBuf) -> Self {
        let ws_type = detect_type(&path);
        Self { path, ws_type }
    }

    /// For SPM projects, returns the directory containing Package.swift.
    /// For Xcode projects, returns the parent directory of the .xcworkspace.
    pub fn working_dir(&self) -> &Path {
        match self.ws_type {
            WorkspaceType::Spm => self.path.parent().unwrap_or(&self.path),
            WorkspaceType::Xcode => self.path.parent().unwrap_or(&self.path),
        }
    }
}

impl std::fmt::Display for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path.display())
    }
}

fn detect_type(path: &Path) -> WorkspaceType {
    if path.file_name().is_some_and(|n| n == "Package.swift") {
        WorkspaceType::Spm
    } else {
        WorkspaceType::Xcode
    }
}

/// Detect all workspace candidates under `root` (depth <= 4).
pub fn detect_workspaces(root: &Path) -> Vec<Workspace> {
    let mut results = Vec::new();
    for entry in WalkDir::new(root)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if name == "Package.swift" || (name.ends_with(".xcworkspace") && !path.starts_with(".")) {
            results.push(Workspace::new(path.to_path_buf()));
        }
    }
    results
}

/// Resolve workspace: use explicit path, or auto-detect from current dir.
pub fn resolve_workspace(explicit: Option<&Path>) -> Result<Workspace> {
    if let Some(p) = explicit {
        return Ok(Workspace::new(p.to_path_buf()));
    }

    let cwd = std::env::current_dir()?;
    let candidates = detect_workspaces(&cwd);

    match candidates.len() {
        0 => bail!("no .xcworkspace or Package.swift found (searched depth 4)"),
        1 => Ok(candidates.into_iter().next().unwrap()),
        _ => {
            let labels: Vec<String> = candidates.iter().map(|w| w.to_string()).collect();
            let sel = dialoguer::Select::new()
                .with_prompt("Multiple workspaces found, select one")
                .items(&labels)
                .default(0)
                .interact()?;
            Ok(candidates.into_iter().nth(sel).unwrap())
        }
    }
}
