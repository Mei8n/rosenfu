use rosenfu_core::world::*;
// Small observed OFM geometry fixture, OSM contributors / ODbL-1.0.
// Revision 20261101_080013_pt, tiles 14/14552/6451 and 14/14553/6451.
fn fixture() -> (World, AlignmentId) {
    let f: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/ofm_double_crossing.json")).unwrap();
    let point = |v: &serde_json::Value| GeoPoint {
        longitude: v[0].as_f64().unwrap(),
        latitude: v[1].as_f64().unwrap(),
    };
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let id = w
        .build_alignment(
            c,
            "Seam rail".into(),
            f["rail"].as_array().unwrap().iter().map(point).collect(),
        )
        .unwrap();
    for (i, r) in f["roads"].as_array().unwrap().iter().enumerate() {
        let x = 14552 + i;
        let url =
            format!("https://tiles.openfreemap.org/planet/20261101_080013_pt/14/{x}/6451.pbf");
        w.add_road_snapshot(RoadSnapshot {
            revision: "20261101_080013_pt".into(),
            source: url.clone(),
            source_url: url,
            source_sha256: "a".repeat(64),
            attribution: "© OpenStreetMap contributors".into(),
            license: "ODbL-1.0".into(),
            coverage_verified: false,
            road_coverage: vec![CoverageBounds {
                west: x as f64 / 16384. * 360. - 180.,
                east: (x + 1) as f64 / 16384. * 360. - 180.,
                south: 35.67514743608467,
                north: 35.6929946320988,
            }],
            roads: vec![RoadWay {
                id: format!(
                    "ofm:20261101_080013_pt:14:{x}:6451:{}:{}",
                    if i == 0 { 471 } else { 836 },
                    if i == 0 { 11 } else { 2 }
                ),
                coverage: None,
                classification: Some(RoadClassification {
                    class: "tertiary".into(),
                    subclass: None,
                    access: None,
                    oneway: None,
                }),
                points: r["points"].as_array().unwrap().iter().map(point).collect(),
                layer: None,
                level: None,
                bridge: None,
                tunnel: None,
            }],
        })
        .unwrap();
    }
    (w, id)
}
fn result(w: &World, id: AlignmentId) -> RoadAssessment {
    w.road_assessment(id).unwrap()
}
#[test]
fn observed_quantized_boundary_crossing_is_one_hypothesis_not_distance_clustering() {
    let (w, id) = fixture();
    let r = result(&w, id);
    assert_eq!(r.crossings.len(), 1);
    assert!(!r.roads_ready);
    assert_eq!(
        r.crossings[0].road_id,
        "ofm:20261101_080013_pt:14:14552:6451:471:11"
    );
    assert_eq!(r.crossings[0].point.latitude, 35.68534327496442);
    let mut unsupported = w.snapshot().clone();
    for s in &mut unsupported.road_snapshots {
        s.roads[0].classification = None;
    }
    let raw = result(&World::new(unsupported).unwrap(), id);
    assert_eq!(raw.crossings.len(), 2);
    let delta = (raw.crossings[1].point.latitude - raw.crossings[0].point.latitude) * 111320.;
    assert!((delta - 0.634098).abs() < 0.00001);
}
#[test]
fn canonical_result_survives_reload_snapshot_order_duplicates_and_rail_resampling() {
    let (mut w, id) = fixture();
    let original = result(&w, id);
    let saved = serde_json::to_string(w.snapshot()).unwrap();
    assert_eq!(
        result(
            &World::new(serde_json::from_str(&saved).unwrap()).unwrap(),
            id
        ),
        original
    );
    let s = w.snapshot().road_snapshots[0].clone();
    w.add_road_snapshot(s).unwrap();
    assert_eq!(result(&w, id), original);
    let mut draft = w.snapshot().clone();
    draft.road_snapshots.reverse();
    assert_eq!(result(&World::new(draft.clone()).unwrap(), id), original);
    let points = &mut draft.alignments[0].points;
    points.insert(
        1,
        GeoPoint {
            longitude: points[0].longitude,
            latitude: (points[0].latitude + points[1].latitude) / 2.,
        },
    );
    let sampled = result(&World::new(draft.clone()).unwrap(), id);
    assert_eq!(sampled.crossings.len(), 1);
    assert_eq!(sampled.crossings[0].road_id, original.crossings[0].road_id);
    assert_eq!(sampled.crossings[0].point, original.crossings[0].point);
    draft.alignments[0].points.reverse();
    let reverse = result(&World::new(draft).unwrap(), id);
    assert_eq!(reverse.crossings.len(), 1);
    assert_eq!(reverse.crossings[0].point, original.crossings[0].point);
    let before = w.snapshot().clone();
    let mut edited = before.clone();
    for p in &mut edited.alignments[0].points {
        p.longitude += 0.01;
    }
    w.replace(edited.clone()).unwrap();
    assert!(result(&w, id).crossings.is_empty());
    w.replace(before).unwrap();
    assert_eq!(result(&w, id), original);
    w.replace(edited).unwrap();
    assert!(result(&w, id).crossings.is_empty());
}
#[test]
fn altered_access_class_direction_height_and_unknown_structures_do_not_merge() {
    let (w, id) = fixture();
    for case in 0..7 {
        let mut draft = w.snapshot().clone();
        let road = &mut draft.road_snapshots[1].roads[0];
        match case {
            0 => road.classification.as_mut().unwrap().access = Some("private".into()),
            1 => road.classification.as_mut().unwrap().class = "service".into(),
            2 => road.classification.as_mut().unwrap().subclass = Some("alley".into()),
            3 => road.classification.as_mut().unwrap().oneway = Some("1".into()),
            4 => road.layer = Some("unknown".into()),
            5 => road.bridge = Some("yes".into()),
            _ => road.layer = Some("1".into()),
        }
        let checked = result(&World::new(draft.clone()).unwrap(), id);
        draft.road_snapshots[1].source_url = "https://example.org/other-supplier".into();
        assert_eq!(checked, result(&World::new(draft).unwrap(), id));
    }
    let mut different = w.snapshot().clone();
    different.road_snapshots[1].revision = "other".into();
    different.road_snapshots[1].source_url = different.road_snapshots[1]
        .source_url
        .replace("20261101_080013_pt", "other");
    assert!(World::new(different).is_err());
}
#[test]
fn parallel_and_multiline_competitors_keep_all_observations() {
    let (w, id) = fixture();
    for both in [false, true] {
        let mut draft = w.snapshot().clone();
        for (i, s) in draft.road_snapshots.iter_mut().enumerate() {
            if i == 1 && !both {
                continue;
            }
            let mut parallel = s.roads[0].clone();
            parallel.id = format!(
                "ofm:20261101_080013_pt:14:{}:6451:{}:12",
                14552 + i,
                if i == 0 { 471 } else { 836 }
            );
            for p in &mut parallel.points {
                p.latitude += 0.1 / 111320.;
            }
            s.roads.push(parallel);
        }
        assert_eq!(
            result(&World::new(draft).unwrap(), id).crossings.len(),
            if both { 4 } else { 3 }
        );
    }
}
#[test]
fn separate_neighboring_crossings_and_third_tile_do_not_form_transitive_clusters() {
    let (w, id) = fixture();
    let mut draft = w.snapshot().clone();
    for (i, s) in draft.road_snapshots.iter_mut().enumerate() {
        let mut other = s.roads[0].clone();
        other.id = format!("ofm:20261101_080013_pt:14:{}:6451:900:0", 14552 + i);
        for p in &mut other.points {
            p.latitude += 5. / 111320.;
        }
        s.roads.push(other);
    }
    let mut third = draft.road_snapshots[1].clone();
    third.source_url = third.source_url.replace("14553", "14554");
    third.source = third.source_url.clone();
    third.roads.clear();
    draft.road_snapshots.push(third);
    assert_eq!(result(&World::new(draft).unwrap(), id).crossings.len(), 2);
}

