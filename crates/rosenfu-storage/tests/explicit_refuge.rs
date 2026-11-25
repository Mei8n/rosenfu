use rosenfu_core::operation::{
    OperationError, OperationRuntime, RailWaitReason, RailWaitScope, Timetable, TrainState, Vehicle,
};
use rosenfu_core::world::{
    AlignmentConnection, AlignmentEndpoint, AlignmentEndpointRef, AlignmentId, GeoPoint,
    OperationServiceConfig, RouteSegment, SectionInfrastructure, World,
};
use std::collections::BTreeSet;

fn fixture(
    different_branch: bool,
    local_uses_siding: bool,
    width_scale: f64,
    platform_length: f64,
) -> (World, Vec<OperationServiceConfig>, [AlignmentId; 4]) {
    use AlignmentEndpoint::{End, Start};

    let mut world = World::default();
    let player = world.add_player("QA".into()).unwrap();
    let owner = world.add_company(player, "Rail".into()).unwrap();
    let p = |longitude, rise| GeoPoint {
        longitude,
        latitude: 35. + rise,
    };
    let west_points = vec![p(139.50, 0.), p(139.51, 0.)];
    let east_points = vec![p(139.522, 0.), p(139.532, 0.)];
    let main_points = vec![p(139.51, 0.), p(139.522, 0.)];
    let siding_points = vec![
        p(139.51, 0.),
        p(139.512, 0.00006 * width_scale),
        p(139.515, 0.00015 * width_scale),
        p(139.517, 0.00015 * width_scale),
        p(139.520, 0.00006 * width_scale),
        p(139.522, 0.),
    ];
    let west = world
        .build_alignment(owner, "West approach".into(), west_points)
        .unwrap();
    let east = world
        .build_alignment(owner, "East approach".into(), east_points)
        .unwrap();
    let main = world
        .build_alignment(owner, "Main (local platform)".into(), main_points)
        .unwrap();
    let siding = world
        .build_alignment(owner, "Independent refuge track".into(), siding_points)
        .unwrap();
    let mut draft = world.snapshot().clone();
    for a in &mut draft.alignments {
        if [west, east].contains(&a.id) {
            a.infrastructure = vec![SectionInfrastructure {
                track_count: 2,
                ..Default::default()
            }];
        }
    }
    world.replace(draft).unwrap();
    let link = |left, right| AlignmentConnection {
        a: AlignmentEndpointRef {
            alignment_id: left,
            endpoint: End,
        },
        b: AlignmentEndpointRef {
            alignment_id: right,
            endpoint: Start,
        },
    };
    let wm = link(west, main);
    let me = link(main, east);
    let ws = link(west, siding);
    let se = link(siding, east);
    for l in [&wm, &me, &ws, &se] {
        world.connect_alignment_endpoints(owner, l.a, l.b).unwrap();
    }
    let a = world
        .place_station_on_alignment(owner, "A".into(), west, 0.25, 80., 10.)
        .unwrap();
    let local = if local_uses_siding { siding } else { main };
    let b = world
        .place_station_on_alignment(owner, "B".into(), local, 0.50, platform_length, 10.)
        .unwrap();
    let c = world
        .place_station_on_alignment(owner, "C".into(), east, 0.75, 80., 10.)
        .unwrap();
    let (local_path, alternate_path) = if local_uses_siding {
        (vec![ws, se], vec![wm, me])
    } else {
        (vec![wm, me], vec![ws, se])
    };
    let express = if different_branch {
        alternate_path
    } else {
        local_path.clone()
    };
    let patterns = [
        ("Local", local_path, Some(b), 180_000),
        ("Express", express, None, 0),
    ];
    let configs = patterns
        .into_iter()
        .map(|(name, connections, inter, dwell_ms)| {
            let legs = vec![RouteSegment::ConnectedAlignments {
                from: a,
                to: c,
                connections,
            }];
            let preview = world.preview_service_path(a, legs.clone()).unwrap();
            let stops = preview
                .occurrences
                .iter()
                .filter(|o| o.station_id == a || o.station_id == c || inter == Some(o.station_id))
                .map(|o| o.reference.clone())
                .collect();
            let service_id = world
                .add_service_path(owner, name.into(), a, legs, stops)
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
                dwell_ms,
            }
        })
        .collect();
    (
        world,
        configs,
        [
            west,
            east,
            local,
            if local_uses_siding { main } else { siding },
        ],
    )
}

