use std::process::Command;

use anyhow::{Result, bail};
use serde::Deserialize;

use crate::util::{parse_cli_json, run_cmd};
use crate::workspace::{Workspace, WorkspaceType};

/// List available schemes for a workspace.
pub fn list_schemes(ws: &Workspace) -> Result<Vec<String>> {
    match ws.ws_type {
        WorkspaceType::Spm => list_schemes_spm(ws),
        WorkspaceType::Xcode => list_schemes_xcode(ws),
    }
}

// --- SPM ---

#[derive(Deserialize)]
struct SpmPackage {
    name: String,
    #[serde(default)]
    products: Vec<SpmProduct>,
    #[serde(default)]
    targets: Vec<SpmTarget>,
}

#[derive(Deserialize)]
struct SpmProduct {
    name: String,
    #[serde(rename = "type")]
    product_type: serde_json::Value,
}

#[derive(Deserialize)]
struct SpmTarget {
    name: String,
    #[serde(rename = "type")]
    target_type: Option<String>,
}

fn list_schemes_spm(ws: &Workspace) -> Result<Vec<String>> {
    let output = run_cmd(
        Command::new("swift")
            .args(["package", "dump-package"])
            .current_dir(ws.working_dir()),
    )?;

    let pkg: SpmPackage = parse_cli_json(&output)?;
    let mut schemes = Vec::new();

    for p in &pkg.products {
        // Include executables and libraries.
        if let serde_json::Value::Object(ref m) = p.product_type {
            if m.contains_key("executable") || m.contains_key("library") {
                schemes.push(p.name.clone());
            }
        }
    }
    for t in &pkg.targets {
        if t.target_type.as_deref() == Some("executable") && !schemes.contains(&t.name) {
            schemes.push(t.name.clone());
        }
    }
    if schemes.is_empty() {
        schemes.push(pkg.name);
    }
    Ok(schemes)
}

// --- Xcode ---

#[derive(Deserialize)]
struct XcodebuildList {
    workspace: Option<XcodebuildListInner>,
    project: Option<XcodebuildListInner>,
}

#[derive(Deserialize)]
struct XcodebuildListInner {
    schemes: Vec<String>,
}

fn list_schemes_xcode(ws: &Workspace) -> Result<Vec<String>> {
    let output = run_cmd(
        Command::new("xcodebuild")
            .args(["-list", "-json", "-workspace"])
            .arg(&ws.path),
    )?;

    let list: XcodebuildList = parse_cli_json(&output)?;
    let schemes = list
        .workspace
        .or(list.project)
        .map(|inner| inner.schemes)
        .unwrap_or_default();

    if schemes.is_empty() {
        bail!("no schemes found in workspace");
    }
    Ok(schemes)
}

/// List available build configurations for a workspace.
pub fn list_configurations(ws: &Workspace) -> Result<Vec<String>> {
    match ws.ws_type {
        WorkspaceType::Spm => Ok(vec!["Debug".into(), "Release".into()]),
        WorkspaceType::Xcode => {
            let output = run_cmd(
                Command::new("xcodebuild")
                    .args(["-list", "-json", "-workspace"])
                    .arg(&ws.path),
            )?;
            #[derive(Deserialize)]
            struct ListWithConfigs {
                workspace: Option<ConfigInner>,
                project: Option<ConfigInner>,
            }
            #[derive(Deserialize)]
            struct ConfigInner {
                configurations: Vec<String>,
            }
            let list: ListWithConfigs = parse_cli_json(&output)?;
            let configs = list
                .workspace
                .or(list.project)
                .map(|inner| inner.configurations)
                .unwrap_or_else(|| vec!["Debug".into(), "Release".into()]);
            Ok(configs)
        }
    }
}

/// Resolve scheme: use explicit name, or prompt user.
pub fn resolve_scheme(ws: &Workspace, explicit: Option<&str>) -> Result<String> {
    if let Some(s) = explicit {
        return Ok(s.to_string());
    }
    let schemes = list_schemes(ws)?;
    match schemes.len() {
        0 => bail!("no schemes found"),
        1 => Ok(schemes.into_iter().next().unwrap()),
        _ => {
            let sel = dialoguer::Select::new()
                .with_prompt("Select scheme")
                .items(&schemes)
                .default(0)
                .interact()?;
            Ok(schemes.into_iter().nth(sel).unwrap())
        }
    }
}

/// Resolve configuration: use explicit name, or default to Debug when
/// configs are exactly [Debug, Release].
pub fn resolve_configuration(
    ws: &Workspace,
    explicit: Option<&str>,
) -> Result<String> {
    if let Some(c) = explicit {
        return Ok(c.to_string());
    }
    let configs = list_configurations(ws)?;
    // Default to Debug when the standard pair is available.
    if configs.len() == 2
        && configs.contains(&"Debug".to_string())
        && configs.contains(&"Release".to_string())
    {
        return Ok("Debug".into());
    }
    match configs.len() {
        1 => Ok(configs.into_iter().next().unwrap()),
        _ => {
            let sel = dialoguer::Select::new()
                .with_prompt("Select configuration")
                .items(&configs)
                .default(0)
                .interact()?;
            Ok(configs.into_iter().nth(sel).unwrap())
        }
    }
}
