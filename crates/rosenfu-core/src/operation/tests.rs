use super::*;
fn service(id: &str, resource: &str) -> ServiceRunSpec {
    ServiceRunSpec {
        service_id: id.into(),
        vehicle: Vehicle {
            max_speed_kmh: 72.0,
            acceleration_mps2: 1.0,
            braking_mps2: 1.0,
            length_m: 20.0,
        },
        timetable: Timetable {
            first_departure_ms: 0,
            last_departure_ms: 0,
            headway_ms: 60000,
        },
        legs: vec![RunLeg {
            from: "1".into(),
            to: "2".into(),
            dwell_ms: 10000,
            sections: vec![Section {
                length_m: 100.0,
                speed_limit_kmh: 72.0,
                resources: vec![resource.into()],
            }],
        }],
    }
}
#[test]
fn triangular_short_run() {
    let s = service("1", "a");
    assert_eq!(running_time_ms(&s.legs[0].sections, &s.vehicle), Ok(20000));
}
#[test]
fn lower_cap_increases_time() {
    let s = service("1", "a");
    let mut slow = s.clone();
    slow.legs[0].sections[0].speed_limit_kmh = 18.;
    assert!(running_time_ms(&slow.legs[0].sections, &slow.vehicle).unwrap() > 20000);
}
#[test]
fn conflicting_trains_wait_but_independent_run() {
    let mut r = OperationRuntime::new(vec![
        service("1", "a"),
        service("2", "a"),
        service("3", "b"),
    ])
    .unwrap();
    r.advance_to(0, 10).unwrap();
    let t: Vec<_> = r.trains().collect();
    assert!(matches!(t[0].state, TrainState::Moving { .. }));
    assert!(matches!(t[1].state, TrainState::Waiting { .. }));
    assert!(matches!(t[2].state, TrainState::Moving { .. }));
    let out = r.advance_to(50000, 100).unwrap();
    assert_eq!(out.arrivals.len(), 3);
    assert!(out
        .arrivals
        .iter()
        .any(|a| a.train.service_id == "2" && a.delay_ms > 0));
    assert_eq!(r.trains().count(), 0);
    assert!(r.occupied.is_empty());
}
#[test]
fn frame_chunking_and_save_replay_are_deterministic() {
    let mut s = service("1", "a");
    s.timetable.last_departure_ms = 120000;
    let mut one = OperationRuntime::new(vec![s]).unwrap();
    let mut many = one.clone();
    let expected = one.advance_to(200000, 100).unwrap().arrivals;
    let mut actual = Vec::new();
    for now in (0..=200000).step_by(1000) {
        actual.extend(many.advance_to(now, 100).unwrap().arrivals);
    }
    assert_eq!(expected, actual);
    assert_eq!(one.save(), many.save());
    let restored = OperationRuntime::restore(many.save()).unwrap();
    assert_eq!(restored.trains, one.trains);
}
#[test]
fn pause_speed_and_backward_time() {
    let mut r = OperationRuntime::new(vec![service("1", "a")]).unwrap();
    assert_eq!(r.wall_target(1000), Ok(0));
    r.set_paused(false);
    r.set_speed(4).unwrap();
    assert_eq!(r.wall_target(1000), Ok(4000));
    r.advance_to(4000, 100).unwrap();
    assert_eq!(r.advance_to(3999, 100), Err(OperationError::InvalidClock));
    assert_eq!(r.set_speed(0), Err(OperationError::InvalidClock));
}
#[test]
fn budget_is_backpressure_not_dropped_trains() {
    let mut s = service("1", "a");
    s.timetable.last_departure_ms = 200000;
    let mut r = OperationRuntime::new(vec![s]).unwrap();
    let mut baseline = r.clone();
    let expected = baseline.advance_to(300000, 1000).unwrap().arrivals;
    let mut actual = Vec::new();
    loop {
        let result = r.advance_to(300000, 1).unwrap();
        actual.extend(result.arrivals);
        if result.reached_target {
            break;
        }
    }
    assert_eq!(actual, expected);
}
#[test]
fn invalid_input_rejected() {
    let mut s = service("1", "a");
    s.timetable.headway_ms = 0;
    assert!(OperationRuntime::new(vec![s]).is_err());
    let mut s = service("1", "a");
    s.vehicle.acceleration_mps2 = f64::NAN;
    assert!(OperationRuntime::new(vec![s]).is_err());
}
#[test]
fn intermediate_stop_affects_passenger_time() {
    let mut s = service("1", "a");
    let mut l = s.legs[0].clone();
    l.from = "2".into();
    l.to = "3".into();
    s.legs.push(l);
    let r = OperationRuntime::new(vec![s]).unwrap();
    let offers = r.passenger_offers();
    assert_eq!(offers[0].ride_seconds, 20.);
    assert_eq!(offers[0].through_seconds, 0.);
    assert_eq!(offers[1].ride_seconds, 20.);
    assert_eq!(offers[1].through_seconds, 10.);
}

