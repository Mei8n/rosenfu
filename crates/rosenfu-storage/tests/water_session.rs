//! Headless execution of the actual desktop preview/build pipeline.
#[allow(dead_code, unused_imports)]
#[path = "../../../src-tauri/src/session.rs"]
mod session;
use serde_json::json;
use session::{PlannedAlignment, WorldEditor};
fn plan(lon: f64, lat: f64, level: &str) -> PlannedAlignment {
    serde_json::from_value(json!({"name":"water model regression","points":[{"longitude":lon,"latitude":lat},{"longitude":lon+0.001,"latitude":lat}],"trackType":"normal","infrastructure":{"level":level,"track_count":1},"stations":[]})).unwrap()
}
#[test]
fn actual_water_preview_build_are_atomic_and_use_the_same_reason() {
    let mut editor = WorldEditor::new();
    let before = editor.snapshot();
    let preview = editor
        .preview_construction_plan(vec![plan(136.1, 35.25, "ground")], vec![])
        .unwrap_err();
    let build = editor
        .build_construction_plan_with_roads(vec![plan(136.1, 35.25, "ground")], vec![], vec![])
        .unwrap_err();
    assert_eq!(preview, build);
    assert!(build.contains("水域上"));
    assert_eq!(editor.snapshot(), before);
}
#[test]
fn actual_land_and_six_metre_bridge_survive_save_roundtrip() {
    for (lon, lat, level) in [(139.7671, 35.6812, "ground"), (136.1, 35.25, "elevated")] {
        let mut editor = WorldEditor::new();
        let preview = editor
            .preview_construction_plan(vec![plan(lon, lat, level)], vec![])
            .unwrap();
        let built = editor
            .build_construction_plan_with_roads(vec![plan(lon, lat, level)], vec![], vec![])
            .unwrap();
        assert_eq!(preview, built);
        let world = rosenfu_core::world::World::new(built.clone()).unwrap();
        let decoded = rosenfu_storage::decode(&rosenfu_storage::encode(&world).unwrap()).unwrap();
        assert_eq!(decoded.snapshot(), &built);
    }
}
