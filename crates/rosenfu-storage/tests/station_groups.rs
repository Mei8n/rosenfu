use rosenfu_core::world::*;
use rosenfu_storage::{decode, encode, SCHEMA_VERSION};
#[test]
fn station_groups_roundtrip() {
    let mut world = World::default();
    let p = world.add_player("P".into()).unwrap();
    let c = world.add_company(p, "C".into()).unwrap();
    for (lat, lon) in [(0., 0.01), (0.01, 0.)] {
        let id = world
            .build_alignment(
                c,
                "線路".into(),
                vec![
                    GeoPoint {
                        latitude: 35. - lat,
                        longitude: 139. - lon,
                    },
                    GeoPoint {
                        latitude: 35. + lat,
                        longitude: 139. + lon,
                    },
                ],
            )
            .unwrap();
        world
            .place_station_on_alignment(c, "駅".into(), id, 0.5, 100., 12.)
            .unwrap();
    }
    assert_eq!(world.snapshot().station_groups.len(), 1);
    assert_eq!(SCHEMA_VERSION, 19);
    let encoded = encode(&world).unwrap();
    let restored = decode(&encoded).unwrap();
    assert_eq!(
        restored.snapshot().station_groups,
        world.snapshot().station_groups
    );
}
