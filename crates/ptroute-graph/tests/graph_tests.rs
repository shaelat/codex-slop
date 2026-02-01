use ptroute_graph::{build_graph, layout_graph};
use ptroute_model::{Edge, GraphFile, Hop, Node, TraceFile, TraceRun};

fn hop(ttl: u32, ip: Option<&str>, rtt: &[Option<f64>]) -> Hop {
    Hop {
        ttl,
        ip: ip.map(|value| value.to_string()),
        rtt_ms: rtt.to_vec(),
    }
}

#[test]
fn build_graph_counts_nodes_edges() {
    let trace = TraceFile {
        version: 1,
        runs: vec![
            TraceRun {
                target: "1.1.1.1".to_string(),
                timestamp_utc: "2026-02-01T12:00:00Z".to_string(),
                hops: vec![
                    hop(1, Some("10.0.0.1"), &[Some(1.0), None]),
                    hop(2, Some("10.0.0.2"), &[Some(3.0)]),
                    hop(3, None, &[None, None]),
                ],
            },
            TraceRun {
                target: "2.2.2.2".to_string(),
                timestamp_utc: "2026-02-01T12:01:00Z".to_string(),
                hops: vec![
                    hop(1, Some("10.0.0.1"), &[Some(1.2)]),
                    hop(2, Some("10.0.0.3"), &[Some(4.2)]),
                ],
            },
        ],
    };

    let graph = build_graph(&trace);
    assert_eq!(graph.version, 1);

    let node = |id: &str| graph.nodes.iter().find(|node| node.id == id).unwrap();
    assert_eq!(node("10.0.0.1").seen, 2);
    assert_eq!(node("10.0.0.1").loss_probes, 1);
    assert_eq!(node("10.0.0.2").seen, 1);
    assert_eq!(node("10.0.0.3").seen, 1);
    assert_eq!(node("unknown").seen, 1);
    assert_eq!(node("unknown").loss_probes, 2);

    let edge = |from: &str, to: &str| {
        graph
            .edges
            .iter()
            .find(|edge| edge.from == from && edge.to == to)
            .unwrap()
    };

    let edge_a = edge("10.0.0.1", "10.0.0.2");
    assert_eq!(edge_a.seen, 1);
    assert!((edge_a.rtt_delta_ms_avg - 2.0).abs() < 1e-6);

    let edge_b = edge("10.0.0.2", "unknown");
    assert_eq!(edge_b.seen, 1);
    assert_eq!(edge_b.rtt_delta_ms_avg, 0.0);

    let edge_c = edge("10.0.0.1", "10.0.0.3");
    assert_eq!(edge_c.seen, 1);
    assert!((edge_c.rtt_delta_ms_avg - 3.0).abs() < 1e-6);
}

#[test]
fn layout_is_deterministic_for_seed() {
    let graph = GraphFile {
        version: 1,
        nodes: vec![
            Node {
                id: "a".to_string(),
                seen: 1,
                loss_probes: 0,
            },
            Node {
                id: "b".to_string(),
                seen: 1,
                loss_probes: 0,
            },
            Node {
                id: "c".to_string(),
                seen: 1,
                loss_probes: 0,
            },
        ],
        edges: vec![
            Edge {
                from: "a".to_string(),
                to: "b".to_string(),
                seen: 1,
                rtt_delta_ms_avg: 1.0,
            },
            Edge {
                from: "b".to_string(),
                to: "c".to_string(),
                seen: 1,
                rtt_delta_ms_avg: 1.0,
            },
        ],
    };

    let scene_a = layout_graph(&graph, 42);
    let scene_b = layout_graph(&graph, 42);

    assert_eq!(scene_a, scene_b);
}

#[test]
fn layout_changes_with_seed() {
    let graph = GraphFile {
        version: 1,
        nodes: vec![
            Node {
                id: "a".to_string(),
                seen: 1,
                loss_probes: 0,
            },
            Node {
                id: "b".to_string(),
                seen: 1,
                loss_probes: 0,
            },
        ],
        edges: vec![Edge {
            from: "a".to_string(),
            to: "b".to_string(),
            seen: 1,
            rtt_delta_ms_avg: 1.0,
        }],
    };

    let scene_a = layout_graph(&graph, 1);
    let scene_b = layout_graph(&graph, 2);

    let z_a = scene_a.nodes[0].position[2];
    let z_b = scene_b.nodes[0].position[2];
    assert_ne!(z_a, z_b);
}
