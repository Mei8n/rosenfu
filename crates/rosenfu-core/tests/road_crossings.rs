use rosenfu_core::world::*;
fn point(x: f64, y: f64) -> GeoPoint {
    GeoPoint {
        longitude: 139. + x,
        latitude: 35. + y,
    }
}
fn world() -> (World, AlignmentId) {
    let mut world = World::default();
    let p = world.add_player("P".into()).unwrap();
    let c = world.add_company(p, "C".into()).unwrap();
    let id = world
        .build_alignment(c, "Rail".into(), vec![point(-0.01, 0.), point(0.01, 0.)])
        .unwrap();
    (world, id)
}
fn road() -> RoadWay {
    RoadWay {
        id: "way/1".into(),
        coverage: None,
        points: vec![point(0., -0.01), point(0., 0.01)],
        layer: None,
        level: None,
        bridge: None,
        tunnel: None,
    }
}
fn bounds() -> CoverageBounds {
    CoverageBounds {
        west: 138.9,
        east: 139.1,
        south: 34.9,
        north: 35.1,
    }
}
fn snapshot() -> RoadSnapshot {
    RoadSnapshot {
        revision: "fixture-v1".into(),
        source: "synthetic-test".into(),
        source_url: "https://example.org/fixture".into(),
        source_sha256: "a".repeat(64),
        attribution: "Synthetic geometry".into(),
        license: "CC0".into(),
        coverage_verified: true,
        road_coverage: vec![bounds()],
        roads: vec![road()],
    }
}
fn assessment(a: &Alignment, s: RoadSnapshot) -> RoadAssessment {
    assess_road_crossings(a, &[s])
}
#[test]
fn ground_crossing_and_duplicate_geometry() {
    let (mut w, id) = world();
    let mut s = snapshot();
    let mut duplicate = road();
    duplicate.id = "way/2".into();
    s.roads.push(duplicate);
    w.add_road_snapshot(s.clone()).unwrap();
    w.add_road_snapshot(s).unwrap();
    assert_eq!(w.snapshot().road_snapshots.len(), 1);
    let r = w.road_assessment(id).unwrap();
    assert!(r.roads_ready);
    assert!(!r.unknown_road_height);
    assert_eq!(r.crossings.len(), 1);
    assert!((r.crossings[0].position - 0.5).abs() < 1e-6);
    let saved = serde_json::to_string(w.snapshot()).unwrap();
    let loaded = World::new(serde_json::from_str(&saved).unwrap()).unwrap();
    assert_eq!(loaded.road_assessment(id).unwrap(), r);
}
#[test]
fn missing_or_gapped_coverage_is_advisory_not_a_build_rejection() {
    let (mut w, id) = world();
    assert!(!w.road_assessment(id).unwrap().roads_ready);
    let c = w.snapshot().companies[0].id;
    assert!(w
        .build_alignment(
            c,
            "Still builds".into(),
            vec![point(-0.01, 0.02), point(0.01, 0.02)]
        )
        .is_ok());
    let mut s = snapshot();
    s.road_coverage[0].east = 138.999;
    w.add_road_snapshot(s).unwrap();
    assert!(!w.road_assessment(id).unwrap().roads_ready);
    assert!(w.road_assessment(id).unwrap().crossings.is_empty());
}
#[test]
fn adjacent_snapshot_coverage_is_unioned_and_existing_region_retained() {
    let (mut w, id) = world();
    let mut west = snapshot();
    west.road_coverage[0].east = 139.;
    let mut east = snapshot();
    east.road_coverage[0].west = 139.;
    w.add_road_snapshot(west).unwrap();
    w.add_road_snapshot(east).unwrap();
    let r = w.road_assessment(id).unwrap();
    assert!(r.roads_ready);
    assert_eq!(r.crossings.len(), 1);
    assert_eq!(w.snapshot().road_snapshots.len(), 2);
}
#[test]
fn separated_and_unknown_roads_do_not_create_crossings() {
    let (w, _) = world();
    let a = &w.snapshot().alignments[0];
    for (field, value) in [
        ("layer", "1"),
        ("layer", "-1"),
        ("level", "2"),
        ("bridge", "yes"),
        ("tunnel", "yes"),
    ] {
        let mut s = snapshot();
        let r = &mut s.roads[0];
        match field {
            "layer" => r.layer = Some(value.into()),
            "level" => r.level = Some(value.into()),
            "bridge" => r.bridge = Some(value.into()),
            _ => r.tunnel = Some(value.into()),
        };
        let r = assessment(a, s);
        assert!(r.crossings.is_empty());
        assert!(!r.unknown_road_height);
    }
    let mut s = snapshot();
    s.roads[0].layer = Some("unknown".into());
    let r = assessment(a, s);
    assert!(r.crossings.is_empty());
    assert!(r.unknown_road_height);
}
#[test]
fn actual_physical_height_controls_ramps_and_section_profiles() {
    let (w, _) = world();
    let mut a = w.snapshot().alignments[0].clone();
    for height in [-6., 6.] {
        a.vertical_profile = Some(VerticalProfile {
            start_height_m: height,
            end_height_m: height,
            start_tangent_m: 0.,
            end_tangent_m: 0.,
        });
        assert!(assessment(&a, snapshot()).crossings.is_empty());
    }
    a.vertical_profile = Some(VerticalProfile {
        start_height_m: -6.,
        end_height_m: 6.,
        start_tangent_m: 0.,
        end_tangent_m: 0.,
    });
    assert_eq!(assessment(&a, snapshot()).crossings.len(), 1);
    a.points = vec![point(-0.01, 0.), point(0., 0.), point(0.01, 0.)];
    a.anchor_indices = Some(vec![0, 1, 2]);
    a.vertical_profile = None;
    a.section_profiles = vec![
        VerticalProfile {
            start_height_m: 6.,
            end_height_m: 0.,
            start_tangent_m: 0.,
            end_tangent_m: 0.,
        },
        VerticalProfile {
            start_height_m: 0.,
            end_height_m: 0.,
            start_tangent_m: 0.,
            end_tangent_m: 0.,
        },
    ];
    let mut s = snapshot();
    s.roads[0].points = vec![point(-0.005, -0.01), point(-0.005, 0.01)];
    assert!(assessment(&a, s).crossings.is_empty());
    let mut s = snapshot();
    s.roads[0].points = vec![point(0.005, -0.01), point(0.005, 0.01)];
    assert_eq!(assessment(&a, s).crossings.len(), 1);
}
#[test]
fn touching_endpoints_and_tangencies_are_not_crossings_but_split_vertices_are() {
    let (w, _) = world();
    let a = &w.snapshot().alignments[0];
    for points in [
        vec![point(0., -0.01), point(0., 0.)],
        vec![point(-0.001, -0.01), point(0., 0.), point(0.001, -0.01)],
        vec![point(-0.01, -0.01), point(-0.01, 0.01)],
    ] {
        let mut s = snapshot();
        s.roads[0].points = points;
        assert!(assessment(a, s).crossings.is_empty());
    }
    let mut s = snapshot();
    s.roads[0].points = vec![point(0., -0.01), point(0., 0.), point(0., 0.01)];
    assert_eq!(assessment(a, s).crossings.len(), 1);
    let mut s = snapshot();
    s.roads[0].points = vec![point(0., -0.01), point(0., 0.)];
    let mut other = road();
    other.id = "way/2".into();
    other.points = vec![point(0., 0.), point(0., 0.01)];
    s.roads.push(other);
    assert_eq!(assessment(a, s).crossings.len(), 1);
}
#[test]
fn edits_removal_and_undo_rederive_without_stale_records() {
    let (mut w, id) = world();
    w.add_road_snapshot(snapshot()).unwrap();
    let before = w.snapshot().clone();
    let mut edited = before.clone();
    edited.alignments[0].vertical_profile = Some(VerticalProfile {
        start_height_m: 6.,
        end_height_m: 6.,
        start_tangent_m: 0.,
        end_tangent_m: 0.,
    });
    w.replace(edited).unwrap();
    assert!(w.road_assessment(id).unwrap().crossings.is_empty());
    let mut removed = before.clone();
    removed.alignments.clear();
    w.replace(removed).unwrap();
    assert!(w.road_assessment(id).is_err());
    w = World::new(before).unwrap();
    assert_eq!(w.road_assessment(id).unwrap().crossings.len(), 1);
}
#[test]
fn malformed_or_conflicting_snapshots_are_rejected_atomically() {
    let (mut w, _) = world();
    w.add_road_snapshot(snapshot()).unwrap();
    let before = w.snapshot().clone();
    let mut bad = snapshot();
    bad.revision = "different".into();
    assert!(w.add_road_snapshot(bad).is_err());
    let mut bad = snapshot();
    bad.roads[0].points[0] = point(0.01, -0.01);
    assert!(w.add_road_snapshot(bad).is_err());
    let mut bad = snapshot();
    bad.road_coverage[0].west = f64::NAN;
    assert!(w.add_road_snapshot(bad).is_err());
    let mut bad = snapshot();
    bad.roads.push(road());
    assert!(w.add_road_snapshot(bad).is_err());
    assert_eq!(w.snapshot(), &before);
}

