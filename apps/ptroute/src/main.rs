use anyhow::{anyhow, Result};
use chrono::{SecondsFormat, Utc};
use clap::{Args, Parser, Subcommand};
use ptroute_graph::{build_graph, layout_graph};
use ptroute_model::{SceneFile, TraceFile, TraceRun};
use ptroute_render::{render_scene, render_scene_progressive, write_png, RenderSettings};
use ptroute_trace::{parse_traceroute_n_with_target, run_traceroute, TraceSettings};
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Parser)]
#[command(name = "ptroute", version, about = "PathTraceRoute CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Trace(TraceArgs),
    Build(BuildArgs),
    Layout(LayoutArgs),
    Render(RenderArgs),
    Run(RunArgs),
}

#[derive(Args)]
#[command(
    about = "Run traceroute in numeric mode. Only target networks you own or have permission to test."
)]
struct TraceArgs {
    #[arg(long)]
    targets: Option<PathBuf>,

    #[arg(long = "target")]
    target_list: Vec<String>,

    #[arg(long)]
    out: PathBuf,

    #[arg(long, default_value_t = 30)]
    max_hops: u32,

    #[arg(long, default_value_t = 3)]
    probes: u32,

    #[arg(long, default_value_t = 2000)]
    timeout_ms: u64,

    #[arg(long, default_value_t = 4)]
    concurrency: usize,

    #[arg(long, default_value_t = 1)]
    repeat: u32,

    #[arg(long, default_value_t = 0)]
    interval_ms: u64,
}

#[derive(Args)]
struct BuildArgs {
    #[arg(long = "in")]
    in_path: PathBuf,

    #[arg(long)]
    out: PathBuf,
}

#[derive(Args)]
struct LayoutArgs {
    #[arg(long = "in")]
    in_path: PathBuf,

    #[arg(long)]
    out: PathBuf,

    #[arg(long, default_value_t = 1)]
    seed: u64,
}

#[derive(Args)]
struct RenderArgs {
    #[arg(long = "in")]
    in_path: PathBuf,

    #[arg(long)]
    out: PathBuf,

    #[arg(long, default_value_t = 1600)]
    width: u32,

    #[arg(long, default_value_t = 900)]
    height: u32,

    #[arg(long, default_value_t = 64)]
    spp: u32,

    #[arg(long, default_value_t = 6)]
    bounces: u32,

    #[arg(long, default_value_t = 1)]
    seed: u64,

    #[arg(long, default_value_t = 32)]
    progress_every: u32,

    #[arg(long, default_value_t = 0)]
    threads: usize,

    #[arg(long, default_value_t = 0)]
    progressive_every: u32,
}

#[derive(Args)]
struct RunArgs {
    #[arg(long)]
    targets: Option<PathBuf>,

    #[arg(long = "target")]
    target_list: Vec<String>,

    #[arg(long)]
    out_dir: Option<PathBuf>,

    #[arg(long, default_value_t = 1)]
    seed: u64,

    #[arg(long, default_value_t = 1600)]
    width: u32,

    #[arg(long, default_value_t = 900)]
    height: u32,

    #[arg(long, default_value_t = 64)]
    spp: u32,

    #[arg(long, default_value_t = 6)]
    bounces: u32,

    #[arg(long, default_value_t = 32)]
    progress_every: u32,

    #[arg(long, default_value_t = 0)]
    threads: usize,

    #[arg(long, default_value_t = 0)]
    progressive_every: u32,

    #[arg(long, default_value_t = 30)]
    max_hops: u32,

    #[arg(long, default_value_t = 3)]
    probes: u32,

    #[arg(long, default_value_t = 2000)]
    timeout_ms: u64,

    #[arg(long, default_value_t = 4)]
    concurrency: usize,

    #[arg(long, default_value_t = 1)]
    repeat: u32,

    #[arg(long, default_value_t = 0)]
    interval_ms: u64,

    #[arg(long)]
    resume: bool,

    #[arg(long)]
    force: bool,

    #[arg(long)]
    plain: bool,

    #[arg(long)]
    open: bool,
}

#[derive(Serialize)]
struct RunArgsSummary {
    targets_file: Option<PathBuf>,
    targets: Vec<String>,
    out_dir: PathBuf,
    seed: u64,
    width: u32,
    height: u32,
    spp: u32,
    bounces: u32,
    progress_every: u32,
    threads: usize,
    progressive_every: u32,
    max_hops: u32,
    probes: u32,
    timeout_ms: u64,
    concurrency: usize,
    repeat: u32,
    interval_ms: u64,
    resume: bool,
    force: bool,
    plain: bool,
    open: bool,
}

#[derive(Serialize)]
struct RunOutputs {
    traces: PathBuf,
    graph: PathBuf,
    scene: PathBuf,
    render: PathBuf,
    run: PathBuf,
}

#[derive(Serialize)]
struct HostInfo {
    os: String,
    arch: String,
}

