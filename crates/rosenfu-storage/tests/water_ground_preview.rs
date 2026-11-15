//! The real session preview/build pipeline supplies the geometry and heights.
//! This does not register an IPC command or enforce a construction refusal.
#[allow(dead_code)]
#[path = "../../../src-tauri/src/session.rs"]
mod session;
use rosenfu_core::water_observations::{
    observe_ground_water, WaterAreaObservation, WaterInterpretation,
};
use rosenfu_core::world::GeoPoint;
use serde_json::json;
use session::{PlannedAlignment, WorldEditor};

fn fixture() -> (WaterAreaObservation, Vec<GeoPoint>) {
    let value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/kanmon-water-ground.json")).unwrap();
    (
        serde_json::from_value(value["evidence"].clone()).unwrap(),
        serde_json::from_value(value["points"].clone()).unwrap(),
    )
}
fn request(points: &[GeoPoint]) -> PlannedAlignment {
    serde_json::from_value(json!({"name":"Water observation test","points":points,"trackType":"normal","designSpeedKmh":80,"stations":[]})).unwrap()
}
fn p(x: f64, y: f64) -> GeoPoint {
    GeoPoint {
        longitude: x,
        latitude: y,
    }
}
fn ring(w: f64, s: f64, e: f64, n: f64) -> Vec<GeoPoint> {
    vec![p(w, s), p(e, s), p(e, n), p(w, n), p(w, s)]
}

#[test]
fn water_ground_real_https_fixture_preview_and_commit_geometry_match() {
    let (area, points) = fixture();
    let mut editor = WorldEditor::new();
    let before = editor.snapshot();
    let candidate = editor
        .preview_construction_plan(vec![request(&points)], vec![])
        .unwrap();
    let preview = observe_ground_water(&before, &candidate, std::slice::from_ref(&area)).unwrap();
    assert_eq!(preview.contacts.len(), 1);
    assert_eq!(
        preview.contacts[0].interpretation,
        WaterInterpretation::MappedMarineSurface
    );
    assert!(preview.unevaluated_profiles.is_empty());
    assert_eq!(editor.snapshot(), before);
    let committed = editor.build_plan(vec![request(&points)]).unwrap();
    assert_eq!(candidate, committed);
    assert_eq!(
        preview,
        observe_ground_water(&before, &committed, &[area]).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&preview).unwrap()["coverage"],
        "unknown"
    );
    assert!(serde_json::to_value(preview)
        .unwrap()
        .get("buildable")
        .is_none());
}
#[test]
fn water_ground_covered_height_and_basin_stay_unresolved() {
    let (mut area, points) = fixture();
    let editor = WorldEditor::new();
    let before = editor.snapshot();
    let candidate = editor
        .preview_construction_plan(vec![request(&points)], vec![])
        .unwrap();
    for (key, value, expected) in [
        (
            "covered",
            "yes",
            WaterInterpretation::CoveredOrHeightUnresolved,
        ),
        (
            "tunnel",
            "culvert",
            WaterInterpretation::CoveredOrHeightUnresolved,
        ),
        ("layer", "2", WaterInterpretation::CoveredOrHeightUnresolved),
        ("basin", "detention", WaterInterpretation::BasinUnresolved),
    ] {
        area.tags.insert(key.into(), value.into());
        let r = observe_ground_water(&before, &candidate, std::slice::from_ref(&area)).unwrap();
        assert_eq!(r.contacts[0].interpretation, expected);
        area.tags.remove(key);
    }
    area.source_url = "https://download.geofabrik.de/asia/japan/kanto-261114.osm.pbf".into();
    assert_eq!(
        observe_ground_water(&before, &candidate, &[area])
            .unwrap()
            .contacts[0]
            .interpretation,
        WaterInterpretation::InlandSurfaceUnresolved
    );
}
#[test]
fn water_ground_holes_and_between_endpoint_intersections() {
    let (mut area, _) = fixture();
    area.rings = vec![
        ring(139., 35., 139.02, 35.02),
        ring(139.005, 35.005, 139.015, 35.015),
    ];
    let editor = WorldEditor::new();
    let before = editor.snapshot();
    let hole = editor
        .preview_construction_plan(
            vec![request(&[p(139.008, 35.01), p(139.012, 35.01)])],
            vec![],
        )
        .unwrap();
    assert!(
        observe_ground_water(&before, &hole, std::slice::from_ref(&area))
            .unwrap()
            .contacts
            .is_empty()
    );
    let cross = editor
        .preview_construction_plan(vec![request(&[p(138.99, 35.01), p(139.03, 35.01)])], vec![])
        .unwrap();
    assert_eq!(
        observe_ground_water(&before, &cross, &[area])
            .unwrap()
            .contacts
            .len(),
        1
    );
}
#[test]
fn water_ground_nonflat_profile_is_unevaluated_not_safe() {
    let (area, _) = fixture();
    let editor = WorldEditor::new();
    let before = editor.snapshot();
    let mut r = request(&[p(139., 35.), p(139.02, 35.)]);
    r.start_level = Some(rosenfu_core::world::InfrastructureLevel::Ground);
    r.infrastructure.level = rosenfu_core::world::InfrastructureLevel::Elevated;
    let candidate = editor.preview_construction_plan(vec![r], vec![]).unwrap();
    let result = observe_ground_water(&before, &candidate, &[area]).unwrap();
    assert_eq!(result.unevaluated_profiles.len(), 1);
    assert!(result.contacts.is_empty());
}
#[test]
fn water_ground_malformed_source_is_not_empty_evidence() {
    let (mut area, points) = fixture();
    let editor = WorldEditor::new();
    let before = editor.snapshot();
    let candidate = editor
        .preview_construction_plan(vec![request(&points)], vec![])
        .unwrap();
    area.rings[0].pop();
    assert!(observe_ground_water(&before, &candidate, &[area]).is_err());
    assert_eq!(editor.snapshot(), before);
    assert!(observe_ground_water(&before, &candidate, &[])
        .unwrap()
        .contacts
        .is_empty());
}

