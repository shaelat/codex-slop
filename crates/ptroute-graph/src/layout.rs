use ptroute_model::{GraphFile, SceneEdge, SceneFile, SceneNode};
use std::collections::{HashMap, VecDeque};

pub fn layout_graph(graph: &GraphFile, seed: u64) -> SceneFile {
    if graph.nodes.is_empty() {
        return SceneFile {
            version: 1,
            nodes: Vec::new(),
            edges: Vec::new(),
        };
    }

    let mut indegree: HashMap<&str, u32> = HashMap::new();
    let mut outdegree: HashMap<&str, u32> = HashMap::new();
    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();

    for node in &graph.nodes {
        indegree.insert(node.id.as_str(), 0);
        outdegree.insert(node.id.as_str(), 0);
    }

    for edge in &graph.edges {
        *outdegree.entry(edge.from.as_str()).or_insert(0) += 1;
        *indegree.entry(edge.to.as_str()).or_insert(0) += 1;
        adjacency
            .entry(edge.from.as_str())
            .or_default()
            .push(edge.to.as_str());
    }

    for neighbors in adjacency.values_mut() {
        neighbors.sort();
    }

    let mut starts: Vec<&str> = indegree
        .iter()
        .filter_map(|(id, deg)| if *deg == 0 { Some(*id) } else { None })
        .collect();

    if starts.is_empty() {
        let min_in = indegree.values().min().copied().unwrap_or(0);
        starts = indegree
            .iter()
            .filter_map(|(id, deg)| if *deg == min_in { Some(*id) } else { None })
            .collect();
    }

    starts.sort();

    let mut depth: HashMap<&str, u32> = HashMap::new();
    let mut queue: VecDeque<&str> = VecDeque::new();

    for start in starts {
        depth.insert(start, 0);
        queue.push_back(start);
    }

    while let Some(node) = queue.pop_front() {
        let next_depth = depth.get(node).copied().unwrap_or(0) + 1;
        if let Some(neighbors) = adjacency.get(node) {
            for &neighbor in neighbors {
                if !depth.contains_key(neighbor) {
                    depth.insert(neighbor, next_depth);
                    queue.push_back(neighbor);
                }
            }
        }
    }

    let max_depth = depth.values().copied().max().unwrap_or(0);
    let fallback_depth = max_depth + 1;
    let lane_spacing = 2.0_f32;
    let jitter_scale = 0.5_f32;

    let mut nodes_sorted: Vec<_> = graph.nodes.iter().collect();
    nodes_sorted.sort_by(|a, b| a.id.cmp(&b.id));

    let nodes: Vec<SceneNode> = nodes_sorted
        .into_iter()
        .map(|node| {
            let degree = indegree.get(node.id.as_str()).copied().unwrap_or(0)
                + outdegree.get(node.id.as_str()).copied().unwrap_or(0);
            let bucket = degree_bucket(degree);
            let x = depth
                .get(node.id.as_str())
                .copied()
                .unwrap_or(fallback_depth) as f32;
            let y = bucket as f32 * lane_spacing;
            let z = jitter(seed, &node.id) * jitter_scale;
            SceneNode {
                id: node.id.clone(),
                position: [x, y, z],
                seen: node.seen,
                loss_probes: node.loss_probes,
            }
        })
        .collect();

    let edges: Vec<SceneEdge> = graph
        .edges
        .iter()
        .map(|edge| SceneEdge {
            from: edge.from.clone(),
            to: edge.to.clone(),
            seen: edge.seen,
            rtt_delta_ms_avg: edge.rtt_delta_ms_avg,
        })
        .collect();

    SceneFile {
        version: 1,
        nodes,
        edges,
    }
}

fn degree_bucket(degree: u32) -> i32 {
    if degree == 0 {
        0
    } else {
        (f64::from(degree).log2().floor() as i32).max(0)
    }
}

fn jitter(seed: u64, id: &str) -> f32 {
    let mut hash = 0xcbf29ce484222325u64 ^ seed;
    for byte in id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let unit = hash as f64 / u64::MAX as f64;
    (unit as f32) * 2.0 - 1.0
}
