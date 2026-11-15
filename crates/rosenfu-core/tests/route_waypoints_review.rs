use rosenfu_core::world::*;
fn setup(n: usize) -> (World, CompanyId, Vec<StationId>) {
    let mut w = World::default();
    let p = w.add_player("p".into()).unwrap();
    let c = w.add_company(p, "c".into()).unwrap();
    let s = (0..n)
        .map(|i| {
            w.add_station(c, format!("S{i}"), 35.0, 139.0 + i as f64 * 0.001)
                .unwrap()
        })
        .collect();
    (w, c, s)
}

#[test]
fn review_waypoint_backtracking_and_stable_repeated_queries() {
    let (mut w, c, s) = setup(7);
    for (a, b) in [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 6),
        (1, 4),
        (4, 2),
        (0, 5),
        (5, 3),
        (5, 2),
    ] {
        w.add_track(c, s[a], s[b]).unwrap();
    }
    let before = w.snapshot().clone();
    let all = w.route_candidates(s[0], s[6]).unwrap();
    assert!(!all.truncated);
    for via in [
        vec![s[2], s[3]],
        vec![s[4], s[2]],
        vec![s[3], s[2]],
        vec![s[1]],
        vec![],
    ] {
        let expected: Vec<_> = all
            .candidates
            .iter()
            .filter(|candidate| {
                let positions: Option<Vec<_>> = via
                    .iter()
                    .map(|v| candidate.stations.iter().position(|p| p == v))
                    .collect();
                positions.is_some_and(|p| p.windows(2).all(|x| x[0] < x[1]))
            })
            .cloned()
            .collect();
        for _ in 0..3 {
            let found = w.route_candidates_via(s[0], s[6], &via).unwrap();
            assert!(!found.truncated);
            assert_eq!(found.candidates, expected);
            assert_eq!(w.snapshot(), &before);
        }
    }
}
#[test]
fn review_budget_is_reported_incomplete_not_unique() {
    let (mut w, c, s) = setup(4);
    for _ in 0..40 {
        w.add_track(c, s[0], s[1]).unwrap();
    }
    w.add_track(c, s[1], s[2]).unwrap();
    w.add_track(c, s[2], s[3]).unwrap();
    let found = w.route_candidates_via(s[0], s[3], &[s[1], s[2]]).unwrap();
    assert_eq!(found.status, RouteSearchStatus::Incomplete);
    assert!(found.truncated);
    assert_eq!(found.candidates.len(), 32);
}
#[test]
fn review_station_group_does_not_create_physical_connection() {
    let (mut w, c, _) = setup(0);
    let p = |x: f64, y: f64| GeoPoint {
        latitude: 35.0 + y / 111000.,
        longitude: 139.0 + x / 91000.,
    };
    let h = w
        .build_alignment(c, "h".into(), vec![p(-500., 0.), p(500., 0.)])
        .unwrap();
    let v = w
        .build_alignment(c, "v".into(), vec![p(0., -500.), p(0., 500.)])
        .unwrap();
    let a = w
        .place_station_on_alignment(c, "A".into(), h, 0.2, 40., 12.)
        .unwrap();
    let b = w
        .place_station_on_alignment(c, "B".into(), h, 0.5, 100., 12.)
        .unwrap();
    let d = w
        .place_station_on_alignment(c, "D".into(), v, 0.5, 100., 12.)
        .unwrap();
    let e = w
        .place_station_on_alignment(c, "E".into(), v, 0.8, 40., 12.)
        .unwrap();
    assert_eq!(
        w.snapshot().station_group_id(b),
        w.snapshot().station_group_id(d)
    );
    assert_eq!(
        w.route_candidates_via(a, e, &[b, d]).unwrap().status,
        RouteSearchStatus::NoPath
    );
}
