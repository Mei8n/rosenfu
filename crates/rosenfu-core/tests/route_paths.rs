use rosenfu_core::world::*;
fn sample() -> (World, CompanyId, AlignmentId, Vec<StationId>) {
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
    (w, c, a, s)
}
fn segment(a: AlignmentId, from: StationId, to: StationId) -> RouteSegment {
    RouteSegment::Alignment {
        alignment_id: a,
        from,
        to,
    }
}
#[test]
fn physical_path_and_membership_are_independent_and_atomic() {
    let (mut w, c, a, s) = sample();
    let tracks = w.snapshot().tracks.clone();
    let route = w
        .add_planned_route(
            c,
            "Route".into(),
            s[0],
            vec![segment(a, s[0], s[2])],
            vec![s[0], s[2]],
        )
        .unwrap();
    assert_eq!(w.route_path_stations(route).unwrap(), s);
    assert_eq!(
        w.snapshot().route_paths[0].member_stations,
        vec![s[0], s[2]]
    );
    assert_eq!(w.snapshot().tracks, tracks);
    assert!(w.snapshot().services.is_empty());
    let before = w.clone();
    for members in [
        vec![],
        vec![s[0]],
        vec![s[0], s[2], s[1]],
        vec![s[0], s[1], s[1], s[2]],
        vec![s[0], StationId(999), s[2]],
    ] {
        assert!(w
            .add_planned_route(c, "Bad".into(), s[0], vec![segment(a, s[0], s[2])], members)
            .is_err());
        assert_eq!(w, before);
    }
}
#[test]
fn adding_interior_station_preserves_path_and_does_not_add_membership() {
    let (mut w, c, a, s) = sample();
    let route = w
        .add_planned_route(c, "R".into(), s[0], vec![segment(a, s[0], s[2])], s.clone())
        .unwrap();
    let added = w
        .place_station_on_alignment(c, "New".into(), a, 0.3, 100.0, 10.0)
        .unwrap();
    assert_eq!(
        w.route_path_stations(route).unwrap(),
        vec![s[0], added, s[1], s[2]]
    );
    assert_eq!(w.snapshot().route_paths[0].member_stations, s);
    let preview = w
        .preview_route_membership(s[0], vec![segment(a, s[0], s[2])], vec![added])
        .unwrap();
    assert_eq!(preview.member_stations, s);
}
#[test]
fn invalid_references_duplicate_sidecars_and_dual_sources_are_rejected() {
    let (mut w, c, a, s) = sample();
    let track = w.add_track(c, s[0], s[1]).unwrap();
    w.add_planned_route(c, "R".into(), s[0], vec![segment(a, s[0], s[2])], s.clone())
        .unwrap();
    let before = w.clone();
    let mut bad = w.snapshot().clone();
    bad.route_paths.push(bad.route_paths[0].clone());
    assert_eq!(w.replace(bad), Err(WorldError::InvalidRoutePath));
    let mut bad = w.snapshot().clone();
    bad.route_paths[0].route_id = RouteId(999);
    assert_eq!(w.replace(bad), Err(WorldError::InvalidRoutePath));
    let mut bad = w.snapshot().clone();
    bad.routes[0].tracks = vec![track];
    assert_eq!(w.replace(bad), Err(WorldError::InvalidRoutePath));
    let mut bad = w.snapshot().clone();
    bad.route_paths.clear();
    assert_eq!(w.replace(bad), Err(WorldError::EmptyRoute));
    let mut bad = w.snapshot().clone();
    bad.station_placements.retain(|p| p.station_id != s[0]);
    assert_eq!(w.replace(bad), Err(WorldError::InvalidRoutePath));
    let mut bad = w.snapshot().clone();
    bad.routes.clear();
    assert_eq!(w.replace(bad), Err(WorldError::InvalidRoutePath));
    assert_eq!(w, before);
}
#[test]
fn mixed_path_reverse_order_and_service_independence() {
    let (mut w, c, a, s) = sample();
    let d = w.add_station(c, "D".into(), 35.0, 139.2).unwrap();
    let t = w.add_track(c, s[2], d).unwrap();
    w.add_service(c, "Independent".into(), s[2], vec![t], vec![0, 1])
        .unwrap();
    let services = w.snapshot().services.clone();
    let r = w
        .add_planned_route(
            c,
            "R".into(),
            d,
            vec![RouteSegment::Track { track_id: t }, segment(a, s[2], s[0])],
            vec![d, s[2], s[0]],
        )
        .unwrap();
    assert_eq!(w.route_path_stations(r).unwrap(), vec![d, s[2], s[1], s[0]]);
    let mut edit = w.snapshot().clone();
    edit.routes[0].name = "Renamed".into();
    w.replace(edit).unwrap();
    assert_eq!(w.snapshot().services, services);
}
#[test]
fn malformed_or_repeated_paths_and_empty_path_do_not_allocate_ids() {
    let (mut w, c, a, s) = sample();
    let before = w.clone();
    for segments in [
        vec![],
        vec![segment(AlignmentId(999), s[0], s[2])],
        vec![segment(a, s[1], s[2])],
        vec![segment(a, s[0], s[2]), segment(a, s[2], s[0])],
        vec![RouteSegment::Track {
            track_id: TrackId(999),
        }],
    ] {
        assert!(w
            .add_planned_route(c, "R".into(), s[0], segments, vec![s[0], s[2]])
            .is_err());
        assert_eq!(w, before);
    }
}