#[test]
fn native_fixed_refuge_and_same_branch_comparison() {
    for (refuge, siding_holder) in [(false, false), (true, false), (true, true)] {
        let (mut world, c, [west, east, main, siding]) = fixture(refuge, siding_holder, 1., 80.);
        assert_eq!(world.snapshot().alignment_connections.len(), 4);
        assert_eq!(world.snapshot().alignments.len(), 4);
        world.configure_operation(c.clone()).unwrap();
        let rt = &mut world.operation_mut().unwrap().runtime;
        let local = rt.service_spec(&c[0].service_id.0.to_string()).unwrap();
        let express = rt.service_spec(&c[1].service_id.0.to_string()).unwrap();
        assert_eq!(local.legs.len(), 2);
        assert_eq!(express.legs.len(), 1);
        let resources = |leg: &rosenfu_core::operation::RunLeg| -> BTreeSet<String> {
            leg.sections
                .iter()
                .flat_map(|s| s.resources.iter().cloned())
                .collect()
        };
        let local_entry = resources(&local.legs[0]);
        let local_exit = resources(&local.legs[1]);
        let express_path = resources(&express.legs[0]);
        for (set, lock) in [
            (&local_entry, format!("rail:join:{}:End", west.0)),
            (&local_exit, format!("rail:join:{}:Start", east.0)),
        ] {
            assert!(set.contains(&lock));
            assert!(express_path.contains(&lock));
        }
        let local_branch = format!("rail:{}:section:0:lane:0", main.0);
        let express_branch = format!(
            "rail:{}:section:0:lane:0",
            if refuge { siding.0 } else { main.0 }
        );
        assert!(local_entry.contains(&local_branch));
        assert!(express_path.contains(&express_branch));
        assert_eq!(express_path.contains(&local_branch), !refuge);
        assert_eq!(rt.save().format, 7);
        let first = rt.advance_to(1_000, 128).unwrap();
        assert!(first.reached_target);
        assert_eq!(
            rt.trains()
                .filter(|t| matches!(t.state, TrainState::Waiting { .. }))
                .count(),
            1
        );
        let replay = rt.preview_passenger_replay(1_000_000, 128).unwrap();
        let id0 = c[0].service_id.0.to_string();
        let id1 = c[1].service_id.0.to_string();
        let a = replay
            .completed_legs
            .iter()
            .find(|r| r.train.service_id == id0 && r.sequence == 0)
            .unwrap();
        let b = replay
            .completed_legs
            .iter()
            .find(|r| r.train.service_id == id0 && r.sequence == 1)
            .unwrap();
        let e = replay
            .completed_legs
            .iter()
            .find(|r| r.train.service_id == id1)
            .unwrap();
        println!("distinct={refuge} siding_holder={siding_holder}: arrives_B={} leaves_B={} express_enters={} express_arrives={}", a.arrived_ms,b.entered_ms,e.entered_ms,e.arrived_ms);
        let ready = a.arrived_ms + c[0].dwell_ms;
        if refuge {
            assert!(e.entered_ms < ready);
            assert!(e.arrived_ms < b.entered_ms);
            assert!(b.entered_ms > ready); // shared outgoing throat
        } else {
            assert_eq!(b.entered_ms, ready);
            assert!(e.entered_ms > b.arrived_ms);
            assert_eq!(
                (a.arrived_ms, b.entered_ms, e.entered_ms, e.arrived_ms),
                (102_022, 282_022, 553_209, 732_454)
            );
        }
    }
}

