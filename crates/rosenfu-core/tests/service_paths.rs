use rosenfu_core::world::*;
fn sample() -> (
    World,
    CompanyId,
    AlignmentId,
    Vec<StationId>,
    Vec<RouteSegment>,
) {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let a = w
        .build_alignment(
            c,
            "A".into(),
            vec![
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.0,
                },
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.1,
                },
            ],
        )
        .unwrap();
    let s = [0.1, 0.5, 0.9]
        .map(|position| {
            w.place_station_on_alignment(c, "S".into(), a, position, 100.0, 10.0)
                .unwrap()
        })
        .to_vec();
    let seg = vec![RouteSegment::Alignment {
        alignment_id: a,
        from: s[0],
        to: s[2],
    }];
    (w, c, a, s, seg)
}
fn all(w: &World, start: StationId, segments: Vec<RouteSegment>) -> Vec<StationOccurrenceRef> {
    w.preview_service_path(start, segments)
        .unwrap()
        .occurrences
        .into_iter()
        .map(|o| o.reference)
        .collect()
}
#[test]
fn inserting_station_does_not_retarget_or_add_stops() {
    let (mut w, c, a, s, segments) = sample();
    let stops = all(&w, s[0], segments.clone());
    let id = w
        .add_service_path(c, "S".into(), s[0], segments, stops.clone())
        .unwrap();
    let added = w
        .place_station_on_alignment(c, "New".into(), a, 0.3, 100.0, 10.0)
        .unwrap();
    assert_eq!(w.snapshot().service_paths[0].stops, stops);
    assert!(w.snapshot().services[0].tracks.is_empty());
    assert!(w.snapshot().services[0].stops.is_empty());
    assert_eq!(
        w.service_path_occurrences(id)
            .unwrap()
            .iter()
            .map(|o| o.station_id)
            .collect::<Vec<_>>(),
        vec![s[0], added, s[1], s[2]]
    );
    assert_eq!(World::new(w.snapshot().clone()).unwrap(), w);
}
#[test]
fn reverse_mixed_paths_and_revisits_have_unambiguous_boundary_references() {
    let (mut w, c, a, s, _) = sample();
    let x = w.add_station(c, "X".into(), 35.0, 139.2).unwrap();
    let t = w.add_track(c, x, s[2]).unwrap();
    let segments = vec![
        RouteSegment::Track { track_id: t },
        RouteSegment::Alignment {
            alignment_id: a,
            from: s[2],
            to: s[0],
        },
        RouteSegment::Alignment {
            alignment_id: a,
            from: s[0],
            to: s[2],
        },
    ];
    let preview = w.preview_service_path(x, segments.clone()).unwrap();
    assert_eq!(
        preview
            .occurrences
            .iter()
            .map(|o| o.station_id)
            .collect::<Vec<_>>(),
        vec![x, s[2], s[1], s[0], s[1], s[2]]
    );
    assert_eq!(
        preview.occurrences[1].reference,
        StationOccurrenceRef::OnLeg {
            leg_key: 1,
            station_id: s[2]
        }
    );
    assert_eq!(
        preview.occurrences[3].reference,
        StationOccurrenceRef::OnLeg {
            leg_key: 2,
            station_id: s[0]
        }
    );
    let stops = vec![
        StationOccurrenceRef::Start,
        preview.occurrences[4].reference.clone(),
        preview.occurrences[5].reference.clone(),
    ];
    w.add_service_path(c, "Loop".into(), x, segments, stops)
        .unwrap();
}
#[test]
fn invalid_stop_references_and_missing_endpoints_are_atomic() {
    let (mut w, c, _, s, segments) = sample();
    let valid = all(&w, s[0], segments.clone());
    let before = w.clone();
    for stops in [
        vec![],
        vec![valid[0].clone()],
        vec![valid[0].clone(), valid[1].clone()],
        vec![
            valid[0].clone(),
            valid[1].clone(),
            valid[1].clone(),
            valid[2].clone(),
        ],
        vec![
            valid[0].clone(),
            StationOccurrenceRef::OnLeg {
                leg_key: 999,
                station_id: s[1],
            },
            valid[2].clone(),
        ],
        vec![
            valid[0].clone(),
            StationOccurrenceRef::OnLeg {
                leg_key: 1,
                station_id: s[0],
            },
            valid[2].clone(),
        ],
    ] {
        assert!(w
            .add_service_path(c, "Bad".into(), s[0], segments.clone(), stops)
            .is_err());
        assert_eq!(w, before);
    }
}
#[test]
fn saved_path_changes_movement_and_referenced_deletion_are_rejected_atomically() {
    let (mut w, c, a, s, segments) = sample();
    let stops = all(&w, s[0], segments.clone());
    w.add_service_path(c, "S".into(), s[0], segments, stops)
        .unwrap();
    let before = w.clone();
    let mut draft = w.snapshot().clone();
    draft.service_paths[0].stops.remove(1);
    assert_eq!(
        w.replace(draft),
        Err(WorldError::ServicePathEditUnsupported)
    );
    let mut draft = w.snapshot().clone();
    draft.station_placements[1].position = 0.6;
    let point = draft.alignments[0].point_at(0.6);
    draft.stations[1].latitude = point.latitude;
    draft.stations[1].longitude = point.longitude;
    assert_eq!(
        w.replace(draft),
        Err(WorldError::ServicePathEditUnsupported)
    );
    let mut draft = w.snapshot().clone();
    draft.stations.retain(|x| x.id != s[1]);
    draft.station_placements.retain(|x| x.station_id != s[1]);
    assert!(w.replace(draft).is_err());
    let mut draft = w.snapshot().clone();
    draft.service_paths.clear();
    assert!(w.replace(draft).is_err());
    let mut draft = w.snapshot().clone();
    draft.service_paths.push(draft.service_paths[0].clone());
    assert!(w.replace(draft).is_err());
    let mut draft = w.snapshot().clone();
    draft.service_paths[0].next_leg_key = 1;
    assert!(w.replace(draft).is_err());
    assert_eq!(w, before);
    w.update_station_placement(s[1], 200.0, 10.0).unwrap();
    let mut draft = w.snapshot().clone();
    draft.services[0].name = "Rename".into();
    w.replace(draft).unwrap();
    assert_eq!(w.snapshot().alignments[0].id, a);
}
#[test]
fn dual_sources_and_geographic_crossing_are_not_connections() {
    let (mut w, c, _, s, segments) = sample();
    let stops = all(&w, s[0], segments.clone());
    w.add_service_path(c, "S".into(), s[0], segments, stops)
        .unwrap();
    let t = w.add_track(c, s[0], s[2]).unwrap();
    let before = w.clone();
    let mut draft = w.snapshot().clone();
    draft.services[0].tracks = vec![t];
    draft.services[0].stops = vec![0, 1];
    assert!(w.replace(draft).is_err());
    let x = w.add_station(c, "Nearby".into(), 35.0, 139.0).unwrap();
    assert!(w
        .preview_service_path(x, vec![RouteSegment::Track { track_id: t }])
        .is_err());
    assert_eq!(w.snapshot().service_paths, before.snapshot().service_paths);
}

