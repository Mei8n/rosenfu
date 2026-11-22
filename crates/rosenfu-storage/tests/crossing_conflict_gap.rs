#[allow(dead_code, unused_imports)]
#[path = "../../../src-tauri/src/session.rs"]
mod session;
use rosenfu_core::operation::{Timetable, TrainState, Vehicle};
use rosenfu_core::world::{InfrastructureLevel, OperationServiceConfig, RouteSegment, World};
use session::{PlannedAlignment, WorldEditor};

const CENTER: [f64; 2] = [139.55, 35.7]; // Inland Musashino area; geometry is fictitious.
fn plan(name: &str, points: [[f64; 2]; 2], level: InfrastructureLevel) -> PlannedAlignment {
    serde_json::from_value(serde_json::json!({
        "name":name,"points":points.map(|p|serde_json::json!({"longitude":p[0],"latitude":p[1]})),
        "trackType":"normal","infrastructure":{"track_count":1,"level":level},"stations":[]
    }))
    .unwrap()
}
fn fixture(crosses: bool, elevated: bool) -> (World, [OperationServiceConfig; 2]) {
    let [x, y] = CENTER;
    let level = if elevated {
        InfrastructureLevel::Elevated
    } else {
        InfrastructureLevel::Ground
    };
    let dy = 0.01 * y.to_radians().cos();
    let b = if crosses {
        [[x, y - dy], [x, y + dy]]
    } else {
        [[x - 0.01, y + 0.007], [x + 0.01, y + 0.007]]
    };
    let draft = WorldEditor::new()
        .build_construction_plan(
            vec![
                plan(
                    "EW",
                    [[x - 0.01, y], [x + 0.01, y]],
                    InfrastructureLevel::Ground,
                ),
                plan("NS or control", b, level),
            ],
            vec![],
        )
        .expect("native session ordinary construction accepts both separate alignments");
    assert_eq!(draft.alignments.len(), 2);
    assert!(draft.alignment_connections.is_empty());
    let ids = draft
        .alignments
        .iter()
        .map(|line| line.id)
        .collect::<Vec<_>>();
    let owner = draft.companies[0].id;
    let mut world = World::new(draft).unwrap();
    let configs = std::array::from_fn(|i| {
        let id = ids[i];
        let a = world
            .place_station_on_alignment(owner, format!("S{i}"), id, 0.1, 80., 10.)
            .unwrap();
        let b = world
            .place_station_on_alignment(owner, format!("T{i}"), id, 0.9, 80., 10.)
            .unwrap();
        let segments = vec![RouteSegment::Alignment {
            alignment_id: id,
            from: a,
            to: b,
        }];
        let preview = world.preview_service_path(a, segments.clone()).unwrap();
        let service_id = world
            .add_service_path(
                owner,
                format!("Service {i}"),
                a,
                segments,
                preview
                    .occurrences
                    .iter()
                    .map(|o| o.reference.clone())
                    .collect(),
            )
            .unwrap();
        OperationServiceConfig {
            service_id,
            timetable: Timetable {
                first_departure_ms: 0,
                last_departure_ms: 0,
                headway_ms: 60000,
            },
            vehicle: Vehicle {
                max_speed_kmh: 72.,
                acceleration_mps2: 1.,
                braking_mps2: 1.,
                length_m: 40.,
            },
            dwell_ms: 10000,
        }
    });
    (world, configs)
}
fn departure(crosses: bool, elevated: bool) -> (usize, usize) {
    let (mut w, configs) = fixture(crosses, elevated);
    if crosses {
        let (a, b) = (&w.snapshot().alignments[0], &w.snapshot().alignments[1]);
        let (p, q) = (a.point_at(0.5), b.point_at(0.5));
        assert!(
            (p.latitude - q.latitude).abs() < 1e-10 && (p.longitude - q.longitude).abs() < 1e-10
        );
        assert_eq!(
            a.height_at_position_m(0.5) == b.height_at_position_m(0.5),
            !elevated
        );
        assert!((a.length_m() - b.length_m()).abs() < 0.01); // Equal approach times at equal speeds.
    }
    w.configure_operation(configs.to_vec()).unwrap();
    w.operation_mut()
        .unwrap()
        .runtime
        .advance_to(0, 100)
        .unwrap();
    let counts = |w: &World| {
        let trains = w.operation().unwrap().runtime.trains();
        let states = trains.map(|t| &t.state).collect::<Vec<_>>();
        (
            states
                .iter()
                .filter(|s| matches!(s, TrainState::Moving { .. }))
                .count(),
            states
                .iter()
                .filter(|s| matches!(s, TrainState::Waiting { .. }))
                .count(),
        )
    };
    let before = counts(&w);
    let mut reload = rosenfu_storage::decode(&rosenfu_storage::encode(&w).unwrap()).unwrap();
    assert_eq!(counts(&reload), before);
    if crosses && !elevated && before == (1, 1) {
        let runtime = &w.operation().unwrap().runtime;
        let waiting = runtime
            .trains()
            .find(|t| matches!(t.state, TrainState::Waiting { .. }))
            .unwrap()
            .key
            .clone();
        let released = runtime
            .trains()
            .find_map(|t| match t.state {
                TrainState::Moving { release_ms, .. } => Some(release_ms),
                _ => None,
            })
            .unwrap();
        for editor in [&mut w, &mut reload] {
            let rt = &mut editor.operation_mut().unwrap().runtime;
            rt.advance_to(released - 1, 100).unwrap();
            assert!(matches!(
                rt.trains().find(|t| t.key == waiting).unwrap().state,
                TrainState::Waiting { .. }
            ));
            rt.advance_to(released, 100).unwrap();
            let delayed = rt.trains().find(|t| t.key == waiting).unwrap();
            assert!(
                matches!(delayed.state,TrainState::Moving{entered_ms,..} if entered_ms==released)
            );
            assert_eq!(delayed.delay_ms, released);
        }
        assert_eq!(w.operation_save(), reload.operation_save());
    }

    println!("crosses={crosses} elevated={elevated} moving_waiting={before:?}");
    before
}
#[test]
fn parallel_and_grade_separated_are_nonconflicting_controls() {
    assert_eq!(departure(false, false), (2, 0));
    assert_eq!(departure(true, true), (2, 0));
}
#[test]
fn planar_crossing_should_serialize_at_simultaneous_departure() {
    assert_eq!(departure(true, false), (1, 1));
}
#[test]
fn pre_crossing_runtime_is_accepted_byte_exact_without_changing_active_locks() {
    use rosenfu_core::operation::OperationRuntime;
    let (mut w, configs) = fixture(true, false);
    let prior_specs = configs
        .iter()
        .map(|c| w.compile_operation(c).unwrap())
        .collect();
    w.configure_operation(configs.to_vec()).unwrap();
    w.operation_mut().unwrap().runtime = OperationRuntime::new(prior_specs).unwrap();
    w.operation_mut()
        .unwrap()
        .runtime
        .advance_to(0, 100)
        .unwrap();
    let prior = rosenfu_storage::encode(&w).unwrap();
    let reread = rosenfu_storage::decode(&prior).unwrap();
    assert_eq!(reread.operation_save(), w.operation_save());
    assert_eq!(rosenfu_storage::encode(&reread).unwrap(), prior);
    assert_eq!(
        reread
            .operation()
            .unwrap()
            .runtime
            .trains()
            .filter(|t| matches!(t.state, TrainState::Moving { .. }))
            .count(),
        2
    );
}

