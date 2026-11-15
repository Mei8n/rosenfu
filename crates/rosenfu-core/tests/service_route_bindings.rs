mod common;
use common::*;
use rosenfu_core::world::*;
fn setup() -> (World, CompanyId, Vec<StationId>, Vec<RouteSegment>) {
    let (w, c, a, s) = line(4);
    let t = s.windows(2).map(|p| segment(a, p[0], p[1])).collect();
    (w, c, s, t)
}
fn route(w: &mut World, c: CompanyId, start: StationId, t: Vec<RouteSegment>) -> RouteId {
    let members = w
        .preview_route_membership(start, t.clone(), vec![])
        .unwrap()
        .member_stations;
    w.add_planned_route(c, "R".into(), start, t, members)
        .unwrap()
}
#[test]
fn path_subset_reverse_and_independent_are_explicit_and_atomic() {
    let (mut w, c, s, t) = setup();
    let r = route(&mut w, c, s[0], t.clone());
    let a = w.snapshot().alignments[0].id;
    let service = service(&mut w, c, s[2], vec![segment(a, s[2], s[1])]);
    assert!(w.snapshot().service_route_bindings.is_empty());
    w.set_service_route_binding(c, service, ServiceRouteBindingKind::Route { route_id: r })
        .unwrap();
    let before = w.snapshot().clone();
    let mut incompatible = before.clone();
    incompatible.route_paths[0].segments = vec![t[0].clone()];
    incompatible.route_paths[0].member_stations = s[..2].to_vec();
    assert_eq!(
        w.replace(incompatible).unwrap_err(),
        WorldError::InvalidServiceRouteBinding
    );
    assert_eq!(w.snapshot(), &before);
    w.set_service_route_binding(c, service, ServiceRouteBindingKind::Independent)
        .unwrap();
    let mut independent = w.snapshot().clone();
    independent.route_paths[0].segments = vec![t[0].clone()];
    independent.route_paths[0].member_stations = s[..2].to_vec();
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
    let r = route(&mut w, c, s[0], t.clone());
    let svc = service(&mut w, c, s[0], vec![t[0].clone()]);
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
fn repeated_visits_and_different_alignment_ids_are_not_collapsed() {
    let (mut w, c, s, t) = setup();
    let r = route(&mut w, c, s[0], vec![t[0].clone(), t[1].clone()]);
    let a = w.snapshot().alignments[0].id;
    let looping = service(
        &mut w,
        c,
        s[0],
        vec![t[0].clone(), segment(a, s[1], s[0]), t[0].clone()],
    );
    let before = w.snapshot().clone();
    assert!(w
        .set_service_route_binding(c, looping, ServiceRouteBindingKind::Route { route_id: r })
        .is_err());
    assert_eq!(w.snapshot(), &before);
    let parallel = w
        .build_alignment(
            c,
            "Other".into(),
            vec![
                GeoPoint {
                    latitude: 36.,
                    longitude: 139.,
                },
                GeoPoint {
                    latitude: 36.,
                    longitude: 139.1,
                },
            ],
        )
        .unwrap();
    let p = w
        .place_station_on_alignment(c, "P".into(), parallel, 0.1, 20., 10.)
        .unwrap();
    let q = w
        .place_station_on_alignment(c, "Q".into(), parallel, 0.9, 20., 10.)
        .unwrap();
    let other = service(&mut w, c, p, vec![segment(parallel, p, q)]);
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

#[test]
fn through_routes_cover_physical_path_and_protect_each_dependency() {
    let (mut w, c, s, t) = setup();
    let r1 = route(&mut w, c, s[0], vec![t[0].clone()]);
    let r2 = route(&mut w, c, s[1], t[1..].to_vec());
    let svc = service(&mut w, c, s[0], t.clone());
    let original = w.snapshot().service_paths.clone();
    w.set_service_route_binding(
        c,
        svc,
        ServiceRouteBindingKind::Routes {
            route_ids: vec![r1, r2],
        },
    )
    .unwrap();
    assert_eq!(w.snapshot().service_paths, original);
    let before = w.snapshot().clone();
    let mut changed = before.clone();
    let path = changed
        .route_paths
        .iter_mut()
        .find(|p| p.route_id == r2)
        .unwrap();
    path.segments = vec![t[1].clone()];
    path.member_stations = s[1..3].to_vec();
    assert!(w.replace(changed).is_err());
    assert_eq!(w.snapshot(), &before);
    let a = w.snapshot().alignments[0].id;
    let reverse = service(&mut w, c, s[3], vec![segment(a, s[3], s[0])]);
    w.set_service_route_binding(
        c,
        reverse,
        ServiceRouteBindingKind::Routes {
            route_ids: vec![r2, r1],
        },
    )
    .unwrap();
}

#[test]
fn through_routes_reject_gaps_duplicate_or_unused_routes_atomically() {
    let (mut w, c, s, t) = setup();
    let r1 = route(&mut w, c, s[0], vec![t[0].clone()]);
    let r2 = route(&mut w, c, s[2], vec![t[2].clone()]);
    let svc = service(&mut w, c, s[0], t.clone());
    for ids in [vec![r1, r2], vec![r1, r1], vec![r1]] {
        let before = w.snapshot().clone();
        assert!(w
            .set_service_route_binding(c, svc, ServiceRouteBindingKind::Routes { route_ids: ids })
            .is_err());
        assert_eq!(w.snapshot(), &before);
    }
    let short = service(&mut w, c, s[0], vec![t[0].clone()]);
    let before = w.snapshot().clone();
    assert!(w
        .set_service_route_binding(
            c,
            short,
            ServiceRouteBindingKind::Routes {
                route_ids: vec![r1, r2]
            }
        )
        .is_err());
    assert_eq!(w.snapshot(), &before);
}