#[test]
fn adding_missing_tile_can_change_canonical_but_snapshot_undo_restores_exact_result() {
    let (complete, id) = fixture();
    let mut partial = complete.snapshot().clone();
    let left = partial.road_snapshots.remove(0);
    let mut w = World::new(partial.clone()).unwrap();
    let before = result(&w, id);
    assert_eq!(
        before.crossings[0].road_id,
        "ofm:20261101_080013_pt:14:14553:6451:836:2"
    );
    w.add_road_snapshot(left).unwrap();
    assert_eq!(result(&w, id), result(&complete, id));
    let after = w.snapshot().clone();
    w.replace(partial).unwrap();
    assert_eq!(result(&w, id), before);
    w.replace(after).unwrap();
    assert_eq!(result(&w, id), result(&complete, id));
}
#[test]
fn curved_rail_and_non_surface_rail_do_not_receive_boundary_merge() {
    let (w, id) = fixture();
    let mut d = w.snapshot().clone();
    let start = d.alignments[0].points[0];
    let end = d.alignments[0].points[1];
    let latitude = (35.68534327496442 + 35.6853489711398) / 2.;
    d.alignments[0].points = vec![
        start,
        GeoPoint {
            longitude: start.longitude,
            latitude: latitude - 0.25 / 111320.,
        },
        GeoPoint {
            longitude: start.longitude + 0.05 / (111320. * 35.685_f64.to_radians().cos()),
            latitude,
        },
        GeoPoint {
            longitude: start.longitude,
            latitude: latitude + 0.25 / 111320.,
        },
        end,
    ];
    assert_eq!(result(&World::new(d).unwrap(), id).crossings.len(), 2);
    let mut d = w.snapshot().clone();
    d.alignments[0].vertical_profile = Some(VerticalProfile {
        start_height_m: 6.,
        end_height_m: 6.,
        start_tangent_m: 0.,
        end_tangent_m: 0.,
    });
    assert!(result(&World::new(d).unwrap(), id).crossings.is_empty());
}
#[test]
fn two_separate_boundaries_in_three_tiles_keep_two_crossings() {
    let (w, first) = fixture();
    let mut d = w.snapshot().clone();
    let dx = 360. / 16384.;
    let mut middle_extra = d.road_snapshots[0].roads[0].clone();
    middle_extra.id = "ofm:20261101_080013_pt:14:14553:6451:900:0".into();
    for p in &mut middle_extra.points {
        p.longitude += dx;
    }
    d.road_snapshots[1].roads.push(middle_extra);
    let mut right = d.road_snapshots[1].clone();
    right.source_url = right.source_url.replace("14553", "14554");
    right.source = right.source_url.clone();
    right.roads.truncate(1);
    right.roads[0].id = "ofm:20261101_080013_pt:14:14554:6451:901:0".into();
    for p in &mut right.roads[0].points {
        p.longitude += dx;
    }
    for b in &mut right.road_coverage {
        b.west += dx;
        b.east += dx;
    }
    d.road_snapshots.push(right);
    let points = d.alignments[0]
        .points
        .iter()
        .map(|p| GeoPoint {
            longitude: p.longitude + dx,
            latitude: p.latitude,
        })
        .collect();
    let owner = d.companies[0].id;
    let mut w = World::new(d).unwrap();
    let second = w
        .build_alignment(owner, "Second seam".into(), points)
        .unwrap();
    let all = w.road_assessments();
    assert_eq!(all[&first].crossings.len(), 1);
    assert_eq!(all[&second].crossings.len(), 1);
    assert_eq!(
        all[&second].crossings[0].road_id,
        "ofm:20261101_080013_pt:14:14553:6451:900:0"
    );
}
#[test]
fn boundary_comparison_budget_never_returns_partial_merge_decisions() {
    let (w, id) = fixture();
    let mut d = w.snapshot().clone();
    for (i, s) in d.road_snapshots.iter_mut().enumerate() {
        let base = s.roads[0].clone();
        for n in 0..101 {
            let mut r = base.clone();
            r.id = format!(
                "ofm:20261101_080013_pt:14:{}:6451:{}:0",
                14552 + i,
                1000 + n
            );
            s.roads.push(r);
        }
    }
    // Identical observations still have distinct fragment identities. Exhausted
    // correspondence evidence must not turn them into a distance-only cluster.
    assert_eq!(result(&World::new(d).unwrap(), id).crossings.len(), 204);
}

