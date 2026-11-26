use rosenfu_core::operation::{OperationRuntime, Timetable, TrainState, Vehicle};
use rosenfu_core::world::{
    GeoPoint, InfrastructureLevel, OperationServiceConfig, RouteSegment, SectionInfrastructure,
    World,
};

fn fixture(
    level: InfrastructureLevel,
    tracks: u8,
    shared_terminal: bool,
    opposite: bool,
) -> (World, Vec<OperationServiceConfig>) {
    let mut w = World::default();
    let player = w.add_player("QA".into()).unwrap();
    let owner = w.add_company(player, "Rail".into()).unwrap();
    let points = (0..4)
        .map(|index| GeoPoint {
            latitude: 35.0,
            longitude: 139.0 + f64::from(index) * 0.0044,
        })
        .collect();
    let line = w.build_alignment(owner, "Corridor".into(), points).unwrap();
    let mut draft = w.snapshot().clone();
    draft.alignments[0].anchor_indices = Some(vec![0, 1, 2, 3]);
    draft.alignments[0].infrastructure = vec![
        SectionInfrastructure {
            track_count: tracks,
            level
        };
        3
    ];
    w.replace(draft).unwrap();
    let mut station = |name: &str, position| {
        w.place_station_on_alignment(owner, name.into(), line, position, 60., 10.)
            .unwrap()
    };
    let a = station("A", 0.10);
    let b = station("B", 0.23);
    let d = station("D", 0.90);
    let paths = [
        (a, d),
        if opposite {
            (d, a)
        } else if shared_terminal {
            (a, d)
        } else {
            (a, b)
        },
    ];
    let configs = paths
        .into_iter()
        .enumerate()
        .map(|(i, (from, to))| {
            let legs = vec![RouteSegment::Alignment {
                alignment_id: line,
                from,
                to,
            }];
            let preview = w.preview_service_path(from, legs.clone()).unwrap();
            let stops = vec![
                preview.occurrences.first().unwrap().reference.clone(),
                preview.occurrences.last().unwrap().reference.clone(),
            ];
            let service_id = w
                .add_service_path(owner, format!("Service {i}"), from, legs, stops)
                .unwrap();
            let time = 1_000 * i as u64;
            OperationServiceConfig {
                service_id,
                timetable: Timetable {
                    first_departure_ms: time,
                    last_departure_ms: time,
                    headway_ms: 60_000,
                },
                vehicle: Vehicle {
                    max_speed_kmh: 72.,
                    acceleration_mps2: 1.,
                    braking_mps2: 1.,
                    length_m: 40.,
                },
                dwell_ms: 10_000,
            }
        })
        .collect();
    (w, configs)
}
fn at(w: &mut World, ms: u64) -> rosenfu_core::operation::AdvanceResult {
    w.operation_mut()
        .unwrap()
        .runtime
        .advance_to(ms, 128)
        .unwrap()
}
fn waiting_at(w: &World, config: &OperationServiceConfig) -> bool {
    w.operation().unwrap().runtime.trains().any(|t| {
        t.key.service_id == config.service_id.0.to_string()
            && matches!(t.state, TrainState::Waiting { .. })
    })
}

#[test]
fn follower_uses_rear_cleared_early_block_before_leader_arrives_and_save_reopens() {
    let (mut w, c) = fixture(InfrastructureLevel::Ground, 2, false, false);
    let legacy = c
        .iter()
        .map(|conf| w.compile_operation(conf).unwrap())
        .collect();
    w.configure_operation(c.clone()).unwrap();
    assert_eq!(w.operation_save().unwrap().runtime.format, 5);
    let mut old = w.clone();
    old.operation_mut().unwrap().runtime = OperationRuntime::new(legacy).unwrap();
    at(&mut w, 1000);
    at(&mut old, 1000);
    assert!(waiting_at(&w, &c[1]));
    let bytes = rosenfu_storage::encode(&w).unwrap();
    let mut loaded = rosenfu_storage::decode(&bytes).unwrap();
    assert_eq!(rosenfu_storage::encode(&loaded).unwrap(), bytes);
    assert!(!loaded.operation_save().unwrap().invalidated);
    let mut edited = World::new(w.snapshot().clone()).unwrap();
    edited.retain_operation_from(&w);
    assert_eq!(edited.operation_save(), w.operation_save());
    let mut boundary = loaded.clone();
    let now = at(&mut w, 600_000).arrivals;
    assert_eq!(at(&mut loaded, 600_000).arrivals, now);
    let previous = at(&mut old, 600_000).arrivals;
    let id = c[1].service_id.0.to_string();
    let find = |events: &[rosenfu_core::operation::Arrival]| {
        events
            .iter()
            .find(|a| a.train.service_id == id)
            .map(|a| (a.at_ms, a.delay_ms))
            .unwrap()
    };
    let (arrival, hold) = find(&now);
    let (slow, old_hold) = find(&previous);
    println!("following arrival_ms={arrival}, hold_ms={hold}; previous arrival_ms={slow}, hold_ms={old_hold}");
    assert!(0 < hold && hold < old_hold && arrival < slow);
    assert!(
        hold + c[1].timetable.first_departure_ms
            < now
                .iter()
                .find(|a| a.train.service_id == c[0].service_id.0.to_string())
                .unwrap()
                .at_ms
    );
    let entry = hold + c[1].timetable.first_departure_ms;
    at(&mut boundary, entry - 1);
    assert!(waiting_at(&boundary, &c[1]));
    at(&mut boundary, entry);
    assert!(!waiting_at(&boundary, &c[1]));
}