#[test]
fn corrupted_waiting_and_duplicate_train_restore_is_rejected() {
    let mut r = OperationRuntime::new(vec![service("1", "a")]).unwrap();
    r.advance_to(1000, 100).unwrap();
    let mut saved = r.save();
    saved.trains[0].state = TrainState::Waiting { ready_ms: 0 };
    assert!(OperationRuntime::restore(saved).is_err());
    let mut saved = r.save();
    saved.trains.push(saved.trains[0].clone());
    assert!(OperationRuntime::restore(saved).is_err());
    let mut saved = r.save();
    if let TrainState::Moving {
        ref mut arrival_ms, ..
    } = saved.trains[0].state
    {
        *arrival_ms += 1;
    }
    assert!(OperationRuntime::restore(saved).is_err());
}

const FIRST: &str = "rail:1:section:0:lane:0";
const SECOND: &str = "rail:1:section:1:lane:0";
const THIRD: &str = "rail:1:section:2:lane:0";

fn occupancy_fixture() -> Vec<ServiceRunSpec> {
    let mut leader = service("1", FIRST);
    leader.legs[0]
        .sections
        .extend([SECOND, THIRD].map(|r| service("1", r).legs[0].sections[0].clone()));
    // An identical following service needs all three locks and must wait until
    // the end. A short service on the cleared block, in either direction, can
    // depart earlier. Parallel uses a distinct lane.
    vec![
        leader.clone(),
        service("2", FIRST),
        service("3", "rail:1:section:0:lane:1"),
        ServiceRunSpec {
            service_id: "4".into(),
            ..leader
        },
    ]
}

#[test]
fn progressive_tail_release_allows_only_subset_following() {
    let mut specs = occupancy_fixture();
    specs[1].legs[0].from = "2".into();
    specs[1].legs[0].to = "1".into();
    let mut r = OperationRuntime::new_with_progressive_rail(specs.clone()).unwrap();
    let mut legacy = OperationRuntime::new(specs).unwrap();
    r.advance_to(0, 100).unwrap();
    let key = |id: &str| TrainKey {
        service_id: id.to_owned(),
        scheduled_ms: 0,
    };
    assert!(matches!(
        r.trains[&key("2")].state,
        TrainState::Waiting { .. }
    ));
    assert!(matches!(
        r.trains[&key("3")].state,
        TrainState::Moving { .. }
    ));
    assert!(matches!(
        r.trains[&key("4")].state,
        TrainState::Waiting { .. }
    ));
    let at = r.services["1"].legs[0].early_releases[FIRST];
    let leg_end = r.services["1"].legs[0].duration_ms + r.services["1"].legs[0].clearance_ms;
    assert!(at < leg_end);
    r.advance_to(at - 1, 100).unwrap();
    assert_eq!(r.occupied[FIRST], key("1"));
    r.advance_to(at, 100).unwrap();
    legacy.advance_to(at, 100).unwrap();
    assert_eq!(r.occupied[FIRST], key("2"));
    assert_eq!(r.occupied[THIRD], key("1"));
    assert_eq!(legacy.occupied[FIRST], key("1"));
    assert_eq!(legacy.save().format, 2);
    assert_eq!(r.save().format, 3);
    assert!(matches!(
        r.trains[&key("4")].state,
        TrainState::Waiting { .. }
    ));
    r.advance_to(leg_end, 100).unwrap();
    // Old owner cannot remove the new train's lock at its final release.
    assert_eq!(r.occupied[FIRST], key("2"));
}

