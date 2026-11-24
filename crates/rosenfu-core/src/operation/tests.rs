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
fn staggered_and_simultaneous_movements_preserve_arrival_order() {
    let specs = (0..128)
        .rev()
        .map(|index| {
            let id = format!("{index:03}");
            let mut spec = service(&id, &id);
            let departure = index / 4 * 100;
            spec.timetable.first_departure_ms = departure;
            spec.timetable.last_departure_ms = departure;
            spec
        })
        .collect();
    let mut whole = OperationRuntime::new(specs).unwrap();
    whole.advance_to(3_100, 100).unwrap();
    assert_eq!(whole.trains().count(), 128);
    let mut budgeted = OperationRuntime::restore(whole.save()).unwrap();

    let result = whole.advance_to(100_000, 1_000).unwrap();
    assert!(result.reached_target);
    let arrivals = result.arrivals;
    assert_eq!(arrivals.len(), 128);
    for (index, arrival) in arrivals.iter().enumerate() {
        let departure = (index / 4 * 100) as u64;
        assert_eq!(arrival.train.service_id, format!("{index:03}"));
        assert_eq!(arrival.train.scheduled_ms, departure);
        assert_eq!(arrival.at_ms, departure + 20_000);
        assert_eq!(arrival.delay_ms, 0);
    }

    let mut actual = Vec::new();
    for _ in 0..128 {
        let step = budgeted.advance_to(100_000, 1).unwrap();
        actual.extend(step.arrivals);
        if step.reached_target {
            break;
        }
    }
    assert_eq!(actual, arrivals);
    assert_eq!(budgeted.save(), whole.save());
    assert!(whole.occupied.is_empty());
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

#[test]
fn current_rail_wait_reports_actual_join_lane_and_unknown_owners() {
    for (resource, expected) in [
        ("rail:join:1:End", Some(RailWaitReason::Junction)),
        ("rail:lane-join:1:1:0:2:0:0", Some(RailWaitReason::Junction)),
        ("rail:1:point:1", Some(RailWaitReason::Junction)),
        ("rail:1:section:0:lane:0", Some(RailWaitReason::Track)),
        ("future_reservation", None),
    ] {
        let first = observed_scope_service("1", false, resource, 0, 0);
        let second = observed_scope_service("2", false, resource, 1_000, 1_000);
        let mut runtime = OperationRuntime::new(vec![first, second]).unwrap();
        let key = TrainKey {
            service_id: "2".into(),
            scheduled_ms: 1_000,
        };
        runtime.advance_to(999, 10).unwrap();
        assert_eq!(runtime.rail_wait_reason(&key), None);
        runtime.advance_to(1_000, 10).unwrap();
        assert!(matches!(
            runtime.trains[&key].state,
            TrainState::Waiting { ready_ms: 1_000 }
        ));
        assert_eq!(runtime.rail_wait_reason(&key), expected);
        runtime.advance_to(21_000, 10).unwrap();
        assert!(matches!(
            runtime.trains[&key].state,
            TrainState::Moving {
                entered_ms: 21_000,
                ..
            }
        ));
        assert_eq!(runtime.rail_wait_reason(&key), None);
    }
}

#[test]
fn active_rail_schedule_cutover_keeps_occupancy_and_original_births() {
    let mut a = service("1", "rail:1:section:0:lane:0");
    a.legs[0].sections[0].length_m = 1_000.;
    a.timetable.last_departure_ms = 120_000;
    let mut b = service("2", "rail:2:section:0:lane:0");
    b.timetable = Timetable {
        first_departure_ms: 10_000,
        last_departure_ms: 130_000,
        headway_ms: 60_000,
    };
    let mut current = OperationRuntime::new(vec![a, b]).unwrap();
    current.advance_to(35_000, 100).unwrap();
    let old = current.clone();
    let requested = Timetable {
        first_departure_ms: 90_000,
        last_departure_ms: 180_000,
        headway_ms: 90_000,
    };
    current.set_paused(false);
    assert_eq!(
        current.handoff_rail_timetable("1", requested.clone()),
        Err(OperationError::InvalidClock)
    );
    current.set_paused(true);
    current
        .handoff_rail_timetable("1", requested.clone())
        .unwrap();
    assert_eq!(current.trains, old.trains);
    assert_eq!(current.occupied, old.occupied);
    assert_eq!(current.services["2"], old.services["2"]);
    assert_eq!(current.rail_admission, old.rail_admission);
    assert_eq!(current.now_ms(), 35_000);
    assert!(current.paused());
    let mixed = current.save();
    assert_eq!(
        mixed.next_departures,
        vec![
            ("1".to_owned(), Some(90_000)),
            ("2".to_owned(), Some(70_000))
        ]
    );
    assert_eq!(mixed.timetable_handoffs.len(), 1);
    let mut another = requested.clone();
    another.headway_ms = 120_000;
    assert_eq!(
        current.handoff_rail_timetable("1", another),
        Err(OperationError::TimetableHandoffPending)
    );
    assert_eq!(current.save(), mixed);
    let mut altered = mixed.clone();
    altered.timetable_handoffs.clear();
    assert_eq!(
        OperationRuntime::restore(altered),
        Err(OperationError::InvalidTimetable)
    );
    let mut altered = mixed.clone();
    altered.timetable_handoffs[0].1.cutover_ms = 36_000;
    assert_eq!(
        OperationRuntime::restore(altered),
        Err(OperationError::InvalidTimetable)
    );
    let mut altered = mixed.clone();
    altered.next_departures[0].1 = Some(60_000);
    assert_eq!(
        OperationRuntime::restore(altered),
        Err(OperationError::InvalidTimetable)
    );

    let mut reopened = OperationRuntime::restore(
        serde_json::from_slice(&serde_json::to_vec(&mixed).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(reopened, current);
    let expected = current.advance_to(300_000, 100).unwrap().arrivals;
    let actual = (40_000..=300_000)
        .step_by(5_000)
        .flat_map(|now| reopened.advance_to(now, 100).unwrap().arrivals)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert_eq!(reopened, current);
    assert!(current.save().timetable_handoffs.is_empty());
    let births = |id| {
        expected
            .iter()
            .filter(|e| e.train.service_id == id)
            .map(|e| e.train.scheduled_ms)
            .collect::<Vec<_>>()
    };
    assert_eq!(births("1"), [0, 90_000, 180_000]);
    assert_eq!(births("2"), [70_000, 130_000]);
}

#[test]
fn live_cutover_never_spawns_a_departure_equal_to_its_processed_boundary() {
    let mut a = service("1", "rail:1:section:0:lane:0");
    a.legs[0].sections[0].length_m = 1_000.;
    let mut runtime = OperationRuntime::new(vec![a]).unwrap();
    runtime.advance_to(35_000, 100).unwrap();
    runtime
        .handoff_rail_timetable(
            "1",
            Timetable {
                first_departure_ms: 35_000,
                last_departure_ms: 95_000,
                headway_ms: 60_000,
            },
        )
        .unwrap();
    assert_eq!(runtime.services["1"].next_departure, Some(95_000));
    assert_eq!(runtime.advance_to(35_000, 100).unwrap().arrivals, []);
    assert_eq!(runtime.trains().count(), 1);
    assert_eq!(OperationRuntime::restore(runtime.save()).unwrap(), runtime);
}

#[test]
fn fixed_rail_wait_scope_distinguishes_actual_connection_reservation_owner_and_request() {
    let near = ["rail:join:3:End", "rail:join:5:Start"];
    let far = ["rail:join:4:Start", "rail:join:5:End"];
    let siding = ["rail:join:4:Start", "rail:join:6:End"];
    let run = |id, keys: &[&str]| {
        let mut spec = service(id, keys[0]);
        spec.legs[0].sections[0].resources = keys.iter().map(|s| (*s).to_owned()).collect();
        spec
    };
    let key = |id: &str| TrainKey {
        service_id: id.into(),
        scheduled_ms: 0,
    };
    let start = |a, b| {
        let mut r = OperationRuntime::new(vec![a, b]).unwrap();
        r.advance_to(0, 32).unwrap();
        r
    };
    let joined = [near, far].concat();
    let split = start(run("1", &near), run("2", &siding));
    assert!(split
        .trains()
        .all(|train| matches!(train.state, TrainState::Moving { entered_ms: 0, .. })));
    assert_eq!(split.rail_wait_advisory(&key("2")), None);

    let mut main_first = start(run("1", &joined), run("2", &siding));
    assert_eq!(main_first.rail_wait_advisory(&key("1")), None);
    assert_eq!(
        main_first.rail_wait_advisory(&key("2")),
        Some((
            RailWaitReason::Junction,
            Some(RailWaitScope::HeldAcrossConnections)
        ))
    );
    let serialized = main_first.save();
    let round_trip = OperationRuntime::restore(serialized.clone()).unwrap();
    assert_eq!(round_trip.save(), serialized);
    assert_eq!(
        round_trip.rail_wait_advisory(&key("2")),
        main_first.rail_wait_advisory(&key("2"))
    );
    let TrainState::Moving { release_ms, .. } = main_first
        .trains()
        .find(|t| t.key == key("1"))
        .unwrap()
        .state
    else {
        panic!()
    };
    main_first.advance_to(release_ms, 32).unwrap();
    assert_eq!(main_first.rail_wait_advisory(&key("2")), None);

    let side_first = start(run("1", &siding), run("2", &joined));
    assert_eq!(
        side_first.rail_wait_advisory(&key("2")),
        Some((
            RailWaitReason::Junction,
            Some(RailWaitScope::NeedsAcrossConnections)
        ))
    );
    let ordinary = start(run("1", &far), run("2", &siding));
    assert_eq!(
        ordinary.rail_wait_advisory(&key("2")),
        Some((RailWaitReason::Junction, None))
    );
    let crossing = [
        "rail:crossing:fixture",
        joined[0],
        joined[1],
        joined[2],
        joined[3],
    ];
    let other = ["rail:crossing:fixture", siding[0], siding[1]];
    let crossing_first = start(run("1", &crossing), run("2", &other));
    assert_eq!(
        crossing_first.rail_wait_advisory(&key("2")),
        Some((RailWaitReason::Crossing, None))
    );
}
