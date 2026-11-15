use rosenfu_core::world::*;
use rosenfu_storage::{backup_path, decode, encode, save_world};
use serde_json::{json, Value};
use std::fs;

fn alignment_service() -> (World, AlignmentId, CompanyId, ServiceId) {
    let mut world = World::default();
    let player = world.add_player("P".into()).unwrap();
    let company = world.add_company(player, "C".into()).unwrap();
    let alignment = world
        .build_alignment(
            company,
            "A".into(),
            vec![
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.0,
                },
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.1,
                },
            ],
        )
        .unwrap();
    let a = world
        .place_station_on_alignment(company, "A".into(), alignment, 0.1, 100.0, 10.0)
        .unwrap();
    let b = world
        .place_station_on_alignment(company, "B".into(), alignment, 0.8, 100.0, 10.0)
        .unwrap();
    let service = world
        .add_service_path(
            company,
            "S".into(),
            a,
            vec![RouteSegment::Alignment {
                alignment_id: alignment,
                from: a,
                to: b,
            }],
            vec![
                StationOccurrenceRef::Start,
                StationOccurrenceRef::OnLeg {
                    leg_key: 1,
                    station_id: b,
                },
            ],
        )
        .unwrap();
    (world, alignment, company, service)
}

#[test]
fn alignment_service_survives_new_station_and_roundtrip() {
    let (mut world, alignment, company, service) = alignment_service();
    let original = world.snapshot().service_paths.clone();
    world
        .place_station_on_alignment(company, "New".into(), alignment, 0.4, 100.0, 10.0)
        .unwrap();
    assert_eq!(world.snapshot().service_paths, original);
    assert_eq!(world.service_path_occurrences(service).unwrap().len(), 3);
    let bytes = encode(&world).unwrap();
    assert_eq!(decode(&bytes).unwrap(), world);
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["world"]["service_paths"][0]["legs"][0]["key"], "1");
    assert_eq!(json["world"]["service_paths"][0]["next_leg_key"], "2");
}

#[test]
fn malformed_service_path_rejected_without_overwriting_save_or_backup() {
    let (world, _, _, _) = alignment_service();
    let good: Value = serde_json::from_slice(&encode(&world).unwrap()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.json");
    for (pointer, value) in [
        ("/world/service_paths/0/service_id", json!("999")),
        ("/world/service_paths/0/legs/0/key", json!(1)),
        ("/world/service_paths/0/legs/0/key", json!("01")),
        ("/world/service_paths/0/next_leg_key", json!("1")),
        ("/world/service_paths/0/stops/1/leg_key", json!("999")),
        ("/world/service_paths/0/stops/1/station_id", json!("999")),
    ] {
        let mut bad = good.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        let bytes = serde_json::to_vec(&bad).unwrap();
        assert!(decode(&bytes).is_err(), "{pointer}");
        fs::write(&path, &bytes).unwrap();
        fs::write(backup_path(&path), b"recovery sentinel").unwrap();
        assert!(save_world(&path, &world).is_err(), "{pointer}");
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(fs::read(backup_path(&path)).unwrap(), b"recovery sentinel");
    }
}
