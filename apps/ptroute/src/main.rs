use anyhow::{anyhow, Result};
use chrono::{SecondsFormat, Utc};
use clap::{Args, Parser, Subcommand};
use ptroute_graph::{build_graph, layout_graph};
use ptroute_model::{SceneFile, TraceFile, TraceRun};
use ptroute_render::{render_scene, render_scene_progressive, write_png, RenderSettings};
use ptroute_trace::{parse_traceroute_n_with_target, run_traceroute, TraceSettings};
use std::fs;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

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

    #[arg(long, default_value_t = 32)]
    progress_every: u32,

    #[arg(long, default_value_t = 0)]
    threads: usize,

    #[arg(long, default_value_t = 0)]
    progressive_every: u32,
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
        return Err(anyhow!(
            "no targets provided (use --targets or --target)"
        ));
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
                        let timestamp_utc =
                            Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
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
            fs::create_dir_all(parent)
                .map_err(|err| anyhow!("failed to create output directory {:?}: {}", parent, err))?;
        }
    }

    if args.progressive_every > 0 {
        render_scene_progressive(&scene, &settings, args.progressive_every, |image, done| {
            if let Err(err) = write_png(&args.out, image) {
                eprintln!("failed to write png: {err}");
            } else {
                eprintln!("render: wrote {} spp to {:?}", done, args.out);
            }
        });
        Ok(())
    } else {
        let image = render_scene(&scene, &settings);
        write_png(&args.out, &image).map_err(|err| anyhow!("failed to write png: {err}"))?;
        Ok(())
    }
}

fn write_json<T: serde::Serialize>(path: &PathBuf, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|err| anyhow!("failed to create output directory {:?}: {}", parent, err))?;
        }
    }

    let json = serde_json::to_string_pretty(value)?;
    fs::write(path, json).map_err(|err| anyhow!("failed to write output {:?}: {}", path, err))?;
    Ok(())
}
