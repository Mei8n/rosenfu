use rosenfu_core::operation::{Timetable, TrainState, Vehicle};
use rosenfu_core::world::{
    AlignmentConnection,
    AlignmentEndpoint::{End, Start},
    AlignmentEndpointRef, AlignmentId, GeoPoint, OperationServiceConfig, RouteSegment,
    SectionInfrastructure, StationId, World,
};

#[path = "../../rosenfu-core/tests/fixtures/opposing_meet_fixture.rs"]
mod fixture;
use fixture::meeting_loop;

// Ordinary format 2 intentionally keeps the far throat reserved through rear
// clearance. A physical second bay alone does not enable an opposing meet.
#[test]
fn ordinary_single_track_serializes_opposing_arrivals_even_with_distinct_bays() {
    for separate in [false, true] {
        let (mut world, configs, [west, east, main, side], [a, b, s, c]) = meeting_loop(separate);
        world.configure_operation(configs.to_vec()).unwrap();
        let ids = configs.map(|x| x.service_id.0.to_string());
        let rt = &mut world.operation_mut().unwrap().runtime;
        let specs = ids.each_ref().map(|id| rt.service_spec(id).unwrap());
        assert_eq!(
            specs.map(|s| s.legs.len()),
            [2, 2],
            "auto reversal would confound a two-movement test"
        );
        assert_eq!(rt.save().format, 2);
        let keys = |r: usize| {
            specs[r].legs[0]
                .sections
                .iter()
                .flat_map(|s| s.resources.iter().cloned())
                .collect::<std::collections::BTreeSet<_>>()
        };
        let east_in = keys(0);
        let west_in = keys(1);
        let stem = |id: AlignmentId, end: &str| format!("rail:join:{}:{end}", id.0);
        assert!(east_in.contains(&stem(west, "End")));
        assert!(west_in.contains(&stem(east, "Start")));
        assert!(
            east_in.contains(&stem(east, "Start")),
            "outgoing throat added before B"
        );
        assert!(
            west_in.contains(&stem(west, "End")),
            "outgoing throat added before B"
        );
        let lane = |id: AlignmentId| format!("rail:{}:section:0:lane:0", id.0);
        assert!(east_in.contains(&lane(main)));
        assert!(west_in.contains(&lane(if separate { side } else { main })));
        assert_eq!(west_in.contains(&lane(main)), !separate);
        rt.advance_to(0, 128).unwrap();
        let [eb, wb] = ids
            .each_ref()
            .map(|id| rt.trains().find(|t| &t.key.service_id == id).unwrap());
        assert!(matches!(eb.state, TrainState::Moving { entered_ms: 0, .. }));
        assert_eq!(wb.state, TrainState::Waiting { ready_ms: 0 });
        let replay = rt.preview_passenger_replay(1_000_000, 128).unwrap();
        assert!(replay.complete);
        let first = |i| {
            replay
                .completed_legs
                .iter()
                .find(|r| r.train.service_id == ids[i] && r.sequence == 0)
                .unwrap()
        };
        let last = |i| {
            replay
                .completed_legs
                .iter()
                .find(|r| r.train.service_id == ids[i] && r.sequence == 1)
                .unwrap()
        };
        assert_eq!(first(0).to, b.0.to_string());
        assert_eq!(first(1).to, if separate { s } else { b }.0.to_string());
        assert_eq!(last(0).to, c.0.to_string());
        assert_eq!(last(1).to, a.0.to_string());
        let west_entry = first(1).entered_ms;
        assert!(west_entry > first(0).arrived_ms);

        let advance = |world: &mut World, to| {
            let result = world
                .operation_mut()
                .unwrap()
                .runtime
                .advance_to(to, 128)
                .unwrap();
            assert!(result.reached_target);
            result.arrivals
        };
        let before = advance(&mut world, west_entry - 1);
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].at_ms, first(0).arrived_ms);
        let bytes = rosenfu_storage::encode(&world).unwrap();
        let mut reopened = rosenfu_storage::decode(&bytes).unwrap();
        assert_eq!(rosenfu_storage::encode(&reopened).unwrap(), bytes);
        assert!(advance(&mut reopened, west_entry).is_empty());
        let westbound = reopened
            .operation()
            .unwrap()
            .runtime
            .trains()
            .find(|train| train.key.service_id == ids[1])
            .unwrap();
        assert!(
            matches!(westbound.state, TrainState::Moving { entered_ms, .. } if entered_ms == west_entry)
        );
        let after = advance(&mut reopened, 1_000_000);
        assert_eq!(after.len(), 3);
        assert_eq!(after, advance(&mut world, 1_000_000));
        assert_eq!(world.operation_save(), reopened.operation_save());
        assert_eq!(reopened.operation().unwrap().runtime.trains().count(), 0);
        let live = before
            .into_iter()
            .chain(after)
            .map(|event| (event.train, event.station_id, event.at_ms))
            .collect::<std::collections::BTreeSet<_>>();
        let observed = replay
            .completed_legs
            .into_iter()
            .map(|leg| (leg.train, leg.to, leg.arrived_ms))
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(live, observed);
    }
}

