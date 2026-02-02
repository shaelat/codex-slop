use anyhow::{anyhow, Result};
mod invade;
use chrono::{SecondsFormat, Utc};
use clap::{Args, Parser, Subcommand};
use crossterm::{cursor, event, execute, terminal};
use ptroute_graph::{build_graph, layout_graph};
use ptroute_model::{SceneFile, TraceFile, TraceRun};
use ptroute_render::{render_scene, render_scene_progressive, write_png, RenderSettings};
use ptroute_trace::{run_traces, TraceJobResult, TraceSettings};
use ptroute_trace::{stream_for_target, TraceEvent};
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

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
    Doctor(DoctorArgs),
    Invade(InvadeArgs),
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

#[derive(Clone, Copy)]
enum UiMode {
    Plain,
    Retro,
}

impl UiMode {
    fn use_color(self) -> bool {
        matches!(self, UiMode::Retro)
    }
}

struct Ui {
    mode: UiMode,
}

impl Ui {
    fn new(plain: bool) -> Self {
        Self {
            mode: if plain { UiMode::Plain } else { UiMode::Retro },
        }
    }

    fn banner(&self) {
        if self.mode.use_color() {
            eprintln!(
                "[36m[BOOT][0m PathTraceRoute Loader v{}",
                env!("CARGO_PKG_VERSION")
            );
        } else {
            eprintln!(
                "[BOOT] PathTraceRoute Loader v{}",
                env!("CARGO_PKG_VERSION")
            );
        }
    }

    fn step_ok(&self, step: &str, detail: &str) {
        if self.mode.use_color() {
            eprintln!("[32m[OK ][0m {step}  {detail}");
        } else {
            eprintln!("[OK ] {step}  {detail}");
        }
    }

    fn step_skip(&self, step: &str, detail: &str) {
        if self.mode.use_color() {
            eprintln!("[33m[SKIP][0m {step}  {detail}");
        } else {
            eprintln!("[SKIP] {step}  {detail}");
        }
    }

    fn done(&self, detail: &str) {
        if self.mode.use_color() {
            eprintln!("[35m[DONE][0m {detail}");
        } else {
            eprintln!("[DONE] {detail}");
        }
    }
}

#[derive(Args)]
struct DoctorArgs {
    #[arg(long, default_value = "output")]
    out_dir: PathBuf,
}

#[derive(Args)]
struct InvadeArgs {
    #[arg(long)]
    targets: Option<PathBuf>,

    #[arg(long = "target")]
    target_list: Vec<String>,

    #[arg(long, default_value_t = 30)]
    max_hops: u32,

    #[arg(long, default_value_t = 3)]
    probes: u32,

    #[arg(long, default_value_t = 2000)]
    timeout_ms: u64,

    #[arg(long, default_value_t = 4)]
    concurrency: usize,

    #[arg(long)]
    watch: bool,

    #[arg(long, default_value_t = 1)]
    waves: u32,

    #[arg(long, default_value_t = 2000)]
    wave_interval_ms: u64,

    #[arg(long, default_value_t = 80)]
    refresh_ms: u64,

    #[arg(long, default_value_t = 80.0)]
    warn_rtt: f64,

    #[arg(long, default_value_t = 200.0)]
    bad_rtt: f64,

    #[arg(long, default_value_t = 0.34)]
    warn_loss: f64,

    #[arg(long, default_value_t = 0.67)]
    bad_loss: f64,

    #[arg(long)]
    plain: bool,

    #[arg(long)]
    ascii_only: bool,

    #[arg(long)]
    no_ansi: bool,

    #[arg(long)]
    save_traces: Option<PathBuf>,

    #[arg(long)]
    log_raw: bool,

    #[arg(long)]
    out_dir: Option<PathBuf>,
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
        Commands::Doctor(args) => run_doctor(args),
        Commands::Invade(args) => run_invade(args),
    }
}