#[test]
fn same_full_path_still_waits_but_opposing_physical_lane_does_not() {
    for (level, opposite, format) in [
        (InfrastructureLevel::Ground, false, 5),
        (InfrastructureLevel::Ground, true, 5),
        (InfrastructureLevel::Elevated, true, 2),
    ] {
        let (mut w, c) = fixture(level, 2, !opposite, opposite);
        w.configure_operation(c.clone()).unwrap();
        assert_eq!(w.operation_save().unwrap().runtime.format, format);
        at(&mut w, 1_000);
        assert_eq!(waiting_at(&w, &c[1]), !opposite);
        let arrivals = at(&mut w, 600_000).arrivals;
        let lag = arrivals
            .iter()
            .find(|a| a.train.service_id == c[1].service_id.0.to_string())
            .unwrap()
            .delay_ms;
        assert_eq!(lag == 0, opposite);
    }
}

#[test]
fn grade_single_track_turnback_and_forged_ingress_never_admit_format_three() {
    for (level, track) in [
        (InfrastructureLevel::Ground, 1),
        (InfrastructureLevel::Elevated, 2),
    ] {
        let (mut w, c) = fixture(level, track, false, false);
        w.configure_operation(c).unwrap();
        let mut op = w.operation_save().unwrap();
        assert_ne!(op.runtime.format, 5);
        op.runtime.format = 3;
        assert!(World::new(w.snapshot().clone())
            .unwrap()
            .restore_operation(Some(op))
            .is_err());
    }

    let (mut w, c) = fixture(InfrastructureLevel::Ground, 2, false, false);
    w.configure_operation(c).unwrap();
    at(&mut w, 1000);
    let (mut elevated, _) = fixture(InfrastructureLevel::Elevated, 2, false, false);
    elevated.retain_operation_from(&w);
    assert!(elevated.operation_save().unwrap().invalidated);
    let frozen = rosenfu_storage::encode(&elevated).unwrap();
    let reopened = rosenfu_storage::decode(&frozen).unwrap();
    assert!(reopened.operation_save().unwrap().invalidated);
    assert!(reopened.operation_save().unwrap().runtime.paused);
    assert_eq!(rosenfu_storage::encode(&reopened).unwrap(), frozen);
}

#[test]
fn same_timetable_short_headway_still_reserves_every_required_section() {
    let (mut w, mut configs) = fixture(InfrastructureLevel::Ground, 2, false, false);
    configs.truncate(1);
    configs[0].timetable.last_departure_ms = 1_000;
    configs[0].timetable.headway_ms = 1_000;
    w.configure_operation(configs).unwrap();
    assert_eq!(w.operation_save().unwrap().runtime.format, 5);
    at(&mut w, 1_000);
    let lead_clear = w
        .operation()
        .unwrap()
        .runtime
        .trains()
        .find_map(|t| match t.state {
            TrainState::Moving { release_ms, .. } => Some(release_ms),
            _ => None,
        })
        .unwrap();
    at(&mut w, lead_clear - 1);
    assert!(w
        .operation()
        .unwrap()
        .runtime
        .trains()
        .any(|t| { t.key.scheduled_ms == 1_000 && matches!(t.state, TrainState::Waiting { .. }) }));
    at(&mut w, lead_clear);
    assert!(w.operation().unwrap().runtime.trains().any(|t| {
        t.key.scheduled_ms == 1_000
            && matches!(t.state, TrainState::Moving { entered_ms, .. } if entered_ms == lead_clear)
    }));
}