// A B waypoint in the stored path is distinct from a service calling at B.
// Ordinary format 2 can admit both approaches when both calls split their
// resource scopes. A through run still needs the far throat at admission.
#[test]
fn explicit_bay_calls_allow_an_opposing_meet_but_a_through_leg_contests_the_far_throat() {
    use rosenfu_core::operation::RailWaitReason;
    use std::collections::BTreeSet;

    for west_first in [false, true] {
        let mut reference = None;
        let mut main_center_to_end_m = 0.0;
        for through in [false, true] {
            let (mut world, mut configs, [west, east, _, _], [start, main, side, end]) =
                meeting_loop(true);
            if west_first {
                configs.swap(0, 1);
            }
            // Configure the same explicit center waypoint the planner exposes
            // to callers; toggling its selected stop must never reroute it.
            let mut routes = Vec::new();
            for ((from, mid, to, skip, name), config) in [
                (start, main, end, through, "East"),
                (end, side, start, false, "West"),
            ]
            .into_iter()
            .zip(&configs)
            {
                let owner = world.snapshot().companies[0].id;
                let service = config.service_id;
                let expected = world
                    .snapshot()
                    .service_paths
                    .iter()
                    .find(|p| p.service_id == service)
                    .unwrap()
                    .clone();
                let candidates = world.route_candidates_via(from, to, &[mid]).unwrap();
                assert!(!candidates.truncated);
                let route = candidates
                    .candidates
                    .into_iter()
                    .find(|r| r.stations == [from, mid, to])
                    .unwrap()
                    .segments;
                let preview = world
                    .preview_service_path_replacement(owner, service, from, route.clone())
                    .unwrap();
                let stops = preview
                    .occurrences
                    .into_iter()
                    .filter(|p| !skip || p.station_id != mid)
                    .map(|p| p.reference)
                    .collect::<Vec<_>>();
                assert_eq!(stops.len(), if skip { 2 } else { 3 });
                routes.push(route.clone());
                world
                    .replace_service_path(
                        owner,
                        service,
                        expected,
                        name.to_owned(),
                        from,
                        route,
                        stops,
                    )
                    .unwrap();
            }
            if through {
                assert_eq!(reference.as_ref(), Some(&routes), "not a reroute");
            } else {
                reference = Some(routes);
            }
            world.configure_operation(configs.to_vec()).unwrap();
            let ids = configs.each_ref().map(|c| c.service_id.0.to_string());
            let native_format2 = world.operation_save().unwrap();
            let ready = rosenfu_storage::encode(&world).unwrap();
            let mut reopened = rosenfu_storage::decode(&ready).unwrap();
            assert_eq!(rosenfu_storage::encode(&reopened).unwrap(), ready);
            for candidate in [&mut world, &mut reopened] {
                let live = &mut candidate.operation_mut().unwrap().runtime;
                live.advance_to(0, 64).unwrap();
                let by_direction = ids.each_ref().map(|id| {
                    &live
                        .trains()
                        .find(|t| &t.key.service_id == id)
                        .unwrap()
                        .state
                });
                assert!(matches!(
                    by_direction[1],
                    TrainState::Moving { entered_ms: 0, .. }
                ));
                if through {
                    assert_eq!(by_direction[0], &TrainState::Waiting { ready_ms: 0 });
                } else {
                    assert!(matches!(
                        by_direction[0],
                        TrainState::Moving { entered_ms: 0, .. }
                    ));
                }
            }
            assert_eq!(reopened.operation_save(), world.operation_save());

            // Run legacy tie order independently to preserve the comparison
            // baseline and the exact handling of already active old saves.
            let mut legacy = rosenfu_core::operation::OperationRuntime::new(
                native_format2.runtime.services.clone(),
            )
            .unwrap();
            let rt = &mut legacy;
            assert_eq!(rt.save().format, 2);
            let specs = ids.each_ref().map(|key| rt.service_spec(key).unwrap());
            assert_eq!(
                specs.map(|s| s.legs.len()),
                [if through { 1 } else { 2 }, 2]
            );
            let main_max_mps = specs[0].vehicle.max_speed_kmh / 3.6;
            if !through {
                main_center_to_end_m = specs[0].legs[1].sections.iter().map(|s| s.length_m).sum();
                assert!(main_center_to_end_m > 0.0);
            }
            let lockset = specs.map(|spec| {
                spec.legs[0]
                    .sections
                    .iter()
                    .flat_map(|s| s.resources.iter().cloned())
                    .collect::<BTreeSet<_>>()
            });
            assert!(lockset[0].contains(&format!("rail:join:{}:End", west.0)));
            assert_eq!(
                lockset[0].contains(&format!("rail:join:{}:Start", east.0)),
                through
            );
            assert_eq!(lockset[0].is_disjoint(&lockset[1]), !through);
            rt.advance_to(0, 64).unwrap();
            let train = |id: usize| rt.trains().find(|t| t.key.service_id == ids[id]).unwrap();
            let first = usize::from(west_first);
            let release = match train(first).state {
                TrainState::Moving {
                    entered_ms: 0,
                    release_ms,
                    ..
                } => release_ms,
                ref state => panic!("priority train failed to leave: {state:?}"),
            };
            if through {
                let waiter = train(1 - first);
                assert_eq!(waiter.state, TrainState::Waiting { ready_ms: 0 });
                assert_eq!(
                    rt.rail_wait_reason(&waiter.key),
                    Some(RailWaitReason::Junction)
                );
            } else {
                assert!(matches!(
                    train(1 - first).state,
                    TrainState::Moving { entered_ms: 0, .. }
                ));
            }

            if through && !west_first {
                let mut old_active = native_format2;
                old_active.runtime = rt.save();
                let mut old = World::new(world.snapshot().clone()).unwrap();
                old.restore_operation(Some(old_active.clone())).unwrap();
                assert_eq!(old.operation_save(), Some(old_active));
                assert!(matches!(
                    old.operation()
                        .unwrap()
                        .runtime
                        .trains()
                        .next()
                        .unwrap()
                        .state,
                    TrainState::Moving { entered_ms: 0, .. }
                ));
                // Dense repeat schedules cannot guarantee independent meets.
                let mut repeating = configs.clone();
                for c in &mut repeating {
                    c.timetable.last_departure_ms = 60_000;
                }
                let mut recurring = World::new(world.snapshot().clone()).unwrap();
                recurring.configure_operation(repeating.to_vec()).unwrap();
                let runtime = &mut recurring.operation_mut().unwrap().runtime;
                runtime.advance_to(0, 64).unwrap();
                assert!(matches!(
                    runtime
                        .trains()
                        .find(|t| t.key.service_id == ids[0])
                        .unwrap()
                        .state,
                    TrainState::Moving { entered_ms: 0, .. }
                ));
                assert_spaced_opposing_repeats(&world, &configs, &ids);
                assert_refuge_priority_with_staggered_windows(&world, &configs, &ids);
            }

            let replay = rt.preview_passenger_replay(1_000_000, 64).unwrap();
            assert!(replay.complete);
            assert_eq!(replay.completed_legs.len(), if through { 3 } else { 4 });
            let leg = |i, sequence| {
                replay
                    .completed_legs
                    .iter()
                    .find(|r| r.train.service_id == ids[i] && r.sequence == sequence)
                    .unwrap()
            };
            let e = leg(0, 0);
            let w = leg(1, 0);
            assert_eq!(e.to, (if through { end } else { main }).0.to_string());
            assert_eq!(w.to, side.0.to_string());
            if !through {
                assert!(
                    e.arrived_ms.max(w.arrived_ms) < leg(0, 1).entered_ms.min(leg(1, 1).entered_ms),
                    "both trains must be stationary in different bays for a nonempty interval"
                );
            } else if west_first {
                assert_eq!(e.entered_ms, release);
                // A head moving at no more than vehicle max cannot reach the
                // terminal sooner than this after passing the center. Round the
                // bound outwards to avoid tying it to simulator ms rounding.
                let remaining_ms = (main_center_to_end_m / main_max_mps * 1000.0).floor() as u64;
                let center_pass_latest_ms = e.arrived_ms - remaining_ms + 1;
                assert!(w.arrived_ms < e.entered_ms && e.entered_ms <= center_pass_latest_ms);
                assert!(
                    center_pass_latest_ms < leg(1, 1).entered_ms,
                    "the through train passes the center while the local is parked"
                );
            } else {
                assert_eq!(w.entered_ms, release);
                assert!(
                    e.arrived_ms < w.entered_ms,
                    "the local cannot enter before the through train clears"
                );
            }
        }
    }
}

