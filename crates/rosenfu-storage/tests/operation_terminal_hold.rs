use rosenfu_core::{
    operation::{Timetable, TrainState, Vehicle},
    world::{GeoPoint, OperationServiceConfig, RouteSegment, World},
};
use rosenfu_storage::{
    decode, encode, load_world_with_version, save_world, StorageError, SCHEMA_VERSION,
};

#[test]
fn current_save_reopens_terminal_hold_without_admitting_opposing_train() {
    let mut world = World::default();
    let player = world.add_player("p".into()).unwrap();
    let company = world.add_company(player, "c".into()).unwrap();
    let alignment = world
        .build_alignment(
            company,
            "straight corridor".into(),
            (0..4)
                .map(|i| GeoPoint {
                    latitude: 35.0,
                    longitude: 139.0 + 0.0044 * i as f64,
                })
                .collect(),
        )
        .unwrap();
    let mut draft = world.snapshot().clone();
    draft.alignments[0].anchor_indices = Some(vec![0, 1, 2, 3]);
    world.replace(draft).unwrap();
    let a = world
        .place_station_on_alignment(company, "A".into(), alignment, 0.10, 60., 10.)
        .unwrap();
    let b = world
        .place_station_on_alignment(company, "B".into(), alignment, 0.90, 60., 10.)
        .unwrap();
    let configs: Vec<_> = [(a, b), (b, a)]
        .into_iter()
        .map(|(from, to)| {
            let segments = vec![RouteSegment::Alignment {
                alignment_id: alignment,
                from,
                to,
            }];
            let stops = world
                .preview_service_path(from, segments.clone())
                .unwrap()
                .occurrences
                .iter()
                .map(|stop| stop.reference.clone())
                .collect();
            let service_id = world
                .add_service_path(company, "shuttle".into(), from, segments, stops)
                .unwrap();
            OperationServiceConfig {
                service_id,
                timetable: Timetable {
                    first_departure_ms: 0,
                    last_departure_ms: 0,
                    headway_ms: 60_000,
                },
                vehicle: Vehicle {
                    max_speed_kmh: 72.,
                    acceleration_mps2: 1.,
                    braking_mps2: 1.,
                    length_m: 40.,
                },
                dwell_ms: 30_000,
            }
        })
        .collect();
    let leader = configs[0].service_id.0.to_string();
    let opposing = configs[1].service_id.0.to_string();
    world.configure_operation(configs).unwrap();

    let advance = |world: &mut World, to| {
        world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(to, 100)
            .unwrap()
    };
    advance(&mut world, 0);
    let current = world.operation_save().unwrap();
    assert_eq!(current.runtime.format, 4);
    let moving = current
        .runtime
        .trains
        .iter()
        .find(|train| train.key.service_id == leader)
        .unwrap();
    let TrainState::Moving {
        arrival_ms,
        release_ms,
        ..
    } = moving.state
    else {
        panic!("leader did not depart");
    };
    assert_eq!(release_ms, arrival_ms + 30_000);
    advance(&mut world, release_ms - 1);

    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("terminal.json");
    save_world(&path, &world).unwrap();
    let (mut reopened, version) = load_world_with_version(&path).unwrap();
    assert_eq!(version, rosenfu_storage::file_version(&path).unwrap());
    assert_eq!(reopened.operation_save(), world.operation_save());
    let result = advance(&mut reopened, release_ms);
    assert_eq!(result, advance(&mut world, release_ms));
    let saved = reopened.operation_save().unwrap();
    let returning = saved
        .runtime
        .trains
        .iter()
        .find(|train| train.key.service_id == leader)
        .unwrap();
    assert_eq!(returning.key, moving.key);
    assert_eq!(returning.leg, 1);
    assert!(saved
        .runtime
        .trains
        .iter()
        .any(|train| train.key.service_id == opposing
            && matches!(train.state, TrainState::Waiting { .. })));
    assert_eq!(
        advance(&mut reopened, 500_000),
        advance(&mut world, 500_000),
    );
    assert_eq!(reopened.operation_save(), world.operation_save());

    let bytes = std::fs::read(&path).unwrap();
    let mut document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(document["schema_version"], SCHEMA_VERSION);
    document["schema_version"] = serde_json::json!(26);
    assert!(matches!(
        decode(&serde_json::to_vec(&document).unwrap()),
        Err(StorageError::InvalidHeader)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);

    // The nested TrainKey intentionally permits unknown fields. A typed
    // parser's skip still must not bypass normal JSON range and depth checks.
    let canonical = std::str::from_utf8(&bytes).unwrap();
    let edit = |raw: &str| {
        canonical.replacen(
            r#""key": {"#,
            &format!(r#""key": {{ "future_field": {raw},"#),
            1,
        )
    };
    let harmless = edit("123");
    assert_ne!(harmless.as_bytes(), bytes);
    assert_eq!(
        decode(harmless.as_bytes()).unwrap().operation_save(),
        load_world_with_version(&path).unwrap().0.operation_save()
    );

    let too_deep = format!("{}0{}", "[".repeat(127), "]".repeat(127));
    let backup = rosenfu_storage::backup_path(&path);
    save_world(&path, &world).unwrap();
    let prior_backup = std::fs::read(&backup).unwrap();
    for (bad, expected) in [
        ("1e999", "number out of range"),
        (&too_deep, "recursion limit exceeded"),
    ] {
        let malformed = edit(bad);
        assert!(matches!(
            decode(malformed.as_bytes()),
            Err(StorageError::Json(error)) if error.to_string().contains(expected)
        ));
        std::fs::write(&path, malformed.as_bytes()).unwrap();
        assert!(matches!(
            load_world_with_version(&path),
            Err(StorageError::Json(error)) if error.to_string().contains(expected)
        ));
        assert!(matches!(
            save_world(&path, &world),
            Err(StorageError::Json(error)) if error.to_string().contains(expected)
        ));
        assert_eq!(std::fs::read(&path).unwrap(), malformed.as_bytes());
        assert_eq!(std::fs::read(&backup).unwrap(), prior_backup);
    }
}

fn connected_neck(
    reverse_construction: bool,
    main_tracks: u8,
    lead_tracks: u8,
) -> (World, OperationServiceConfig, [rosenfu_core::world::AlignmentId; 2]) {
    connected_neck_at(reverse_construction, main_tracks, lead_tracks, 139.)
}

fn connected_neck_at(
    reverse_construction: bool,
    main_tracks: u8,
    lead_tracks: u8,
    longitude: f64,
) -> (World, OperationServiceConfig, [rosenfu_core::world::AlignmentId; 2]) {
    use rosenfu_core::world::{
        AlignmentConnection, AlignmentEndpoint, AlignmentEndpointRef, SectionInfrastructure,
    };
    let mut world = World::default();
    let player = world.add_player("Player".into()).unwrap();
    let company = world.add_company(player, "Rail".into()).unwrap();
    let points = |x, y| {
        let point = |longitude| GeoPoint {
            latitude: 35.,
            longitude,
        };
        if reverse_construction {
            vec![point(y), point(x)]
        } else {
            vec![point(x), point(y)]
        }
    };
    let approach = world
        .build_alignment(company, "Approach".into(), points(longitude, longitude + 0.008))
        .unwrap();
    let lead = world
        .build_alignment(company, "Terminal lead".into(), points(longitude + 0.008, longitude + 0.012))
        .unwrap();
    let mut draft = world.snapshot().clone();
    for (a, tracks) in draft.alignments.iter_mut().zip([main_tracks, lead_tracks]) {
        a.infrastructure = vec![SectionInfrastructure {
            track_count: tracks,
            ..Default::default()
        }];
    }
    world.replace(draft).unwrap();
    let (near, far) = if reverse_construction {
        (AlignmentEndpoint::End, AlignmentEndpoint::Start)
    } else {
        (AlignmentEndpoint::Start, AlignmentEndpoint::End)
    };
    let a = AlignmentEndpointRef {
        alignment_id: approach,
        endpoint: far,
    };
    let b = AlignmentEndpointRef {
        alignment_id: lead,
        endpoint: near,
    };
    world.connect_alignment_endpoints(company, a, b).unwrap();
    let origin_t = if reverse_construction { 0.8 } else { 0.2 };
    let origin = world
        .place_station_on_alignment(company, "A".into(), approach, origin_t, 60., 10.)
        .unwrap();
    let terminal = world
        .place_station_on_alignment(company, "B".into(), lead, 0.5, 60., 10.)
        .unwrap();
    let segments = vec![RouteSegment::ConnectedAlignments {
        from: origin,
        to: terminal,
        connections: vec![AlignmentConnection { a, b }],
    }];
    let stops = world
        .preview_service_path(origin, segments.clone())
        .unwrap()
        .occurrences
        .into_iter()
        .map(|occurrence| occurrence.reference)
        .collect();
    let service_id = world
        .add_service_path(company, "Shuttle".into(), origin, segments, stops)
        .unwrap();
    let config = OperationServiceConfig {
        service_id,
        timetable: Timetable {
            first_departure_ms: 0,
            last_departure_ms: 8_000,
            headway_ms: 8_000,
        },
        vehicle: Vehicle {
            max_speed_kmh: 72.,
            acceleration_mps2: 1.,
            braking_mps2: 1.,
            length_m: 40.,
        },
        dwell_ms: 120_000,
    };
    (world, config, [approach, lead])
}

#[test]
fn proved_connected_neck_keeps_the_train_during_dwell_and_returns_on_the_other_lane() {
    for reverse in [false, true] {
        let (mut world, config, [approach, lead]) = connected_neck(reverse, 2, 1);
        world.configure_operation(vec![config.clone()]).unwrap();
        let setup = world.operation_save().unwrap();
        assert!(setup.has_connected_terminal_neck());
        let spec = &setup.runtime.services[0];
        assert_eq!(spec.legs.len(), 2);
        let locks = |i: usize| {
            spec.legs[i]
                .sections
                .iter()
                .flat_map(|s| s.resources.iter().cloned())
                .collect::<std::collections::BTreeSet<_>>()
        };
        let lane = |line: rosenfu_core::world::AlignmentId, n| {
            format!("rail:{}:section:0:lane:{n}", line.0)
        };
        let (out, back) = (locks(0), locks(1));
        assert!(out.contains(&lane(approach, u8::from(reverse))));
        assert!(back.contains(&lane(approach, u8::from(!reverse))));
        assert!(!back.contains(&lane(approach, u8::from(reverse))));
        assert!(out.contains(&lane(lead, 0)) && back.contains(&lane(lead, 0)));
        let advance = |w: &mut World, at| {
            w.operation_mut().unwrap().runtime.advance_to(at, 64).unwrap()
        };
        advance(&mut world, 0);
        let TrainState::Moving {
            arrival_ms,
            release_ms,
            ..
        } = world.operation_save().unwrap().runtime.trains[0].state
        else {
            panic!("first train should leave immediately");
        };
        assert!(arrival_ms > config.timetable.last_departure_ms);
        assert_eq!(release_ms, arrival_ms + config.dwell_ms);
        advance(&mut world, release_ms - 1);
        let waiting = world.operation_save().unwrap();
        assert_eq!(waiting.runtime.trains.len(), 2);
        assert_eq!(waiting.runtime.trains[0].leg, 0);
        assert_eq!(
            waiting.runtime.trains[1].state,
            TrainState::Waiting {
                ready_ms: config.timetable.last_departure_ms
            }
        );
        let frozen = encode(&world).unwrap();
        let mut resumed = decode(&frozen).unwrap();
        assert_eq!(encode(&resumed).unwrap(), frozen);
        let mut damaged = waiting.clone();
        damaged.runtime.trains[0].delay_ms = 1;
        assert!(resumed.restore_operation(Some(damaged)).is_err());
        assert_eq!(resumed.operation_save(), Some(waiting.clone()));

        advance(&mut world, release_ms);
        advance(&mut resumed, release_ms);
        let returning = world.operation_save().unwrap();
        assert_eq!(resumed.operation_save(), Some(returning.clone()));
        assert_eq!(returning.runtime.trains[0].key, waiting.runtime.trains[0].key);
        assert_eq!(returning.runtime.trains[0].leg, 1);
        assert_eq!(returning.runtime.trains[1], waiting.runtime.trains[1]);
        let TrainState::Moving {
            entered_ms,
            release_ms: returned_rear_clear,
            ..
        } = returning.runtime.trains[0].state
        else {
            panic!("same train must return after dwelling");
        };
        assert_eq!(entered_ms, release_ms);
        let returning_bytes = encode(&resumed).unwrap();
        let mut resumed = decode(&returning_bytes).unwrap();
        assert_eq!(encode(&resumed).unwrap(), returning_bytes);
        assert_eq!(
            advance(&mut resumed, returned_rear_clear),
            advance(&mut world, returned_rear_clear)
        );
        assert_eq!(resumed.operation_save(), world.operation_save());
        let next = resumed.operation_save().unwrap().runtime.trains.remove(0);
        assert_eq!(next.key, waiting.runtime.trains[1].key);
        assert!(matches!(
            next.state,
            TrainState::Moving { entered_ms, .. } if entered_ms == returned_rear_clear
        ));

        let mut legacy = setup;
        legacy.runtime =
            rosenfu_core::operation::OperationRuntime::new(legacy.runtime.services)
                .unwrap()
                .save();
        let mut legacy_world = World::new(world.snapshot().clone()).unwrap();
        legacy_world.restore_operation(Some(legacy.clone())).unwrap();
        let legacy_bytes = encode(&legacy_world).unwrap();
        let legacy_header: serde_json::Value = serde_json::from_slice(&legacy_bytes).unwrap();
        assert_eq!(legacy_header["schema_version"], SCHEMA_VERSION);
        assert_eq!(decode(&legacy_bytes).unwrap().operation_save(), Some(legacy));

        // Only the new capability receives the high outer envelope. Current
        // older-schema paths must reject even a syntactically valid downgrade.
        let mut wire: serde_json::Value = serde_json::from_slice(&frozen).unwrap();
        assert_eq!(wire["schema_version"], 33);
        let mut bogus_standard = wire.clone();
        bogus_standard["operation"]["standard_timetable"] = serde_json::json!({});
        assert!(matches!(
            decode(&serde_json::to_vec(&bogus_standard).unwrap()),
            Err(StorageError::UnsupportedVersion(33))
        ));
        wire["schema_version"] = SCHEMA_VERSION.into();
        let downgraded = serde_json::to_vec(&wire).unwrap();
        assert!(matches!(
            decode(&downgraded),
            Err(StorageError::UnsupportedVersion(33))
        ));
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("terminal.json");
        std::fs::write(&path, &downgraded).unwrap();
        assert!(matches!(
            save_world(&path, &world),
            Err(StorageError::UnsupportedVersion(33))
        ));
        assert_eq!(std::fs::read(&path).unwrap(), downgraded);
    }
}

#[test]
fn a_four_track_approach_or_double_terminal_does_not_get_the_new_turnback_mode() {
    for (approach, lead) in [(2, 2), (4, 1)] {
        let (mut world, config, _) = connected_neck(false, approach, lead);
        world.configure_operation(vec![config]).unwrap();
        let save = world.operation_save().unwrap();
        assert!(!save.has_connected_terminal_neck());
        if lead == 2 {
            assert_eq!(save.runtime.services[0].legs.len(), 1);
        }
        let wire: serde_json::Value = serde_json::from_slice(&encode(&world).unwrap()).unwrap();
        assert_eq!(wire["schema_version"], SCHEMA_VERSION);
    }
}

#[test]
fn a_vehicle_longer_than_the_terminal_platform_does_not_get_the_new_mode() {
    let (mut world, mut config, _) = connected_neck(false, 2, 1);
    config.vehicle.length_m = 75.;
    world.configure_operation(vec![config]).unwrap();
    let save = world.operation_save().unwrap();
    assert!(!save.has_connected_terminal_neck());
    let wire: serde_json::Value = serde_json::from_slice(&encode(&world).unwrap()).unwrap();
    assert_eq!(wire["schema_version"], SCHEMA_VERSION);
}

#[test]
fn adjacent_street_tram_and_connected_neck_envelopes_cannot_mask_each_other() {
    use rosenfu_core::{
        ofm_evidence::{
            AuthoredTramGuide, BusTileProof, BusVertex, RawEvidence, TileBinding, DECODER_RULE,
        },
        world::TrackType,
    };
    let (mut world, config, _) = connected_neck(false, 2, 1);
    let owner = world.snapshot().companies[0].id;
    world.configure_operation(vec![config.clone()]).unwrap();
    let neck_bytes = encode(&world).unwrap();
    let neck_wire: serde_json::Value = serde_json::from_slice(&neck_bytes).unwrap();
    assert_eq!(neck_wire["schema_version"], 33);

    let tiles: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../rosenfu-core/tests/fixtures/ofm_authored_adjacent_tram_synthetic.json"
    ))
    .unwrap();
    let proof = |i: usize| BusTileProof {
        tile_key: tiles[i]["tileKey"].as_str().unwrap().to_owned(),
        tile_raw_sha256: tiles[i]["tileRawSha256"].as_str().unwrap().to_owned(),
    };
    let first = proof(0);
    let vertex = BusVertex {
        feature: 0,
        line: 0,
        vertex: 0,
    };
    let guide = AuthoredTramGuide {
        tile_key: first.tile_key,
        tile_raw_sha256: first.tile_raw_sha256,
        adjacent_end: Some(proof(1)),
        start: vertex,
        end: vertex,
        start_fraction_q: Some(500_000),
        end_fraction_q: Some(500_000),
    };
    let cached = tiles
        .iter()
        .map(|t| {
            let sha = t["tileRawSha256"].as_str().unwrap().to_owned();
            (
                RawEvidence {
                    sha256: sha.clone(),
                    bytes: serde_json::from_value(t["raw"].clone()).unwrap(),
                },
                TileBinding {
                    canonical_url: t["tileKey"].as_str().unwrap().to_owned(),
                    revision: t["revision"].as_str().unwrap().to_owned(),
                    raw_sha256: sha,
                    decoder_rule: DECODER_RULE.into(),
                    z: 14,
                    x: t["x"].as_u64().unwrap() as u32,
                    y: t["y"].as_u64().unwrap() as u32,
                },
            )
        })
        .collect();
    let (mut mixed, path) = world
        .staged_street_tram_for_construction(&guide, cached)
        .unwrap();
    mixed
        .build_alignment_from_road_path(
            owner,
            "Adjacent tram".into(),
            TrackType::Tram,
            path.descriptor,
        )
        .unwrap();
    assert!(mixed.snapshot().carries_adjacent_street_tram_proof());
    assert!(mixed.operation_save().unwrap().has_connected_terminal_neck());
    assert!(matches!(
        rosenfu_storage::ensure_world_saveable(&mixed),
        Err(StorageError::UnsupportedVersion(33))
    ));

    let folder = tempfile::tempdir().unwrap();
    let save = folder.path().join("terminal.json");
    save_world(&save, &world).unwrap();
    assert!(matches!(
        save_world(&save, &mixed),
        Err(StorageError::UnsupportedVersion(33))
    ));
    assert_eq!(std::fs::read(&save).unwrap(), neck_bytes);
    assert!(!rosenfu_storage::backup_path(&save).exists());

    // A newly configured service in an existing adjacent-tram world is not
    // switched into a conflicting optional mode. It saves in the old format.
    let mut fallback = World::new(mixed.snapshot().clone()).unwrap();
    fallback.configure_operation(vec![config]).unwrap();
    assert!(!fallback.operation_save().unwrap().has_connected_terminal_neck());
    let normal = encode(&fallback).unwrap();
    assert_eq!(encode(&decode(&normal).unwrap()).unwrap(), normal);
    let mut tram_wire: serde_json::Value = serde_json::from_slice(&normal).unwrap();
    assert_eq!(tram_wire["schema_version"], 31);

    tram_wire["operation"] = neck_wire["operation"].clone();
    for version in [31, 33] {
        tram_wire["schema_version"] = version.into();
        assert!(matches!(
            decode(&serde_json::to_vec(&tram_wire).unwrap()),
            Err(StorageError::UnsupportedVersion(33))
        ));
    }
}

