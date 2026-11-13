use rosenfu_core::world::*;
use rosenfu_storage::{backup_path, decode, encode, load_world, save_world};
use serde_json::{json, Value};
use std::fs;

fn schema_four() -> Value {
    let mut world = decode(include_bytes!("fixtures/schema-1-world.json")).unwrap();
    world
        .add_service(
            CompanyId(2),
            "Revisit".into(),
            StationId(3),
            vec![TrackId(6), TrackId(6)],
            vec![0, 2],
        )
        .unwrap();
    world
        .add_planned_route(
            CompanyId(2),
            "Sidecar".into(),
            StationId(3),
            vec![RouteSegment::Track {
                track_id: TrackId(6),
            }],
            vec![StationId(3), StationId(4)],
        )
        .unwrap();
    let mut value: Value = serde_json::from_slice(&encode(&world).unwrap()).unwrap();
    value["schema_version"] = json!(4);
    value["world"]
        .as_object_mut()
        .unwrap()
        .remove("source_records");
    value["world"]
        .as_object_mut()
        .unwrap()
        .remove("alignment_provenance");
    value["world"]
        .as_object_mut()
        .unwrap()
        .remove("service_paths");
    value
}

#[test]
fn schema_four_preserves_every_existing_field_and_original_backup() {
    let old = schema_four();
    let bytes = serde_json::to_vec_pretty(&old).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v4.json");
    fs::write(&path, &bytes).unwrap();
    let world = load_world(&path).unwrap();
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let current: Value = serde_json::from_slice(&encode(&world).unwrap()).unwrap();
    assert_eq!(current["schema_version"], 7);
    assert_eq!(current["world"]["service_paths"], json!([]));
    for (key, value) in old["world"].as_object().unwrap() {
        assert_eq!(&current["world"][key], value, "changed {key}");
    }
    save_world(&path, &world).unwrap();
    assert_eq!(fs::read(backup_path(&path)).unwrap(), bytes);
    assert_eq!(load_world(&path).unwrap(), world);
}

#[test]
fn all_legacy_versions_reject_service_path_injection_even_when_empty() {
    for version in 1..=4 {
        let mut value = if version == 1 {
            serde_json::from_slice(include_bytes!("fixtures/schema-1-world.json")).unwrap()
        } else {
            schema_four()
        };
        value["schema_version"] = json!(version);
        if version == 2 || version == 3 {
            value["world"]["routes"] = json!([]);
            value["world"]
                .as_object_mut()
                .unwrap()
                .remove("route_paths");
        }
        assert!(
            decode(&serde_json::to_vec(&value).unwrap()).is_ok(),
            "baseline {version}"
        );
        for injected in [json!([]), json!([{"service_id":"10"}])] {
            value["world"]["service_paths"] = injected;
            assert!(
                decode(&serde_json::to_vec(&value).unwrap()).is_err(),
                "version {version}"
            );
        }
    }
}

#[test]
fn current_requires_both_sidecars_and_schema_four_requires_its_old_fields() {
    let old = schema_four();
    let current: Value = serde_json::from_slice(
        &encode(&decode(&serde_json::to_vec(&old).unwrap()).unwrap()).unwrap(),
    )
    .unwrap();
    for field in ["service_paths", "route_paths"] {
        for replacement in [None, Some(json!(null)), Some(json!({}))] {
            let mut bad = current.clone();
            match replacement {
                Some(value) => bad["world"][field] = value,
                None => {
                    bad["world"].as_object_mut().unwrap().remove(field);
                }
            }
            assert!(
                decode(&serde_json::to_vec(&bad).unwrap()).is_err(),
                "{field}"
            );
        }
    }
    for field in ["route_paths", "services"] {
        let mut bad = old.clone();
        bad["world"].as_object_mut().unwrap().remove(field);
        assert!(
            decode(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "v4 {field}"
        );
    }
}

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
fn schema_five_alignment_service_survives_new_station_and_roundtrip() {
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
        ("/world/services/0/stops", json!([0, 1])),
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

#[test]
fn schema_four_retains_previously_optional_alignment_fields() {
    let mut old = schema_four();
    old["world"].as_object_mut().unwrap().remove("alignments");
    old["world"]
        .as_object_mut()
        .unwrap()
        .remove("station_placements");
    let world = decode(&serde_json::to_vec(&old).unwrap()).unwrap();
    assert!(world.snapshot().alignments.is_empty());
    assert!(world.snapshot().station_placements.is_empty());
}