fn configure_through_and_siding_paths(
    base: &mut World,
    configs: &[OperationServiceConfig; 2],
    [start, main, siding, end]: [StationId; 4],
) -> Vec<RouteSegment> {
    let owner = base.snapshot().companies[0].id;
    let through = configs[0].service_id;
    let path = base
        .snapshot()
        .service_paths
        .iter()
        .find(|path| path.service_id == through)
        .unwrap()
        .clone();
    let route = base
        .route_candidates_via(start, end, &[main])
        .unwrap()
        .candidates
        .into_iter()
        .find(|c| c.stations == [start, main, end])
        .unwrap()
        .segments;
    let stops = base
        .preview_service_path_replacement(owner, through, start, route.clone())
        .unwrap()
        .occurrences
        .into_iter()
        .filter(|p| p.station_id != main)
        .map(|p| p.reference)
        .collect::<Vec<_>>();
    base.replace_service_path(
        owner,
        through,
        path,
        "Through".into(),
        start,
        route.clone(),
        stops,
    )
    .unwrap();

    let local = configs[1].service_id;
    let local_path = base
        .snapshot()
        .service_paths
        .iter()
        .find(|path| path.service_id == local)
        .unwrap()
        .clone();
    let local_route = base
        .route_candidates_via(end, start, &[siding])
        .unwrap()
        .candidates
        .into_iter()
        .find(|c| c.stations == [end, siding, start])
        .unwrap()
        .segments;
    let local_stops = base
        .preview_service_path_replacement(owner, local, end, local_route.clone())
        .unwrap()
        .occurrences
        .into_iter()
        .map(|p| p.reference)
        .collect();
    base.replace_service_path(
        owner,
        local,
        local_path,
        "Siding".into(),
        end,
        local_route,
        local_stops,
    )
    .unwrap();

    route
}