#[test]
fn repeated_last_use_and_unknown_point_or_bus_never_release_early() {
    let mut spec = occupancy_fixture().remove(0);
    spec.legs[0].sections[0]
        .resources
        .push("rail:1:point:1".into());
    let mut bus = spec.clone();
    bus.service_id = "bus:1".into();
    let mut repeated = spec.clone();
    repeated.service_id = "5".into();
    repeated.legs[0].sections[2].resources.push(FIRST.into());
    let r = OperationRuntime::new_with_progressive_rail(vec![spec, repeated, bus]).unwrap();
    assert_eq!(
        r.services["1"].legs[0]
            .early_releases
            .keys()
            .collect::<Vec<_>>(),
        vec![FIRST]
    );
    assert!(r.services["5"].legs[0].early_releases.is_empty());
    assert!(r.services["bus:1"].legs[0].early_releases.is_empty());
}

#[test]
fn progressive_snapshot_mid_release_and_event_chunking_match() {
    let specs = occupancy_fixture();
    let mut r = OperationRuntime::new_with_progressive_rail(specs).unwrap();
    let mut stepped = r.clone();
    let boundary = r.services["1"].legs[0].early_releases[FIRST];
    let mut events = r.advance_to(boundary, 100).unwrap().arrivals;
    let bytes = serde_json::to_vec(&r.save()).unwrap();
    let restored = OperationRuntime::restore(serde_json::from_slice(&bytes).unwrap()).unwrap();
    assert_eq!(restored, r);
    events.extend(r.advance_to(200_000, 100).unwrap().arrivals);
    let mut stepped_events = Vec::new();
    loop {
        let result = stepped.advance_to(200_000, 1).unwrap();
        stepped_events.extend(result.arrivals);
        if result.reached_target {
            break;
        }
    }
    assert_eq!(events, stepped_events);
    assert_eq!(r.save(), stepped.save());
    assert!(r.occupied.is_empty());
}

#[test]
fn terminal_hold_keeps_same_train_and_blocks_opposing_admission_until_return_clears() {
    fn round_trip(id: &str, resource: &str) -> ServiceRunSpec {
        let mut spec = service(id, resource);
        spec.legs[0].dwell_ms = 30_000;
        let mut reverse = spec.legs[0].clone();
        std::mem::swap(&mut reverse.from, &mut reverse.to);
        spec.legs.push(reverse);
        spec
    }
    let rail = "rail:1:section:0:lane:0";
    let mut runtime = OperationRuntime::new_with_terminal_hold(vec![
        round_trip("1", rail),
        round_trip("2", rail),
        round_trip("3", "rail:2:section:0:lane:0"),
    ])
    .unwrap();
    runtime.advance_to(0, 100).unwrap();
    let leader = TrainKey {
        service_id: "1".into(),
        scheduled_ms: 0,
    };
    let other = TrainKey {
        service_id: "2".into(),
        scheduled_ms: 0,
    };
    assert!(matches!(
        runtime
            .trains
            .values()
            .find(|t| t.key.service_id == "3")
            .unwrap()
            .state,
        TrainState::Moving { entered_ms: 0, .. }
    ));
    let arrived = runtime.advance_to(20_001, 100).unwrap();
    assert_eq!(
        arrived
            .arrivals
            .iter()
            .filter(|a| a.train == leader)
            .count(),
        1
    );
    assert_eq!(Some(&leader), runtime.occupied.get(rail));
    let saved: OperationSave =
        serde_json::from_slice(&serde_json::to_vec(&runtime.save()).unwrap()).unwrap();
    assert_eq!(saved.format, 4);
    assert!(matches!(
        OperationRuntime::restore(saved.clone()),
        Err(OperationError::InvalidClock)
    ));
    let mut bad = saved.clone();
    let TrainState::Moving { release_ms, .. } = &mut bad.trains[0].state else {
        panic!("leader must retain the lane at the terminal")
    };
    *release_ms -= 1;
    assert!(matches!(
        OperationRuntime::restore_with_terminal_hold(bad),
        Err(OperationError::InvalidClock)
    ));
    let mut reopened = OperationRuntime::restore_with_terminal_hold(saved).unwrap();
    let before_advance = reopened.save();
    let continuous = runtime.advance_to(200_000, 100).unwrap();
    reopened.advance_to(49_999, 100).unwrap();
    assert_eq!(Some(&leader), reopened.occupied.get(rail));
    assert!(matches!(
        reopened.trains[&other].state,
        TrainState::Waiting { ready_ms: 0 }
    ));
    reopened.advance_to(50_000, 100).unwrap();
    assert_eq!(1, reopened.trains[&leader].leg);
    assert_eq!(0, reopened.trains[&leader].delay_ms);
    assert_eq!(Some(&leader), reopened.occupied.get(rail));
    reopened.advance_to(70_999, 100).unwrap();
    assert!(matches!(
        reopened.trains[&other].state,
        TrainState::Waiting { .. }
    ));
    reopened.advance_to(71_000, 100).unwrap();
    assert_eq!(71_000, reopened.trains[&other].delay_ms);
    assert_eq!(Some(&other), reopened.occupied.get(rail));
    reopened.advance_to(200_000, 100).unwrap();
    assert_eq!(runtime.save(), reopened.save());
    let mut replay = OperationRuntime::restore_with_terminal_hold(before_advance).unwrap();
    assert_eq!(continuous, replay.advance_to(200_000, 100).unwrap());
}

