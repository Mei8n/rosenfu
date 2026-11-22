use rosenfu_core::operation::{
    OperationError, OperationRuntime, Timetable, Train, TrainState, Vehicle,
};
use rosenfu_core::world::{
    GeoPoint, OperationServiceConfig, RouteSegment, SectionInfrastructure, World,
};

fn corridor() -> (World, Vec<OperationServiceConfig>) {
    let mut world = World::default();
    let player = world.add_player("QA".into()).unwrap();
    let owner = world.add_company(player, "Rail".into()).unwrap();
    let points = (0..6)
        .map(|i| GeoPoint {
            latitude: 35.,
            longitude: 139. + i as f64 * 0.0035,
        })
        .collect();
    let line = world
        .build_alignment(owner, "Monotone corridor".into(), points)
        .unwrap();
    let mut draft = world.snapshot().clone();
    draft.alignments[0].anchor_indices = Some((0..6).collect());
    draft.alignments[0].infrastructure = vec![
        SectionInfrastructure {
            track_count: 2,
            ..Default::default()
        };
        5
    ];
    world.replace(draft).unwrap();
    let mut station = |name: &str, position| {
        world
            .place_station_on_alignment(owner, name.into(), line, position, 60., 10.)
            .unwrap()
    };
    let a = station("A", 0.10);
    let b = station("B", 0.30);
    let d = station("D", 0.70);
    let c = station("C", 0.90);
    let configs = [
        ("Stopping leader", a, c, Some(b), 0, 72., 30_000),
        ("Same platform approach", a, b, None, 1000, 72., 0),
        ("Downstream slow movement", d, c, None, 1000, 9., 0),
        (
            "Opposite lane through same station",
            b,
            a,
            None,
            1000,
            72.,
            0,
        ),
    ]
    .into_iter()
    .map(|(name, from, to, midpoint, time, speed, dwell_ms)| {
        let legs = vec![RouteSegment::Alignment {
            alignment_id: line,
            from,
            to,
        }];
        let preview = world.preview_service_path(from, legs.clone()).unwrap();
        let stops = preview
            .occurrences
            .iter()
            .filter(|o| {
                o.station_id == from || o.station_id == to || midpoint == Some(o.station_id)
            })
            .map(|o| o.reference.clone())
            .collect();
        let service_id = world
            .add_service_path(owner, name.into(), from, legs, stops)
            .unwrap();
        OperationServiceConfig {
            service_id,
            timetable: Timetable {
                first_departure_ms: time,
                last_departure_ms: time,
                headway_ms: 60_000,
            },
            vehicle: Vehicle {
                max_speed_kmh: speed,
                acceleration_mps2: 1.,
                braking_mps2: 1.,
                length_m: 40.,
            },
            dwell_ms,
        }
    })
    .collect();
    (world, configs)
}

fn advance(w: &mut World, ms: u64) -> rosenfu_core::operation::AdvanceResult {
    w.operation_mut()
        .unwrap()
        .runtime
        .advance_to(ms, 128)
        .unwrap()
}

fn train(w: &World, config: &OperationServiceConfig) -> Train {
    w.operation()
        .unwrap()
        .runtime
        .trains()
        .find(|t| t.key.service_id == config.service_id.0.to_string())
        .unwrap()
        .clone()
}

