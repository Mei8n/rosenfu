use rosenfu_core::operation::{Timetable, Vehicle};
use rosenfu_core::world::{
    GeoPoint, OperationServiceConfig, RouteSegment, SectionInfrastructure, ServiceId, World,
};

#[derive(Clone, Copy)]
enum Layout {
    Shared,
    OppositeDirection,
    IndependentPairs,
}

fn configured(
    local_departure: u64,
    express_departure: u64,
    layout: Layout,
) -> (World, [ServiceId; 2]) {
    let mut world = World::default();
    let player = world.add_player("QA".into()).unwrap();
    let owner = world.add_company(player, "Rail".into()).unwrap();
    let mut line = |l, lat| {
        world
            .build_alignment(
                owner,
                l,
                vec![
                    GeoPoint {
                        latitude: lat,
                        longitude: 139.50,
                    },
                    GeoPoint {
                        latitude: lat,
                        longitude: 139.53,
                    },
                ],
            )
            .unwrap()
    };
    let local_track = line("Local lane pair".into(), 35.);
    let express_track = if matches!(layout, Layout::IndependentPairs) {
        line("Independent parallel lane pair".into(), 35.00016)
    } else {
        local_track
    };
    let mut draft = world.snapshot().clone();
    for a in &mut draft.alignments {
        a.infrastructure = vec![SectionInfrastructure {
            track_count: 2,
            ..Default::default()
        }];
    }
    world.replace(draft).unwrap();
    let a = world
        .place_station_on_alignment(owner, "A".into(), local_track, 0.1, 100., 5.)
        .unwrap();
    let b = world
        .place_station_on_alignment(owner, "B".into(), local_track, 0.5, 100., 5.)
        .unwrap();
    let c = world
        .place_station_on_alignment(owner, "C".into(), local_track, 0.9, 100., 5.)
        .unwrap();
    let (x, y) = match layout {
        Layout::Shared => (a, c),
        Layout::OppositeDirection => (c, a),
        Layout::IndependentPairs => (
            world
                .place_station_on_alignment(owner, "Ax".into(), express_track, 0.1, 100., 5.)
                .unwrap(),
            world
                .place_station_on_alignment(owner, "Cy".into(), express_track, 0.9, 100., 5.)
                .unwrap(),
        ),
    };
    let entries = [
        (
            "Ordinary",
            local_track,
            a,
            c,
            vec![a, b, c],
            local_departure,
            45.,
            100_000,
        ),
        (
            "Express",
            express_track,
            x,
            y,
            vec![x, y],
            express_departure,
            90.,
            0,
        ),
    ];
    let configs = entries
        .into_iter()
        .map(
            |(name, alignment_id, from, to, served, departure, max_speed_kmh, dwell_ms)| {
                let legs = vec![RouteSegment::Alignment {
                    alignment_id,
                    from,
                    to,
                }];
                let preview = world.preview_service_path(from, legs.clone()).unwrap();
                let stops = preview
                    .occurrences
                    .into_iter()
                    .filter(|p| served.contains(&p.station_id))
                    .map(|p| p.reference)
                    .collect();
                OperationServiceConfig {
                    service_id: world
                        .add_service_path(owner, name.into(), from, legs, stops)
                        .unwrap(),
                    timetable: Timetable {
                        first_departure_ms: departure,
                        last_departure_ms: departure,
                        headway_ms: 60_000,
                    },
                    vehicle: Vehicle {
                        max_speed_kmh,
                        acceleration_mps2: 1.,
                        braking_mps2: 1.,
                        length_m: 40.,
                    },
                    dwell_ms,
                }
            },
        )
        .collect::<Vec<_>>();
    let ids = [configs[0].service_id, configs[1].service_id];
    world.configure_operation(configs).unwrap();
    (world, ids)
}

#[test]
fn only_the_closing_express_on_the_same_directional_lane_waits() {
    for (layout, local_dep, express_dep, format, expected_entry_ms) in [
        (Layout::Shared, 0, 20_000, 6, 399_886),
        (Layout::Shared, 20_000, 0, 6, 0),
        (Layout::Shared, 0, 450_000, 6, 450_000),
        (Layout::OppositeDirection, 0, 20_000, 6, 20_000),
        (Layout::IndependentPairs, 0, 20_000, 2, 20_000),
    ] {
        let (mut world, [local, express]) = configured(local_dep, express_dep, layout);
        let operation = world.operation().unwrap();
        assert_eq!(operation.runtime.save().format, format);
        let replay = operation
            .runtime
            .preview_passenger_replay(900_000, 128)
            .unwrap();
        let express_key = express.0.to_string();
        let local_key = local.0.to_string();
        let actual = replay
            .completed_legs
            .iter()
            .find(|l| l.train.service_id == express_key)
            .unwrap();
        assert_eq!(actual.entered_ms, expected_entry_ms);
        assert_eq!(
            actual.cumulative_reservation_wait_ms,
            expected_entry_ms - express_dep
        );
        let movement = world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(900_000, 128)
            .unwrap();
        assert!(movement
            .arrivals
            .iter()
            .any(|row| row.train.service_id == express_key && row.at_ms == actual.arrived_ms));
        assert_eq!(
            movement
                .arrivals
                .iter()
                .filter(|row| row.train.service_id == local_key)
                .count(),
            2
        );
        eprintln!("format {format} local={local_dep} express={express_dep}: actual entered={expected_entry_ms}, reservation wait={} ms", actual.cumulative_reservation_wait_ms);
    }
}