fn attach_classic_test_bus(world: &mut World) -> rosenfu_core::world::BusOperationConfig {
    use rosenfu_core::road_transport::{BusRoute, BusStop, BusStopOccurrence, RoadTransport};

    let mut draft = world.snapshot().clone();
    let owner = draft.companies[0].id;
    // The approved synthetic graph has two directed east/north road edges at
    // 135E. A real Bus compiler sees ordinary integer-edge stop occurrences.
    let mut graph = RoadTransport::synthetic_qa_graph();
    let stops = [
        ("A", 0, "qa:0-1", 0.2),
        ("B", 0, "qa:0-1", 0.8),
        ("C", 1, "qa:1-2", 0.8),
    ];
    graph.bus_stops = stops
        .iter()
        .map(|&(name, _, edge, position)| BusStop {
            id: name.into(),
            owner,
            name: name.into(),
            edge_id: edge.into(),
            position,
            location_kind: Default::default(),
        })
        .collect();
    graph.bus_routes = vec![BusRoute {
        id: "R".into(),
        owner,
        name: "Independent QA route".into(),
        edges: ["qa:0-1".into(), "qa:1-2".into()].to_vec(),
        stops: stops
            .iter()
            .map(|&(stop, edge_index, _, _)| BusStopOccurrence {
                stop_id: stop.into(),
                edge_index,
            })
            .collect(),
    }];
    draft.road_transport = graph;
    world.replace(draft).unwrap();
    rosenfu_core::world::BusOperationConfig {
        route_id: "R".into(),
        timetable: Timetable {
            first_departure_ms: 0,
            last_departure_ms: 0,
            headway_ms: 1,
        },
        vehicle: Vehicle {
            max_speed_kmh: 60.,
            acceleration_mps2: 1.,
            braking_mps2: 1.2,
            length_m: 12.,
        },
        assumed_speed_kmh: 30.,
        dwell_ms: 90_000,
        turnaround_ms: None,
    }
}

