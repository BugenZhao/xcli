use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::destination::Destination;
use crate::util::{parse_cli_json, run_cmd};
use crate::workspace::{Workspace, WorkspaceType};

// ---------------------------------------------------------------------------
// Build settings
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildSettingsEntry {
    pub build_settings: serde_json::Map<String, serde_json::Value>,
}

/// Get build settings for the given scheme/configuration/sdk.
pub fn get_build_settings(
    ws: &Workspace,
    scheme: &str,
    configuration: &str,
    destination_raw: Option<&str>,
    derived_data: Option<&str>,
) -> Result<Vec<BuildSettingsEntry>> {
    let mut cmd = Command::new("xcodebuild");
    cmd.args([
        "-showBuildSettings",
        "-scheme",
        scheme,
        "-configuration",
        configuration,
    ]);
    if let Some(dest) = destination_raw {
        cmd.args(["-destination", dest]);
    }
    if let Some(dd) = derived_data {
        cmd.args(["-derivedDataPath", dd]);
    }
    if ws.ws_type == WorkspaceType::Xcode {
        cmd.arg("-workspace").arg(&ws.path);
    } else {
        cmd.current_dir(ws.working_dir());
    }
    cmd.arg("-json");

    let output = run_cmd(&mut cmd)?;
    let entries: Vec<BuildSettingsEntry> = parse_cli_json(&output)?;
    Ok(entries)
}

/// Extract a string build setting value.
fn setting(entry: &BuildSettingsEntry, key: &str) -> Option<String> {
    entry
        .build_settings
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Resolved paths and identifiers needed for launching.
#[derive(Debug)]
pub struct LaunchInfo {
    pub app_path: PathBuf,
    pub executable_path: Option<PathBuf>,
    pub bundle_id: String,
    /// The platform name from build settings (e.g. "macosx", "iphoneos").
    pub platform_name: Option<String>,
}

/// Get the launch info from build settings (picks the first entry).
pub fn get_launch_info(
    ws: &Workspace,
    scheme: &str,
    configuration: &str,
    dest: &Destination,
    derived_data: Option<&str>,
) -> Result<LaunchInfo> {
    let dest_str = dest.xcodebuild_destination_string(false);
    let entries = get_build_settings(
        ws,
        scheme,
        configuration,
        Some(&dest_str),
        derived_data,
    )?;
    let entry = entries
        .first()
        .context("no build settings returned by xcodebuild")?;

    let target_build_dir = setting(entry, "TARGET_BUILD_DIR")
        .context("TARGET_BUILD_DIR not found in build settings")?;
    let bundle_id = setting(entry, "PRODUCT_BUNDLE_IDENTIFIER")
        .context("PRODUCT_BUNDLE_IDENTIFIER not found in build settings")?;

    // App path: TARGET_BUILD_DIR / (WRAPPER_NAME or FULL_PRODUCT_NAME or PRODUCT_NAME.app)
    let app_name = setting(entry, "WRAPPER_NAME")
        .or_else(|| setting(entry, "FULL_PRODUCT_NAME"))
        .or_else(|| setting(entry, "PRODUCT_NAME").map(|n| format!("{n}.app")))
        .context("cannot determine app name from build settings")?;
    let app_path = PathBuf::from(&target_build_dir).join(&app_name);

    // Executable path (for macOS): TARGET_BUILD_DIR / EXECUTABLE_PATH
    let executable_path =
        setting(entry, "EXECUTABLE_PATH").map(|ep| PathBuf::from(&target_build_dir).join(ep));

    let platform_name = setting(entry, "PLATFORM_NAME");

    Ok(LaunchInfo {
        app_path,
        executable_path,
        bundle_id,
        platform_name,
    })
}

// ---------------------------------------------------------------------------
// Build execution
// ---------------------------------------------------------------------------

pub struct BuildOptions<'a> {
    pub ws: &'a Workspace,
    pub scheme: &'a str,
    pub configuration: &'a str,
    pub destination_raw: &'a str,
    pub derived_data: Option<&'a str>,
    pub allow_provisioning_updates: bool,
    pub skip_codesigning: bool,
    pub xcbeautify: Option<bool>,
    pub extra_args: &'a [String],
    pub extra_env: &'a [(String, String)],
}

/// Run `xcodebuild build` with the given options.
pub fn build(opts: &BuildOptions) -> Result<()> {
    let mut args: Vec<String> = Vec::new();

    // Build settings from extra_args (KEY=VALUE).
    for arg in opts.extra_args {
        if arg.contains('=') && !arg.starts_with('-') {
            args.push(arg.clone());
        }
    }

    args.extend([
        "-scheme".into(),
        opts.scheme.into(),
        "-configuration".into(),
        opts.configuration.into(),
        "-destination".into(),
        opts.destination_raw.into(),
    ]);

    if let Some(dd) = opts.derived_data {
        args.extend(["-derivedDataPath".into(), dd.into()]);
    }
    if opts.allow_provisioning_updates {
        args.push("-allowProvisioningUpdates".into());
    }
    if opts.skip_codesigning {
        args.extend([
            "CODE_SIGN_IDENTITY=".into(),
            "CODE_SIGNING_REQUIRED=NO".into(),
            "CODE_SIGNING_ALLOWED=NO".into(),
        ]);
    }
    if opts.ws.ws_type == WorkspaceType::Xcode {
        args.extend(["-workspace".into(), opts.ws.path.display().to_string()]);
    }

    args.push("build".into());

    // Non-build-setting extra args (flags).
    for arg in opts.extra_args {
        if (!arg.contains('=') || arg.starts_with('-')) && arg != "build" {
            args.push(arg.clone());
        }
    }

    if opts.xcbeautify.unwrap_or_else(which_xcbeautify) {
        // Pipe through xcbeautify.
        run_piped_xcodebuild(&args, opts)?;
    } else {
        run_plain_xcodebuild(&args, opts)?;
    }

    Ok(())
}

fn run_plain_xcodebuild(args: &[String], opts: &BuildOptions) -> Result<()> {
    let mut cmd = Command::new("xcodebuild");
    cmd.args(args);
    for (k, v) in opts.extra_env {
        cmd.env(k, v);
    }
    if opts.ws.ws_type == WorkspaceType::Spm {
        cmd.current_dir(opts.ws.working_dir());
    }
    crate::util::run_cmd_inherit(&mut cmd).context("xcodebuild build failed")
}

fn run_piped_xcodebuild(args: &[String], opts: &BuildOptions) -> Result<()> {
    use std::process::Stdio;

    let mut cmd = Command::new("xcodebuild");
    cmd.args(args).stdout(Stdio::piped());
    for (k, v) in opts.extra_env {
        cmd.env(k, v);
    }
    if opts.ws.ws_type == WorkspaceType::Spm {
        cmd.current_dir(opts.ws.working_dir());
    }

    let mut child = cmd.spawn().context("failed to spawn xcodebuild")?;
    let stdout = child.stdout.take().unwrap();

    let beautify = Command::new("xcbeautify")
        .stdin(stdout)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to spawn xcbeautify")?;

    let build_status = child.wait()?;
    let _ = beautify.wait_with_output();

    if !build_status.success() {
        bail!("xcodebuild build failed ({})", build_status);
    }
    Ok(())
}

fn which_xcbeautify() -> bool {
    Command::new("which")
        .arg("xcbeautify")
        .output()
        .is_ok_and(|o| o.status.success())
}
