use anyhow::{anyhow, Context, Result};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct TraceSettings {
    pub max_hops: u32,
    pub probes: u32,
    pub timeout_ms: u64,
}

impl Default for TraceSettings {
    fn default() -> Self {
        Self {
            max_hops: 30,
            probes: 3,
            timeout_ms: 2000,
        }
    }
}

pub fn run_traceroute(target: &str, settings: &TraceSettings) -> Result<String> {
    let timeout_secs = ((settings.timeout_ms + 999) / 1000).max(1);

    let output = Command::new("traceroute")
        .arg("-n")
        .arg("-q")
        .arg(settings.probes.to_string())
        .arg("-m")
        .arg(settings.max_hops.to_string())
        .arg("-w")
        .arg(timeout_secs.to_string())
        .arg(target)
        .output()
        .with_context(|| format!("failed to spawn traceroute for {target}"))?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "traceroute failed for {target} (status: {}): {}{}",
            output.status,
            stderr,
            stdout
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
