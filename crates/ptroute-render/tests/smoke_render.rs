use ptroute_model::{SceneEdge, SceneFile, SceneNode};
use ptroute_render::{render_scene, RenderSettings};

#[test]
fn render_scene_outputs_image() {
    let scene = SceneFile {
        version: 1,
        nodes: vec![SceneNode {
            id: "node".to_string(),
            position: [0.0, 0.0, 0.0],
            seen: 1,
            loss_probes: 0,
        }],
        edges: vec![SceneEdge {
            from: "node".to_string(),
            to: "node".to_string(),
            seen: 1,
            rtt_delta_ms_avg: 0.0,
        }],
    };

    let settings = RenderSettings {
        width: 32,
        height: 24,
        spp: 2,
        bounces: 2,
        seed: 1,
    };

    let image = render_scene(&scene, &settings);
    assert_eq!(image.width(), settings.width);
    assert_eq!(image.height(), settings.height);
}