#[test]
fn waiting_guidance_distinguishes_same_branch_and_shared_exit_without_changing_dispatch() {
    for distinct in [false, true] {
        let (mut world, configs, _) = fixture(distinct, false, 1., 80.);
        world.configure_operation(configs.clone()).unwrap();
        let local = configs[0].service_id.0.to_string();
        let express = configs[1].service_id.0.to_string();
        let query = |world: &World, service: &str| {
            let runtime = &world.operation().unwrap().runtime;
            let train = runtime
                .trains()
                .find(|t| t.key.service_id == service)
                .unwrap();
            runtime.rail_wait_advisory(&train.key)
        };
        tick(&mut world, 1_000);
        assert!(!query(&world, &express).is_some_and(|(_, scope)| scope.is_some()));
        tick(&mut world, 104_887);
        assert_eq!(query(&world, &local), None); // scheduled dwell, not traffic delay
        if distinct {
            assert_eq!(query(&world, &express), None); // express has entered its own branch
            tick(&mut world, 282_023);
            assert!(matches!(
                query(&world, &local),
                Some((
                    RailWaitReason::Track | RailWaitReason::Junction,
                    Some(RailWaitScope::SharedExitForBypass)
                ))
            ));
            let reopened =
                rosenfu_storage::decode(&rosenfu_storage::encode(&world).unwrap()).unwrap();
            assert_eq!(query(&reopened, &local), query(&world, &local));
            tick(&mut world, 298_253);
            assert_eq!(query(&world, &local), None);
        } else {
            assert_eq!(
                query(&world, &express),
                Some((
                    RailWaitReason::Platform,
                    Some(RailWaitScope::ParkedOnSharedBranch)
                ))
            );
        }
    }
}

fn tick(world: &mut World, at: u64) -> rosenfu_core::operation::AdvanceResult {
    world
        .operation_mut()
        .unwrap()
        .runtime
        .advance_to(at, 128)
        .unwrap()
}

#[test]
fn refuge_wait_resumes_with_exclusive_outgoing_throat() {
    let (mut world, c, [_, _, holder, _]) = fixture(true, false, 1., 80.);
    world.configure_operation(c.clone()).unwrap();
    let id = c[0].service_id.0.to_string();
    let find = |w: &World| {
        w.operation()
            .unwrap()
            .runtime
            .trains()
            .find(|t| t.key.service_id == id)
            .unwrap()
            .clone()
    };
    let reopen = |w: &World| {
        let bytes = rosenfu_storage::encode(w).unwrap();
        let copy = rosenfu_storage::decode(&bytes).unwrap();
        assert_eq!(rosenfu_storage::encode(&copy).unwrap(), bytes);
        copy
    };
    tick(&mut world, 104_887);
    assert_eq!(
        find(&world).state,
        TrainState::Waiting { ready_ms: 282_022 }
    );
    let mut during_dwell = reopen(&world);
    let replay = |w: &World| {
        w.operation()
            .unwrap()
            .runtime
            .preview_passenger_replay(800_000, 128)
            .unwrap()
    };
    assert_eq!(replay(&world), replay(&during_dwell));
    assert_eq!(tick(&mut world, 282_023), tick(&mut during_dwell, 282_023));
    assert_eq!(
        find(&world).state,
        TrainState::Waiting { ready_ms: 282_022 }
    );
    let mut after_dwell = reopen(&world);
    let old = world.operation_save().unwrap();
    let mut forged = old.clone();
    forged
        .runtime
        .trains
        .iter_mut()
        .find(|t| t.key.service_id == id)
        .unwrap()
        .state = TrainState::Waiting { ready_ms: 282_021 };
    assert_eq!(
        after_dwell.restore_operation(Some(forged)),
        Err(OperationError::InvalidClock)
    );
    assert_eq!(after_dwell.operation_save(), Some(old.clone()));
    tick(&mut world, 298_252);
    assert!(matches!(find(&world).state, TrainState::Waiting { .. }));
    tick(&mut world, 298_253);
    assert!(matches!(
        find(&world).state,
        TrainState::Moving {
            entered_ms: 298_253,
            ..
        }
    ));
    assert_eq!(find(&world).delay_ms, 16_231);
    tick(&mut after_dwell, 298_253);
    assert_eq!(world.operation_save(), after_dwell.operation_save());
    assert_eq!(
        tick(&mut world, 1_000_000),
        tick(&mut after_dwell, 1_000_000)
    );

    let mut draft = world.snapshot().clone();
    draft
        .station_placements
        .iter_mut()
        .find(|p| p.alignment_id == holder)
        .unwrap()
        .length_m = 32.;
    let mut edited = World::new(draft).unwrap();
    assert!(edited.restore_operation(Some(old)).is_err());
    edited.retain_operation_from(&during_dwell);
    assert!(edited.operation_save().unwrap().invalidated);
    assert!(reopen(&edited).operation().unwrap().runtime.paused());
}

