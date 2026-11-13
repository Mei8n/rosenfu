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
    let json = r#"{"next_id":"43","players":[],"companies":[{"id":"1","owner":"42","name":"orphan"}],"stations":[],"tracks":[],"routes":[],"services":[]}"#;
    let restored: WorldDraft = serde_json::from_str(json).unwrap();
    assert!(World::new(restored).is_err());
}

#[test]
fn construction_graph_roundtrip_preserves_order_coordinates_and_endpoints() {
    let json = r#"{
      "next_id":"9", "players":[{"id":"1","name":"Player"}],
      "companies":[{"id":"1","owner":"1","name":"Railway"}],
      "stations":[
        {"id":"1","owner":"1","name":"A","latitude":35.125,"longitude":139.25},
        {"id":"2","owner":"1","name":"B","latitude":35.5,"longitude":139.75},
        {"id":"3","owner":"1","name":"C","latitude":35.75,"longitude":139.875}
      ],
      "tracks":[{"id":"8","owner":"1","from":"1","to":"2"},{"id":"4","owner":"1","from":"3","to":"2"}],
      "routes":[{"id":"1","owner":"1","name":"Main","start":"1","tracks":["8","4"]}], "services":[]
    }"#;
    let source: WorldDraft = serde_json::from_str(json).unwrap();
    let restored = serde_json::from_str(&serde_json::to_string(&source).unwrap()).unwrap();
    assert_eq!(World::new(restored).unwrap().snapshot(), &source);
}
