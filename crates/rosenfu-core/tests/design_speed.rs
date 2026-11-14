use rosenfu_core::world::*;

fn sample(kind: TrackType) -> (World, CompanyId, AlignmentId) {
    let mut world = World::default();
    let p = world.add_player("P".into()).unwrap();
    let c = world.add_company(p, "C".into()).unwrap();
    let a = world
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
            kind,
        )
        .unwrap();
    (world, c, a)
}

#[test]
fn legacy_defaults_and_explicit_speed_roundtrip() {
    for (kind, expected) in [
        (TrackType::Normal, 120.0),
        (TrackType::HighSpeed, 300.0),
        (TrackType::Tram, 50.0),
    ] {
        let (mut world, _, id) = sample(kind);
        assert_eq!(
            world.snapshot().alignments[0].effective_design_speed_kmh(),
            expected
        );
        let old = serde_json::to_value(world.snapshot()).unwrap();
        assert!(old["alignments"][0].get("design_speed_kmh").is_none());
        let restored: WorldDraft = serde_json::from_value(old).unwrap();
        assert_eq!(World::new(restored).unwrap(), world);
        world.set_alignment_design_speed(id, 90.0).unwrap();
        let restored: WorldDraft =
            serde_json::from_slice(&serde_json::to_vec(world.snapshot()).unwrap()).unwrap();
        assert_eq!(World::new(restored).unwrap(), world);
    }
}

#[test]
fn invalid_speed_is_atomic_and_extension_preserves_cap() {
    let (mut world, owner, id) = sample(TrackType::Normal);
    world.set_alignment_design_speed(id, 95.0).unwrap();
    for speed in [0.0, -1.0, 600.1, f64::NAN, f64::INFINITY] {
        let before = world.clone();
        assert!(world.set_alignment_design_speed(id, speed).is_err());
        assert_eq!(world, before);
    }
    world
        .extend_alignment(
            owner,
            id,
            AlignmentEndpoint::End,
            vec![
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.01,
                },
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.02,
                },
            ],
        )
        .unwrap();
    assert_eq!(world.snapshot().alignments[0].design_speed_kmh, Some(95.0));
}