#[test]
fn unqualified_geometry_falls_back_and_legacy_snapshots_do_not_upgrade() {
    for (width, length) in [(0.30, 80.), (1., 32.)] {
        let (mut world, c, _) = fixture(true, false, width, length);
        world.configure_operation(c).unwrap();
        let mut saved = world.operation_save().unwrap();
        assert_eq!(saved.runtime.format, 2);
        saved.runtime.format = 7;
        assert_eq!(
            world.restore_operation(Some(saved)),
            Err(OperationError::InvalidClock)
        );
    }
    let (mut world, c, _) = fixture(true, false, 1., 80.);
    world.configure_operation(c.clone()).unwrap();
    let specs = c
        .iter()
        .map(|k| world.compile_operation(k))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    world.operation_mut().unwrap().runtime = OperationRuntime::new(specs).unwrap();
    tick(&mut world, 282_023);
    let bytes = rosenfu_storage::encode(&world).unwrap();
    let restored = rosenfu_storage::decode(&bytes).unwrap();
    assert_eq!(restored.operation_save().unwrap().runtime.format, 2);
    assert_eq!(rosenfu_storage::encode(&restored).unwrap(), bytes);
}

#[test]
fn isolated_unserved_rail_component_does_not_change_the_refuge_or_saved_runtime() {
    for separate_branches in [false, true] {
        let (mut original, configs, [west, _, _, _]) = fixture(separate_branches, false, 1., 80.);
        original.configure_operation(configs.clone()).unwrap();
        let before_timing = original
            .operation()
            .unwrap()
            .runtime
            .preview_passenger_replay(1_000_000, 128)
            .unwrap();
        tick(&mut original, 110_000);
        let before = original.operation_save().unwrap();
        assert_eq!(before.runtime.format, 7);

        let mut cold = World::new(original.snapshot().clone()).unwrap();
        let owner = cold
            .snapshot()
            .alignments
            .iter()
            .find(|a| a.id == west)
            .unwrap()
            .owner;
        let point = |longitude| GeoPoint {
            latitude: 36.,
            longitude,
        };
        let x = cold
            .build_alignment(
                owner,
                "Distant siding A".into(),
                vec![point(140.), point(140.01)],
            )
            .unwrap();
        let y = cold
            .build_alignment(
                owner,
                "Distant siding B".into(),
                vec![point(140.01), point(140.02)],
            )
            .unwrap();
        cold.connect_alignment_endpoints(
            owner,
            AlignmentEndpointRef {
                alignment_id: x,
                endpoint: AlignmentEndpoint::End,
            },
            AlignmentEndpointRef {
                alignment_id: y,
                endpoint: AlignmentEndpoint::Start,
            },
        )
        .unwrap();
        assert_eq!(cold.snapshot().alignments.len(), 6);
        assert_eq!(cold.snapshot().alignment_connections.len(), 5);
        cold.configure_operation(configs).unwrap();
        let replay = cold
            .operation()
            .unwrap()
            .runtime
            .preview_passenger_replay(1_000_000, 128)
            .unwrap();
        assert_eq!(cold.operation_save().unwrap().runtime.format, 7);
        assert_eq!(replay, before_timing);

        let mut active = World::new(cold.snapshot().clone()).unwrap();
        active.retain_operation_from(&original);
        assert_eq!(active.operation_save(), Some(before.clone()));
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("refuge-component.rosenfu");
        rosenfu_storage::save_world(&file, &active).unwrap();
        let (mut reopened, _) = rosenfu_storage::load_world_with_version(&file).unwrap();
        assert_eq!(reopened.operation_save(), Some(before));
        assert_eq!(
            tick(&mut reopened, 1_000_000),
            tick(&mut original, 1_000_000)
        );
        assert_eq!(reopened.operation_save(), original.operation_save());
    }
}