#[test]
fn inline_terminal_dwell_preserves_active_and_frozen_operation_bytes() {
    let (mut world, mut configs) = fixture(InfrastructureLevel::Ground, 2, true, false);
    configs[0].dwell_ms = 30_000;
    world.configure_operation(configs.clone()).unwrap();
    let leader_id = configs[0].service_id.0.to_string();
    at(&mut world, 1_000);
    let lead = world
        .operation()
        .unwrap()
        .runtime
        .trains()
        .find(|t| t.key.service_id == leader_id)
        .unwrap();
    let TrainState::Moving {
        arrival_ms,
        release_ms,
        ..
    } = lead.state
    else {
        panic!("expected leader travelling")
    };
    assert_eq!(release_ms, arrival_ms + 30_000);
    at(&mut world, release_ms - 1);
    assert!(waiting_at(&world, &configs[1]));
    let bytes = rosenfu_storage::encode(&world).unwrap();
    let mut opened = rosenfu_storage::decode(&bytes).unwrap();
    assert_eq!(rosenfu_storage::encode(&opened).unwrap(), bytes);
    let arrivals = at(&mut world, 300_000);
    assert_eq!(at(&mut opened, 300_000), arrivals);
    assert_eq!(opened.operation_save(), world.operation_save());

    // Platform geometry no longer proves the train fits. A live copied
    // runtime cannot be admitted, and a real edit keeps a paused snapshot.
    let mut changed = world.snapshot().clone();
    let terminal = world
        .operation()
        .unwrap()
        .runtime
        .service_spec(&leader_id)
        .unwrap()
        .legs[0]
        .to
        .parse::<u64>()
        .unwrap();
    changed
        .station_placements
        .iter_mut()
        .find(|p| p.station_id.0 == terminal)
        .unwrap()
        .length_m = 32.;
    let mut edited = World::new(changed).unwrap();
    assert!(edited
        .restore_operation(Some(world.operation_save().unwrap()))
        .is_err());
    edited.retain_operation_from(&world);
    assert!(edited.operation_save().unwrap().invalidated);
    let frozen = rosenfu_storage::encode(&edited).unwrap();
    let after = rosenfu_storage::decode(&frozen).unwrap();
    assert!(after.operation_save().unwrap().runtime.paused);
    assert_eq!(rosenfu_storage::encode(&after).unwrap(), frozen);
    edited.configure_operation(configs).unwrap();
    at(&mut edited, 1_000);
    let mut legacy = edited.operation_save().unwrap();
    assert_eq!(legacy.runtime.format, 3);
    let format3 = rosenfu_storage::encode(&edited).unwrap();
    let older = rosenfu_storage::decode(&format3).unwrap();
    assert_eq!(rosenfu_storage::encode(&older).unwrap(), format3);

    // Re-labeling the old snapshot does not manufacture platform qualification.
    legacy.runtime.format = 5;
    assert!(edited.restore_operation(Some(legacy)).is_err());
}

#[test]
fn inline_terminal_restore_rejects_impossible_first_leg_timing() {
    use rosenfu_core::operation::OperationError;

    let (mut world, mut configs) = fixture(InfrastructureLevel::Ground, 2, true, false);
    // Both timetable entries lie inside the observed clock; a fabricated
    // early admission must not be able to shorten the first terminal lease.
    for config in &mut configs {
        config.timetable.first_departure_ms += 1_000;
        config.timetable.last_departure_ms += 1_000;
    }
    configs[0].dwell_ms = 30_000;
    world.configure_operation(configs).unwrap();
    at(&mut world, 3_000);
    let saved = world.operation_save().unwrap();
    assert_eq!(saved.runtime.format, 5);
    let mut reopen = World::new(world.snapshot().clone()).unwrap();
    reopen.restore_operation(Some(saved.clone())).unwrap();

    let mut forged = saved.clone();
    let moving = forged
        .runtime
        .trains
        .iter_mut()
        .find(|t| matches!(t.state, TrainState::Moving { .. }))
        .unwrap();
    let TrainState::Moving {
        entered_ms,
        arrival_ms,
        release_ms,
    } = &mut moving.state
    else {
        unreachable!("selected moving train")
    };
    *entered_ms -= 1;
    *arrival_ms -= 1;
    *release_ms -= 1;
    assert_eq!(
        reopen.restore_operation(Some(forged)),
        Err(OperationError::InvalidClock)
    );

    let mut forged = saved.clone();
    forged
        .runtime
        .trains
        .iter_mut()
        .find(|t| matches!(t.state, TrainState::Moving { .. }))
        .unwrap()
        .delay_ms += 1;
    assert_eq!(
        reopen.restore_operation(Some(forged)),
        Err(OperationError::InvalidClock)
    );

    let mut forged = saved.clone();
    let queue = forged
        .runtime
        .trains
        .iter_mut()
        .find(|t| matches!(t.state, TrainState::Waiting { .. }))
        .unwrap();
    queue.state = TrainState::Waiting {
        ready_ms: queue.key.scheduled_ms + 1,
    };
    assert_eq!(
        reopen.restore_operation(Some(forged)),
        Err(OperationError::InvalidClock)
    );

    let mut forged = saved.clone();
    forged
        .runtime
        .trains
        .iter_mut()
        .find(|t| matches!(t.state, TrainState::Waiting { .. }))
        .unwrap()
        .delay_ms += 1;
    assert_eq!(
        reopen.restore_operation(Some(forged)),
        Err(OperationError::InvalidClock)
    );
    assert_eq!(reopen.operation_save(), Some(saved));

    // A real late admission keeps the recorded first-leg delay on reopen.
    at(&mut world, 100_000);
    let saved = world.operation_save().unwrap();
    assert!(saved.runtime.trains.iter().any(|t| t.delay_ms > 0));
    reopen.restore_operation(Some(saved.clone())).unwrap();
    assert_eq!(reopen.operation_save(), Some(saved));
}