#[test]
fn served_platform_near_crossing_rejects_new_configuration_atomically() {
    use rosenfu_core::operation::OperationError;
    let (mut w, configs) = fixture(true, false);
    w.configure_operation(configs.to_vec()).unwrap();
    let before = w.operation_save();
    let rail = &w.snapshot().alignments[0];
    let (id, owner) = (rail.id, rail.owner);
    let from = w
        .place_station_on_alignment(owner, "Near".into(), id, 0.50, 80., 10.)
        .unwrap();
    let to = w
        .snapshot()
        .station_placements
        .iter()
        .find(|p| p.alignment_id == id && p.position == 0.9)
        .unwrap()
        .station_id;
    let segments = vec![RouteSegment::Alignment {
        alignment_id: id,
        from,
        to,
    }];
    let preview = w.preview_service_path(from, segments.clone()).unwrap();
    let service_id = w
        .add_service_path(
            owner,
            "Near crossing".into(),
            from,
            segments,
            preview
                .occurrences
                .iter()
                .map(|o| o.reference.clone())
                .collect(),
        )
        .unwrap();
    assert_eq!(
        w.configure_operation(vec![
            OperationServiceConfig {
                service_id,
                ..configs[0].clone()
            },
            configs[1].clone()
        ]),
        Err(OperationError::CrossingPlatform)
    );
    assert_eq!(w.operation_save(), before);
}

