use rosenfu_core::world::*;
fn setup() -> (World, CompanyId, Vec<StationId>, Vec<TrackId>) {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let s: Vec<_> = (0..4)
        .map(|i| {
            w.add_station(c, format!("{i}"), 35.0, 139.0 + i as f64 * 0.01)
                .unwrap()
        })
        .collect();
    let t = s
        .windows(2)
        .map(|pair| w.add_track(c, pair[0], pair[1]).unwrap())
        .collect();
    (w, c, s, t)
}
#[test]
fn legacy_subset_reverse_and_independent_are_explicit_and_atomic() {
    let (mut w, c, s, t) = setup();
    let r = w.add_route(c, "R".into(), s[0], t.clone()).unwrap();
    let service = w
        .add_service(c, "S".into(), s[2], vec![t[1]], vec![0, 1])
        .unwrap();
    assert!(w.snapshot().service_route_bindings.is_empty());
    w.set_service_route_binding(c, service, ServiceRouteBindingKind::Route { route_id: r })
        .unwrap();
    let before = w.snapshot().clone();
    let mut incompatible = before.clone();
    incompatible.routes[0].tracks = vec![t[0]];
    assert_eq!(
        w.replace(incompatible).unwrap_err(),
        WorldError::InvalidServiceRouteBinding
    );
    assert_eq!(w.snapshot(), &before);
    w.set_service_route_binding(c, service, ServiceRouteBindingKind::Independent)
        .unwrap();
    let mut independent = w.snapshot().clone();
    independent.routes[0].tracks = vec![t[0]];
    w.replace(independent).unwrap();
    assert_eq!(w.snapshot().services, before.services);
}
#[test]
fn alignment_subspans_and_reverse_preserve_physical_order() {
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
    let s: Vec<_> = [0.1, 0.3, 0.6, 0.9]
        .into_iter()
        .map(|pos| {
            w.place_station_on_alignment(c, "S".into(), a, pos, 20.0, 10.0)
                .unwrap()
        })
        .collect();
    let seg = |from, to| RouteSegment::Alignment {
        alignment_id: a,
        from,
        to,
    };
    let r = w
        .add_planned_route(
            c,
            "R".into(),
            s[0],
            vec![seg(s[0], s[1]), seg(s[1], s[3])],
            s.clone(),
        )
        .unwrap();
    let legs = vec![seg(s[2], s[1])];
    let preview = w.preview_service_path(s[2], legs.clone()).unwrap();
    let svc = w
        .add_service_path(
            c,
            "S".into(),
            s[2],
            legs,
            preview
                .occurrences
                .iter()
                .map(|o| o.reference.clone())
                .collect(),
        )
        .unwrap();
    w.set_service_route_binding(c, svc, ServiceRouteBindingKind::Route { route_id: r })
        .unwrap();
    let before = w.snapshot().clone();
    let mut shorter = before.clone();
    shorter.route_paths[0].segments = vec![seg(s[0], s[1])];
    shorter.route_paths[0].member_stations = vec![s[0], s[1]];
    assert!(w.replace(shorter).is_err());
    assert_eq!(w.snapshot(), &before);
}
#[test]
fn duplicate_missing_and_wrong_owner_bindings_are_rejected() {
    let (mut w, c, s, t) = setup();
    let r = w.add_route(c, "R".into(), s[0], t.clone()).unwrap();
    let svc = w
        .add_service(c, "S".into(), s[0], vec![t[0]], vec![0, 1])
        .unwrap();
    assert!(w
        .set_service_route_binding(CompanyId(999), svc, ServiceRouteBindingKind::Independent)
        .is_err());
    assert!(w
        .set_service_route_binding(
            c,
            svc,
            ServiceRouteBindingKind::Route {
                route_id: RouteId(999)
            }
        )
        .is_err());
    w.set_service_route_binding(c, svc, ServiceRouteBindingKind::Route { route_id: r })
        .unwrap();
    let mut d = w.snapshot().clone();
    d.service_route_bindings
        .push(d.service_route_bindings[0].clone());
    assert!(World::new(d).is_err());
}

#[test]
fn repeated_visits_and_different_track_ids_are_not_collapsed() {
    let (mut w, c, s, t) = setup();
    let r = w.add_route(c, "R".into(), s[0], vec![t[0], t[1]]).unwrap();
    let looping = w
        .add_service(c, "Loop".into(), s[0], vec![t[0], t[0], t[0]], vec![0, 3])
        .unwrap();
    let before = w.snapshot().clone();
    assert!(w
        .set_service_route_binding(c, looping, ServiceRouteBindingKind::Route { route_id: r })
        .is_err());
    assert_eq!(w.snapshot(), &before);
    let parallel = w.add_track(c, s[0], s[1]).unwrap();
    let other = w
        .add_service(c, "Other".into(), s[0], vec![parallel], vec![0, 1])
        .unwrap();
    assert!(w
        .set_service_route_binding(c, other, ServiceRouteBindingKind::Route { route_id: r })
        .is_err());
}

#[test]
fn crossing_station_groups_do_not_make_routes_physically_interchangeable() {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let mut lines = vec![];
    for (dx, dy) in [(0.02, 0.0), (0.0, 0.02)] {
        let a = w
            .build_alignment(
                c,
                "A".into(),
                vec![
                    GeoPoint {
                        latitude: 35.0 - dy,
                        longitude: 139.0 - dx,
                    },
                    GeoPoint {
                        latitude: 35.0 + dy,
                        longitude: 139.0 + dx,
                    },
                ],
            )
            .unwrap();
        let s: Vec<_> = [0.1, 0.5, 0.9]
            .into_iter()
            .map(|pos| {
                w.place_station_on_alignment(c, "S".into(), a, pos, 100.0, 10.0)
                    .unwrap()
            })
            .collect();
        lines.push((a, s));
    }
    assert_eq!(w.snapshot().station_groups.len(), 1);
    let r = w
        .add_planned_route(
            c,
            "R".into(),
            lines[0].1[0],
            vec![RouteSegment::Alignment {
                alignment_id: lines[0].0,
                from: lines[0].1[0],
                to: lines[0].1[2],
            }],
            lines[0].1.clone(),
        )
        .unwrap();
    let segments = vec![RouteSegment::Alignment {
        alignment_id: lines[1].0,
        from: lines[1].1[0],
        to: lines[1].1[2],
    }];
    let preview = w
        .preview_service_path(lines[1].1[0], segments.clone())
        .unwrap();
    let svc = w
        .add_service_path(
            c,
            "S".into(),
            lines[1].1[0],
            segments,
            preview
                .occurrences
                .iter()
                .map(|o| o.reference.clone())
                .collect(),
        )
        .unwrap();
    let before = w.snapshot().clone();
    assert!(w
        .set_service_route_binding(c, svc, ServiceRouteBindingKind::Route { route_id: r })
        .is_err());
    assert_eq!(w.snapshot(), &before);
}