#[test]
fn non_stopping_interior_station_deletion_is_also_protected() {
    let (mut w, c, _, s, segments) = sample();
    let all_stops = all(&w, s[0], segments.clone());
    w.add_service_path(
        c,
        "Express".into(),
        s[0],
        segments,
        vec![all_stops[0].clone(), all_stops[2].clone()],
    )
    .unwrap();
    let before = w.clone();
    let mut draft = w.snapshot().clone();
    draft.stations.retain(|station| station.id != s[1]);
    draft
        .station_placements
        .retain(|placement| placement.station_id != s[1]);
    assert_eq!(
        w.replace(draft),
        Err(WorldError::ServicePathEditUnsupported)
    );
    assert_eq!(w, before);
}

#[test]
fn geometry_changes_and_stop_reordering_are_atomic_but_route_changes_are_independent() {
    let (mut w, c, _, s, segments) = sample();
    let stops = all(&w, s[0], segments.clone());
    w.add_service_path(c, "S".into(), s[0], segments.clone(), stops)
        .unwrap();
    let route = w
        .add_planned_route(c, "R".into(), s[0], segments, s.clone())
        .unwrap();
    let before = w.clone();
    let mut draft = w.snapshot().clone();
    draft.alignments[0].points[1].longitude += 0.01;
    for placement in &draft.station_placements {
        let point = draft.alignments[0].point_at(placement.position);
        let station = draft
            .stations
            .iter_mut()
            .find(|station| station.id == placement.station_id)
            .unwrap();
        station.latitude = point.latitude;
        station.longitude = point.longitude;
    }
    assert_eq!(
        w.replace(draft),
        Err(WorldError::ServicePathEditUnsupported)
    );
    assert_eq!(w, before);
    let mut draft = w.snapshot().clone();
    draft.service_paths[0].stops.swap(1, 2);
    assert!(w.replace(draft).is_err());
    assert_eq!(w, before);
    let mut draft = w.snapshot().clone();
    draft.routes[0].name = "Renamed".into();
    w.replace(draft).unwrap();
    let mut draft = w.snapshot().clone();
    draft.routes.retain(|r| r.id != route);
    draft.route_paths.retain(|r| r.route_id != route);
    w.replace(draft).unwrap();
    assert_eq!(w.snapshot().service_paths, before.snapshot().service_paths);
    assert_eq!(w.snapshot().services, before.snapshot().services);
}
#[test]
fn referenced_track_endpoint_edits_are_rejected_even_if_traversal_is_equivalent() {
    let (mut w, c, _, s, _) = sample();
    let t = w.add_track(c, s[0], s[2]).unwrap();
    let segments = vec![RouteSegment::Track { track_id: t }];
    let stops = all(&w, s[0], segments.clone());
    w.add_service_path(c, "S".into(), s[0], segments, stops)
        .unwrap();
    let before = w.clone();
    let mut draft = w.snapshot().clone();
    draft.tracks[0].from = s[2];
    draft.tracks[0].to = s[0];
    assert_eq!(
        w.replace(draft),
        Err(WorldError::ServicePathEditUnsupported)
    );
    assert_eq!(w, before);
}

#[test]
fn extension_rejects_service_in_use_without_any_mutation() {
    let (mut w, c, a, stations, segments) = sample();
    let stops = all(&w, stations[0], segments.clone());
    w.add_service_path(c, "S".into(), stations[0], segments, stops)
        .unwrap();
    let before = w.snapshot().clone();
    for endpoint in [AlignmentEndpoint::Start, AlignmentEndpoint::End] {
        let old = &before.alignments[0];
        let anchor = if endpoint == AlignmentEndpoint::Start {
            old.points[0]
        } else {
            *old.points.last().unwrap()
        };
        let next = GeoPoint {
            latitude: anchor.latitude + 0.01,
            longitude: anchor.longitude,
        };
        assert_eq!(
            w.extend_alignment(c, a, endpoint, vec![anchor, next]),
            Err(WorldError::ServicePathEditUnsupported)
        );
        assert_eq!(w.snapshot(), &before);
    }
}
