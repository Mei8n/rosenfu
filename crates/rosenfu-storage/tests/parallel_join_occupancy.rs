use rosenfu_core::operation::{Timetable, TrainState, Vehicle};
use rosenfu_core::world::{
    AlignmentConnection, AlignmentEndpoint, AlignmentEndpointRef, GeoPoint, InfrastructureLevel,
    OperationServiceConfig, RouteSegment, SectionInfrastructure, World,
};

fn fixture(
    counts: [u8; 2],
    reverse_right: bool,
    opposing: bool,
    curved_join: bool,
) -> (World, Vec<OperationServiceConfig>) {
    use AlignmentEndpoint::{End, Start};
    let mut w = World::default();
    let player = w.add_player("QA".into()).unwrap();
    let owner = w.add_company(player, "Rail".into()).unwrap();
    let point = |longitude, latitude| GeoPoint {
        longitude,
        latitude,
    };
    let join = point(139.55, 35.7);
    let left = w
        .build_alignment(owner, "Left".into(), vec![point(139.54, 35.7), join])
        .unwrap();
    let outside = point(139.56, if curved_join { 35.7001 } else { 35.7 });
    let right_points = if reverse_right {
        vec![outside, join]
    } else {
        vec![join, outside]
    };
    let right = w
        .build_alignment(owner, "Right".into(), right_points)
        .unwrap();
    let mut d = w.snapshot().clone();
    for (line, number) in d.alignments.iter_mut().zip(counts) {
        line.infrastructure = vec![SectionInfrastructure {
            track_count: number,
            level: InfrastructureLevel::Ground,
        }];
    }
    w.replace(d).unwrap();
    let link = AlignmentConnection {
        a: AlignmentEndpointRef {
            alignment_id: left,
            endpoint: End,
        },
        b: AlignmentEndpointRef {
            alignment_id: right,
            endpoint: if reverse_right { End } else { Start },
        },
    };
    w.connect_alignment_endpoints(owner, link.a, link.b)
        .unwrap();
    let a = w
        .place_station_on_alignment(owner, "A".into(), left, 0.15, 80., 10.)
        .unwrap();
    let b = w
        .place_station_on_alignment(
            owner,
            "B".into(),
            right,
            if reverse_right { 0.15 } else { 0.85 },
            80.,
            10.,
        )
        .unwrap();
    let configs = [(a, b), if opposing { (b, a) } else { (a, b) }]
        .into_iter()
        .enumerate()
        .map(|(i, (from, to))| {
            let legs = vec![RouteSegment::ConnectedAlignments {
                from,
                to,
                connections: vec![if from == a {
                    link.clone()
                } else {
                    AlignmentConnection {
                        a: link.b,
                        b: link.a,
                    }
                }],
            }];
            let preview = w.preview_service_path(from, legs.clone()).unwrap();
            let stops = preview
                .occurrences
                .iter()
                .map(|o| o.reference.clone())
                .collect();
            let service_id = w
                .add_service_path(owner, format!("Service {i}"), from, legs, stops)
                .unwrap();
            OperationServiceConfig {
                service_id,
                timetable: Timetable {
                    first_departure_ms: 0,
                    last_departure_ms: 0,
                    headway_ms: 60_000,
                },
                vehicle: Vehicle {
                    max_speed_kmh: 72.0,
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

fn sample(w: &mut World, configs: Vec<OperationServiceConfig>) -> (usize, usize) {
    w.configure_operation(configs).unwrap();
    w.operation_mut()
        .unwrap()
        .runtime
        .advance_to(0, 128)
        .unwrap();
    let runtime = &w.operation().unwrap().runtime;
    let moving = runtime
        .trains()
        .filter(|t| matches!(t.state, TrainState::Moving { .. }))
        .count();
    let waiting = runtime
        .trains()
        .filter(|t| matches!(t.state, TrainState::Waiting { .. }))
        .count();
    (moving, waiting)
}

#[test]
fn opposing_parallel_lanes_on_connected_straight_track_do_not_make_each_other_wait() {
    for reverse in [false, true] {
        let (mut w, c) = fixture([2, 2], reverse, true, false);
        let observed = sample(&mut w, c);
        println!("reversed_alignment={reverse}, moving_waiting={observed:?}");
        assert_eq!(observed, (2, 0));
        let arrivals = w
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(600_000, 128)
            .unwrap()
            .arrivals;
        assert_eq!(arrivals.len(), 2);
        assert!(arrivals.iter().all(|arrival| arrival.delay_ms == 0));
    }
}
#[test]
fn shared_lanes_single_track_and_unqualified_angles_still_wait() {
    for (counts, opposing, curve) in [
        ([2, 2], false, false),
        ([1, 1], true, false),
        ([2, 1], true, false),
        ([2, 2], true, true),
    ] {
        let (mut w, c) = fixture(counts, false, opposing, curve);
        assert_eq!(
            sample(&mut w, c),
            (1, 1),
            "{counts:?} opposing={opposing} curve={curve}"
        );
        let arrivals = w
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(600_000, 128)
            .unwrap()
            .arrivals;
        let peak_wait = arrivals
            .iter()
            .map(|arrival| arrival.delay_ms)
            .max()
            .unwrap();
        assert!(peak_wait > 0);
        println!("{counts:?} opposing={opposing} angle={curve} wait_ms={peak_wait}");
    }
}

#[test]
fn old_runtime_keeps_active_shared_reservations_and_exact_save_bytes() {
    use rosenfu_core::operation::OperationRuntime;
    let (mut w, configs) = fixture([2, 2], false, true, false);
    let legacy = configs
        .iter()
        .map(|c| w.compile_operation(c).unwrap())
        .collect();
    w.configure_operation(configs).unwrap();
    w.operation_mut().unwrap().runtime = OperationRuntime::new(legacy).unwrap();
    w.operation_mut()
        .unwrap()
        .runtime
        .advance_to(1_000, 128)
        .unwrap();
    let waiters = |w: &World| {
        w.operation()
            .unwrap()
            .runtime
            .trains()
            .filter(|t| matches!(t.state, TrainState::Waiting { .. }))
            .count()
    };
    assert_eq!(waiters(&w), 1);
    let bytes = rosenfu_storage::encode(&w).unwrap();
    let restored = rosenfu_storage::decode(&bytes).unwrap();
    assert!(!restored.operation_save().unwrap().parallel_join_lanes);
    assert_eq!(waiters(&restored), 1);
    assert_eq!(rosenfu_storage::encode(&restored).unwrap(), bytes);
    let mut edited = World::new(w.snapshot().clone()).unwrap();
    edited.retain_operation_from(&restored);
    assert_eq!(edited.operation_save(), restored.operation_save());
}

#[test]
fn guarded_current_save_restores_exact_lanes_and_invalidates_changes() {
    let (mut w, configs) = fixture([2, 2], true, true, false);
    assert_eq!(sample(&mut w, configs), (2, 0));
    let bytes = rosenfu_storage::encode(&w).unwrap();
    let restored = rosenfu_storage::decode(&bytes).unwrap();
    assert!(w.operation_save().unwrap().parallel_join_lanes);
    assert_eq!(w.operation_save(), restored.operation_save());
    let mut retained = World::new(w.snapshot().clone()).unwrap();
    retained.retain_operation_from(&restored);
    assert_eq!(retained.operation_save(), restored.operation_save());
    let (mut angled, _) = fixture([2, 2], true, true, true);
    angled.retain_operation_from(&restored);
    assert!(angled.operation_save().unwrap().invalidated);
    assert!(angled.operation_save().unwrap().runtime.paused);

    let mut missing: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    missing["operation"]
        .as_object_mut()
        .unwrap()
        .remove("parallel_join_lanes");
    assert!(rosenfu_storage::decode(&serde_json::to_vec(&missing).unwrap()).is_err());
    let mut partial: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let sections = partial["operation"]["runtime"]["services"][0]["legs"][0]["sections"]
        .as_array_mut()
        .unwrap();
    let row = sections
        .iter_mut()
        .find(|section| {
            section["resources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r.as_str().unwrap().starts_with("rail:lane-join:"))
        })
        .unwrap();
    row["resources"]
        .as_array_mut()
        .unwrap()
        .retain(|r| !r.as_str().unwrap().starts_with("rail:lane-join:"));
    assert!(rosenfu_storage::decode(&serde_json::to_vec(&partial).unwrap()).is_err());
    let mut old_header: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    old_header["schema_version"] = 26.into();
    old_header
        .as_object_mut()
        .unwrap()
        .remove("fixed_od_scenario");
    assert!(rosenfu_storage::decode(&serde_json::to_vec(&old_header).unwrap()).is_err());
}
