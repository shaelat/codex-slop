//! Shared data structures for PathTraceRoute.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraceFile {
    pub version: u32,
    pub runs: Vec<TraceRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraceRun {
    pub target: String,
    pub timestamp_utc: String,
    pub hops: Vec<Hop>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Hop {
    pub ttl: u32,
    pub ip: Option<String>,
    pub rtt_ms: Vec<Option<f64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphFile {
    pub version: u32,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Node {
    pub id: String,
    pub seen: u32,
    pub loss_probes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub seen: u32,
    pub rtt_delta_ms_avg: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SceneFile {
    pub version: u32,
    pub nodes: Vec<SceneNode>,
    pub edges: Vec<SceneEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SceneNode {
    pub id: String,
    pub position: [f32; 3],
    pub seen: u32,
    pub loss_probes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SceneEdge {
    pub from: String,
    pub to: String,
    pub seen: u32,
    pub rtt_delta_ms_avg: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_file_round_trip_is_stable() {
        let trace = TraceFile {
            version: 1,
            runs: vec![TraceRun {
                target: "1.1.1.1".to_string(),
                timestamp_utc: "2026-02-01T12:34:56Z".to_string(),
                hops: vec![
                    Hop {
                        ttl: 1,
                        ip: Some("192.168.1.1".to_string()),
                        rtt_ms: vec![Some(1.2), Some(1.1), Some(1.3)],
                    },
                    Hop {
                        ttl: 2,
                        ip: Some("10.0.0.1".to_string()),
                        rtt_ms: vec![Some(5.2), None, Some(5.1)],
                    },
                    Hop {
                        ttl: 3,
                        ip: None,
                        rtt_ms: vec![None, None, None],
                    },
                ],
            }],
        };

        let json = serde_json::to_string_pretty(&trace).unwrap();
        let decoded: TraceFile = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string_pretty(&decoded).unwrap();

        assert_eq!(trace, decoded);
        assert_eq!(json, json2);
    }

    #[test]
    fn graph_file_round_trip_is_stable() {
        let graph = GraphFile {
            version: 1,
            nodes: vec![
                Node {
                    id: "192.168.1.1".to_string(),
                    seen: 10,
                    loss_probes: 0,
                },
                Node {
                    id: "10.0.0.1".to_string(),
                    seen: 10,
                    loss_probes: 2,
                },
            ],
            edges: vec![Edge {
                from: "192.168.1.1".to_string(),
                to: "10.0.0.1".to_string(),
                seen: 10,
                rtt_delta_ms_avg: 4.0,
            }],
        };

        let json = serde_json::to_string_pretty(&graph).unwrap();
        let decoded: GraphFile = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string_pretty(&decoded).unwrap();

        assert_eq!(graph, decoded);
        assert_eq!(json, json2);
    }

    #[test]
    fn scene_file_round_trip_is_stable() {
        let scene = SceneFile {
            version: 1,
            nodes: vec![SceneNode {
                id: "192.168.1.1".to_string(),
                position: [0.0, 0.5, -0.25],
                seen: 10,
                loss_probes: 0,
            }],
            edges: vec![SceneEdge {
                from: "192.168.1.1".to_string(),
                to: "10.0.0.1".to_string(),
                seen: 10,
                rtt_delta_ms_avg: 4.0,
            }],
        };

        let json = serde_json::to_string_pretty(&scene).unwrap();
        let decoded: SceneFile = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string_pretty(&decoded).unwrap();

        assert_eq!(scene, decoded);
        assert_eq!(json, json2);
    }
}