#[test]
fn one_block_local_keeps_lane_until_rear_clear_despite_through_express() {
    let (mut world, [local, express]) = configured(0, 20_000, Layout::Shared);
    let local_key = local.0.to_string();
    let express_key = express.0.to_string();
    let tick = |w: &mut World, at| {
        w.operation_mut()
            .unwrap()
            .runtime
            .advance_to(at, 128)
            .unwrap()
    };
    let train = |w: &World, id: &str| {
        w.operation()
            .unwrap()
            .runtime
            .trains()
            .find(|t| t.key.service_id == id)
            .unwrap()
            .clone()
    };
    assert_eq!(world.operation_save().unwrap().runtime.format, 6);
    let until_dwell = tick(&mut world, 150_000);
    assert_eq!(until_dwell.arrivals.len(), 1);
    assert_eq!(until_dwell.arrivals[0].at_ms, 99_943);
    use rosenfu_core::operation::TrainState::{Moving, Waiting};
    assert_eq!(
        train(&world, &local_key).state,
        Waiting { ready_ms: 199_943 }
    );
    assert_eq!(
        train(&world, &express_key).state,
        Waiting { ready_ms: 20_000 }
    );

    let bytes = rosenfu_storage::encode(&world).unwrap();
    let mut restored = rosenfu_storage::decode(&bytes).unwrap();
    assert_eq!(rosenfu_storage::encode(&restored).unwrap(), bytes);
    assert_eq!(restored.operation_save(), world.operation_save());
    let planned = world.operation().unwrap().runtime.passenger_offers();

    let replay = world
        .operation()
        .unwrap()
        .runtime
        .preview_passenger_replay(900_000, 128)
        .unwrap();
    assert_eq!(tick(&mut world, 199_943), tick(&mut restored, 199_943));
    assert!(matches!(
        train(&world, &local_key).state,
        Moving {
            entered_ms: 199_943,
            ..
        }
    ));
    assert_eq!(
        train(&world, &express_key).state,
        Waiting { ready_ms: 20_000 }
    );
    assert_eq!(tick(&mut world, 300_000), tick(&mut restored, 300_000));
    assert_eq!(restored.operation_save(), world.operation_save());
    let Moving {
        arrival_ms,
        release_ms,
        ..
    } = train(&world, &local_key).state
    else {
        panic!("expected final platform to still occupy the shared lane")
    };
    assert_eq!(arrival_ms, 299_886);
    // Both sides of B occupy one actual block. Departing B does not
    // unlock it: the local still traverses B→C on that physical lane, and
    // the usual final-platform dwell/rear-clear rule governs final release.
    assert_eq!(release_ms, 399_886);
    tick(&mut world, release_ms - 1);
    assert_eq!(
        train(&world, &express_key).state,
        Waiting { ready_ms: 20_000 }
    );
    let mut before_rear_clear =
        rosenfu_storage::decode(&rosenfu_storage::encode(&world).unwrap()).unwrap();
    let exact = tick(&mut world, release_ms);
    assert_eq!(exact, tick(&mut before_rear_clear, release_ms));
    assert!(
        matches!(train(&world, &express_key).state, Moving { entered_ms, .. } if entered_ms == release_ms)
    );
    let followed = replay
        .completed_legs
        .iter()
        .find(|l| l.train.service_id == express_key)
        .unwrap();
    assert_eq!(followed.entered_ms, release_ms);
    assert_eq!(followed.cumulative_reservation_wait_ms, release_ms - 20_000);
    assert_eq!(
        world.operation().unwrap().runtime.passenger_offers(),
        planned
    );
    assert_eq!(world.operation_save(), before_rear_clear.operation_save());
    eprintln!("safe shared block: local leaves B at 199943 ms; express enters at modeled whole-block release {release_ms} ms");
}

#[test]
fn existing_fixed_runtime_for_one_block_remains_a_readable_unchanged_legacy_save() {
    use rosenfu_core::operation::OperationRuntime;
    let (mut world, [local, express]) = configured(0, 20_000, Layout::Shared);
    let specs = world.operation_save().unwrap().runtime.services;
    world.operation_mut().unwrap().runtime = OperationRuntime::new(specs).unwrap();
    world
        .operation_mut()
        .unwrap()
        .runtime
        .advance_to(150_000, 128)
        .unwrap();
    let bytes = rosenfu_storage::encode(&world).unwrap();
    let restored = rosenfu_storage::decode(&bytes).unwrap();
    use rosenfu_core::operation::TrainState::{Moving, Waiting};
    let runtime = &restored.operation().unwrap().runtime;
    assert!(runtime
        .trains()
        .any(|t| t.key.service_id == local.0.to_string()
            && matches!(t.state, Waiting { ready_ms: 199_943 })));
    assert!(runtime
        .trains()
        .any(|t| t.key.service_id == express.0.to_string()
            && matches!(
                t.state,
                Moving {
                    entered_ms: 103_143,
                    ..
                }
            )));
    assert_eq!(restored.operation_save().unwrap().runtime.format, 2);
    assert_eq!(restored.operation_save(), world.operation_save());
    assert_eq!(rosenfu_storage::encode(&restored).unwrap(), bytes);
}
