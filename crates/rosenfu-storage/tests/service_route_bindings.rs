use rosenfu_core::world::*;
use rosenfu_storage::{decode, encode, SCHEMA_VERSION};
fn world() -> World {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let a = w.add_station(c, "A".into(), 35.0, 139.0).unwrap();
    let b = w.add_station(c, "B".into(), 35.0, 139.1).unwrap();
    let t = w.add_track(c, a, b).unwrap();
    w.add_route(c, "R".into(), a, vec![t]).unwrap();
    w.add_service(c, "S".into(), a, vec![t], vec![0, 1])
        .unwrap();
    w
}
#[test]
fn schema16_loads_as_unknown_and_schema17_roundtrips_explicit_binding() {
    let mut w = world();
    let mut old: serde_json::Value = serde_json::from_slice(&encode(&w).unwrap()).unwrap();
    old["schema_version"] = 16.into();
    old["world"]
        .as_object_mut()
        .unwrap()
        .remove("service_route_bindings");
    let bytes = serde_json::to_vec(&old).unwrap();
    let restored = decode(&bytes).unwrap();
    assert!(restored.snapshot().service_route_bindings.is_empty());
    assert_eq!(restored.snapshot().services, w.snapshot().services);
    let c = w.snapshot().companies[0].id;
    let s = w.snapshot().services[0].id;
    let r = w.snapshot().routes[0].id;
    w.set_service_route_binding(c, s, ServiceRouteBindingKind::Route { route_id: r })
        .unwrap();
    let encoded = encode(&w).unwrap();
    assert_eq!(SCHEMA_VERSION, 17);
    assert_eq!(decode(&encoded).unwrap(), w);
    // Loading does not write or mutate the original document.
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
        old
    );
}
#[test]
fn schema17_requires_bindings_and_older_schema_cannot_smuggle_them() {
    let mut value: serde_json::Value = serde_json::from_slice(&encode(&world()).unwrap()).unwrap();
    value["schema_version"] = 16.into();
    assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
    value["schema_version"] = 17.into();
    value["world"]
        .as_object_mut()
        .unwrap()
        .remove("service_route_bindings");
    assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
}
