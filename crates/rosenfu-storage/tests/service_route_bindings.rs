use rosenfu_core::world::*;
use rosenfu_storage::{decode, encode, SCHEMA_VERSION};
fn world() -> World {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let line = w
        .build_alignment(
            c,
            "Line".into(),
            vec![
                GeoPoint {
                    latitude: 35.,
                    longitude: 139.,
                },
                GeoPoint {
                    latitude: 35.,
                    longitude: 139.1,
                },
            ],
        )
        .unwrap();
    let a = w
        .place_station_on_alignment(c, "A".into(), line, 0.1, 20., 12.)
        .unwrap();
    let b = w
        .place_station_on_alignment(c, "B".into(), line, 0.9, 20., 12.)
        .unwrap();
    let segment = RouteSegment::Alignment {
        alignment_id: line,
        from: a,
        to: b,
    };
    w.add_planned_route(c, "R".into(), a, vec![segment.clone()], vec![a, b])
        .unwrap();
    let preview = w.preview_service_path(a, vec![segment.clone()]).unwrap();
    w.add_service_path(
        c,
        "S".into(),
        a,
        vec![segment],
        preview
            .occurrences
            .into_iter()
            .map(|o| o.reference)
            .collect(),
    )
    .unwrap();
    w
}
#[test]
fn current_schema_roundtrips_explicit_binding() {
    let mut w = world();
    let c = w.snapshot().companies[0].id;
    let s = w.snapshot().services[0].id;
    let r = w.snapshot().routes[0].id;
    w.set_service_route_binding(c, s, ServiceRouteBindingKind::Route { route_id: r })
        .unwrap();
    let encoded = encode(&w).unwrap();
    assert_eq!(SCHEMA_VERSION, 23);
    assert_eq!(decode(&encoded).unwrap(), w);
}
#[test]
fn current_schema_requires_explicit_bindings_array() {
    let mut value: serde_json::Value = serde_json::from_slice(&encode(&world()).unwrap()).unwrap();
    value["world"]
        .as_object_mut()
        .unwrap()
        .remove("service_route_bindings");
    assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn through_binding_roundtrip_and_invalid_duplicate_rejected() {
    let mut w = world();
    let c = w.snapshot().companies[0].id;
    let s = w.snapshot().services[0].id;
    let r = w.snapshot().routes[0].id;
    let start = w.snapshot().routes[0].start;
    let path = w.snapshot().route_paths[0].clone();
    let r2 = w
        .add_planned_route(c, "R2".into(), start, path.segments, path.member_stations)
        .unwrap();
    w.set_service_route_binding(
        c,
        s,
        ServiceRouteBindingKind::Routes {
            route_ids: vec![r, r2],
        },
    )
    .unwrap();
    let bytes = encode(&w).unwrap();
    assert_eq!(decode(&bytes).unwrap(), w);
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["world"]["service_route_bindings"][0]["binding"]["route_ids"][1] =
        serde_json::to_value(r).unwrap();
    assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
}
