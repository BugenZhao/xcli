use std::collections::HashMap;
use std::process::Command;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::util::{parse_cli_json, run_cmd};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Destination {
    Simulator {
        udid: String,
        name: String,
        os: String,
    },
    Device {
        /// Traditional UDID (for xcodebuild -destination).
        udid: String,
        /// CoreDevice identifier (for devicectl commands).
        identifier: String,
        name: String,
        device_type: String,
    },
    MacOS {
        arch: String,
    },
}

impl std::fmt::Display for Destination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Destination::Simulator { name, os, .. } => {
                write!(f, "[Simulator] {name} ({os})")
            }
            Destination::Device {
                name, device_type, ..
            } => write!(f, "[Device] {name} ({device_type})"),
            Destination::MacOS { arch } => {
                write!(f, "[macOS] My Mac ({arch})")
            }
        }
    }
}

impl Destination {
    /// Build the `-destination` string for xcodebuild.
    pub fn xcodebuild_destination_string(&self, rosetta: bool) -> String {
        match self {
            Destination::Simulator { udid, .. } => {
                let mut s = format!("platform=iOS Simulator,id={udid}");
                if rosetta {
                    s.push_str(",arch=x86_64");
                }
                s
            }
            Destination::Device { udid, .. } => {
                format!("platform=iOS,id={udid}")
            }
            Destination::MacOS { arch } => {
                format!("platform=macOS,arch={arch}")
            }
        }
    }

    /// SDK name for `-showBuildSettings -sdk`.
    pub fn sdk(&self) -> Option<&str> {
        match self {
            Destination::MacOS { .. } => Some("macosx"),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Listing
// ---------------------------------------------------------------------------

/// List all available destinations (simulators + devices + macOS).
pub fn list_destinations() -> Result<Vec<Destination>> {
    let mut dests = Vec::new();

    // Physical devices
    if let Ok(devs) = list_devices() {
        dests.extend(devs);
    }

    // Simulators
    if let Ok(sims) = list_simulators() {
        dests.extend(sims);
    }

    // macOS
    let arch = current_arch();
    dests.push(Destination::MacOS {
        arch: arch.to_string(),
    });

    Ok(dests)
}

fn current_arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86_64"
    }
}

// --- Simulators via `xcrun simctl list --json devices` ---

#[derive(Deserialize)]
struct SimctlOutput {
    devices: HashMap<String, Vec<SimDevice>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SimDevice {
    udid: String,
    name: String,
    state: Option<String>,
    #[serde(default)]
    is_available: bool,
}

fn list_simulators() -> Result<Vec<Destination>> {
    let output = run_cmd(Command::new("xcrun").args(["simctl", "list", "--json", "devices"]))?;
    let simctl: SimctlOutput = parse_cli_json(&output)?;

    let mut results = Vec::new();
    for (runtime, devices) in &simctl.devices {
        let os = runtime_to_os(runtime);
        for dev in devices {
            if !dev.is_available {
                continue;
            }
            let name = match &dev.state {
                Some(s) if s == "Booted" => format!("{} (Booted)", dev.name),
                _ => dev.name.clone(),
            };
            results.push(Destination::Simulator {
                udid: dev.udid.clone(),
                name,
                os: os.clone(),
            });
        }
    }
    Ok(results)
}

/// Extract a human-readable OS string from a runtime identifier.
/// e.g. "com.apple.CoreSimulator.SimRuntime.iOS-17-2" -> "iOS 17.2"
fn runtime_to_os(runtime: &str) -> String {
    let s = runtime
        .strip_prefix("com.apple.CoreSimulator.SimRuntime.")
        .unwrap_or(runtime);
    // s is like "iOS-17-2" or "watchOS-10-1"
    // Split on first '-' to separate OS name from version.
    if let Some(pos) = s.find('-') {
        let os_name = &s[..pos];
        let version = s[pos + 1..].replace('-', ".");
        format!("{os_name} {version}")
    } else {
        s.to_string()
    }
}

// --- Physical devices via `xcrun devicectl` ---

#[derive(Deserialize)]
struct DeviceCtlOutput {
    result: DeviceCtlResult,
}

#[derive(Deserialize)]
struct DeviceCtlResult {
    devices: Vec<DeviceCtlDevice>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceCtlDevice {
    identifier: String,
    device_properties: DeviceProperties,
    hardware_properties: HardwareProperties,
}

#[derive(Deserialize)]
struct DeviceProperties {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HardwareProperties {
    device_type: Option<String>,
    udid: Option<String>,
}

fn list_devices() -> Result<Vec<Destination>> {
    let tmp = tempfile::NamedTempFile::new()?;
    let tmp_path = tmp.path().to_path_buf();

    run_cmd(Command::new("xcrun").args([
        "devicectl",
        "list",
        "devices",
        "--json-output",
        &tmp_path.to_string_lossy(),
        "--timeout",
        "10",
    ]))?;

    let json_str = std::fs::read_to_string(&tmp_path)?;
    let output: DeviceCtlOutput = parse_cli_json(&json_str)?;

    let results = output
        .result
        .devices
        .into_iter()
        .map(|d| {
            let traditional_udid = d
                .hardware_properties
                .udid
                .unwrap_or_else(|| d.identifier.clone());
            Destination::Device {
                udid: traditional_udid,
                identifier: d.identifier,
                name: d.device_properties.name,
                device_type: d
                    .hardware_properties
                    .device_type
                    .unwrap_or_else(|| "Unknown".into()),
            }
        })
        .collect();
    Ok(results)
}

/// Resolve destination: use explicit spec, or prompt user.
pub fn resolve_destination(explicit: Option<&str>) -> Result<Destination> {
    if let Some(spec) = explicit {
        return parse_destination_spec(spec);
    }

    let dests = list_destinations()?;
    if dests.is_empty() {
        bail!("no destinations available");
    }

    let labels: Vec<String> = dests.iter().map(|d| d.to_string()).collect();
    let sel = dialoguer::Select::new()
        .with_prompt("Select destination")
        .items(&labels)
        .default(0)
        .interact()?;
    Ok(dests.into_iter().nth(sel).unwrap())
}

/// Parse a destination spec like "simulator:<udid>", "device:<udid>", or
/// "macos".
fn parse_destination_spec(spec: &str) -> Result<Destination> {
    if spec == "macos" {
        return Ok(Destination::MacOS {
            arch: current_arch().to_string(),
        });
    }
    if let Some(udid) = spec.strip_prefix("simulator:") {
        return Ok(Destination::Simulator {
            udid: udid.to_string(),
            name: String::new(),
            os: String::new(),
        });
    }
    if let Some(udid) = spec.strip_prefix("device:") {
        return Ok(Destination::Device {
            udid: udid.to_string(),
            identifier: udid.to_string(),
            name: String::new(),
            device_type: String::new(),
        });
    }
    bail!("invalid destination spec: {spec}\nExpected: simulator:<udid>, device:<udid>, or macos")
}