#[test]
fn spur_or_spatial_overlap_still_reject_candidate_and_freeze_an_existing_refuge() {
    for connect_spur in [true, false] {
        let (mut original, configs, [west, _, _, _]) = fixture(true, false, 1., 80.);
        original.configure_operation(configs.clone()).unwrap();
        tick(&mut original, 110_000);
        let before = original.operation_save().unwrap();
        assert_eq!(before.runtime.format, 7);
        let mut altered = World::new(original.snapshot().clone()).unwrap();
        let owner = altered
            .snapshot()
            .alignments
            .iter()
            .find(|a| a.id == west)
            .unwrap()
            .owner;
        let point = |latitude, longitude| GeoPoint {
            latitude,
            longitude,
        };
        let points = if connect_spur {
            vec![point(35., 139.49), point(35., 139.50)]
        } else {
            // Both new vertices are remote from the loop, but the straight
            // span between them crosses its west approach at ground level.
            vec![point(34.98, 139.505), point(35.02, 139.505)]
        };
        let extra = altered
            .build_alignment(owner, "Unqualified extra track".into(), points)
            .unwrap();
        if connect_spur {
            altered
                .connect_alignment_endpoints(
                    owner,
                    AlignmentEndpointRef {
                        alignment_id: extra,
                        endpoint: AlignmentEndpoint::End,
                    },
                    AlignmentEndpointRef {
                        alignment_id: west,
                        endpoint: AlignmentEndpoint::Start,
                    },
                )
                .unwrap();
        }
        altered.configure_operation(configs).unwrap();
        assert_eq!(altered.operation_save().unwrap().runtime.format, 2);
        let cold = altered.operation_save();
        assert!(altered.restore_operation(Some(before.clone())).is_err());
        assert_eq!(altered.operation_save(), cold);
        altered.retain_operation_from(&original);
        let frozen = altered.operation_save().unwrap();
        assert!(frozen.invalidated);
        assert!(frozen.runtime.paused);
        assert_eq!(frozen.runtime, before.runtime);
        let bytes = rosenfu_storage::encode(&altered).unwrap();
        assert_eq!(
            rosenfu_storage::decode(&bytes).unwrap().operation_save(),
            Some(frozen)
        );
    }
}

