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
            let ids = configs.map(|c| c.service_id.0.to_string());
            let rt = &mut world.operation_mut().unwrap().runtime;
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
