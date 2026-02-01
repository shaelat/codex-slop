# PathTraceRoute

PathTraceRoute (ptroute) is a Rust CLI that:
- runs traceroute in numeric mode,
- merges hops into a directed graph,
- lays that graph out in 3D,
- renders a CPU path-traced PNG with nodes and glowing links.

This repository is an evolving prototype, but the core end-to-end flow works.

## Quick start

Build the CLI:

```bash
cargo build -p ptroute
```

Create a targets file (one host/IP per line):

```text
1.1.1.1
8.8.8.8
```

Run the full pipeline:

```bash
cargo run -p ptroute -- trace  --targets examples/targets.txt --out output/traces.json
cargo run -p ptroute -- build  --in output/traces.json   --out output/graph.json
cargo run -p ptroute -- layout --in output/graph.json    --out output/scene.json --seed 1
cargo run -p ptroute -- render --in output/scene.json    --out output/render.png \
  --width 1600 --height 900 --spp 200 --bounces 6 --seed 1
```

## Requirements

- Rust toolchain (stable).
- System `traceroute` command available on PATH.
  - macOS: built-in.
  - Linux: install `traceroute` package for your distro.
- macOS and Linux are supported for tracing. Windows is not yet supported.

## Safety and permissions

Only run traceroute against networks and targets you own or have permission to test.

## CLI overview

### ptroute trace
Runs `traceroute -n` and writes `traces.json`.

```bash
ptroute trace --targets examples/targets.txt --out output/traces.json
ptroute trace --target 1.1.1.1 --target 8.8.8.8 --out output/traces.json
```

Options:
- `--targets <file>`: newline-separated targets file.
- `--target <host>`: repeatable; add targets on the command line.
- `--out <file>`: output JSON.
- `--max-hops <n>`: default 30.
- `--probes <n>`: default 3.
- `--timeout-ms <ms>`: default 2000.
- `--concurrency <n>`: default 4 (currently not implemented; runs sequentially).
- `--repeat <n>`: default 1 (multiple runs per target).
- `--interval-ms <ms>`: default 0 (pause between repeats).

### ptroute build
Consumes `traces.json`, produces `graph.json`.

```bash
ptroute build --in output/traces.json --out output/graph.json
```

### ptroute layout
Consumes `graph.json`, produces `scene.json`.

```bash
ptroute layout --in output/graph.json --out output/scene.json --seed 1
```

Layout notes:
- Deterministic for a given seed.
- X axis approximates hop depth, Y groups nodes by degree bucket, Z adds stable jitter.

### ptroute render
Consumes `scene.json`, produces `render.png`.

```bash
ptroute render --in output/scene.json --out output/render.png \
  --width 1600 --height 900 --spp 200 --bounces 6 --seed 1
```

Options:
- `--width`, `--height`: image size.
- `--spp`: samples per pixel (higher = less noise, slower).
- `--bounces`: max path bounces.
- `--seed`: deterministic sampling.
- `--progress-every <n>`: log progress every N scanlines.
- `--threads <n>`: 0 uses Rayon default (usually all cores).
- `--progressive-every <n>`: write a PNG every N samples for preview.

Rendering notes:
- Nodes are matte spheres.
- Links are currently rendered as chains of small emissive spheres.
- BVH acceleration is enabled for faster intersection.

## Outputs and formats

The pipeline produces JSON files plus a PNG:
- `traces.json`: raw traceroute runs
- `graph.json`: merged hop graph
- `scene.json`: 3D positions for render
- `render.png`: final image

High-level schema (see `crates/ptroute-model/src/lib.rs` for exact structs):

```json
{
  "version": 1,
  "runs": [
    {
      "target": "1.1.1.1",
      "timestamp_utc": "2026-02-01T12:34:56Z",
      "hops": [
        {"ttl": 1, "ip": "192.168.1.1", "rtt_ms": [1.2, 1.1, 1.3]},
        {"ttl": 2, "ip": null, "rtt_ms": [null, null, null]}
      ]
    }
  ]
}
```

## Troubleshooting

- `command not found: traceroute`:
  - Install the system traceroute tool (Linux) or ensure it is on PATH.
- `ptroute: command not found`:
  - Use `cargo run -p ptroute -- ...` or `cargo build -p ptroute` and run `target/debug/ptroute`.
- Parsing errors:
  - The CLI always uses numeric mode (`-n`). If you feed custom traces, ensure they match numeric traceroute output.
- Render feels stuck:
  - Use `--progress-every 8` or `--progressive-every 10` to get frequent updates.
  - Try a smaller image or lower `--spp` first.

## Development

Common commands:

```bash
cargo fmt
cargo clippy
cargo test
```

Targeted tests:

```bash
cargo test -p ptroute-trace
cargo test -p ptroute-graph
cargo test -p ptroute-render
```

## Roadmap

See `docs/spec.md` for the living roadmap and milestone list.
