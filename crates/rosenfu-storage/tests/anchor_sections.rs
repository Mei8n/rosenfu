use rosenfu_core::world::{GeoPoint, World};
use rosenfu_storage::{decode, encode, SCHEMA_VERSION};

#[test]
fn schema_fourteen_without_boundaries_remains_readable_and_new_boundaries_roundtrip() {
    let mut world = World::default();
    let player = world.add_player("Player".into()).unwrap();
    let company = world.add_company(player, "Company".into()).unwrap();
    let id = world
        .build_alignment(
            company,
            "Line".into(),
            vec![
                GeoPoint {
                    latitude: 35.,
                    longitude: 139.,
                },
                GeoPoint {
                    latitude: 35.,
                    longitude: 139.01,
                },
                GeoPoint {
                    latitude: 35.,
                    longitude: 139.02,
                },
            ],
        )
        .unwrap();
    let mut legacy: serde_json::Value = serde_json::from_slice(&encode(&world).unwrap()).unwrap();
    legacy["schema_version"] = 14.into();
    legacy["world"]
        .as_object_mut()
        .unwrap()
        .remove("service_route_bindings");
    let restored = decode(&serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert_eq!(restored.snapshot().alignments[0].anchor_indices, None);
    let mut draft = world.snapshot().clone();
    draft
        .alignments
        .iter_mut()
        .find(|a| a.id == id)
        .unwrap()
        .anchor_indices = Some(vec![0, 1, 2]);
    world.replace(draft).unwrap();
    let bytes = encode(&world).unwrap();
    assert_eq!(SCHEMA_VERSION, 17);
    assert_eq!(decode(&bytes).unwrap().snapshot(), world.snapshot());
    for indices in [
        vec![],
        vec![0],
        vec![1, 2],
        vec![0, 0, 2],
        vec![0, 3, 2],
        vec![0, 1],
    ] {
        let before = world.snapshot().clone();
        let mut invalid = before.clone();
        invalid.alignments[0].anchor_indices = Some(indices);
        assert!(world.replace(invalid).is_err());
        assert_eq!(world.snapshot(), &before);
    }
}