#[test]
fn a_parked_through_train_owns_its_lane_until_it_can_leave() {
    let (mut world, c) = corridor();
    world.configure_operation(c.clone()).unwrap();
    assert_eq!(world.operation_save().unwrap().runtime.format, 6);
    let mut old = world.clone();
    let specs = c
        .iter()
        .map(|config| world.compile_operation(config))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    old.operation_mut().unwrap().runtime = OperationRuntime::new(specs).unwrap();
    advance(&mut world, 1000);
    advance(&mut old, 1000);
    assert!(matches!(
        train(&world, &c[3]).state,
        TrainState::Moving {
            entered_ms: 1000,
            ..
        }
    ));
    let TrainState::Moving {
        arrival_ms,
        release_ms,
        ..
    } = train(&world, &c[0]).state
    else {
        panic!("leader")
    };
    let TrainState::Moving {
        release_ms: downstream_release,
        ..
    } = train(&world, &c[2]).state
    else {
        panic!("downstream")
    };
    assert_eq!(
        (arrival_ms, release_ms, downstream_release),
        (35_710, 37_710, 147_020)
    );
    let snapshots = world.operation_save();
    let replay = |world: &World| {
        world
            .operation()
            .unwrap()
            .runtime
            .preview_passenger_replay(240_000, 128)
            .unwrap()
    };
    let observed = replay(&world);
    let legacy = replay(&old);
    assert_eq!(world.operation_save(), snapshots);
    let find = |replay: &rosenfu_core::operation::PassengerReplay, config: usize, leg: usize| {
        let id = c[config].service_id.0.to_string();
        replay
            .completed_legs
            .iter()
            .find(|event| event.train.service_id == id && event.sequence == leg)
            .map(|event| {
                (
                    event.entered_ms,
                    event.arrived_ms,
                    event.cumulative_reservation_wait_ms,
                )
            })
            .unwrap()
    };
    let continue_at = find(&observed, 0, 1);
    let follower = find(&observed, 1, 0);
    let old_follower = find(&legacy, 1, 0);
    assert_eq!(find(&observed, 0, 0), find(&legacy, 0, 0));
    assert_eq!(find(&observed, 3, 0), find(&legacy, 3, 0));
    assert_eq!(old_follower.0, release_ms);
    assert_eq!(continue_at.0, downstream_release);
    assert_eq!(
        continue_at.2,
        downstream_release - (arrival_ms + c[0].dwell_ms)
    );
    assert_eq!(follower, (180_930, 216_640, 179_930));
    println!("leader={continue_at:?} follower={follower:?} legacy_follower={old_follower:?}");

    let mut during_dwell = world.clone();
    advance(&mut during_dwell, release_ms);
    assert_eq!(
        train(&during_dwell, &c[0]).state,
        TrainState::Waiting {
            ready_ms: arrival_ms + c[0].dwell_ms
        }
    );
    let bytes = rosenfu_storage::encode(&during_dwell).unwrap();
    let mut reopened_dwell = rosenfu_storage::decode(&bytes).unwrap();
    assert_eq!(rosenfu_storage::encode(&reopened_dwell).unwrap(), bytes);

    let parked = arrival_ms + c[0].dwell_ms + 1;
    advance(&mut old, parked);
    assert!(matches!(
        train(&old, &c[1]).state,
        TrainState::Moving { .. }
    ));
    let prior_bytes = rosenfu_storage::encode(&old).unwrap();
    assert_eq!(
        rosenfu_storage::encode(&rosenfu_storage::decode(&prior_bytes).unwrap()).unwrap(),
        prior_bytes
    );
    advance(&mut world, parked);
    advance(&mut reopened_dwell, parked);
    assert_eq!(world.operation_save(), reopened_dwell.operation_save());
    assert_eq!(train(&world, &c[0]).leg, 1);
    assert!(matches!(
        train(&world, &c[0]).state,
        TrainState::Waiting { .. }
    ));
    assert!(
        matches!(train(&world, &c[1]).state, TrainState::Waiting { .. }),
        "same lane admitted while the leader is still parked and blocked downstream"
    );
    let bytes = rosenfu_storage::encode(&world).unwrap();
    let mut reopened_wait = rosenfu_storage::decode(&bytes).unwrap();
    assert_eq!(rosenfu_storage::encode(&reopened_wait).unwrap(), bytes);

    advance(&mut world, downstream_release - 1);
    assert!(matches!(
        train(&world, &c[0]).state,
        TrainState::Waiting { .. }
    ));
    advance(&mut world, downstream_release);
    assert!(
        matches!(train(&world, &c[0]).state, TrainState::Moving { entered_ms, .. }
        if entered_ms == downstream_release)
    );
    let bytes = rosenfu_storage::encode(&world).unwrap();
    let mut reopened_through = rosenfu_storage::decode(&bytes).unwrap();
    advance(&mut reopened_wait, downstream_release);
    assert_eq!(
        reopened_wait.operation_save(),
        reopened_through.operation_save()
    );
    assert_eq!(rosenfu_storage::encode(&reopened_through).unwrap(), bytes);
    advance(&mut world, follower.0 - 1);
    assert!(matches!(
        train(&world, &c[1]).state,
        TrainState::Waiting { .. }
    ));
    advance(&mut world, follower.0);
    assert!(
        matches!(train(&world, &c[1]).state, TrainState::Moving { entered_ms, .. }
        if entered_ms == follower.0)
    );
    let finish = 400_000;
    assert!(advance(&mut world, finish).reached_target);
    assert_eq!(
        advance(&mut reopened_through, finish),
        advance(&mut reopened_wait, finish)
    );
    assert_eq!(reopened_through.operation_save(), world.operation_save());
    assert_eq!(world.operation().unwrap().runtime.trains().count(), 0);
}