fn run_trace(args: TraceArgs) -> Result<()> {
    let mut targets: Vec<String> = Vec::new();

    if let Some(path) = args.targets.clone() {
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

    targets.extend(args.target_list.clone());

    if targets.is_empty() {
        return Err(anyhow!("no targets provided (use --targets or --target)"));
    }

    let settings = TraceSettings {
        max_hops: args.max_hops,
        probes: args.probes,
        timeout_ms: args.timeout_ms,
    };

    let results = run_traces(
        &targets,
        &settings,
        args.repeat,
        args.interval_ms,
        args.concurrency,
    );

    let mut runs: Vec<TraceRun> = Vec::new();

    for TraceJobResult {
        target: _,
        repeat: _,
        result,
    } in results
    {
        match result {
            Ok(parsed) => {
                let timestamp_utc = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
                runs.push(TraceRun {
                    target: parsed.target,
                    timestamp_utc,
                    hops: parsed.hops,
                });
            }
            Err(message) => {
                eprintln!("{message}");
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
    let started = SystemTime::now();
    let started_at_utc = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let ui = Ui::new(args.plain);

    ui.banner();

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

    let skip_trace = allow_skip && traces_path.exists();
    if skip_trace {
        ui.step_skip("trace ", &format!("{}", traces_path.display()));
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
        ui.step_ok(
            "trace ",
            &format!(
                "{} ({} target(s), repeat {})",
                traces_path.display(),
                args_summary.targets.len(),
                args.repeat
            ),
        );
    }

    let skip_build = allow_skip && graph_path.exists();
    if skip_build {
        ui.step_skip("build ", &format!("{}", graph_path.display()));
    } else {
        run_build(BuildArgs {
            in_path: traces_path.clone(),
            out: graph_path.clone(),
        })?;
        let (nodes, edges) = graph_counts(&graph_path);
        ui.step_ok(
            "build ",
            &format!(
                "{} (nodes {}, edges {})",
                graph_path.display(),
                nodes,
                edges
            ),
        );
    }

    let skip_layout = allow_skip && scene_path.exists();
    if skip_layout {
        ui.step_skip("layout", &format!("{}", scene_path.display()));
    } else {
        run_layout(LayoutArgs {
            in_path: graph_path.clone(),
            out: scene_path.clone(),
            seed: args.seed,
        })?;
        ui.step_ok(
            "layout",
            &format!("{} (seed {})", scene_path.display(), args.seed),
        );
    }

    let skip_render = allow_skip && render_path.exists();
    if skip_render {
        ui.step_skip("render", &format!("{}", render_path.display()));
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
        ui.step_ok(
            "render",
            &format!(
                "{} ({}x{}, spp {}, bounces {}, threads {})",
                render_path.display(),
                args.width,
                args.height,
                args.spp,
                args.bounces,
                args.threads
            ),
        );
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

    let elapsed = started.elapsed().unwrap_or_default().as_secs_f64();
    ui.done(&format!("elapsed {:.1}s", elapsed));

    Ok(())
}

fn run_invade(args: InvadeArgs) -> Result<()> {
    let use_ansi = !args.no_ansi && io::stdout().is_terminal();
    let interactive = use_ansi && !args.plain;

    let mut targets: Vec<String> = Vec::new();
    if let Some(path) = args.targets.clone() {
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
    targets.extend(args.target_list.clone());
    if targets.is_empty() {
        return Err(anyhow!("no targets provided (use --targets or --target)"));
    }

    if !interactive {
        let output = render_invade_demo(80, true);
        println!("{output}");
        return Ok(());
    }

    let running = Arc::new(AtomicBool::new(true));
    let running_ctrlc = Arc::clone(&running);
    ctrlc::set_handler(move || {
        running_ctrlc.store(false, Ordering::SeqCst);
    })
    .map_err(|err| anyhow!("failed to install ctrl-c handler: {err}"))?;

    let _guard = TermGuard::enter()?;

    let (term_w, term_h) = terminal::size().unwrap_or((80, 24));

    // Start streaming for the first target only (M3 single-target streaming).
    let settings = TraceSettings {
        max_hops: args.max_hops,
        probes: args.probes,
        timeout_ms: args.timeout_ms,
    };
    let target = targets[0].clone();
    let rx = stream_for_target(&target, &settings)?;

    let mut state = invade::AppState {
        wave: 1,
        targets: vec![invade::TargetView {
            name: target.clone(),
            hops: Vec::new(),
        }],
        last_detail: None,
    };

    while running.load(Ordering::SeqCst) {
        while let Ok(event) = rx.try_recv() {
            match event {
                TraceEvent::HopUpdate { ttl, ip, rtts } => {
                    let loss = if rtts.is_empty() {
                        1.0
                    } else {
                        let lost =
                            rtts.iter().filter(|v: &&Option<f64>| v.is_none()).count() as f64;
                        lost / rtts.len() as f64
                    };
                    let mut rtts_vals: Vec<f64> = rtts.iter().copied().flatten().collect();
                    rtts_vals.sort_by(|a: &f64, b: &f64| {
                        a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
                    });
                    let median_rtt = if rtts_vals.is_empty() {
                        None
                    } else {
                        Some(rtts_vals[rtts_vals.len() / 2])
                    };

                    let hop = invade::HopView {
                        ttl,
                        ip: ip.clone(),
                        loss,
                        median_rtt,
                    };
                    let hops = &mut state.targets[0].hops;
                    let idx = (ttl.saturating_sub(1)) as usize;
                    if hops.len() <= idx {
                        hops.resize_with(idx + 1, || invade::HopView {
                            ttl: 0,
                            ip: None,
                            loss: 1.0,
                            median_rtt: None,
                        });
                    }
                    hops[idx] = hop;
                    state.last_detail = Some(format!(
                        "target={} ttl={} ip={} rtt={:.1?}ms loss={:.0}%",
                        target,
                        ttl,
                        ip.unwrap_or_else(|| "*".to_string()),
                        median_rtt,
                        loss * 100.0
                    ));
                }
                TraceEvent::Done { .. } => {
                    running.store(false, Ordering::SeqCst);
                }
                TraceEvent::Error { message } => {
                    state.last_detail = Some(message);
                }
            }
        }

        let buffer = invade::render_map(
            &state,
            &invade::UiOpts {
                plain: args.plain,
                ascii_only: args.ascii_only,
            },
            term_w,
            term_h,
        );
        draw_frame(&buffer)?;

        if event::poll(std::time::Duration::from_millis(args.refresh_ms))
            .map_err(|err| anyhow!("event poll failed: {err}"))?
        {
            if let event::Event::Key(key) =
                event::read().map_err(|err| anyhow!("event read failed: {err}"))?
            {
                if matches!(
                    key.code,
                    event::KeyCode::Char('q') | event::KeyCode::Char('Q')
                ) {
                    break;
                }
            }
        }
    }

    Ok(())
}

fn render_invade_demo(term_w: u16, plain: bool) -> String {
    let state = invade::AppState {
        wave: 1,
        targets: vec![
            invade::TargetView {
                name: "1.1.1.1".to_string(),
                hops: (1..=6)
                    .map(|ttl| invade::HopView {
                        ttl,
                        ip: Some("10.0.0.1".to_string()),
                        loss: 0.0,
                        median_rtt: Some(10.0 + ttl as f64),
                    })
                    .collect(),
            },
            invade::TargetView {
                name: "8.8.8.8".to_string(),
                hops: (1..=5)
                    .map(|ttl| invade::HopView {
                        ttl,
                        ip: Some("192.168.0.1".to_string()),
                        loss: 0.0,
                        median_rtt: Some(12.0 + ttl as f64),
                    })
                    .collect(),
            },
        ],
        last_detail: Some("Last hop: demo ttl=4 ip=10.0.0.1 rtt=12.3ms loss=0%".to_string()),
    };
    let opts = invade::UiOpts {
        plain,
        ascii_only: plain,
    };
    invade::render_map(&state, &opts, term_w, 24)
}

fn draw_frame(buffer: &str) -> Result<()> {
    let mut stdout = io::stdout();
    execute!(
        stdout,
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0)
    )
    .map_err(|err| anyhow!("failed to clear screen: {err}"))?;
    stdout
        .write_all(buffer.as_bytes())
        .map_err(|err| anyhow!("failed to write frame: {err}"))?;
    stdout
        .flush()
        .map_err(|err| anyhow!("failed to flush: {err}"))?;
    Ok(())
}

struct TermGuard;

impl TermGuard {
    fn enter() -> Result<Self> {
        terminal::enable_raw_mode().map_err(|err| anyhow!("failed to enable raw mode: {err}"))?;
        if let Err(err) = execute!(io::stdout(), terminal::EnterAlternateScreen, cursor::Hide) {
            let _ = terminal::disable_raw_mode();
            return Err(anyhow!("failed to enter alt screen: {err}"));
        }
        Ok(Self)
    }
}

impl Drop for TermGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), cursor::Show, terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

fn run_doctor(args: DoctorArgs) -> Result<()> {
    let mut ok = true;

    if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
        eprintln!("[OK ] os: tracing supported");
    } else {
        eprintln!("[FAIL] os: tracing unsupported (macOS/Linux only)");
        eprintln!("       tip: you can still use build/layout/render with existing traces.json");
        ok = false;
    }

    match Command::new("traceroute")
        .arg("-n")
        .arg("-m")
        .arg("1")
        .arg("127.0.0.1")
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                eprintln!("[OK ] traceroute: available");
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!("[FAIL] traceroute: command failed");
                if !stderr.trim().is_empty() {
                    eprintln!("       details: {}", stderr.trim());
                }
                eprintln!(
                    "       tip: install traceroute (e.g., apt/yum/pacman install traceroute)"
                );
                ok = false;
            }
        }
        Err(_) => {
            eprintln!("[FAIL] traceroute: not found on PATH");
            eprintln!("       tip: install traceroute (e.g., apt/yum/pacman install traceroute)");
            ok = false;
        }
    }

    if let Err(err) = fs::create_dir_all(&args.out_dir) {
        eprintln!("[FAIL] output dir: {:?} ({})", args.out_dir, err);
        ok = false;
    } else {
        let probe = args.out_dir.join(".ptroute-write-test");
        match fs::write(&probe, b"ok") {
            Ok(_) => {
                let _ = fs::remove_file(&probe);
                eprintln!("[OK ] output dir: writable ({:?})", args.out_dir);
            }
            Err(err) => {
                eprintln!(
                    "[FAIL] output dir: {:?} not writable ({})",
                    args.out_dir, err
                );
                ok = false;
            }
        }
    }

    if ok {
        Ok(())
    } else {
        Err(anyhow!("doctor found issues"))
    }
}

fn graph_counts(path: &PathBuf) -> (usize, usize) {
    if let Ok(contents) = fs::read_to_string(path) {
        if let Ok(graph) = serde_json::from_str::<ptroute_model::GraphFile>(&contents) {
            return (graph.nodes.len(), graph.edges.len());
        }
    }
    (0, 0)
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