#[test]
fn connected_neck_and_verified_classic_bus_keep_exact_independent_run_lifetimes() {
    let (mut mixed, rail, _) = connected_neck_at(false, 2, 1, 139.);
    let bus = attach_classic_test_bus(&mut mixed);
    let mut control_rail = mixed.clone();
    let mut control_bus = mixed.clone();
    control_rail.configure_operation(vec![rail.clone()]).unwrap();
    control_bus
        .configure_all_operation(vec![], vec![bus.clone()])
        .unwrap();
    mixed.configure_operation(vec![rail.clone()]).unwrap();
    mixed
        .reconfigure_idle_operation(vec![rail.clone()], vec![bus.clone()], "bus:R")
        .unwrap();
    assert!(mixed
        .operation_save()
        .unwrap()
        .has_connected_terminal_neck_with_independent_classic_bus());
    let state = |w: &World, id: &str| {
        w.operation()
            .unwrap()
            .runtime
            .trains()
            .filter(|train| train.key.service_id == id)
            .cloned()
            .collect::<Vec<_>>()
    };
    let advance = |w: &mut World, at| {
        w.operation_mut()
            .unwrap()
            .runtime
            .advance_to(at, 256)
            .unwrap()
    };
    let rail_id = rail.service_id.0.to_string();
    for w in [&mut mixed, &mut control_bus, &mut control_rail] {
        advance(w, 0);
    }
    let TrainState::Moving {
        arrival_ms: bus_arrival,
        release_ms: bus_release,
        ..
    } = state(&control_bus, "bus:R")[0].state
    else {
        panic!("classic Bus is moving");
    };
    let TrainState::Moving {
        release_ms: rail_release,
        ..
    } = state(&control_rail, &rail_id)[0].state
    else {
        panic!("neck service is moving");
    };
    assert!(bus_release < bus_arrival + bus.dwell_ms);
    assert!(bus_release > rail.timetable.last_departure_ms);
    for w in [&mut mixed, &mut control_bus, &mut control_rail] {
        advance(w, bus_release);
    }
    assert_eq!(state(&mixed, &rail_id), state(&control_rail, &rail_id));
    let bus_wait = state(&mixed, "bus:R");
    assert_eq!(bus_wait, state(&control_bus, "bus:R"));
    assert_eq!(bus_wait[0].leg, 1);
    assert_eq!(
        bus_wait[0].state,
        TrainState::Waiting {
            ready_ms: bus_arrival + bus.dwell_ms
        }
    );
    let live = mixed.operation_save().unwrap();
    let bytes = rosenfu_storage::encode(&mixed).unwrap();
    let wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(wire["schema_version"], 35);
    assert_eq!(
        wire["capabilities"],
        serde_json::json!(["classic_bus_run_v1", "rail_terminal_neck_v1"])
    );
    let mut reopened = rosenfu_storage::decode(&bytes).unwrap();
    assert_eq!(reopened.operation_save(), Some(live.clone()));
    assert_eq!(rosenfu_storage::encode(&reopened).unwrap(), bytes);

    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("combined.json");
    for version in [27, 33, 34, 96] {
        let mut wrong = wire.clone();
        wrong["schema_version"] = version.into();
        let prior = serde_json::to_vec(&wrong).unwrap();
        std::fs::write(&path, &prior).unwrap();
        assert!(matches!(
            rosenfu_storage::load_world(&path),
            Err(StorageError::UnsupportedVersion(_))
        ));
        assert!(matches!(
            save_world(&path, &mixed),
            Err(StorageError::UnsupportedVersion(_))
        ));
        assert_eq!(std::fs::read(&path).unwrap(), prior);
    }
    let mut bad = live.clone();
    let r = bad
        .runtime
        .services
        .iter()
        .find(|spec| spec.service_id == rail_id)
        .unwrap()
        .legs[0]
        .sections[0]
        .resources[0]
        .clone();
    let road = bad
        .runtime
        .services
        .iter_mut()
        .find(|spec| spec.service_id == "bus:R")
        .unwrap();
    road.legs[0].sections[0].resources[0] = r;
    assert!(reopened.restore_operation(Some(bad)).is_err());
    assert_eq!(reopened.operation_save(), Some(live));
    let mut at = [rail_release, bus_arrival + bus.dwell_ms, 1_000_000];
    at.sort_unstable();
    let sort_arrivals = |mut arrivals: Vec<rosenfu_core::operation::Arrival>| {
        arrivals.sort_by(|a, b| {
            (a.at_ms, &a.train, &a.station_id).cmp(&(b.at_ms, &b.train, &b.station_id))
        });
        arrivals
    };
    for point in at {
        let merged = advance(&mut mixed, point).arrivals;
        assert_eq!(advance(&mut reopened, point).arrivals, merged);
        let mut separate = advance(&mut control_bus, point).arrivals;
        separate.extend(advance(&mut control_rail, point).arrivals);
        assert_eq!(sort_arrivals(merged), sort_arrivals(separate));
        assert_eq!(state(&mixed, &rail_id), state(&control_rail, &rail_id));
        assert_eq!(state(&mixed, "bus:R"), state(&control_bus, "bus:R"));
        assert_eq!(reopened.operation_save(), mixed.operation_save());
    }
    assert!(mixed.operation().unwrap().runtime.trains().next().is_none());
}

#[test]
fn rail_near_a_traversed_bus_edge_falls_back_despite_disjoint_resource_names() {
    let (mut near, rail, _) = connected_neck_at(false, 2, 1, 135.);
    let bus = attach_classic_test_bus(&mut near);
    let mut rail_only = near.clone();
    rail_only.configure_operation(vec![rail.clone()]).unwrap();
    assert!(rail_only.operation_save().unwrap().has_connected_terminal_neck());
    near.configure_all_operation(vec![rail], vec![bus]).unwrap();
    assert_eq!(near.operation_save().unwrap().runtime.format, 2);

    let (mut separated, rail, _) = connected_neck_at(false, 2, 1, 139.);
    let bus = attach_classic_test_bus(&mut separated);
    separated.configure_all_operation(vec![rail], vec![bus]).unwrap();
    separated
        .operation_mut()
        .unwrap()
        .runtime
        .advance_to(0, 64)
        .unwrap();
    near.retain_operation_from(&separated);
    let frozen = near.operation_save().unwrap();
    assert!(frozen.invalidated && frozen.runtime.paused);
    assert_eq!(
        frozen.runtime.trains,
        separated.operation_save().unwrap().runtime.trains
    );
}
