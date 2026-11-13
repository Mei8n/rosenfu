use rosenfu_core::world::*;

#[test]
fn typed_construction_and_roundtrip_preserve_all_three_classes() {
    let mut world = World::default();
    let player = world.add_player("P".into()).unwrap();
    let company = world.add_company(player, "C".into()).unwrap();
    let points = vec![
        GeoPoint {
            latitude: 35.,
            longitude: 139.,
        },
        GeoPoint {
            latitude: 35.,
            longitude: 139.01,
        },
    ];
    for kind in [TrackType::Normal, TrackType::HighSpeed, TrackType::Tram] {
        world
            .build_alignment_typed(company, "A".into(), points.clone(), kind)
            .unwrap();
        assert_eq!(world.snapshot().alignments.last().unwrap().track_type, kind);
    }
    world
        .build_alignment(company, "Legacy".into(), points)
        .unwrap();
    assert_eq!(
        world.snapshot().alignments.last().unwrap().track_type,
        TrackType::Normal
    );
    let encoded = serde_json::to_vec(world.snapshot()).unwrap();
    let restored: WorldDraft = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(World::new(restored).unwrap(), world);
}

#[test]
fn current_alignment_requires_known_type_and_rejects_missing_values() {
    let base = serde_json::json!({"id":"3","owner":"2","name":"A","points":[{"latitude":35.,"longitude":139.},{"latitude":35.,"longitude":139.01}],"track_type":"normal"});
    for value in ["normal", "high_speed", "tram"] {
        let mut record = base.clone();
        record["track_type"] = value.into();
        assert!(serde_json::from_value::<Alignment>(record).is_ok());
    }
    for value in [
        serde_json::Value::Null,
        serde_json::json!("unknown"),
        serde_json::json!(3),
    ] {
        let mut record = base.clone();
        record["track_type"] = value;
        assert!(serde_json::from_value::<Alignment>(record).is_err());
    }
    let mut missing = base;
    missing.as_object_mut().unwrap().remove("track_type");
    assert!(serde_json::from_value::<Alignment>(missing).is_err());
}
