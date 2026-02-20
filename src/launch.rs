use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::build::LaunchInfo;
use crate::destination::Destination;
use crate::util::run_cmd;

pub struct LaunchOptions<'a> {
    pub dest: &'a Destination,
    pub info: &'a LaunchInfo,
    pub args: &'a [String],
    pub env: &'a [(String, String)],
    pub foreground_simulator: bool,
}

/// Launch the built app on the resolved destination.
pub fn launch(opts: &LaunchOptions) -> Result<()> {
    match opts.dest {
        Destination::MacOS { .. } => launch_macos(opts),
        Destination::Simulator { udid, .. } => launch_simulator(udid, opts),
        Destination::Device { identifier, .. } => launch_device(identifier, opts),
    }
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------

fn launch_macos(opts: &LaunchOptions) -> Result<()> {
    let exec = opts
        .info
        .executable_path
        .as_ref()
        .context("executable path not found in build settings")?;

    if !exec.exists() {
        bail!("executable not found: {}", exec.display());
    }

    eprintln!("Running: {}", exec.display());
    let mut cmd = Command::new(exec);
    cmd.args(opts.args);
    for (k, v) in opts.env {
        cmd.env(k, v);
    }
    crate::util::run_cmd_inherit(&mut cmd).context("macOS app execution failed")
}

// ---------------------------------------------------------------------------
// Simulator
// ---------------------------------------------------------------------------

fn launch_simulator(udid: &str, opts: &LaunchOptions) -> Result<()> {
    // 1. Boot simulator (ignore error if already booted).
    let _ = run_cmd(Command::new("xcrun").args(["simctl", "boot", udid]));

    // 2. Open Simulator.app.
    if opts.foreground_simulator {
        let _ = Command::new("open").args(["-a", "Simulator"]).status();
    } else {
        let _ = Command::new("open")
            .args(["-g", "-a", "Simulator"])
            .status();
    }

    // 3. Install app.
    let app_path = opts.info.app_path.display().to_string();
    eprintln!("Installing on simulator {udid}...");
    run_cmd(Command::new("xcrun").args(["simctl", "install", udid, &app_path]))?;

    // 4. Launch app.
    eprintln!("Launching {}...", opts.info.bundle_id);
    let mut cmd = Command::new("xcrun");
    cmd.args([
        "simctl",
        "launch",
        "--console-pty",
        "--terminate-running-process",
        udid,
        &opts.info.bundle_id,
    ]);
    cmd.args(opts.args);

    // Environment: prefix with SIMCTL_CHILD_.
    for (k, v) in opts.env {
        cmd.env(format!("SIMCTL_CHILD_{k}"), v);
    }

    crate::util::run_cmd_inherit(&mut cmd).context("simctl launch failed")
}

// ---------------------------------------------------------------------------
// Physical device
// ---------------------------------------------------------------------------

fn launch_device(udid: &str, opts: &LaunchOptions) -> Result<()> {
    // 1. Install app.
    let app_path = opts.info.app_path.display().to_string();
    eprintln!("Installing on device {udid}...");
    run_cmd(Command::new("xcrun").args([
        "devicectl",
        "device",
        "install",
        "app",
        "--device",
        udid,
        &app_path,
    ]))?;

    // 2. Determine if --console is supported (Xcode 16+).
    let use_console = xcode_major_version() >= Some(16);

    // 3. Launch app.
    let tmp = tempfile::NamedTempFile::new()?;
    let json_path = tmp.path().to_string_lossy().to_string();

    eprintln!("Launching {}...", opts.info.bundle_id);
    let mut cmd = Command::new("xcrun");
    cmd.args(["devicectl", "device", "process", "launch"]);
    if use_console {
        cmd.arg("--console");
    }
    cmd.args([
        "--json-output",
        &json_path,
        "--terminate-existing",
        "--device",
        udid,
        &opts.info.bundle_id,
    ]);
    cmd.args(opts.args);

    // Environment: prefix with DEVICECTL_CHILD_.
    for (k, v) in opts.env {
        cmd.env(format!("DEVICECTL_CHILD_{k}"), v);
    }

    crate::util::run_cmd_inherit(&mut cmd).context("devicectl launch failed")?;

    // 4. Read JSON output for PID.
    if let Ok(json_str) = std::fs::read_to_string(tmp.path()) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json_str) {
            if let Some(pid) = v
                .pointer("/result/process/processIdentifier")
                .and_then(|p| p.as_u64())
            {
                eprintln!("App launched with PID: {pid}");
            }
        }
    }

    Ok(())
}

fn xcode_major_version() -> Option<u32> {
    let output = run_cmd(Command::new("xcrun").args(["xcodebuild", "-version"])).ok()?;
    // First line: "Xcode 16.0" or similar.
    let first_line = output.lines().next()?;
    let version_str = first_line.strip_prefix("Xcode ")?;
    let major = version_str.split('.').next()?;
    major.parse().ok()
}
