use ptroute_model::{Edge, GraphFile, Hop, Node, TraceFile};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
struct NodeStats {
    seen: u32,
    loss_probes: u32,
}

#[derive(Default)]
struct EdgeStats {
    seen: u32,
    sum_delta: f64,
    delta_count: u32,
}

pub fn build_graph(trace_file: &TraceFile) -> GraphFile {
    let mut node_stats: HashMap<String, NodeStats> = HashMap::new();
    let mut edge_stats: HashMap<(String, String), EdgeStats> = HashMap::new();

    for run in &trace_file.runs {
        let mut seen_this_run: HashSet<String> = HashSet::new();

        for hop in &run.hops {
            let id = hop_id(hop);
            if seen_this_run.insert(id.clone()) {
                node_stats.entry(id.clone()).or_default().seen += 1;
            }
            let loss_count = hop
                .rtt_ms
                .iter()
                .filter(|probe| probe.is_none())
                .count() as u32;
            node_stats.entry(id).or_default().loss_probes += loss_count;
        }

        for window in run.hops.windows(2) {
            let from = hop_id(&window[0]);
            let to = hop_id(&window[1]);
            let stats = edge_stats.entry((from.clone(), to.clone())).or_default();
            stats.seen += 1;

            if let (Some(rtt_a), Some(rtt_b)) = (first_rtt(&window[0]), first_rtt(&window[1])) {
                stats.sum_delta += rtt_b - rtt_a;
                stats.delta_count += 1;
            }
        }
    }

    let mut nodes: Vec<Node> = node_stats
        .into_iter()
        .map(|(id, stats)| Node {
            id,
            seen: stats.seen,
            loss_probes: stats.loss_probes,
        })
        .collect();
    nodes.sort_by(|a, b| a.id.cmp(&b.id));

    let mut edges: Vec<Edge> = edge_stats
        .into_iter()
        .map(|((from, to), stats)| Edge {
            from,
            to,
            seen: stats.seen,
            rtt_delta_ms_avg: if stats.delta_count > 0 {
                stats.sum_delta / stats.delta_count as f64
            } else {
                0.0
            },
        })
        .collect();
    edges.sort_by(|a, b| match a.from.cmp(&b.from) {
        std::cmp::Ordering::Equal => a.to.cmp(&b.to),
        other => other,
    });

    GraphFile {
        version: 1,
        nodes,
        edges,
    }
}

fn hop_id(hop: &Hop) -> String {
    hop.ip.clone().unwrap_or_else(|| "unknown".to_string())
}

fn first_rtt(hop: &Hop) -> Option<f64> {
    hop.rtt_ms.iter().copied().flatten().next()
}
