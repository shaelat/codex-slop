use crate::parser::parse_traceroute_n_with_target;
use anyhow::{anyhow, Context, Result};
use std::process::Command;
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

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

pub trait TracerouteRunner {
    fn run(&self, target: &str, settings: &TraceSettings) -> Result<String>;
}

#[derive(Debug, Clone)]
pub struct SystemTracerouteRunner;

impl TracerouteRunner for SystemTracerouteRunner {
    fn run(&self, target: &str, settings: &TraceSettings) -> Result<String> {
        run_traceroute(target, settings)
    }
}

#[derive(Debug, Clone)]
pub struct TraceJobResult {
    pub target: String,
    pub repeat: u32,
    pub result: Result<crate::parser::ParsedTraceRun, String>,
}

pub fn run_traces(
    targets: &[String],
    settings: &TraceSettings,
    repeat: u32,
    interval_ms: u64,
    concurrency: usize,
) -> Vec<TraceJobResult> {
    run_traces_with_runner(
        targets,
        settings,
        repeat,
        interval_ms,
        concurrency,
        Arc::new(SystemTracerouteRunner),
    )
}

pub fn run_traces_with_runner<R: TracerouteRunner + Send + Sync + 'static>(
    targets: &[String],
    settings: &TraceSettings,
    repeat: u32,
    interval_ms: u64,
    concurrency: usize,
    runner: Arc<R>,
) -> Vec<TraceJobResult> {
    if targets.is_empty() || repeat == 0 {
        return Vec::new();
    }

    let total_jobs = targets.len() * repeat as usize;
    let (tx, rx) = mpsc::channel();
    let semaphore = Arc::new(Semaphore::new(concurrency.max(1)));

    let mut handles = Vec::new();
    for (target_index, target) in targets.iter().cloned().enumerate() {
        let tx = tx.clone();
        let settings = settings.clone();
        let runner = Arc::clone(&runner);
        let semaphore = Arc::clone(&semaphore);
        let target_clone = target.clone();
        let handle = thread::spawn(move || {
            let base_index = target_index * repeat as usize;
            for rep in 0..repeat {
                let raw = {
                    let _permit = semaphore.acquire();
                    runner.run(&target_clone, &settings)
                };

                let result = match raw {
                    Ok(output) => match parse_traceroute_n_with_target(&output, &target_clone) {
                        Ok(parsed) => Ok(parsed),
                        Err(err) => Err(format_parse_error(
                            &target_clone,
                            rep,
                            &err.to_string(),
                            &output,
                        )),
                    },
                    Err(err) => Err(format_run_error(&target_clone, rep, &err.to_string())),
                };

                let job = TraceJobResult {
                    target: target_clone.clone(),
                    repeat: rep,
                    result,
                };
                let _ = tx.send((base_index + rep as usize, job));

                if interval_ms > 0 && rep + 1 < repeat {
                    thread::sleep(Duration::from_millis(interval_ms));
                }
            }
        });
        handles.push(handle);
    }

    drop(tx);

    let mut results: Vec<Option<TraceJobResult>> = vec![None; total_jobs];
    for _ in 0..total_jobs {
        if let Ok((idx, job)) = rx.recv() {
            results[idx] = Some(job);
        }
    }

    for handle in handles {
        let _ = handle.join();
    }

    results.into_iter().filter_map(|job| job).collect()
}

fn format_run_error(target: &str, repeat: u32, message: &str) -> String {
    format!("traceroute failed for {target} (repeat {repeat}): {message}")
}

fn format_parse_error(target: &str, repeat: u32, message: &str, output: &str) -> String {
    let snippet = output.lines().take(3).collect::<Vec<_>>().join(" | ");
    if snippet.is_empty() {
        format!("parse failed for {target} (repeat {repeat}): {message}")
    } else {
        format!("parse failed for {target} (repeat {repeat}): {message} (output: {snippet})")
    }
}

struct Semaphore {
    max: usize,
    state: Mutex<usize>,
    cvar: Condvar,
}

impl Semaphore {
    fn new(max: usize) -> Self {
        Self {
            max,
            state: Mutex::new(0),
            cvar: Condvar::new(),
        }
    }

    fn acquire(&self) -> Permit<'_> {
        let mut guard = self.state.lock().unwrap();
        while *guard >= self.max {
            guard = self.cvar.wait(guard).unwrap();
        }
        *guard += 1;
        Permit { semaphore: self }
    }

    fn release(&self) {
        let mut guard = self.state.lock().unwrap();
        if *guard > 0 {
            *guard -= 1;
        }
        self.cvar.notify_one();
    }
}

struct Permit<'a> {
    semaphore: &'a Semaphore,
}

impl<'a> Drop for Permit<'a> {
    fn drop(&mut self) {
        self.semaphore.release();
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