#[test]
fn water_ground_output_limit_returns_no_partial_assessment() {
    let (area, points) = fixture();
    let editor = WorldEditor::new();
    let before = editor.snapshot();
    let candidate = editor
        .preview_construction_plan(vec![request(&points)], vec![])
        .unwrap();
    let areas: Vec<_> = (0..1025)
        .map(|i| {
            let mut a = area.clone();
            a.observation_id = format!("fixture/{i}");
            a
        })
        .collect();
    assert_eq!(
        observe_ground_water(&before, &candidate, &areas).unwrap_err(),
        "water contact output limit"
    );
    assert_eq!(editor.snapshot(), before);
}
#[test]
fn water_ground_all_changed_paths_and_legacy_levels_are_accounted() {
    let (area, points) = fixture();
    let editor = WorldEditor::new();
    let before = editor.snapshot();
    let mut candidate = editor
        .preview_construction_plan(vec![request(&points), request(&points)], vec![])
        .unwrap();
    for path in &mut candidate.alignments {
        path.vertical_profile = None;
        path.section_profiles.clear();
        path.infrastructure.clear();
    }
    let result = observe_ground_water(&before, &candidate, std::slice::from_ref(&area)).unwrap();
    assert_eq!(result.contacts.len(), 2);
    assert_ne!(
        result.contacts[0].alignment_id,
        result.contacts[1].alignment_id
    );
    candidate.alignments[1].infrastructure = vec![rosenfu_core::world::SectionInfrastructure {
        track_count: 1,
        level: rosenfu_core::world::InfrastructureLevel::Elevated,
    }];
    let result = observe_ground_water(&before, &candidate, &[area]).unwrap();
    assert_eq!(result.contacts.len(), 1);
    assert_eq!(
        result.unevaluated_profiles,
        vec![candidate.alignments[1].id]
    );
}
