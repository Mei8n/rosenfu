use rosenfu_core::world::{GeoPoint, World};
use rosenfu_storage::{decode, encode};
#[test]
fn curved_geometry_and_platform_resize_roundtrip() {
    let mut world = World::default();
    let p = world.add_player("P".into()).unwrap();
    let c = world.add_company(p, "C".into()).unwrap();
    let a = world
        .build_alignment(
            c,
            "First line".into(),
            vec![
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.0,
                },
                GeoPoint {
                    latitude: 35.005,
                    longitude: 139.001,
                },
                GeoPoint {
                    latitude: 35.01,
                    longitude: 139.0,
                },
            ],
        )
        .unwrap();
    let s = world
        .place_station_on_alignment(c, "Station".into(), a, 0.0, 100.0, 20.0)
        .unwrap();
    world.update_station_placement(s, 220.0, 24.0).unwrap();
    assert_eq!(world.snapshot().station_placements[0].length_m, 220.0);
    assert_eq!(decode(&encode(&world).unwrap()).unwrap(), world);
    let before = world.clone();
    assert!(world.update_station_placement(s, f64::NAN, 24.0).is_err());
    assert_eq!(world, before);
}