#[test]
fn saved_crossing_marker_rejects_partially_or_fully_stripped_locks() {
    let (mut w, configs) = fixture(true, false);
    w.configure_operation(configs.to_vec()).unwrap();
    let saved = w.operation_save().unwrap();
    assert!(saved.crossing_reservations);
    let mut forged: serde_json::Value =
        serde_json::from_slice(&rosenfu_storage::encode(&w).unwrap()).unwrap();
    forged.as_object_mut().unwrap().remove("fixed_od_scenario");
    forged["schema_version"] = 26.into();
    assert!(rosenfu_storage::decode(&serde_json::to_vec(&forged).unwrap()).is_err());

    for all in [false, true] {
        let mut bad = saved.clone();
        let resources = bad
            .runtime
            .services
            .iter_mut()
            .flat_map(|s| &mut s.legs)
            .flat_map(|l| &mut l.sections)
            .map(|s| &mut s.resources);
        let mut removed = false;
        for list in resources {
            if let Some(i) = list.iter().position(|r| r.starts_with("rail:crossing:")) {
                list.remove(i);
                removed = true;
                if !all {
                    break;
                }
            }
        }
        assert!(removed);
        assert!(w.restore_operation(Some(bad)).is_err());
        assert_eq!(w.operation_save().unwrap(), saved);
    }
}

#[test]
fn runtime_crossing_delay_is_not_part_of_planned_fixed_od() {
    let run = |elevated| {
        let (mut world, configs) = fixture(true, elevated);
        world.configure_operation(configs.to_vec()).unwrap();
        let first = world.operation().unwrap().runtime.passenger_offers();
        let arrivals = world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(600_000, 128)
            .unwrap()
            .arrivals;
        let runtime = &world.operation().unwrap().runtime;
        assert_eq!(runtime.passenger_offers(), first);
        let service = configs
            .iter()
            .map(|c| c.service_id.0.to_string())
            .max()
            .unwrap();
        let offer = first
            .iter()
            .find(|offer| offer.service_id == service && offer.sequence == 0)
            .unwrap();
        let delayed = arrivals
            .iter()
            .find(|a| a.train.service_id == service && a.station_id == offer.to)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.rosenfu.json");
        rosenfu_storage::save_world(&path, &world).unwrap();
        let mut editor = WorldEditor::new();
        editor.load(&path).unwrap();
        let od = editor
            .preview_fixed_od(offer.from.clone(), offer.to.clone(), Some(100.), Some(80.))
            .unwrap();
        let path = od.rail.first().unwrap();
        println!("elevated={elevated} runtime_arrival_ms={} accumulated_delay_ms={} planned_ride_s={} planned_wait_s={} allocated_trips={}", delayed.at_ms, delayed.delay_ms, path.time.ride_seconds, path.time.wait_seconds, path.trips);
        (od, delayed.at_ms, delayed.delay_ms)
    };
    let (ground, ground_arrival, ground_delay) = run(false);
    let (separated, separated_arrival, separated_delay) = run(true);
    assert_ne!(ground_delay, separated_delay);
    assert_eq!(ground_arrival - separated_arrival, ground_delay);
    assert_eq!(separated_delay, 0);
    assert_eq!(ground, separated);
}