#[derive(Serialize)]
struct RunReceipt {
    version: String,
    started_at_utc: String,
    finished_at_utc: String,
    args: RunArgsSummary,
    outputs: RunOutputs,
    host: HostInfo,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Trace(args) => run_trace(args),
        Commands::Build(args) => run_build(args),
        Commands::Layout(args) => run_layout(args),
        Commands::Render(args) => run_render(args),
        Commands::Run(args) => run_run(args),
    }
}

fn run_trace(args: TraceArgs) -> Result<()> {
    let mut targets: Vec<String> = Vec::new();

    if let Some(path) = args.targets {
        let contents = fs::read_to_string(&path)
            .map_err(|err| anyhow!("failed to read targets file {:?}: {}", path, err))?;
        for line in contents.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            targets.push(trimmed.to_string());
        }
    }

    targets.extend(args.target_list);

    if targets.is_empty() {
        return Err(anyhow!("no targets provided (use --targets or --target)"));
    }

    if args.concurrency > 1 {
        eprintln!("warning: --concurrency is not implemented yet; running sequentially");
    }

    let settings = TraceSettings {
        max_hops: args.max_hops,
        probes: args.probes,
        timeout_ms: args.timeout_ms,
    };

    let mut runs: Vec<TraceRun> = Vec::new();

    for target in targets {
        for rep in 0..args.repeat {
            match run_traceroute(&target, &settings) {
                Ok(raw) => match parse_traceroute_n_with_target(&raw, &target) {
                    Ok(parsed) => {
                        let timestamp_utc = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
                        runs.push(TraceRun {
                            target: parsed.target,
                            timestamp_utc,
                            hops: parsed.hops,
                        });
                    }
                    Err(err) => {
                        eprintln!("failed to parse traceroute for {target}: {err}");
                    }
                },
                Err(err) => {
                    eprintln!("traceroute failed for {target}: {err}");
                }
            }

            if args.interval_ms > 0 && rep + 1 < args.repeat {
                sleep(Duration::from_millis(args.interval_ms));
            }
        }
    }

    write_json(&args.out, &TraceFile { version: 1, runs })
}

fn run_build(args: BuildArgs) -> Result<()> {
    let contents = fs::read_to_string(&args.in_path)
        .map_err(|err| anyhow!("failed to read input {:?}: {}", args.in_path, err))?;
    let trace_file: TraceFile = serde_json::from_str(&contents)
        .map_err(|err| anyhow!("failed to parse traces {:?}: {}", args.in_path, err))?;
    let graph = build_graph(&trace_file);
    write_json(&args.out, &graph)
}

fn run_layout(args: LayoutArgs) -> Result<()> {
    let contents = fs::read_to_string(&args.in_path)
        .map_err(|err| anyhow!("failed to read input {:?}: {}", args.in_path, err))?;
    let graph: ptroute_model::GraphFile = serde_json::from_str(&contents)
        .map_err(|err| anyhow!("failed to parse graph {:?}: {}", args.in_path, err))?;
    let scene: SceneFile = layout_graph(&graph, args.seed);
    write_json(&args.out, &scene)
}

fn run_render(args: RenderArgs) -> Result<()> {
    let contents = fs::read_to_string(&args.in_path)
        .map_err(|err| anyhow!("failed to read input {:?}: {}", args.in_path, err))?;
    let scene: SceneFile = serde_json::from_str(&contents)
        .map_err(|err| anyhow!("failed to parse scene {:?}: {}", args.in_path, err))?;

    let settings = RenderSettings {
        width: args.width,
        height: args.height,
        spp: args.spp,
        bounces: args.bounces,
        seed: args.seed,
        progress_every: args.progress_every,
        threads: args.threads,
    };

    if let Some(parent) = args.out.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|err| {
                anyhow!("failed to create output directory {:?}: {}", parent, err)
            })?;
        }
    }

    if args.progressive_every > 0 {
        let mut write_error: Option<anyhow::Error> = None;
        render_scene_progressive(&scene, &settings, args.progressive_every, |image, done| {
            if write_error.is_some() {
                return;
            }

            match write_png(&args.out, image) {
                Ok(()) => eprintln!("render: wrote {} spp to {:?}", done, args.out),
                Err(err) => {
                    write_error = Some(anyhow!("failed to write png: {err}"));
                }
            };
        });
        if let Some(err) = write_error {
            Err(err)
        } else {
            Ok(())
        }
    } else {
        let image = render_scene(&scene, &settings);
        write_png(&args.out, &image).map_err(|err| anyhow!("failed to write png: {err}"))?;
        Ok(())
    }
}