#[test]
fn parked_lane_owner_and_timeline_are_verified_on_restore() {
    let (mut world, c) = corridor();
    world.configure_operation(c.clone()).unwrap();
    advance(&mut world, 65_711);
    let saved = world.operation_save().unwrap();
    let mut reopened = World::new(world.snapshot().clone()).unwrap();
    reopened.restore_operation(Some(saved.clone())).unwrap();

    // Forge a second train onto the held platform while its real owner is
    // parked. Every forged train timestamp is otherwise consistent.
    let mut duplicate = saved.clone();
    let train = duplicate
        .runtime
        .trains
        .iter_mut()
        .find(|t| t.key.service_id == c[1].service_id.0.to_string())
        .unwrap();
    train.state = TrainState::Moving {
        entered_ms: 65_710,
        arrival_ms: 65_710 + 35_710,
        release_ms: 65_710 + 35_710 + 2_000,
    };
    train.delay_ms = 65_710 - train.key.scheduled_ms;
    assert_eq!(
        reopened.restore_operation(Some(duplicate)),
        Err(OperationError::InvalidPath)
    );

    let mut wrong_epoch = saved.clone();
    let leader = wrong_epoch
        .runtime
        .trains
        .iter_mut()
        .find(|t| t.key.service_id == c[0].service_id.0.to_string())
        .unwrap();
    leader.state = TrainState::Waiting { ready_ms: 65_709 };
    assert_eq!(
        reopened.restore_operation(Some(wrong_epoch)),
        Err(OperationError::InvalidClock)
    );
    let mut retrograde = saved.clone();
    retrograde.invalidated = true;
    let old = retrograde
        .runtime
        .services
        .iter_mut()
        .find(|s| s.service_id == c[0].service_id.0.to_string())
        .unwrap();
    let back_edge = old.legs[0].sections.first().unwrap().resources[0].clone();
    old.legs[1].sections[1].resources[0] = back_edge;
    assert_eq!(
        reopened.restore_operation(Some(retrograde)),
        Err(OperationError::InvalidClock)
    );
    assert_eq!(reopened.operation_save(), Some(saved.clone()));

    // Moving platforms outside the proof freezes the current trains rather
    // than resuming through a block whose physical admission changed.
    let mid = world
        .operation()
        .unwrap()
        .runtime
        .service_spec(&c[0].service_id.0.to_string())
        .unwrap()
        .legs[0]
        .to
        .parse::<u64>()
        .unwrap();
    let mut draft = world.snapshot().clone();
    draft
        .station_placements
        .iter_mut()
        .find(|p| p.station_id.0 == mid)
        .unwrap()
        .length_m = 32.;
    let mut edited = World::new(draft).unwrap();
    assert!(edited.restore_operation(Some(saved)).is_err());
    edited.retain_operation_from(&world);
    assert!(edited.operation_save().unwrap().invalidated);
    let bytes = rosenfu_storage::encode(&edited).unwrap();
    let frozen = rosenfu_storage::decode(&bytes).unwrap();
    assert!(frozen.operation_save().unwrap().runtime.paused);
    assert_eq!(rosenfu_storage::encode(&frozen).unwrap(), bytes);
    edited.configure_operation(c).unwrap();
    assert_eq!(edited.operation_save().unwrap().runtime.format, 2);
}

#[test]
fn idle_edit_preserves_an_authentic_fixed_admission_save_and_unrelated_replay() {
    let (mut world, mut configs) = corridor();
    for config in &mut configs {
        config.timetable.first_departure_ms += 500_000;
        config.timetable.last_departure_ms += 500_000;
    }
    world.configure_operation(configs.clone()).unwrap();
    assert_eq!(world.operation_save().unwrap().runtime.format, 6);
    let modern = world.clone();
    let legacy_specs = configs
        .iter()
        .map(|config| world.compile_operation(config))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    world.operation_mut().unwrap().runtime = OperationRuntime::new(legacy_specs).unwrap();
    advance(&mut world, 200_000);
    let mut reopened = rosenfu_storage::decode(&rosenfu_storage::encode(&world).unwrap()).unwrap();
    assert_eq!(reopened, world);
    let target = configs[3].service_id.0.to_string();
    let unaffected_replay = |world: &World| {
        world
            .operation()
            .unwrap()
            .runtime
            .preview_passenger_replay(900_000, 256)
            .unwrap()
            .completed_legs
            .into_iter()
            .filter(|leg| leg.train.service_id != target)
            .collect::<Vec<_>>()
    };
    let old = unaffected_replay(&world);
    assert!(!old.is_empty());
    configs[3].timetable.first_departure_ms += 1_000;
    configs[3].timetable.last_departure_ms += 1_000;
    reopened
        .reconfigure_idle_operation(configs.clone(), vec![], &target)
        .unwrap();
    let state = reopened.operation_save().unwrap();
    assert_eq!(state.runtime.format, 2);
    assert_eq!(state.configs, configs);
    assert_eq!(state.runtime.now_ms, 200_000);
    assert_eq!(unaffected_replay(&reopened), old);
    assert_eq!(
        rosenfu_storage::decode(&rosenfu_storage::encode(&reopened).unwrap()).unwrap(),
        reopened
    );

    // The same API must refuse a new vehicle that would downgrade the
    // whole running model, even with no current trains.
    let mut modern = modern;
    advance(&mut modern, 200_000);
    let mut disqualified = modern.operation().unwrap().configs.clone();
    disqualified[0].vehicle.length_m = 120.;
    let first = disqualified[0].service_id.0.to_string();
    let bytes = rosenfu_storage::encode(&modern).unwrap();
    assert_eq!(
        modern.reconfigure_idle_operation(disqualified, vec![], &first),
        Err(OperationError::ReservationScopeChanged)
    );
    assert_eq!(rosenfu_storage::encode(&modern).unwrap(), bytes);
}