#[test]
fn disconnected_running_service_does_not_defeat_a_meet_but_shared_authority_does() {
    let (mut base, mut configs, _, [start, main, siding, end]) = meeting_loop(true);
    let owner = base.snapshot().companies[0].id;
    let route = configure_through_and_siding_paths(&mut base, &configs, [start, main, siding, end]);

    let tie_at = 20_000;
    for c in &mut configs {
        c.timetable.first_departure_ms = tie_at;
        c.timetable.last_departure_ms = tie_at;
    }
    let ids = configs.each_ref().map(|c| c.service_id.0.to_string());
    let far = base
        .build_alignment(
            owner,
            "Isolated".into(),
            vec![
                GeoPoint {
                    latitude: 35.05,
                    longitude: 139.5,
                },
                GeoPoint {
                    latitude: 35.05,
                    longitude: 139.54,
                },
            ],
        )
        .unwrap();
    let x = base
        .place_station_on_alignment(owner, "X".into(), far, 0.1, 80., 10.)
        .unwrap();
    let y = base
        .place_station_on_alignment(owner, "Y".into(), far, 0.9, 80., 10.)
        .unwrap();
    let isolated_route = vec![RouteSegment::Alignment {
        alignment_id: far,
        from: x,
        to: y,
    }];
    let isolated_stops = base
        .preview_service_path(x, isolated_route.clone())
        .unwrap()
        .occurrences
        .into_iter()
        .map(|p| p.reference)
        .collect();
    let isolated = base
        .add_service_path(owner, "Isolated".into(), x, isolated_route, isolated_stops)
        .unwrap();
    let stops = base
        .preview_service_path(start, route.clone())
        .unwrap()
        .occurrences
        .into_iter()
        .filter(|p| p.station_id != main)
        .map(|p| p.reference)
        .collect();
    let shares_throat = base
        .add_service_path(owner, "Other through".into(), start, route, stops)
        .unwrap();
    let mut reference = None;
    for (extra, large_map) in [
        (None, false),
        (Some(isolated), false),
        (Some(shares_throat), false),
        (Some(isolated), true),
    ] {
        let mut draft = base.snapshot().clone();
        if large_map {
            let mut unused = draft
                .alignments
                .iter()
                .find(|a| a.id == far)
                .unwrap()
                .clone();
            unused.name = "Unrelated unserved infrastructure".into();
            for i in 0..1_050 {
                unused.id = AlignmentId(draft.next_id);
                draft.next_id += 1;
                let p = GeoPoint {
                    latitude: 30.0 + f64::from(i / 50) * 0.04,
                    longitude: 131.0 + f64::from(i % 50) * 0.04,
                };
                unused.points = vec![
                    p,
                    GeoPoint {
                        longitude: p.longitude + 0.01,
                        ..p
                    },
                ];
                draft.alignments.push(unused.clone());
            }
        }
        let mut world = World::new(draft).unwrap();
        let mut selected = configs.to_vec();
        if let Some(id) = extra {
            let mut config = configs[0].clone();
            config.service_id = id;
            let at = if id == isolated { 0 } else { tie_at + 1 };
            config.timetable.first_departure_ms = at;
            config.timetable.last_departure_ms = at;
            selected.push(config);
        }
        world.configure_operation(selected).unwrap();
        let rt = &mut world.operation_mut().unwrap().runtime;
        let arrivals = rt.advance_to(tie_at, 128).unwrap().arrivals;
        assert!(
            arrivals.is_empty(),
            "unrelated train must still be in flight"
        );
        let state = |id: &str| &rt.trains().find(|t| t.key.service_id == id).unwrap().state;
        if extra == Some(shares_throat) {
            // Even a not-yet-born competitor sharing a throat keeps the
            // conservative default tie order for this whole component.
            assert!(
                matches!(state(&ids[0]), TrainState::Moving { entered_ms, .. } if *entered_ms == tie_at)
            );
            assert_eq!(state(&ids[1]), &TrainState::Waiting { ready_ms: tie_at });
            continue;
        }
        if extra == Some(isolated) {
            assert!(matches!(
                state(&isolated.0.to_string()),
                TrainState::Moving { entered_ms: 0, .. }
            ));
        }
        assert_eq!(
            state(&ids[0]),
            &TrainState::Waiting { ready_ms: tie_at },
            "extra={extra:?} large_map={large_map} ids={ids:?}"
        );
        let siding_arrival = match state(&ids[1]) {
            TrainState::Moving {
                entered_ms,
                arrival_ms,
                ..
            } if *entered_ms == tie_at => *arrival_ms,
            state => panic!("siding tie was lost: {state:?}"),
        };
        let target = 1_000_000;
        let rail_events = rt
            .preview_passenger_replay(target, 256)
            .unwrap()
            .completed_legs
            .into_iter()
            .filter(|event| ids.contains(&event.train.service_id))
            .collect::<Vec<_>>();
        assert_eq!(rail_events.len(), 3);
        if extra.is_none() {
            reference = Some(rail_events);
        } else {
            assert_eq!(Some(rail_events), reference);
        }
        rt.advance_to(siding_arrival + 1, 128).unwrap();
        let save = rosenfu_storage::encode(&world).unwrap();
        let mut reopened = rosenfu_storage::decode(&save).unwrap();
        assert_eq!(reopened.operation_save(), world.operation_save());
        let complete = |w: &mut World| {
            w.operation_mut()
                .unwrap()
                .runtime
                .advance_to(target, 256)
                .unwrap()
                .arrivals
        };
        assert_eq!(complete(&mut world), complete(&mut reopened));
        assert_eq!(reopened.operation_save(), world.operation_save());
    }
}