fn run_run(args: RunArgs) -> Result<()> {
    let started_at_utc = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);

    if args.force && args.resume {
        eprintln!("warning: --force overrides --resume; re-running all steps");
    }
    if args.plain {
        eprintln!("warning: --plain is not implemented yet; output will be standard text");
    }

    let out_dir = args.out_dir.clone().unwrap_or_else(default_out_dir);

    if out_dir.exists() {
        if !out_dir.is_dir() {
            return Err(anyhow!(
                "output path {:?} exists and is not a directory",
                out_dir
            ));
        }
        if !args.force && !args.resume {
            return Err(anyhow!(
                "output directory {:?} already exists (use --resume or --force)",
                out_dir
            ));
        }
    } else {
        fs::create_dir_all(&out_dir)
            .map_err(|err| anyhow!("failed to create output directory {:?}: {}", out_dir, err))?;
    }

    let traces_path = out_dir.join("traces.json");
    let graph_path = out_dir.join("graph.json");
    let scene_path = out_dir.join("scene.json");
    let render_path = out_dir.join("render.png");
    let run_path = out_dir.join("run.json");

    let args_summary = RunArgsSummary {
        targets_file: args.targets.clone(),
        targets: args.target_list.clone(),
        out_dir: out_dir.clone(),
        seed: args.seed,
        width: args.width,
        height: args.height,
        spp: args.spp,
        bounces: args.bounces,
        progress_every: args.progress_every,
        threads: args.threads,
        progressive_every: args.progressive_every,
        max_hops: args.max_hops,
        probes: args.probes,
        timeout_ms: args.timeout_ms,
        concurrency: args.concurrency,
        repeat: args.repeat,
        interval_ms: args.interval_ms,
        resume: args.resume,
        force: args.force,
        plain: args.plain,
        open: args.open,
    };

    let allow_skip = args.resume && !args.force;
    let mut skipped = Vec::new();

    let skip_trace = allow_skip && traces_path.exists();
    if skip_trace {
        skipped.push("trace");
    } else {
        run_trace(TraceArgs {
            targets: args.targets,
            target_list: args.target_list,
            out: traces_path.clone(),
            max_hops: args.max_hops,
            probes: args.probes,
            timeout_ms: args.timeout_ms,
            concurrency: args.concurrency,
            repeat: args.repeat,
            interval_ms: args.interval_ms,
        })?;
    }

    let skip_build = allow_skip && graph_path.exists();
    if skip_build {
        skipped.push("build");
    } else {
        run_build(BuildArgs {
            in_path: traces_path.clone(),
            out: graph_path.clone(),
        })?;
    }

    let skip_layout = allow_skip && scene_path.exists();
    if skip_layout {
        skipped.push("layout");
    } else {
        run_layout(LayoutArgs {
            in_path: graph_path.clone(),
            out: scene_path.clone(),
            seed: args.seed,
        })?;
    }

    let skip_render = allow_skip && render_path.exists();
    if skip_render {
        skipped.push("render");
    } else {
        run_render(RenderArgs {
            in_path: scene_path.clone(),
            out: render_path.clone(),
            width: args.width,
            height: args.height,
            spp: args.spp,
            bounces: args.bounces,
            seed: args.seed,
            progress_every: args.progress_every,
            threads: args.threads,
            progressive_every: args.progressive_every,
        })?;
    }

    if !skipped.is_empty() {
        eprintln!("resume: skipped {}", skipped.join(", "));
    }

    let finished_at_utc = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let receipt = RunReceipt {
        version: env!("CARGO_PKG_VERSION").to_string(),
        started_at_utc,
        finished_at_utc,
        args: args_summary,
        outputs: RunOutputs {
            traces: traces_path.clone(),
            graph: graph_path.clone(),
            scene: scene_path.clone(),
            render: render_path.clone(),
            run: run_path.clone(),
        },
        host: HostInfo {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
        },
    };

    write_json(&run_path, &receipt)?;

    if args.open && render_path.exists() {
        open_file(&render_path)?;
    }

    Ok(())
}

fn default_out_dir() -> PathBuf {
    let stamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
    PathBuf::from("output").join(stamp)
}

fn open_file(path: &PathBuf) -> Result<()> {
    let mut cmd = if cfg!(target_os = "macos") {
        let mut cmd = Command::new("open");
        cmd.arg(path);
        cmd
    } else if cfg!(target_os = "linux") {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(path);
        cmd
    } else {
        return Err(anyhow!("--open is not supported on this OS"));
    };

    let status = cmd
        .status()
        .map_err(|err| anyhow!("failed to launch opener: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("open command failed with status: {status}"))
    }
}

fn write_json<T: Serialize>(path: &PathBuf, value: &T) -> Result<()> {
    let json = serde_json::to_vec_pretty(value)?;
    atomic_write(path, &json)
}

fn atomic_write(path: &PathBuf, data: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    if !parent.as_os_str().is_empty() {
        fs::create_dir_all(parent)
            .map_err(|err| anyhow!("failed to create output directory {:?}: {}", parent, err))?;
    }

    let tmp_path = temp_path(path);
    let mut file = fs::File::create(&tmp_path)
        .map_err(|err| anyhow!("failed to create temp file {:?}: {}", tmp_path, err))?;
    file.write_all(data)
        .map_err(|err| anyhow!("failed to write temp file {:?}: {}", tmp_path, err))?;
    file.sync_all()
        .map_err(|err| anyhow!("failed to sync temp file {:?}: {}", tmp_path, err))?;

    if let Err(err) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(anyhow!("failed to replace output {:?}: {}", path, err));
    }

    if let Ok(dir) = fs::File::open(parent) {
        let _ = dir.sync_all();
    }

    Ok(())
}

fn temp_path(path: &PathBuf) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("output");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let tmp_name = format!(".{}.part-{}-{}", file_name, pid, stamp);
    parent.join(tmp_name)
}
