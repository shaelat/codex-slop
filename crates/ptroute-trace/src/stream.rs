use crate::parser::parse_hop_line;
use anyhow::{anyhow, Result};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Sender};
use std::thread;

#[derive(Debug, Clone)]
pub enum TraceEvent {
    HopUpdate {
        ttl: u32,
        ip: Option<String>,
        rtts: Vec<Option<f64>>,
    },
    Done {
        status: i32,
    },
    Error {
        message: String,
    },
}

pub fn spawn_traceroute_stream(
    target: &str,
    settings: &crate::runner::TraceSettings,
    sender: Sender<TraceEvent>,
) -> Result<()> {
    let timeout_secs = ((settings.timeout_ms + 999) / 1000).max(1);

    let mut child = Command::new("traceroute")
        .arg("-n")
        .arg("-q")
        .arg(settings.probes.to_string())
        .arg("-m")
        .arg(settings.max_hops.to_string())
        .arg("-w")
        .arg(timeout_secs.to_string())
        .arg(target)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| anyhow!("failed to spawn traceroute for {target}: {err}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("missing traceroute stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("missing traceroute stderr"))?;

    let tx_out = sender.clone();
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().flatten() {
            if let Ok(hop) = parse_hop_line(&line) {
                let _ = tx_out.send(TraceEvent::HopUpdate {
                    ttl: hop.ttl,
                    ip: hop.ip,
                    rtts: hop.rtt_ms,
                });
            }
        }
    });

    let tx_err = sender.clone();
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        let mut buf = String::new();
        for line in reader.lines().flatten() {
            if !buf.is_empty() {
                buf.push(' ');
            }
            buf.push_str(&line);
        }
        if !buf.is_empty() {
            let _ = tx_err.send(TraceEvent::Error { message: buf });
        }
    });

    let tx_done = sender.clone();
    thread::spawn(move || {
        let status = child.wait().ok();
        let code = status.and_then(|s| s.code()).unwrap_or(-1);
        let _ = tx_done.send(TraceEvent::Done { status: code });
    });

    Ok(())
}

pub fn stream_for_target(
    target: &str,
    settings: &crate::runner::TraceSettings,
) -> Result<mpsc::Receiver<TraceEvent>> {
    let (tx, rx) = mpsc::channel();
    spawn_traceroute_stream(target, settings, tx)?;
    Ok(rx)
}
