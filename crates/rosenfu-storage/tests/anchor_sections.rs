use rosenfu_core::world::{GeoPoint, World};
use rosenfu_storage::{decode, encode, SCHEMA_VERSION};

#[test]
fn anchor_boundaries_roundtrip_and_invalid_boundaries_reject() {
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
    let mut draft = world.snapshot().clone();
    draft
        .alignments
        .iter_mut()
        .find(|a| a.id == id)
        .unwrap()
        .anchor_indices = Some(vec![0, 1, 2]);
    world.replace(draft).unwrap();
    let bytes = encode(&world).unwrap();
    assert_eq!(SCHEMA_VERSION, 22);
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
