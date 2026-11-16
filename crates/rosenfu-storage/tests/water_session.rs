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

#[test]
fn fine_acquisition_rechecks_native_preview_build_without_mutating_world_on_failure() {
    let mut editor = WorldEditor::new();
    let before = editor.snapshot();
    let request = |level: &str| {
        let mut p = plan(130.92709487964845, 33.930210012951626, level);
        p.points[1].longitude = p.points[0].longitude + 0.000115;
        p.points[0].longitude -= 0.000115;
        p
    };
    let pending = editor
        .preview_construction_plan(vec![request("ground")], vec![])
        .unwrap_err();
    assert!(pending.contains("取得"));
    assert_eq!(
        editor.build_plan(vec![request("ground")]).unwrap_err(),
        pending
    );
    assert!(editor.install_water_mask("kanmon", b"broken").is_err());
    assert_eq!(editor.snapshot(), before);
    editor
        .install_water_mask(
            "kanmon",
            include_bytes!("../../../data/water-mask-trial-20261114/kanmon/water.bin"),
        )
        .unwrap();
    let refused = editor
        .preview_construction_plan(vec![request("ground")], vec![])
        .unwrap_err();
    assert!(refused.contains("水域上"));
    assert_eq!(
        editor.build_plan(vec![request("ground")]).unwrap_err(),
        refused
    );
    assert_eq!(editor.snapshot(), before);
    let preview = editor
        .preview_construction_plan(vec![request("elevated")], vec![])
        .unwrap();
    let built = editor.build_plan(vec![request("elevated")]).unwrap();
    assert_eq!(preview, built);
    let world = rosenfu_core::world::World::new(built.clone()).unwrap();
    assert_eq!(
        rosenfu_storage::decode(&rosenfu_storage::encode(&world).unwrap())
            .unwrap()
            .snapshot(),
        &built
    );
}

#[test]
fn unknown_fine_does_not_block_provably_deep_tunnel_but_never_becomes_land() {
    let mut editor = WorldEditor::new();
    assert!(editor
        .preview_construction_plan(vec![plan(138.73, 35.36, "ground")], vec![])
        .unwrap_err()
        .contains("細密"));
    let mut deep = plan(138.73, 35.36, "ground");
    deep.infrastructure.level = rosenfu_core::world::InfrastructureLevel::Underground2;
    let built = editor.build_plan(vec![deep]).unwrap();
    assert_eq!(built.alignments[0].height_at_position_m(0.5), Some(-12.));
    assert!(editor
        .preview_construction_plan(vec![plan(138.73, 35.36, "ground")], vec![])
        .is_err());
}
