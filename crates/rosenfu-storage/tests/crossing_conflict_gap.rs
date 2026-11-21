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