#[test]
fn categorical_rail_levels_and_way_validity_are_respected() {
    let (w, _) = world();
    let mut a = w.snapshot().alignments[0].clone();
    for level in [
        InfrastructureLevel::Elevated,
        InfrastructureLevel::Underground,
    ] {
        a.infrastructure = vec![SectionInfrastructure {
            level,
            track_count: 1,
        }];
        assert!(assessment(&a, snapshot()).crossings.is_empty());
    }
    a.infrastructure.clear();
    let mut s = snapshot();
    let mut scope = bounds();
    scope.east = 138.999;
    s.roads[0].coverage = Some(vec![scope]);
    let r = assessment(&a, s);
    assert!(r.roads_ready);
    assert!(r.crossings.is_empty());
}

#[test]
fn union_with_a_hole_does_not_claim_complete_coverage() {
    let (w, _) = world();
    let mut west = snapshot();
    west.road_coverage[0].east = 138.999;
    let mut east = snapshot();
    east.road_coverage[0].west = 139.001;
    let r = assess_road_crossings(&w.snapshot().alignments[0], &[west, east]);
    assert!(!r.roads_ready);
    assert!(r.crossings.is_empty());
}

fn connected_world(
    a_end: AlignmentEndpoint,
    b_end: AlignmentEndpoint,
) -> (World, AlignmentId, AlignmentId) {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let mut ap = vec![point(-0.01, 0.), point(0., 0.)];
    let mut bp = vec![point(0., 0.), point(0.01, 0.)];
    if a_end == AlignmentEndpoint::Start {
        ap.reverse();
    }
    if b_end == AlignmentEndpoint::End {
        bp.reverse();
    }
    let a = w.build_alignment(c, "West".into(), ap).unwrap();
    let b = w.build_alignment(c, "East".into(), bp).unwrap();
    w.add_road_snapshot(snapshot()).unwrap();
    // Coordinate coincidence is deliberately insufficient.
    assert!(w
        .road_assessments()
        .values()
        .all(|r| r.crossings.is_empty()));
    w.connect_alignment_endpoints(
        c,
        AlignmentEndpointRef {
            alignment_id: a,
            endpoint: a_end,
        },
        AlignmentEndpointRef {
            alignment_id: b,
            endpoint: b_end,
        },
    )
    .unwrap();
    (w, a, b)
}
#[test]
fn explicit_connected_rail_endpoints_create_one_asset_in_all_orientations() {
    for ae in [AlignmentEndpoint::Start, AlignmentEndpoint::End] {
        for be in [AlignmentEndpoint::Start, AlignmentEndpoint::End] {
            let (w, a, b) = connected_world(ae, be);
            let assessments = w.road_assessments();
            assert_eq!(
                assessments
                    .values()
                    .map(|r| r.crossings.len())
                    .sum::<usize>(),
                1
            );
            assert_eq!(assessments[&a].crossings.len(), 1);
            assert!(assessments[&b].crossings.is_empty());
            assert_eq!(
                assessments[&a].crossings[0].position,
                if ae == AlignmentEndpoint::Start {
                    0.
                } else {
                    1.
                }
            );
            assert!(assessments.values().all(|r| r.roads_ready));
        }
    }
}
#[test]
fn connected_endpoint_asset_tracks_detachment_removal_and_undo() {
    let (mut w, a, b) = connected_world(AlignmentEndpoint::End, AlignmentEndpoint::Start);
    let before = w.snapshot().clone();
    let mut detached = before.clone();
    detached.alignment_connections.clear();
    w.replace(detached.clone()).unwrap();
    assert!(w
        .road_assessments()
        .values()
        .all(|r| r.crossings.is_empty()));
    detached.alignments.retain(|x| x.id != b);
    w.replace(detached).unwrap();
    assert!(w.road_assessment(a).unwrap().crossings.is_empty());
    w = World::new(before).unwrap();
    assert_eq!(w.road_assessment(a).unwrap().crossings.len(), 1);
}
#[test]
fn connected_endpoint_uses_physical_height_and_rejects_tangent_road() {
    let (w, _, _) = connected_world(AlignmentEndpoint::End, AlignmentEndpoint::Start);
    let mut draft = w.snapshot().clone();
    for alignment in &mut draft.alignments {
        alignment.vertical_profile = Some(VerticalProfile {
            start_height_m: 6.,
            end_height_m: 6.,
            start_tangent_m: 0.,
            end_tangent_m: 0.,
        });
    }
    let elevated = World::new(draft.clone()).unwrap();
    assert!(elevated
        .road_assessments()
        .values()
        .all(|r| r.crossings.is_empty()));
    draft.alignments[0].vertical_profile = None;
    assert!(World::new(draft).is_err()); // Explicit links cannot join unlike heights.
    let mut draft = w.snapshot().clone();
    draft.road_snapshots[0].roads[0].points =
        vec![point(-0.001, -0.01), point(0., 0.), point(0.001, -0.01)];
    let tangent = World::new(draft).unwrap();
    assert!(tangent
        .road_assessments()
        .values()
        .all(|r| r.crossings.is_empty()));
}