#[test]
fn repeated_refuge_calendar_compares_completed_trains_without_changing_nominal_offers() {
    const BEFORE_DRAIN_MS: u64 = 600_000;
    const DRAIN_MS: u64 = 3_600_000;
    const BUDGET: usize = 256;
    let observe = |headway_ms, different_branch| {
        let (mut world, mut configs, _) = fixture(different_branch, false, 1., 80.);
        for c in &mut configs {
            c.timetable.headway_ms = headway_ms;
            c.timetable.last_departure_ms = 3 * headway_ms;
        }
        let local = configs[0].service_id.0.to_string();
        let express = configs[1].service_id.0.to_string();
        world.configure_operation(configs).unwrap();
        let rt = &world.operation().unwrap().runtime;
        assert_eq!(rt.save().format, 7);
        let save_before_replay = world.operation_save();
        let plans = rt.passenger_offers();
        let report = rt.preview_passenger_replay(DRAIN_MS, BUDGET).unwrap();
        assert!(report.complete);
        assert_eq!(world.operation_save(), save_before_replay);

        // Count final served arrivals, not two arrivals for each local and
        // not nominal offers or a passenger-demand multiplier.
        let finished = report.completed_legs.iter().filter(|leg| {
            (leg.train.service_id == local && leg.sequence == 1)
                || (leg.train.service_id == express && leg.sequence == 0)
        });
        let expected: BTreeSet<_> = [&local, &express]
            .into_iter()
            .flat_map(|id| (0..4).map(move |i| (id.clone(), i * headway_ms)))
            .collect();
        let mut observed = BTreeSet::new();
        let mut early = 0;
        let mut max_origin_wait = 0;
        for leg in finished {
            assert!(observed.insert((leg.train.service_id.clone(), leg.train.scheduled_ms)));
            early += usize::from(leg.arrived_ms < BEFORE_DRAIN_MS);
        }
        for leg in report.completed_legs.iter().filter(|leg| leg.sequence == 0) {
            max_origin_wait = max_origin_wait.max(leg.entered_ms - leg.train.scheduled_ms);
        }
        assert_eq!(observed, expected);

        let advance = |world: &mut World, time| {
            let step = world
                .operation_mut()
                .unwrap()
                .runtime
                .advance_to(time, BUDGET)
                .unwrap();
            assert!(step.reached_target);
            step
        };
        let mid = advance(&mut world, BEFORE_DRAIN_MS);
        let bytes = rosenfu_storage::encode(&world).unwrap();
        let mut reopened = rosenfu_storage::decode(&bytes).unwrap();
        assert_eq!(rosenfu_storage::encode(&reopened).unwrap(), bytes);
        let tail = advance(&mut world, DRAIN_MS);
        assert_eq!(tail, advance(&mut reopened, DRAIN_MS));
        assert_eq!(world.operation_save(), reopened.operation_save());
        assert_eq!(world.operation().unwrap().runtime.trains().count(), 0);
        assert_eq!(mid.arrivals.len() + tail.arrivals.len(), 12);
        assert_eq!(report.completed_legs.len(), 12);
        let live: BTreeSet<_> = mid
            .arrivals
            .into_iter()
            .chain(tail.arrivals)
            .map(|a| (a.train, a.station_id, a.at_ms))
            .collect();
        let replay: BTreeSet<_> = report
            .completed_legs
            .into_iter()
            .map(|l| (l.train, l.to, l.arrived_ms))
            .collect();
        assert_eq!(live, replay);
        (plans, early, max_origin_wait)
    };
    for headway_ms in [120_000, 300_000] {
        let (same_offers, same_early, same_wait) = observe(headway_ms, false);
        let (refuge_offers, refuge_early, refuge_wait) = observe(headway_ms, true);
        // The configured frequency stays fixed; path running times and
        // enough ready-to-arrival observations may legitimately differ.
        let nominal = |offers: Vec<rosenfu_core::operation::PassengerOffer>| {
            offers
                .into_iter()
                .map(|o| o.headway_seconds)
                .collect::<Vec<_>>()
        };
        assert_eq!(nominal(same_offers), nominal(refuge_offers));
        assert_eq!((same_early, refuge_early), (1, 2));
        assert!(refuge_wait < same_wait);
        println!(
            "headway={headway_ms}ms: arrivals before 600s same={same_early}/8 refuge={refuge_early}/8; max origin wait same={same_wait}ms refuge={refuge_wait}ms"
        );
    }
}
