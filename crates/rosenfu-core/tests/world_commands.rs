use rosenfu_core::world::{Player, PlayerId, World, WorldDraft, WorldError};

#[test]
fn allocation_survives_deletion_and_reload_without_reuse() {
    let mut world = World::default();
    let first = world.add_player("A".into()).unwrap();
    let mut deleted = world.snapshot().clone();
    deleted.players.clear();
    world.replace(deleted).unwrap();
    let json = serde_json::to_string(world.snapshot()).unwrap();
    let mut loaded = World::new(serde_json::from_str(&json).unwrap()).unwrap();
    let second = loaded.add_player("B".into()).unwrap();
    assert!(second.0 > first.0);
}

#[test]
fn failed_creation_does_not_consume_id_or_mutate_world() {
    let mut world = World::default();
    let before = world.clone();
    assert!(world.add_player(" ".into()).is_err());
    assert_eq!(world, before);
    assert_eq!(world.add_player("A".into()).unwrap(), PlayerId(1));
}

#[test]
fn construction_commands_generate_consistent_references() {
    let mut world = World::default();
    let player = world.add_player("A".into()).unwrap();
    let company = world.add_company(player, "Railway".into()).unwrap();
    let a = world.add_station(company, "A".into(), 35.0, 139.0).unwrap();
    let b = world.add_station(company, "B".into(), 35.1, 139.1).unwrap();
    let track = world.add_track(company, a, b).unwrap();
    let route = world
        .add_route(company, "Main".into(), a, vec![track])
        .unwrap();
    assert_eq!(route.0, 6);
    assert_eq!(world.snapshot().next_id, 7);
    assert!(world.snapshot().validate().is_ok());
}

#[test]
fn replacement_cannot_inject_ids_or_rewind_counter() {
    let mut world = World::default();
    let mut injected = world.snapshot().clone();
    injected.players.push(Player {
        id: PlayerId(1),
        name: "Forged".into(),
    });
    injected.next_id = 2;
    assert_eq!(world.replace(injected), Err(WorldError::InvalidCounter));
    let id = world.add_player("A".into()).unwrap();
    let mut deleted = world.snapshot().clone();
    deleted.players.clear();
    world.replace(deleted).unwrap();
    let mut reused = world.snapshot().clone();
    reused.players.push(Player {
        id,
        name: "Reused".into(),
    });
    assert_eq!(world.replace(reused), Err(WorldError::UnallocatedId));
}

#[test]
fn exhausted_counter_fails_without_wrapping() {
    let mut world = World::new(WorldDraft {
        next_id: u64::MAX,
        ..WorldDraft::default()
    })
    .unwrap();
    let before = world.clone();
    assert_eq!(world.add_player("A".into()), Err(WorldError::IdExhausted));
    assert_eq!(world, before);
}

#[test]
fn large_ids_are_exact_decimal_strings_on_the_wire() {
    let id = PlayerId(9_007_199_254_740_993);
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"9007199254740993\"");
    assert_eq!(serde_json::from_str::<PlayerId>(&json).unwrap(), id);
    for invalid in ["1", "\"0\"", "\"01\"", "\"-1\"", "\"18446744073709551616\""] {
        assert!(serde_json::from_str::<PlayerId>(invalid).is_err());
    }
}