#[test]
fn split_ways_with_equivalent_surface_tags_join() {
    let (w, _) = world();
    let mut s = snapshot();
    s.roads[0].points = vec![point(0., -0.01), point(0., 0.)];
    let mut other = road();
    other.id = "way/2".into();
    other.points = vec![point(0., 0.), point(0., 0.01)];
    other.layer = Some("0".into());
    other.level = Some("0.0".into());
    other.bridge = Some("no".into());
    other.tunnel = Some("no".into());
    s.roads.push(other);
    assert_eq!(
        assessment(&w.snapshot().alignments[0], s.clone())
            .crossings
            .len(),
        1
    );
    s.roads[1].layer = Some("unknown".into());
    assert!(assessment(&w.snapshot().alignments[0], s)
        .crossings
        .is_empty());
}

#[test]
fn unverified_polygon_never_certifies_coverage() {
    let (mut w, id) = world();
    let mut s = snapshot();
    s.coverage_verified = false;
    w.add_road_snapshot(s.clone()).unwrap();
    let assessment = w.road_assessment(id).unwrap();
    assert!(!assessment.roads_ready);
    assert_eq!(assessment.crossings.len(), 1); // Observed road geometry remains usable.
    let mut value = serde_json::to_value(&s).unwrap();
    value.as_object_mut().unwrap().remove("coverage_verified");
    assert!(serde_json::from_value::<RoadSnapshot>(value).is_err());
}

#[test]
fn broadphase_preserves_boundary_crossings_and_advisory_status() {
    let (w, _) = world();
    let alignment = &w.snapshot().alignments[0];
    let mut s = snapshot();
    // Zero-width road bounding box and an intersection on the coverage edge.
    s.road_coverage[0].west = 139.;
    let expected = assessment(alignment, s.clone());
    assert_eq!(expected.crossings.len(), 1);
    assert!(!expected.roads_ready);
    for i in 0..100 {
        let mut distant = road();
        distant.id = format!("far/{i}");
        distant.points = vec![point(0.02, 0.02), point(0.03, 0.03)];
        distant.layer = Some("unknown".into());
        s.roads.push(distant);
    }
    assert_eq!(assessment(alignment, s.clone()), expected);
    // Nearby unknown structure stays unverified, not silently filtered out.
    s.roads[0].layer = Some("unknown".into());
    let unknown = assessment(alignment, s.clone());
    assert!(unknown.unknown_road_height);
    assert!(unknown.crossings.is_empty());
    s.roads[0].layer = Some("1".into());
    let elevated = assessment(alignment, s);
    assert!(!elevated.unknown_road_height);
    assert!(elevated.crossings.is_empty());
}
