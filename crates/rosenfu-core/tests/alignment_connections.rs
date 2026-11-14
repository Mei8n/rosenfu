use rosenfu_core::world::*;
fn point(x: f64) -> GeoPoint {
    GeoPoint {
        latitude: 35.,
        longitude: x,
    }
}
fn end(id: AlignmentId, endpoint: AlignmentEndpoint) -> AlignmentEndpointRef {
    AlignmentEndpointRef {
        alignment_id: id,
        endpoint,
    }
}
#[test]
fn all_four_orientations_preserve_ids_stations_and_speed_caps() {
    for ae in [AlignmentEndpoint::Start, AlignmentEndpoint::End] {
        for be in [AlignmentEndpoint::Start, AlignmentEndpoint::End] {
            let mut w = World::default();
            let p = w.add_player("P".into()).unwrap();
            let c = w.add_company(p, "C".into()).unwrap();
            let mut ap = vec![point(139.), point(139.1)];
            let mut bp = vec![point(139.1), point(139.2)];
            if ae == AlignmentEndpoint::Start {
                ap.reverse();
            }
            if be == AlignmentEndpoint::End {
                bp.reverse();
            }
            let a = w.build_alignment(c, "A".into(), ap).unwrap();
            let b = w.build_alignment(c, "B".into(), bp).unwrap();
            w.set_alignment_design_speed(a, 120.).unwrap();
            w.set_alignment_design_speed(b, 80.).unwrap();
            w.place_station_on_alignment(c, "S".into(), a, 0.5, 100., 10.)
                .unwrap();
            let before = w.snapshot().clone();
            w.connect_alignment_endpoints(c, end(a, ae), end(b, be))
                .unwrap();
            let after = w.snapshot().clone();
            assert_eq!(before.alignments, after.alignments);
            assert_eq!(before.station_placements, after.station_placements);
            assert_eq!(before.stations, after.stations);
            assert_eq!(before.next_id, after.next_id);
            assert_eq!(after.alignment_connections.len(), 1);
            assert!(w
                .connect_alignment_endpoints(c, end(b, be), end(a, ae))
                .is_err());
            assert_eq!(w.snapshot(), &after);
            let raw = serde_json::to_string(&after).unwrap();
            let restored: WorldDraft = serde_json::from_str(&raw).unwrap();
            assert_eq!(World::new(restored).unwrap().snapshot(), &after);
            // Extension cannot detach an occupied endpoint.
            let origin = point(139.1);
            let out = point(139.15);
            assert!(w.extend_alignment(c, a, ae, vec![origin, out]).is_err());
            assert_eq!(w.snapshot(), &after);
        }
    }
}
#[test]
fn gap_cusp_foreign_owner_and_self_are_atomic_errors() {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let other = w.add_company(p, "Other".into()).unwrap();
    let a = w
        .build_alignment(c, "A".into(), vec![point(139.), point(139.1)])
        .unwrap();
    let gap = w
        .build_alignment(c, "Gap".into(), vec![point(139.11), point(139.2)])
        .unwrap();
    let cusp = w
        .build_alignment(c, "Cusp".into(), vec![point(139.1), point(139.05)])
        .unwrap();
    let foreign = w
        .build_alignment(other, "Foreign".into(), vec![point(139.1), point(139.2)])
        .unwrap();
    let before = w.snapshot().clone();
    for b in [gap, cusp, foreign, a] {
        assert!(w
            .connect_alignment_endpoints(
                c,
                end(a, AlignmentEndpoint::End),
                end(b, AlignmentEndpoint::Start)
            )
            .is_err());
        assert_eq!(w.snapshot(), &before);
    }
}

