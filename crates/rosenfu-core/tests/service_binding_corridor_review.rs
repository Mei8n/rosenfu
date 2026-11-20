use rosenfu_core::world::*;
#[test]
fn connected_three_alignment_binding_preserves_order_and_reverse() {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let point = |x| GeoPoint {
        latitude: 35.,
        longitude: x,
    };
    let lines: Vec<_> = (0..3)
        .map(|i| {
            w.build_alignment(
                c,
                "L".into(),
                vec![
                    point(139. + i as f64 * 0.01),
                    point(139. + (i + 1) as f64 * 0.01),
                ],
            )
            .unwrap()
        })
        .collect();
    for pair in lines.windows(2) {
        w.connect_alignment_endpoints(
            c,
            AlignmentEndpointRef {
                alignment_id: pair[0],
                endpoint: AlignmentEndpoint::End,
            },
            AlignmentEndpointRef {
                alignment_id: pair[1],
                endpoint: AlignmentEndpoint::Start,
            },
        )
        .unwrap();
    }
    let a = w
        .place_station_on_alignment(c, "A".into(), lines[0], 0.2, 20., 10.)
        .unwrap();
    let b = w
        .place_station_on_alignment(c, "B".into(), lines[2], 0.8, 20., 10.)
        .unwrap();
    let candidate = w.route_candidates(a, b).unwrap().candidates.remove(0);
    let route = w
        .add_planned_route(c, "R".into(), a, candidate.segments, vec![a, b])
        .unwrap();
    for (from, to) in [(a, b), (b, a)] {
        let candidate = w.route_candidates(from, to).unwrap().candidates.remove(0);
        let preview = w
            .preview_service_path(from, candidate.segments.clone())
            .unwrap();
        let svc = w
            .add_service_path(
                c,
                "S".into(),
                from,
                candidate.segments,
                preview
                    .occurrences
                    .iter()
                    .map(|o| o.reference.clone())
                    .collect(),
            )
            .unwrap();
        w.set_service_route_binding(c, svc, ServiceRouteBindingKind::Route { route_id: route })
            .unwrap();
    }
    let before = w.snapshot().clone();
    let mut broken = before.clone();
    broken.alignment_connections.remove(0);
    assert!(w.replace(broken).is_err());
    assert_eq!(w.snapshot(), &before);
}
