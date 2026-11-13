use rosenfu_core::world::{
    CompanyId, ServiceId, StationId, TrackId, World, WorldDraft, WorldError,
};

struct Fixture {
    world: World,
    company: CompanyId,
    a: StationId,
    b: StationId,
    c: StationId,
    ab: TrackId,
    bc: TrackId,
}
fn fixture() -> Fixture {
    let mut world = World::default();
    let player = world.add_player("Player".into()).unwrap();
    let company = world.add_company(player, "Railway".into()).unwrap();
    let a = world.add_station(company, "A".into(), 35.0, 139.0).unwrap();
    let b = world.add_station(company, "B".into(), 35.1, 139.1).unwrap();
    let c = world.add_station(company, "C".into(), 35.2, 139.2).unwrap();
    let ab = world.add_track(company, a, b).unwrap();
    let bc = world.add_track(company, c, b).unwrap();
    Fixture {
        world,
        company,
        a,
        b,
        c,
        ab,
        bc,
    }
}

#[test]
fn service_traverses_both_track_directions_and_can_skip_intermediate_stations() {
    let mut f = fixture();
    let next = f.world.snapshot().next_id;
    let id = f
        .world
        .add_service(
            f.company,
            "Express".into(),
            f.a,
            vec![f.ab, f.bc],
            vec![0, 2],
        )
        .unwrap();
    assert_eq!(id, ServiceId(next));
    assert_eq!(f.world.snapshot().next_id, next + 1);
    let service = &f.world.snapshot().services[0];
    assert_eq!(service.tracks, vec![f.ab, f.bc]);
    assert_eq!(service.stops, vec![0, 2]);
    f.world
        .add_service(
            f.company,
            "Reverse".into(),
            f.c,
            vec![f.bc, f.ab],
            vec![0, 1, 2],
        )
        .unwrap();
}

#[test]
fn repeated_station_occurrences_have_independent_stop_indices() {
    let mut f = fixture();
    // A -> B -> A -> B -> C; the first B is passed, the second B is served.
    f.world
        .add_service(
            f.company,
            "Loop".into(),
            f.a,
            vec![f.ab, f.ab, f.ab, f.bc],
            vec![0, 3, 4],
        )
        .unwrap();
    assert_eq!(f.world.snapshot().services[0].stops, vec![0, 3, 4]);
}

#[test]
fn invalid_stop_positions_never_mutate_or_allocate() {
    let mut f = fixture();
    let before = f.world.clone();
    for stops in [
        vec![],
        vec![0],
        vec![2],
        vec![1, 2],
        vec![0, 1],
        vec![0, 2, 1, 2],
        vec![0, 1, 1, 2],
        vec![0, 3, 2],
        vec![0, u32::MAX],
    ] {
        assert_eq!(
            f.world
                .add_service(f.company, "Invalid".into(), f.a, vec![f.ab, f.bc], stops),
            Err(WorldError::InvalidServiceStops)
        );
        assert_eq!(f.world, before);
    }
}

#[test]
fn missing_references_and_disconnected_or_empty_paths_never_mutate() {
    let mut f = fixture();
    let before = f.world.clone();
    for (owner, start, tracks, stops, expected) in [
        (
            CompanyId(999),
            f.a,
            vec![f.ab],
            vec![0, 1],
            WorldError::MissingReference("service.owner"),
        ),
        (
            f.company,
            StationId(999),
            vec![f.ab],
            vec![0, 1],
            WorldError::MissingReference("service.start"),
        ),
        (
            f.company,
            f.a,
            vec![TrackId(999)],
            vec![0, 1],
            WorldError::MissingReference("service.track"),
        ),
        (
            f.company,
            f.a,
            vec![f.bc],
            vec![0, 1],
            WorldError::DisconnectedService,
        ),
        (f.company, f.a, vec![], vec![0], WorldError::EmptyService),
    ] {
        assert_eq!(
            f.world
                .add_service(owner, "Invalid".into(), start, tracks, stops),
            Err(expected)
        );
        assert_eq!(f.world, before);
    }
    assert_eq!(
        f.world
            .add_service(f.company, " ".into(), f.a, vec![f.ab], vec![0, 1]),
        Err(WorldError::EmptyName("service"))
    );
    assert_eq!(f.world, before);
}

#[test]
fn route_changes_or_deletion_do_not_change_service_or_equipment() {
    let mut f = fixture();
    f.world
        .add_route(f.company, "Route".into(), f.a, vec![f.ab, f.bc])
        .unwrap();
    f.world
        .add_service(
            f.company,
            "Service".into(),
            f.a,
            vec![f.ab, f.bc],
            vec![0, 2],
        )
        .unwrap();
    let before = f.world.snapshot().clone();
    let mut changed = before.clone();
    changed.routes[0].name = "Changed".into();
    changed.routes[0].start = f.c;
    changed.routes[0].tracks = vec![f.bc];
    f.world.replace(changed).unwrap();
    assert_eq!(f.world.snapshot().services, before.services);
    assert_eq!(f.world.snapshot().tracks, before.tracks);
    let mut deleted = f.world.snapshot().clone();
    deleted.routes.clear();
    f.world.replace(deleted).unwrap();
    assert_eq!(f.world.snapshot().services, before.services);
}