#[test]
fn connected_corridor_crosses_empty_alignment_and_preserves_service_keys() {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let a = w
        .build_alignment(c, "A".into(), vec![point(139.), point(139.1)])
        .unwrap();
    let b = w
        .build_alignment(c, "Empty bridge".into(), vec![point(139.1), point(139.2)])
        .unwrap();
    let d = w
        .build_alignment(c, "D".into(), vec![point(139.2), point(139.3)])
        .unwrap();
    let s = w
        .place_station_on_alignment(c, "S".into(), a, 0.5, 100., 10.)
        .unwrap();
    let t = w
        .place_station_on_alignment(c, "T".into(), d, 0.5, 100., 10.)
        .unwrap();
    assert_eq!(
        w.route_candidates(s, t).unwrap().status,
        RouteSearchStatus::NoPath
    );
    w.connect_alignment_endpoints(
        c,
        end(a, AlignmentEndpoint::End),
        end(b, AlignmentEndpoint::Start),
    )
    .unwrap();
    w.connect_alignment_endpoints(
        c,
        end(b, AlignmentEndpoint::End),
        end(d, AlignmentEndpoint::Start),
    )
    .unwrap();
    let found = w.route_candidates(s, t).unwrap();
    assert_eq!(found.status, RouteSearchStatus::Unique);
    let segments = found.candidates[0].segments.clone();
    assert_eq!(segments.len(), 1);
    assert!(
        matches!(&segments[0],RouteSegment::ConnectedAlignments{connections,..} if connections.len()==2)
    );
    assert_eq!(
        w.route_candidates(t, s).unwrap().status,
        RouteSearchStatus::Unique
    );
    let route = w
        .add_planned_route(c, "Through".into(), s, segments.clone(), vec![s, t])
        .unwrap();
    let preview = w.preview_service_path(s, segments.clone()).unwrap();
    let service = w
        .add_service_path(
            c,
            "Train".into(),
            s,
            segments,
            preview
                .occurrences
                .iter()
                .map(|o| o.reference.clone())
                .collect(),
        )
        .unwrap();
    assert_eq!(w.route_path_stations(route).unwrap(), vec![s, t]);
    let old = w.snapshot().clone();
    w.extend_alignment(
        c,
        d,
        AlignmentEndpoint::End,
        vec![point(139.3), point(139.4)],
    )
    .unwrap();
    assert_eq!(old.route_paths, w.snapshot().route_paths);
    assert_eq!(old.service_paths, w.snapshot().service_paths);
    assert_eq!(
        w.service_path_occurrences(service)
            .unwrap()
            .iter()
            .map(|o| o.station_id)
            .collect::<Vec<_>>(),
        vec![s, t]
    );
    // Removing a required connection invalidates the saved route, atomically.
    let before = w.snapshot().clone();
    let mut invalid = before.clone();
    invalid.alignment_connections.pop();
    assert!(World::new(invalid).is_err());
    assert_eq!(w.snapshot(), &before);
}

#[test]
fn corridor_preserves_added_intermediate_stations_but_rejects_fake_links() {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let a = w
        .build_alignment(c, "A".into(), vec![point(139.), point(139.1)])
        .unwrap();
    let b = w
        .build_alignment(c, "B".into(), vec![point(139.1), point(139.2)])
        .unwrap();
    let s = w
        .place_station_on_alignment(c, "S".into(), a, 0.2, 100., 10.)
        .unwrap();
    let t = w
        .place_station_on_alignment(c, "T".into(), b, 0.8, 100., 10.)
        .unwrap();
    let link = AlignmentConnection {
        a: end(a, AlignmentEndpoint::End),
        b: end(b, AlignmentEndpoint::Start),
    };
    let segment = RouteSegment::ConnectedAlignments {
        from: s,
        to: t,
        connections: vec![link.clone()],
    };
    assert!(w.preview_service_path(s, vec![segment.clone()]).is_err());
    w.connect_alignment_endpoints(c, link.a, link.b).unwrap();
    let route = w
        .add_planned_route(c, "Through".into(), s, vec![segment], vec![s, t])
        .unwrap();
    let mid = w
        .place_station_on_alignment(c, "Mid".into(), a, 0.8, 100., 10.)
        .unwrap();
    assert_eq!(w.route_path_stations(route).unwrap(), vec![s, mid, t]);
}