#[test]
fn independent_replay_observes_delays_without_changing_plans_or_saves() {
    use rosenfu_core::operation::PassengerReplayError;
    for elevated in [false, true] {
        let (mut world, configs) = fixture(true, elevated);
        world.configure_operation(configs.to_vec()).unwrap();
        world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(1_000, 100)
            .unwrap();
        let bytes = rosenfu_storage::encode(&world).unwrap();
        let restored = rosenfu_storage::decode(&bytes).unwrap();
        let runtime = &world.operation().unwrap().runtime;
        let plans = runtime.passenger_offers();
        let replay = runtime.preview_passenger_replay(600_000, 128).unwrap();
        assert_eq!(
            replay,
            restored
                .operation()
                .unwrap()
                .runtime
                .preview_passenger_replay(600_000, 128)
                .unwrap()
        );
        assert_eq!(plans, runtime.passenger_offers());
        assert_eq!(rosenfu_storage::encode(&restored).unwrap(), bytes);
        assert_eq!(rosenfu_storage::encode(&world).unwrap(), bytes);
        let partial = runtime.preview_passenger_replay(600_000, 1).unwrap();
        assert!(!partial.complete);
        assert!(partial.observed_until_ms < partial.requested_until_ms);
        assert_eq!(
            runtime.preview_passenger_replay(3_601_001, 128),
            Err(PassengerReplayError::OutsideBudget)
        );
        assert_eq!(
            runtime.preview_passenger_replay(600_000, 0),
            Err(PassengerReplayError::OutsideBudget)
        );
        assert!(replay.complete);
        assert_eq!(replay.observed_from_ms, 1_000);
        assert!(replay
            .completed_legs
            .iter()
            .any(|leg| leg.entered_ms < replay.observed_from_ms));
        let service = configs
            .iter()
            .map(|c| c.service_id.0.to_string())
            .max()
            .unwrap();
        let leg = replay
            .completed_legs
            .iter()
            .find(|l| l.train.service_id == service && l.sequence == 0)
            .unwrap();
        let offer = plans
            .iter()
            .find(|o| o.service_id == service && o.sequence == 0)
            .unwrap();
        assert_eq!((&leg.from, &leg.to), (&offer.from, &offer.to));
        assert_eq!(leg.entered_ms, leg.cumulative_reservation_wait_ms);
        assert_eq!(leg.entered_ms == 0, elevated);
        assert!(
            ((leg.arrived_ms - leg.entered_ms) as f64 / 1000. - offer.ride_seconds).abs() < 1e-9
        );
        println!(
            "elevated={elevated} enter_ms={} arrival_ms={} cumulative_wait_ms={}",
            leg.entered_ms, leg.arrived_ms, leg.cumulative_reservation_wait_ms
        );
    }
}

#[test]
fn native_journey_replay_respects_real_occupancy_and_does_not_replace_planned_od() {
    let preview = |elevated| {
        let (mut world, configs) = fixture(true, elevated);
        world.configure_operation(configs.to_vec()).unwrap();
        let service = configs
            .iter()
            .map(|c| c.service_id.0.to_string())
            .max()
            .unwrap();
        let offers = world.operation().unwrap().runtime.passenger_offers();
        let leg = offers
            .iter()
            .find(|o| o.service_id == service && o.sequence == 0)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.rosenfu.json");
        rosenfu_storage::save_world(&path, &world).unwrap();
        let mut editor = WorldEditor::new();
        editor.load(&path).unwrap();
        let before = serde_json::to_value(editor.operation_snapshot()).unwrap();
        let planned = editor
            .preview_fixed_od(leg.from.clone(), leg.to.clone(), Some(100.), Some(80.))
            .unwrap();
        let result = editor
            .preview_rail_journey_replay(&leg.from, &leg.to)
            .unwrap();
        let disk_before = std::fs::read(&path).unwrap();
        let observed = editor
            .preview_observed_fixed_od(leg.from.clone(), leg.to.clone(), Some(100.), Some(130.))
            .unwrap();
        assert_eq!(observed.itinerary.as_ref(), Some(&result));
        assert!((observed.rail_trips + observed.alternative_trips - 100.).abs() < 1e-12);
        assert!(editor
            .preview_observed_fixed_od(
                "bus:unsupported".into(),
                leg.to.clone(),
                Some(100.),
                Some(130.)
            )
            .is_err());
        assert_eq!(std::fs::read(&path).unwrap(), disk_before);
        assert_eq!(
            serde_json::to_value(editor.operation_snapshot()).unwrap(),
            before
        );
        assert_eq!(
            editor
                .preview_fixed_od(leg.from.clone(), leg.to.clone(), Some(100.), Some(80.))
                .unwrap(),
            planned
        );
        assert!(editor
            .preview_rail_journey_replay("missing", &leg.to)
            .is_err());
        (planned, result, observed)
    };
    let (plan_ground, ground, observed_ground) = preview(false);
    let (plan_elevated, elevated, observed_elevated) = preview(true);
    assert!(observed_ground.rail_trips < 50.);
    assert!(observed_elevated.rail_trips > 50.);
    assert_eq!(plan_ground, plan_elevated);
    assert_eq!(ground.arrival_ms, Some(186_480));
    assert_eq!(elevated.arrival_ms, Some(92_240));
    assert_eq!(
        (ground.platform_wait_ms, elevated.platform_wait_ms),
        (94_240, 0)
    );
    assert_eq!((ground.onboard_ms, elevated.onboard_ms), (92_240, 92_240));
}
