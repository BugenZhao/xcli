use std::process::Command;

use anyhow::{Context, Result, bail};

/// Run a command and return its stdout as a string. Fails if exit code != 0.
pub fn run_cmd(cmd: &mut Command) -> Result<String> {
    let output = cmd
        .output()
        .with_context(|| format!("failed to spawn: {:?}", cmd))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "command {:?} failed ({})\n{}",
            cmd,
            output.status,
            stderr.trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Run a command, inheriting stdio (for interactive output like xcodebuild).
pub fn run_cmd_inherit(cmd: &mut Command) -> Result<()> {
    let status = cmd
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .with_context(|| format!("failed to spawn: {:?}", cmd))?;
    if !status.success() {
        bail!("command {:?} failed ({})", cmd, status);
    }
    Ok(())
}

/// Parse JSON from CLI output that may contain non-JSON lines before the actual
/// JSON (warnings, etc). Tries direct parse first, then extracts the JSON
/// portion.
pub fn parse_cli_json<T: serde::de::DeserializeOwned>(output: &str) -> Result<T> {
    // Try direct parse first.
    if let Ok(v) = serde_json::from_str(output) {
        return Ok(v);
    }

    // Find the first '{' or '[' and the matching last '}' or ']'.
    let first_brace = output.find('{');
    let first_bracket = output.find('[');
    let (start, end) = match (first_brace, first_bracket) {
        (Some(b), Some(k)) if b < k => (b, output.rfind('}').unwrap_or(b)),
        (Some(_), Some(k)) => (k, output.rfind(']').unwrap_or(k)),
        (Some(b), None) => (b, output.rfind('}').unwrap_or(b)),
        (None, Some(k)) => (k, output.rfind(']').unwrap_or(k)),
        (None, None) => bail!("no JSON found in output"),
    };

    let json_str = &output[start..=end];
    serde_json::from_str(json_str)
        .with_context(|| format!("failed to parse extracted JSON:\n{json_str}"))
}