#[test]
fn release_and_departure_time_ties_respect_existing_waiting_order_after_resume() {
    let mut later = service("0", "shared");
    later.timetable.first_departure_ms = 21_000;
    later.timetable.last_departure_ms = 21_000;
    let mut runtime =
        OperationRuntime::new(vec![later, service("1", "shared"), service("2", "shared")]).unwrap();

    let before_release = runtime.advance_to(21_000, 2).unwrap();
    assert!(!before_release.reached_target);
    assert_eq!(before_release.now_ms, 20_000);
    assert_eq!(before_release.arrivals[0].train.service_id, "1");

    let mut reopened = OperationRuntime::restore(runtime.save()).unwrap();
    let resumed = reopened.advance_to(21_000, 1).unwrap();
    assert!(resumed.reached_target);
    assert!(resumed.arrivals.is_empty());
    let order = reopened
        .trains()
        .map(|train| (&*train.key.service_id, train.state.clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        order,
        vec![
            ("0", TrainState::Waiting { ready_ms: 21_000 }),
            (
                "2",
                TrainState::Moving {
                    entered_ms: 21_000,
                    arrival_ms: 41_000,
                    release_ms: 42_000,
                },
            ),
        ]
    );
    assert_eq!(
        reopened.advance_to(u64::MAX, 0),
        Err(OperationError::Overflow)
    );
    assert_eq!(reopened.now_ms(), 21_000);
}

fn observed_scope_service(
    id: &str,
    road: bool,
    resource: &str,
    first: u64,
    last: u64,
) -> ServiceRunSpec {
    let (from, to) = if road {
        ("bus:a", "bus:b")
    } else {
        ("11", "12")
    };
    ServiceRunSpec {
        service_id: id.into(),
        vehicle: Vehicle {
            max_speed_kmh: 36.,
            acceleration_mps2: 1.,
            braking_mps2: 1.,
            length_m: 10.,
        },
        timetable: Timetable {
            first_departure_ms: first,
            last_departure_ms: last,
            headway_ms: 20_000,
        },
        legs: vec![RunLeg {
            from: from.into(),
            to: to.into(),
            sections: vec![Section {
                length_m: 100.,
                speed_limit_kmh: 36.,
                resources: vec![resource.into()],
            }],
            dwell_ms: 0,
        }],
    }
}

#[test]
fn rail_cohort_ignores_proven_independent_bus_births_without_changing_the_live_clock_or_state() {
    let rail = observed_scope_service("1", false, "rail:1:section:0:lane:0", 0, 0);
    let bus = observed_scope_service(
        "bus:independent",
        true,
        "road:edge:independent",
        0,
        1_800_000,
    );
    let mixed = OperationRuntime::new(vec![rail.clone(), bus]).unwrap();
    let untouched = mixed.clone();
    assert_eq!(
        mixed.preview_passenger_replay(1_800_000, 256),
        Err(PassengerReplayError::OutsideBudget)
    );
    let expected = OperationRuntime::new(vec![rail])
        .unwrap()
        .preview_passenger_replay(1_800_000, 256)
        .unwrap();
    let observed = mixed.preview_rail_cohort_replay(1_800_000, 256).unwrap();
    assert_eq!(observed, expected);
    assert_eq!(
        observed.earliest_rail_itinerary("11", "12").arrival_ms,
        Some(20_000)
    );
    assert_eq!(mixed, untouched);
}

#[test]
fn rail_cohort_omits_active_independent_road_leases_and_matches_unfiltered_rail_observations() {
    let rail = observed_scope_service("1", false, "rail:1:section:0:lane:0", 0, 0);
    let bus = observed_scope_service("bus:independent", true, "road:edge:independent", 0, 0);
    let mut mixed = OperationRuntime::new(vec![rail, bus]).unwrap();
    mixed.advance_to(5_000, 256).unwrap();
    assert!(mixed.occupied.contains_key("road:edge:independent"));
    assert!(mixed.occupied.contains_key("rail:1:section:0:lane:0"));
    let untouched = mixed.clone();
    let mut expected = mixed.preview_passenger_replay(1_800_000, 256).unwrap();
    expected
        .completed_legs
        .retain(|leg| leg.train.service_id == "1");
    assert_eq!(
        mixed.preview_rail_cohort_replay(1_800_000, 256).unwrap(),
        expected
    );
    assert_eq!(mixed, untouched);
}

#[test]
fn rail_cohort_with_shared_or_unproven_resources_keeps_real_delay_and_the_full_budget_result() {
    let resource = "rail:crossing:1:2";
    let rail = observed_scope_service("1", false, resource, 1_000, 1_000);
    let bus = observed_scope_service("bus:shared", true, resource, 0, 0);
    let mixed = OperationRuntime::new(vec![rail.clone(), bus]).unwrap();
    let full = mixed.preview_passenger_replay(1_800_000, 256).unwrap();
    let result = mixed.preview_rail_cohort_replay(1_800_000, 256).unwrap();
    assert_eq!(result, full);
    assert!(result
        .completed_legs
        .iter()
        .any(|leg| leg.train.service_id == "1" && leg.cumulative_reservation_wait_ms > 0));
    let dense = observed_scope_service("bus:shared", true, resource, 0, 1_800_000);
    let mixed = OperationRuntime::new(vec![rail.clone(), dense]).unwrap();
    assert_eq!(
        mixed.preview_rail_cohort_replay(1_800_000, 256),
        Err(PassengerReplayError::OutsideBudget)
    );
    // Unknown stop namespace also refuses the projection even if every
    // observed resource name is otherwise independent.
    let mut unknown =
        observed_scope_service("bus:noncanonical", true, "road:edge:separate", 0, 1_800_000);
    unknown.legs[0].to = "12".into();
    let mixed = OperationRuntime::new(vec![rail, unknown]).unwrap();
    assert_eq!(
        mixed.preview_rail_cohort_replay(1_800_000, 256),
        Err(PassengerReplayError::OutsideBudget)
    );
}

#[test]
fn rail_cohort_keeps_unexplained_eligible_waiters_on_the_full_event_timeline() {
    let rail = observed_scope_service("1", false, "rail:1:section:0:lane:0", 0, 0);
    let bus = observed_scope_service(
        "bus:independent",
        true,
        "road:edge:independent",
        5_000,
        5_000,
    );
    let mut mixed = OperationRuntime::new(vec![rail, bus]).unwrap();
    let key = TrainKey {
        service_id: "1".into(),
        scheduled_ms: 0,
    };
    mixed.trains.insert(
        key.clone(),
        Train {
            key,
            leg: 0,
            delay_ms: 0,
            state: TrainState::Waiting { ready_ms: 0 },
        },
    );
    mixed.services.get_mut("1").unwrap().next_departure = None;
    assert!(!mixed.can_omit_road_replay());
    let full = mixed.preview_passenger_replay(1_800_000, 256).unwrap();
    assert_eq!(
        mixed.preview_rail_cohort_replay(1_800_000, 256).unwrap(),
        full
    );
    assert_eq!(
        full.earliest_rail_itinerary("11", "12").arrival_ms,
        Some(25_000)
    );
}
