//! Observation diagnostics on unbuilt geometry. Real construction refusal is separately asserted.
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

// Analytical candidates are not construction permission; fine observations may now refuse their build.
fn observed_candidate(
    before: &rosenfu_core::world::WorldDraft,
    points: &[GeoPoint],
    copies: usize,
) -> rosenfu_core::world::WorldDraft {
    let mut world = rosenfu_core::world::World::new(before.clone()).unwrap();
    let owner = before.companies[0].id;
    for _ in 0..copies {
        world
            .build_alignment(owner, "unbuilt diagnostic".into(), points.to_vec())
            .unwrap();
    }
    world.snapshot().clone()
}
#[test]
fn water_ground_real_fixture_observation_does_not_override_native_refusal() {
    let (area, points) = fixture();
    let mut editor = WorldEditor::new();
    let before = editor.snapshot();
    let candidate = observed_candidate(&before, &points, 1);
    let preview = observe_ground_water(&before, &candidate, std::slice::from_ref(&area)).unwrap();
    assert_eq!(preview.contacts.len(), 1);
    assert_eq!(
        preview.contacts[0].interpretation,
        WaterInterpretation::MappedMarineSurface
    );
    assert!(preview.unevaluated_profiles.is_empty());
    assert_eq!(editor.snapshot(), before);
    let preview_error = editor
        .preview_construction_plan(vec![request(&points)], vec![])
        .unwrap_err();
    assert!(preview_error.contains("細密"));
    assert_eq!(
        editor.build_plan(vec![request(&points)]).unwrap_err(),
        preview_error
    );
    assert_eq!(editor.snapshot(), before);
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
    let candidate = observed_candidate(&before, &points, 1);
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
    let candidate = observed_candidate(&before, &points, 1);
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
    let candidate = observed_candidate(&before, &points, 1);
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
    let mut candidate = observed_candidate(&before, &points, 2);
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

#[test]
fn water_ground_profile_interior_crossing_and_tangency_are_candidates_only() {
    use rosenfu_core::world::VerticalProfile;
    let (mut area, _) = fixture();
    area.rings = vec![ring(139.009, 34.999, 139.011, 35.001)];
    let editor = WorldEditor::new();
    let before = editor.snapshot();
    let mut candidate = editor
        .preview_construction_plan(vec![request(&[p(139., 35.), p(139.02, 35.)])], vec![])
        .unwrap();
    for profile in [
        VerticalProfile {
            start_height_m: -6.,
            end_height_m: 6.,
            start_tangent_m: 0.,
            end_tangent_m: 0.,
        },
        VerticalProfile {
            start_height_m: 6.,
            end_height_m: 6.,
            start_tangent_m: -24.,
            end_tangent_m: 24.,
        },
    ] {
        candidate.alignments[0].section_profiles.clear();
        candidate.alignments[0].vertical_profile = Some(profile);
        let r = observe_ground_water(&before, &candidate, std::slice::from_ref(&area)).unwrap();
        assert!(r.contacts.is_empty());
        assert_eq!(r.possible_ground_contacts.len(), 1);
        assert_eq!(r.unevaluated_profiles, vec![candidate.alignments[0].id]);
        assert_eq!(serde_json::to_value(r).unwrap()["coverage"], "unknown");
    }
    // Same-sign endpoints must not hide an interior tangency. A raised minimum
    // beyond the stated tolerance is not reported as a zero-height candidate.
    candidate.alignments[0]
        .vertical_profile
        .as_mut()
        .unwrap()
        .start_height_m += 1e-7;
    candidate.alignments[0]
        .vertical_profile
        .as_mut()
        .unwrap()
        .end_height_m += 1e-7;
    let r = observe_ground_water(&before, &candidate, &[area]).unwrap();
    assert!(r.possible_ground_contacts.is_empty());
    assert_eq!(r.unevaluated_profiles.len(), 1);
}
#[test]
fn water_ground_cubic_three_roots_and_nonuniform_polyline_use_arc_length() {
    let (mut area, _) = fixture();
    let editor = WorldEditor::new();
    let before = editor.snapshot();
    // Two collinear segments of unequal lengths; t=.2 is longitude 139.004,
    // not the fraction .2 of the segment-index count.
    let mut candidate = editor
        .preview_construction_plan(
            vec![request(&[p(139., 35.), p(139.001, 35.), p(139.02, 35.)])],
            vec![],
        )
        .unwrap();
    candidate.alignments[0].section_profiles.clear();
    candidate.alignments[0].vertical_profile = Some(rosenfu_core::world::VerticalProfile {
        start_height_m: -4.8,
        end_height_m: 4.8,
        start_tangent_m: 39.6,
        end_tangent_m: 39.6,
    });
    for longitude in [139.004, 139.01, 139.016] {
        area.rings = vec![ring(
            longitude - 0.00001,
            34.999,
            longitude + 0.00001,
            35.001,
        )];
        let r = observe_ground_water(&before, &candidate, std::slice::from_ref(&area)).unwrap();
        assert_eq!(r.possible_ground_contacts.len(), 1);
        assert!(r.contacts.is_empty());
        assert_eq!(r.unevaluated_profiles.len(), 1);
    }
}
#[test]
fn water_ground_anchor_flat_portion_survives_ramp_and_rejects_instant_step() {
    use rosenfu_core::world::InfrastructureLevel::{Elevated, Ground};
    let (mut area, _) = fixture();
    area.rings = vec![ring(139.002, 34.999, 139.004, 35.001)];
    let editor = WorldEditor::new();
    let before = editor.snapshot();
    let mut req = request(&[p(139., 35.), p(139.01, 35.), p(139.03, 35.)]);
    req.anchor_indices = Some(vec![0, 1, 2]);
    req.anchor_levels = Some(vec![Ground, Ground, Elevated]);
    let mut candidate = editor.preview_construction_plan(vec![req], vec![]).unwrap();
    assert_eq!(candidate.alignments[0].section_profiles.len(), 2);
    let r = observe_ground_water(&before, &candidate, std::slice::from_ref(&area)).unwrap();
    assert_eq!(r.contacts.len(), 1);
    assert!(r.possible_ground_contacts.is_empty());
    assert_eq!(r.unevaluated_profiles.len(), 1);
    candidate.alignments[0].section_profiles.clear();
    candidate.alignments[0].vertical_profile = None;
    candidate.alignments[0].infrastructure = vec![
        rosenfu_core::world::SectionInfrastructure {
            track_count: 1,
            level: Ground,
        },
        rosenfu_core::world::SectionInfrastructure {
            track_count: 1,
            level: Elevated,
        },
    ];
    assert!(observe_ground_water(&before, &candidate, &[area]).is_err());
}