#[test]
fn simultaneous_disjoint_meets_keep_their_own_priority_and_saved_dispatch() {
    use rosenfu_core::operation::OperationRuntime;

    const TIE: u64 = 20_000;
    const TARGET: u64 = 1_000_000;
    let (mut base, a, _, a_stations) = meeting_loop(true);
    let owner = base.snapshot().companies[0].id;
    let (b, _, b_stations) = fixture::append_meeting_loop(&mut base, owner, true, 0.125);
    configure_through_and_siding_paths(&mut base, &a, a_stations);
    let b_route = configure_through_and_siding_paths(&mut base, &b, b_stations);
    let [b_start, b_main, b_siding, _] = b_stations;
    let stops = base
        .preview_service_path(b_start, b_route.clone())
        .unwrap()
        .occurrences
        .into_iter()
        .filter(|p| p.station_id != b_main)
        .map(|p| p.reference)
        .collect();
    let b_extra = base
        .add_service_path(owner, "Later B through".into(), b_start, b_route, stops)
        .unwrap();
    let mut extra = b[0].clone();
    extra.service_id = b_extra;
    extra.timetable.first_departure_ms = TIE + 1;
    extra.timetable.last_departure_ms = TIE + 1;
    let [a, b] = [a, b].map(|pair| {
        pair.map(|mut c| {
            c.timetable.first_departure_ms = TIE;
            c.timetable.last_departure_ms = TIE;
            c
        })
    });
    let ids = [&a, &b].map(|pair| pair.each_ref().map(|c| c.service_id.0.to_string()));
    let make_world = |selected: Vec<_>| {
        let mut world = World::new(base.snapshot().clone()).unwrap();
        world.configure_operation(selected).unwrap();
        let result = world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(TIE, 128)
            .unwrap();
        assert!(result.reached_target);
        assert!(result.arrivals.is_empty());
        world
    };
    let trains = |rt: &OperationRuntime, ids: &[String; 2]| {
        ids.each_ref().map(|id| {
            rt.trains()
                .find(|t| &t.key.service_id == id)
                .unwrap()
                .clone()
        })
    };
    let replay = |rt: &OperationRuntime, ids: &[String; 2]| {
        let replay = rt.preview_passenger_replay(TARGET, 256).unwrap();
        assert!(replay.complete);
        let legs: Vec<_> = replay
            .completed_legs
            .into_iter()
            .filter(|leg| ids.contains(&leg.train.service_id))
            .collect();
        assert_eq!(legs.len(), 3);
        legs
    };

    let isolated = [
        a.to_vec(),
        b.to_vec(),
        [b.to_vec(), vec![extra.clone()]].concat(),
    ]
    .map(make_world);
    let reference = isolated
        .each_ref()
        .into_iter()
        .enumerate()
        .map(|(i, world)| {
            let rt = &world.operation().unwrap().runtime;
            let ids = &ids[usize::from(i != 0)];
            (trains(rt, ids), replay(rt, ids))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        reference[0].0[0].state,
        TrainState::Waiting { ready_ms: TIE }
    );
    assert_eq!(
        reference[1].0[0].state,
        TrainState::Waiting { ready_ms: TIE }
    );
    assert_eq!(
        reference[2].0[1].state,
        TrainState::Waiting { ready_ms: TIE }
    );

    for contested_b in [false, true] {
        let mut selected = [a.to_vec(), b.to_vec()].concat();
        if contested_b {
            selected.push(extra.clone());
        }
        let mut world = make_world(selected);
        let rt = &world.operation().unwrap().runtime;
        for (island, expected) in [0, if contested_b { 2 } else { 1 }].into_iter().enumerate() {
            assert_eq!(trains(rt, &ids[island]), reference[expected].0);
            assert_eq!(replay(rt, &ids[island]), reference[expected].1);
        }

        // Pick a time inside every unopposed siding dwell, not just the
        // first arrival, so the saved world really has both meets in flight.
        let siding_leg = |i: usize, station: StationId, sequence| {
            reference[i]
                .1
                .iter()
                .find(|leg| {
                    leg.train.service_id == ids[i][1]
                        && leg.sequence == sequence
                        && (if sequence == 0 { &leg.to } else { &leg.from })
                            == &station.0.to_string()
                })
                .unwrap()
        };
        let mut save_at = siding_leg(0, a_stations[2], 0).arrived_ms + 1;
        let mut leave_at = siding_leg(0, a_stations[2], 1).entered_ms;
        if !contested_b {
            save_at = save_at.max(siding_leg(1, b_siding, 0).arrived_ms + 1);
            leave_at = leave_at.min(siding_leg(1, b_siding, 1).entered_ms);
        }
        assert!(save_at < leave_at);
        let advance = |world: &mut World, at| {
            let result = world
                .operation_mut()
                .unwrap()
                .runtime
                .advance_to(at, 256)
                .unwrap();
            assert!(result.reached_target);
            result.arrivals
        };
        let arrived = advance(&mut world, save_at);
        for (island, station) in [a_stations[2], b_siding].into_iter().enumerate() {
            if island == 0 || !contested_b {
                assert!(arrived.iter().any(|arrival| {
                    arrival.train.service_id == ids[island][1]
                        && arrival.station_id == station.0.to_string()
                }));
                let rt = &world.operation().unwrap().runtime;
                let local = &trains(rt, &ids[island])[1];
                assert!(match local.state {
                    // The head is at the siding, but the train can still
                    // own its first leg until the rear clears the approach.
                    TrainState::Moving {
                        arrival_ms,
                        release_ms,
                        ..
                    } => {
                        local.leg == 0 && arrival_ms <= save_at && save_at < release_ms
                    }
                    TrainState::Waiting { .. } => local.leg == 1,
                });
            }
        }
        let bytes = rosenfu_storage::encode(&world).unwrap();
        let mut loaded = rosenfu_storage::decode(&bytes).unwrap();
        assert_eq!(rosenfu_storage::encode(&loaded).unwrap(), bytes);
        assert_eq!(advance(&mut world, TARGET), advance(&mut loaded, TARGET));
        assert_eq!(world.operation_save(), loaded.operation_save());
        assert_eq!(loaded.operation().unwrap().runtime.trains().count(), 0);
    }
}

#[test]
fn three_services_can_share_a_throat_if_the_third_cannot_start_until_after_clearance() {
    use rosenfu_core::operation::{OperationError, OperationRuntime};

    const TIE: u64 = 20_000;
    const LATER: u64 = 900_000;
    const TARGET: u64 = 2_000_000;
    let (mut base, mut pair, _, [start, main, siding, end]) = meeting_loop(true);
    let owner = base.snapshot().companies[0].id;
    let route = configure_through_and_siding_paths(&mut base, &pair, [start, main, siding, end]);
    let stops: Vec<_> = base
        .preview_service_path(start, route.clone())
        .unwrap()
        .occurrences
        .into_iter()
        .filter(|p| p.station_id != main)
        .map(|p| p.reference)
        .collect();
    let third = base
        .add_service_path(
            owner,
            "Shared throat".into(),
            start,
            route.clone(),
            stops.clone(),
        )
        .unwrap();
    let fourth = base
        .add_service_path(owner, "Fourth user".into(), start, route, stops)
        .unwrap();
    let ids = pair
        .each_ref()
        .map(|config| config.service_id.0.to_string());
    for config in &mut pair {
        config.timetable.first_departure_ms = TIE;
        config.timetable.last_departure_ms = TIE;
    }
    let extra_at = |at| OperationServiceConfig {
        service_id: third,
        timetable: Timetable {
            first_departure_ms: at,
            last_departure_ms: at,
            headway_ms: 60_000,
        },
        ..pair[0].clone()
    };
    let world_with = |extra: Option<OperationServiceConfig>| {
        let mut world = World::new(base.snapshot().clone()).unwrap();
        world
            .configure_operation(pair.to_vec().into_iter().chain(extra).collect())
            .unwrap();
        world
    };
    let advance = |world: &mut World, to| {
        let result = world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(to, 256)
            .unwrap();
        assert!(result.reached_target);
        result.arrivals
    };
    let states = |rt: &OperationRuntime| {
        ids.each_ref().map(|id| {
            rt.trains()
                .find(|train| &train.key.service_id == id)
                .unwrap()
                .state
                .clone()
        })
    };
    let replay = |rt: &OperationRuntime| {
        let replay = rt.preview_passenger_replay(TARGET, 256).unwrap();
        assert!(replay.complete);
        replay.completed_legs
    };
    let mut isolated = world_with(None);
    advance(&mut isolated, TIE);
    let isolated_rt = &isolated.operation().unwrap().runtime;
    let reference = replay(isolated_rt);
    assert_eq!(reference.len(), 3);
    let expected_initial = states(isolated_rt);
    let siding_arrival = match expected_initial[1] {
        TrainState::Moving {
            entered_ms: TIE,
            arrival_ms,
            ..
        } => arrival_ms,
        ref other => panic!("isolation must admit the siding train: {other:?}"),
    };
    assert_eq!(expected_initial[0], TrainState::Waiting { ready_ms: TIE });

    for at in [0, TIE, TIE + 1, LATER] {
        let mut world = world_with(Some(extra_at(at)));
        advance(&mut world, TIE);
        let rt = &world.operation().unwrap().runtime;
        match at {
            LATER => assert_eq!(states(rt), expected_initial),
            0 => assert_eq!(
                states(rt),
                [
                    TrainState::Waiting { ready_ms: TIE },
                    TrainState::Waiting { ready_ms: TIE }
                ]
            ),
            _ => {
                assert!(matches!(
                    states(rt)[0],
                    TrainState::Moving {
                        entered_ms: TIE,
                        ..
                    }
                ));
                assert_eq!(states(rt)[1], TrainState::Waiting { ready_ms: TIE });
            }
        }
        let actual = replay(rt);
        assert_eq!(actual.len(), 4, "all three trains must finish");
        let third_id = third.0.to_string();
        let later_leg = actual
            .iter()
            .find(|leg| leg.train.service_id == third_id)
            .unwrap();
        if at == LATER {
            assert_eq!(later_leg.entered_ms, LATER);
            assert_eq!(
                actual
                    .into_iter()
                    .filter(|leg| leg.train.service_id != third_id)
                    .collect::<Vec<_>>(),
                reference
            );
            let early = advance(&mut world, siding_arrival + 1);
            assert!(early
                .iter()
                .any(|e| e.train.service_id == ids[1] && e.station_id == siding.0.to_string()));
        }
        world.operation_mut().unwrap().runtime.set_paused(true);
        let save = rosenfu_storage::encode(&world).unwrap();
        let mut reopened = rosenfu_storage::decode(&save).unwrap();
        assert_eq!(reopened.operation_save(), world.operation_save());
        if at == LATER {
            let earlier = extra_at(siding_arrival + 2);
            let mut departures_through_clearance = Vec::new();
            for w in [&mut world, &mut reopened] {
                assert_eq!(
                    w.handoff_active_rail_timetable(earlier.clone()),
                    Err(OperationError::OpposingMeetInProgress)
                );
                assert_eq!(rosenfu_storage::encode(w).unwrap(), save);
                assert_eq!(w.handoff_active_rail_timetable(extra_at(LATER)), Ok(()));
                assert_eq!(w.handoff_active_rail_timetable(extra_at(LATER + 1)), Ok(()));
                departures_through_clearance.push(advance(w, LATER));
                assert_eq!(
                    w.operation().unwrap().runtime.trains().count(),
                    0,
                    "both trains and their rear clearance must precede the third birth"
                );
            }
            assert_eq!(
                departures_through_clearance[0],
                departures_through_clearance[1]
            );
        }
        assert_eq!(advance(&mut world, TARGET), advance(&mut reopened, TARGET));
        assert_eq!(reopened.operation_save(), world.operation_save());
        assert_eq!(reopened.operation().unwrap().runtime.trains().count(), 0);
    }

    let mut crowded = World::new(base.snapshot().clone()).unwrap();
    crowded
        .configure_operation(
            pair.to_vec()
                .into_iter()
                .chain([
                    extra_at(LATER),
                    OperationServiceConfig {
                        service_id: fourth,
                        ..extra_at(LATER + 1)
                    },
                ])
                .collect(),
        )
        .unwrap();
    advance(&mut crowded, TIE);
    let rt = &crowded.operation().unwrap().runtime;
    assert!(matches!(
        states(rt)[0],
        TrainState::Moving {
            entered_ms: TIE,
            ..
        }
    ));
    assert_eq!(states(rt)[1], TrainState::Waiting { ready_ms: TIE });
    assert_eq!(replay(rt).len(), 5);
}

fn assert_spaced_opposing_repeats(
    base: &World,
    configs: &[OperationServiceConfig; 2],
    ids: &[String; 2],
) {
    use rosenfu_core::operation::OperationRuntime;
    const START: u64 = 10_000;
    const GAP: u64 = 900_000;
    let mut spaced = (*configs).clone();
    for config in &mut spaced {
        config.timetable = Timetable {
            first_departure_ms: START,
            last_departure_ms: START + 2 * GAP,
            headway_ms: GAP,
        };
    }
    let mut world = World::new(base.snapshot().clone()).unwrap();
    world.configure_operation(spaced.to_vec()).unwrap();
    assert_eq!(world.operation_save().unwrap().runtime.format, 2);
    let state = |rt: &OperationRuntime, i: usize, birth: u64| {
        rt.trains()
            .find(|t| t.key.service_id == ids[i] && t.key.scheduled_ms == birth)
            .unwrap()
            .state
            .clone()
    };
    let confirm_pair = |w: &mut World, at: u64| {
        let rt = &mut w.operation_mut().unwrap().runtime;
        rt.advance_to(at - 1, 64).unwrap();
        assert_eq!(rt.trains().count(), 0, "previous pair must be clear");
        rt.advance_to(at, 64).unwrap();
        assert_eq!(rt.trains().count(), 2);
        assert_eq!(state(rt, 0, at), TrainState::Waiting { ready_ms: at });
        let TrainState::Moving {
            entered_ms,
            arrival_ms,
            release_ms,
        } = state(rt, 1, at)
        else {
            panic!("siding service must own its inbound route");
        };
        assert_eq!(entered_ms, at);
        assert!(arrival_ms < release_ms);
        rt.advance_to(release_ms - 1, 64).unwrap();
        assert_eq!(state(rt, 0, at), TrainState::Waiting { ready_ms: at });
        rt.advance_to(release_ms, 64).unwrap();
        assert!(matches!(
            state(rt, 0, at), TrainState::Moving { entered_ms, .. } if entered_ms == release_ms
        ));
    };
    confirm_pair(&mut world, START);
    confirm_pair(&mut world, START + GAP);
    let active = rosenfu_storage::encode(&world).unwrap();
    let mut reopened = rosenfu_storage::decode(&active).unwrap();
    assert_eq!(rosenfu_storage::encode(&reopened).unwrap(), active);
    confirm_pair(&mut world, START + 2 * GAP);
    confirm_pair(&mut reopened, START + 2 * GAP);
    assert_eq!(reopened.operation_save(), world.operation_save());

    // An active saved pair keeps its original trains and leases. A change
    // to only the future local grid leaves the third common birth on the
    // timetable but removes the recurrence proof for that new pair.
    let third = START + 2 * GAP;
    let mut changed = rosenfu_storage::decode(&active).unwrap();
    let trains = changed.operation_save().unwrap().runtime.trains;
    let mut replacement = spaced[1].clone();
    replacement.timetable.first_departure_ms = third;
    replacement.timetable.last_departure_ms = third;
    changed.handoff_active_rail_timetable(replacement).unwrap();
    assert_eq!(changed.operation_save().unwrap().runtime.trains, trains);
    let bytes = rosenfu_storage::encode(&changed).unwrap();
    let mut changed = rosenfu_storage::decode(&bytes).unwrap();
    assert_eq!(rosenfu_storage::encode(&changed).unwrap(), bytes);
    let rt = &mut changed.operation_mut().unwrap().runtime;
    rt.advance_to(third - 1, 64).unwrap();
    assert_eq!(rt.trains().count(), 0);
    rt.advance_to(third, 64).unwrap();
    assert!(matches!(
        state(rt, 0, third), TrainState::Moving { entered_ms, .. } if entered_ms == third
    ));

    // Conversely a future handoff can align the grids. Expired records
    // pruned at the end of advance must not make the choice depend on the
    // caller's event batching (nor affect the resumed result).
    let second = START + GAP;
    let mut initially_mismatched = spaced.clone();
    initially_mismatched[1].timetable.last_departure_ms = second;
    let mut aligning = World::new(base.snapshot().clone()).unwrap();
    aligning
        .configure_operation(initially_mismatched.to_vec())
        .unwrap();
    aligning
        .operation_mut()
        .unwrap()
        .runtime
        .advance_to(START, 64)
        .unwrap();
    assert!(matches!(
        state(&aligning.operation().unwrap().runtime, 0, START),
        TrainState::Moving { entered_ms, .. } if entered_ms == START
    ));
    aligning
        .handoff_active_rail_timetable(spaced[1].clone())
        .unwrap();
    let mut aligning_reopened =
        rosenfu_storage::decode(&rosenfu_storage::encode(&aligning).unwrap()).unwrap();
    let split = &mut aligning_reopened.operation_mut().unwrap().runtime;
    split.advance_to(second - 1, 64).unwrap();
    assert_eq!(split.trains().count(), 0);
    split.advance_to(second, 64).unwrap();
    let whole = &mut aligning.operation_mut().unwrap().runtime;
    whole.advance_to(second, 64).unwrap();
    assert_eq!(
        state(whole, 0, second),
        TrainState::Waiting { ready_ms: second }
    );
    assert!(matches!(
        state(whole, 1, second), TrainState::Moving { entered_ms, .. } if entered_ms == second
    ));
    assert_eq!(
        aligning.operation_save(),
        aligning_reopened.operation_save()
    );

    // Both services still have births at START, but later grids disagree.
    // The original deterministic ordering must win rather than special ties.
    spaced[1].timetable.headway_ms += 60_000;
    let mut mismatch = World::new(base.snapshot().clone()).unwrap();
    mismatch.configure_operation(spaced.to_vec()).unwrap();
    let rt = &mut mismatch.operation_mut().unwrap().runtime;
    rt.advance_to(START, 64).unwrap();
    assert!(matches!(
        state(rt, 0, START), TrainState::Moving { entered_ms, .. } if entered_ms == START
    ));
}

fn assert_refuge_priority_with_staggered_windows(
    base: &World,
    configs: &[OperationServiceConfig; 2],
    ids: &[String; 2],
) {
    use rosenfu_core::operation::OperationRuntime;
    const START: u64 = 10_000;
    const GAP: u64 = 900_000;
    let mut staggered = (*configs).clone();
    for (i, config) in staggered.iter_mut().enumerate() {
        config.timetable = Timetable {
            first_departure_ms: START + i as u64 * GAP,
            last_departure_ms: START + (2 + i as u64) * GAP,
            headway_ms: GAP,
        };
    }
    let mut world = World::new(base.snapshot().clone()).unwrap();
    world.configure_operation(staggered.to_vec()).unwrap();
    let state = |rt: &OperationRuntime, i: usize, birth: u64| {
        rt.trains()
            .find(|t| t.key.service_id == ids[i] && t.key.scheduled_ms == birth)
            .unwrap()
            .state
            .clone()
    };
    let step = |w: &mut World, at| {
        w.operation_mut()
            .unwrap()
            .runtime
            .advance_to(at, 128)
            .unwrap();
    };
    step(&mut world, START);
    assert!(matches!(
        state(&world.operation().unwrap().runtime, 0, START),
        TrainState::Moving {
            entered_ms: START,
            ..
        }
    ));
    for n in 1..=2 {
        let tie = START + n * GAP;
        step(&mut world, tie - 1);
        assert_eq!(world.operation().unwrap().runtime.trains().count(), 0);
        let before = rosenfu_storage::encode(&world).unwrap();
        let mut reopened = rosenfu_storage::decode(&before).unwrap();
        step(&mut world, tie);
        step(&mut reopened, tie);
        let rt = &world.operation().unwrap().runtime;
        assert_eq!(state(rt, 0, tie), TrainState::Waiting { ready_ms: tie });
        assert!(matches!(state(rt, 1, tie),
            TrainState::Moving{entered_ms,..} if entered_ms == tie));
        assert_eq!(world.operation_save(), reopened.operation_save());
    }
    let tail = START + 3 * GAP;
    step(&mut world, tail);
    let rt = &world.operation().unwrap().runtime;
    assert_eq!(rt.trains().count(), 1);
    assert!(matches!(state(rt, 1, tail),
        TrainState::Moving{entered_ms,..} if entered_ms == tail));
}