#[test]
fn service_references_protect_shared_equipment_and_edit_failures_are_atomic() {
    let mut f = fixture();
    f.world
        .add_service(
            f.company,
            "Service".into(),
            f.a,
            vec![f.ab, f.bc],
            vec![0, 2],
        )
        .unwrap();
    let before = f.world.clone();
    let mut missing = f.world.snapshot().clone();
    missing.tracks.remove(0);
    assert_eq!(
        f.world.replace(missing),
        Err(WorldError::MissingReference("service.track"))
    );
    assert_eq!(f.world, before);
    let mut invalid = f.world.snapshot().clone();
    invalid.services[0].stops = vec![0, 1];
    assert_eq!(
        f.world.replace(invalid),
        Err(WorldError::InvalidServiceStops)
    );
    assert_eq!(f.world, before);
    let mut valid = f.world.snapshot().clone();
    valid.services[0].stops = vec![0, 1, 2];
    f.world.replace(valid).unwrap();
    assert_eq!(
        f.world.snapshot().services[0].id,
        before.snapshot().services[0].id
    );
    assert_eq!(f.world.snapshot().next_id, before.snapshot().next_id);
}

#[test]
fn ids_counters_and_existing_only_replacement_include_services() {
    let mut f = fixture();
    f.world
        .add_service(f.company, "Service".into(), f.a, vec![f.ab], vec![0, 1])
        .unwrap();
    let before = f.world.snapshot().clone();
    let mut invalid = before.clone();
    invalid.services[0].id = ServiceId(0);
    assert_eq!(invalid.validate(), Err(WorldError::InvalidId("service")));
    invalid = before.clone();
    invalid.services.push(invalid.services[0].clone());
    assert_eq!(invalid.validate(), Err(WorldError::InvalidId("service")));
    invalid = before.clone();
    invalid.next_id = invalid.services[0].id.0;
    assert_eq!(invalid.validate(), Err(WorldError::InvalidCounter));
    let mut removed = before.clone();
    removed.services.clear();
    f.world.replace(removed).unwrap();
    assert_eq!(
        f.world.replace(before.clone()),
        Err(WorldError::UnallocatedId)
    );
    let id = f
        .world
        .add_service(f.company, "Next".into(), f.b, vec![f.ab], vec![0, 1])
        .unwrap();
    assert_eq!(id.0, before.next_id);
}

#[test]
fn service_owner_validation_is_not_a_track_usage_authorization_policy() {
    let mut f = fixture();
    let player = f.world.snapshot().players[0].id;
    let other_company = f.world.add_company(player, "Other".into()).unwrap();
    f.world
        .add_service(other_company, "Shared".into(), f.a, vec![f.ab], vec![0, 1])
        .unwrap();
}

#[test]
fn serialization_preserves_service_identity_path_and_stop_occurrences() {
    let mut f = fixture();
    f.world
        .add_service(f.company, "Loop".into(), f.a, vec![f.ab, f.ab], vec![0, 2])
        .unwrap();
    let json = serde_json::to_value(f.world.snapshot()).unwrap();
    assert!(json["services"][0]["id"].is_string());
    assert_eq!(json["services"][0]["stops"], serde_json::json!([0, 2]));
    let restored: WorldDraft = serde_json::from_value(json).unwrap();
    assert_eq!(World::new(restored).unwrap(), f.world);
    let large_id = ServiceId(9_007_199_254_740_993);
    assert_eq!(
        serde_json::to_string(&large_id).unwrap(),
        "\"9007199254740993\""
    );
}

#[test]
fn services_field_is_required_and_malformed_stops_are_rejected() {
    let mut f = fixture();
    f.world
        .add_service(f.company, "Service".into(), f.a, vec![f.ab], vec![0, 1])
        .unwrap();
    let original = serde_json::to_value(f.world.snapshot()).unwrap();
    let mut missing = original.clone();
    missing.as_object_mut().unwrap().remove("services");
    assert!(serde_json::from_value::<WorldDraft>(missing).is_err());
    for stops in [
        serde_json::json!([-1, 1]),
        serde_json::json!([0, 1.5]),
        serde_json::json!([0, 4294967296_u64]),
        serde_json::json!(["0", "1"]),
    ] {
        let mut invalid = original.clone();
        invalid["services"][0]["stops"] = stops;
        assert!(serde_json::from_value::<WorldDraft>(invalid).is_err());
    }
}

#[test]
fn unknown_world_and_entity_fields_are_rejected_instead_of_lost() {
    let mut f = fixture();
    f.world
        .add_route(f.company, "Route".into(), f.a, vec![f.ab])
        .unwrap();
    f.world
        .add_service(f.company, "Service".into(), f.a, vec![f.ab], vec![0, 1])
        .unwrap();
    let original = serde_json::to_value(f.world.snapshot()).unwrap();
    let mut unknown_world = original.clone();
    unknown_world["future_feature"] = serde_json::json!(true);
    assert!(serde_json::from_value::<WorldDraft>(unknown_world).is_err());
    for collection in [
        "players",
        "companies",
        "stations",
        "tracks",
        "routes",
        "services",
    ] {
        let mut unknown = original.clone();
        unknown[collection][0]["future_feature"] = serde_json::json!(true);
        assert!(
            serde_json::from_value::<WorldDraft>(unknown).is_err(),
            "{collection}"
        );
    }
}
