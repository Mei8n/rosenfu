mod common;
use rosenfu_core::world::{Company, CompanyId, Player, PlayerId, World, WorldDraft};

#[test]
fn json_roundtrip_preserves_ids_and_ownership() {
    let source = WorldDraft {
        next_id: 43,
        players: vec![Player {
            id: PlayerId(42),
            name: "Player".into(),
        }],
        companies: vec![Company {
            id: CompanyId(7),
            owner: PlayerId(42),
            name: "Company".into(),
        }],
        ..WorldDraft::default()
    };
    let json = serde_json::to_string(&source).unwrap();
    let restored: WorldDraft = serde_json::from_str(&json).unwrap();
    assert_eq!(World::new(restored).unwrap().snapshot(), &source);
}

#[test]
fn deserialization_does_not_bypass_reference_validation() {
    let json = r#"{"next_id":"43","players":[],"companies":[{"id":"1","owner":"42","name":"orphan"}],"stations":[],"routes":[],"services":[]}"#;
    let restored: WorldDraft = serde_json::from_str(json).unwrap();
    assert!(World::new(restored).is_err());
}

#[test]
fn construction_graph_roundtrip_preserves_order_coordinates_and_endpoints() {
    let (mut world, company, alignment, stations) = common::line(3);
    world
        .add_planned_route(
            company,
            "Main".into(),
            stations[2],
            vec![common::segment(alignment, stations[2], stations[0])],
            stations.iter().rev().copied().collect(),
        )
        .unwrap();
    common::service(
        &mut world,
        company,
        stations[0],
        vec![common::segment(alignment, stations[0], stations[2])],
    );
    let source = world.snapshot();
    let restored = serde_json::from_str(&serde_json::to_string(source).unwrap()).unwrap();
    assert_eq!(World::new(restored).unwrap().snapshot(), source);
}

#[test]
fn legacy_track_payload_is_rejected_not_silently_migrated() {
    let mut old = serde_json::to_value(WorldDraft::default()).unwrap();
    old["tracks"] = serde_json::json!([]);
    assert!(serde_json::from_value::<WorldDraft>(old).is_err());
}