#[test]
fn dense_vertices_exhaust_geometry_work_without_partial_boundary_evidence() {
    let (w, id) = fixture();
    let mut d = w.snapshot().clone();
    for (i, s) in d.road_snapshots.iter_mut().enumerate() {
        let base = s.roads[0].clone();
        // Concentrate 500 vertices inside the shared 20 m strip, still keeping
        // exactly the same source polyline. Ten line pairs must not cause a
        // quadratic amount of sample-to-edge testing per comparison.
        let (a, b) = if i == 0 {
            (base.points[1], base.points[2])
        } else {
            (base.points[0], base.points[1])
        };
        let mut points = vec![base.points[0]];
        let seam = 139.76806640625;
        let width = 9. / (111320. * 35.685_f64.to_radians().cos());
        for n in 0..500 {
            let x = seam - width + 2. * width * n as f64 / 499.;
            let t = (x - a.longitude) / (b.longitude - a.longitude);
            points.push(GeoPoint {
                longitude: x,
                latitude: a.latitude + t * (b.latitude - a.latitude),
            });
        }
        points.push(*base.points.last().unwrap());
        s.roads[0].points = points;
        for n in 0..2 {
            let mut duplicate = s.roads[0].clone();
            duplicate.id = format!(
                "ofm:20261101_080013_pt:14:{}:6451:{}:0",
                14552 + i,
                1100 + n
            );
            s.roads.push(duplicate);
        }
    }
    assert_eq!(result(&World::new(d).unwrap(), id).crossings.len(), 6);
}
