use rosenfu_core::{
    operation::{Timetable, TrainState, Vehicle},
    world::{GeoPoint, OperationServiceConfig, RouteSegment, World},
};
use rosenfu_storage::{decode, load_world_with_version, save_world, StorageError, SCHEMA_VERSION};

#[test]
fn current_save_reopens_terminal_hold_without_admitting_opposing_train() {
    let mut world = World::default();
    let player = world.add_player("p".into()).unwrap();
    let company = world.add_company(player, "c".into()).unwrap();
    let alignment = world
        .build_alignment(
            company,
            "straight corridor".into(),
            (0..4)
                .map(|i| GeoPoint {
                    latitude: 35.0,
                    longitude: 139.0 + 0.0044 * i as f64,
                })
                .collect(),
        )
        .unwrap();
    let mut draft = world.snapshot().clone();
    draft.alignments[0].anchor_indices = Some(vec![0, 1, 2, 3]);
    world.replace(draft).unwrap();
    let a = world
        .place_station_on_alignment(company, "A".into(), alignment, 0.10, 60., 10.)
        .unwrap();
    let b = world
        .place_station_on_alignment(company, "B".into(), alignment, 0.90, 60., 10.)
        .unwrap();
    let configs: Vec<_> = [(a, b), (b, a)]
        .into_iter()
        .map(|(from, to)| {
            let segments = vec![RouteSegment::Alignment {
                alignment_id: alignment,
                from,
                to,
            }];
            let stops = world
                .preview_service_path(from, segments.clone())
                .unwrap()
                .occurrences
                .iter()
                .map(|stop| stop.reference.clone())
                .collect();
            let service_id = world
                .add_service_path(company, "shuttle".into(), from, segments, stops)
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
                dwell_ms: 30_000,
            }
        })
        .collect();
    let leader = configs[0].service_id.0.to_string();
    let opposing = configs[1].service_id.0.to_string();
    world.configure_operation(configs).unwrap();

    let advance = |world: &mut World, to| {
        world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(to, 100)
            .unwrap()
    };
    advance(&mut world, 0);
    let current = world.operation_save().unwrap();
    assert_eq!(current.runtime.format, 4);
    let moving = current
        .runtime
        .trains
        .iter()
        .find(|train| train.key.service_id == leader)
        .unwrap();
    let TrainState::Moving {
        arrival_ms,
        release_ms,
        ..
    } = moving.state
    else {
        panic!("leader did not depart");
    };
    assert_eq!(release_ms, arrival_ms + 30_000);
    advance(&mut world, release_ms - 1);

    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("terminal.json");
    save_world(&path, &world).unwrap();
    let (mut reopened, version) = load_world_with_version(&path).unwrap();
    assert_eq!(version, rosenfu_storage::file_version(&path).unwrap());
    assert_eq!(reopened.operation_save(), world.operation_save());
    let result = advance(&mut reopened, release_ms);
    assert_eq!(result, advance(&mut world, release_ms));
    let saved = reopened.operation_save().unwrap();
    let returning = saved
        .runtime
        .trains
        .iter()
        .find(|train| train.key.service_id == leader)
        .unwrap();
    assert_eq!(returning.key, moving.key);
    assert_eq!(returning.leg, 1);
    assert!(saved
        .runtime
        .trains
        .iter()
        .any(|train| train.key.service_id == opposing
            && matches!(train.state, TrainState::Waiting { .. })));
    assert_eq!(
        advance(&mut reopened, 500_000),
        advance(&mut world, 500_000),
    );
    assert_eq!(reopened.operation_save(), world.operation_save());

    let bytes = std::fs::read(&path).unwrap();
    let mut document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(document["schema_version"], SCHEMA_VERSION);
    document["schema_version"] = serde_json::json!(26);
    assert!(matches!(
        decode(&serde_json::to_vec(&document).unwrap()),
        Err(StorageError::InvalidHeader)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}
