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
