use rosenfu_core::world::*;
use rosenfu_storage::{decode, encode, SCHEMA_VERSION};

#[test]
fn schema_eight_alignment_without_design_speed_loads_and_nine_roundtrips_cap() {
    let mut world = World::default();
    let p = world.add_player("P".into()).unwrap();
    let c = world.add_company(p, "C".into()).unwrap();
    let id = world
        .build_alignment_typed(
            c,
            "A".into(),
            vec![
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.0,
                },
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.01,
                },
            ],
            TrackType::HighSpeed,
        )
        .unwrap();
    let mut old: serde_json::Value = serde_json::from_slice(&encode(&world).unwrap()).unwrap();
    old["schema_version"] = 8.into();
    let mut restored = decode(&serde_json::to_vec(&old).unwrap()).unwrap();
    assert_eq!(
        restored.snapshot().alignments[0].effective_design_speed_kmh(),
        300.0
    );
    restored.set_alignment_design_speed(id, 240.0).unwrap();
    let encoded = encode(&restored).unwrap();
    assert_eq!(SCHEMA_VERSION, 9);
    assert_eq!(decode(&encoded).unwrap(), restored);
    old["world"]["alignments"][0]["design_speed_kmh"] = 0.into();
    assert!(decode(&serde_json::to_vec(&old).unwrap()).is_err());
}
