use rosenfu_core::world::*;
use rosenfu_storage::{backup_path, decode, encode, load_world, save_world};
use serde_json::{json, Value};
use std::fs;

fn legacy(version: u64) -> Value {
    let fixture: Value =
        serde_json::from_slice(include_bytes!("fixtures/schema-1-world.json")).unwrap();
    if version == 1 {
        return fixture;
    }
    let mut world = decode(&serde_json::to_vec(&fixture).unwrap()).unwrap();
    world
        .add_service(
            CompanyId(2),
            "Legacy".into(),
            StationId(3),
            vec![TrackId(6), TrackId(6)],
            vec![0, 2],
        )
        .unwrap();
    if version >= 4 {
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
    }
    if version == 5 {
        world
            .add_service_path(
                CompanyId(2),
                "New".into(),
                StationId(3),
                vec![RouteSegment::Track {
                    track_id: TrackId(6),
                }],
                vec![
                    StationOccurrenceRef::Start,
                    StationOccurrenceRef::OnLeg {
                        leg_key: 1,
                        station_id: StationId(4),
                    },
                ],
            )
            .unwrap();
    }
    let mut value: Value = serde_json::from_slice(&encode(&world).unwrap()).unwrap();
    value["schema_version"] = json!(version);
    let obj = value["world"].as_object_mut().unwrap();
    obj.remove("source_records");
    obj.remove("alignment_provenance");
    if version < 5 {
        obj.remove("service_paths");
    }
    if version < 4 {
        obj.remove("route_paths");
    }
    value
}

#[test]
fn schemas_one_through_five_preserve_all_fields_and_backup_bytes() {
    for version in 1..=5 {
        let old = legacy(version);
        let bytes = serde_json::to_vec_pretty(&old).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("legacy.json");
        fs::write(&path, &bytes).unwrap();
        let world = load_world(&path).unwrap();
        assert_eq!(fs::read(&path).unwrap(), bytes);
        let current: Value = serde_json::from_slice(&encode(&world).unwrap()).unwrap();
        assert_eq!(current["schema_version"], 9);
        for (key, value) in old["world"].as_object().unwrap() {
            assert_eq!(&current["world"][key], value, "v{version}: {key}");
        }
        assert_eq!(current["world"]["source_records"], json!([]));
        assert_eq!(current["world"]["alignment_provenance"], json!([]));
        save_world(&path, &world).unwrap();
        assert_eq!(fs::read(backup_path(&path)).unwrap(), bytes);
        assert_eq!(load_world(&path).unwrap(), world);
    }
}

#[test]
fn all_old_versions_reject_each_new_sidecar_without_touching_files() {
    for version in 1..=5 {
        for field in ["source_records", "alignment_provenance"] {
            let mut value = legacy(version);
            let good = decode(&serde_json::to_vec(&value).unwrap()).unwrap();
            value["world"][field] = json!([]);
            let bytes = serde_json::to_vec(&value).unwrap();
            assert!(decode(&bytes).is_err(), "v{version}: {field}");
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("bad.json");
            fs::write(&path, &bytes).unwrap();
            fs::write(backup_path(&path), b"recovery").unwrap();
            assert!(save_world(&path, &good).is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
            assert_eq!(fs::read(backup_path(&path)).unwrap(), b"recovery");
        }
    }
}

#[test]
fn schema_five_requires_old_sidecars_and_allows_optional_alignment_arrays() {
    let old = legacy(5);
    for field in ["route_paths", "service_paths"] {
        let mut bad = old.clone();
        bad["world"].as_object_mut().unwrap().remove(field);
        assert!(decode(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
    let mut optional = old;
    optional["world"]
        .as_object_mut()
        .unwrap()
        .remove("alignments");
    optional["world"]
        .as_object_mut()
        .unwrap()
        .remove("station_placements");
    assert!(decode(&serde_json::to_vec(&optional).unwrap()).is_ok());
}

#[test]
fn schema_six_requires_arrays_not_missing_null_or_object() {
    let current: Value = serde_json::from_slice(&encode(&World::default()).unwrap()).unwrap();
    for field in [
        "route_paths",
        "service_paths",
        "source_records",
        "alignment_provenance",
    ] {
        for replacement in [None, Some(json!(null)), Some(json!({}))] {
            let mut bad = current.clone();
            match replacement {
                Some(v) => bad["world"][field] = v,
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
}

fn provenance_world() -> World {
    let mut world = World::default();
    let p = world.add_player("P".into()).unwrap();
    let c = world.add_company(p, "C".into()).unwrap();
    let a = world
        .build_alignment(
            c,
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
    let mut draft = world.snapshot().clone();
    let key = format!("{}:{}", "a".repeat(64), "b".repeat(64));
    draft.source_records.push(SourceRecord {
        key: key.clone(),
        dataset_id: "fixture".into(),
        revision: "r1".into(),
        graph_sha256: "a".repeat(64),
        source_sha256: "b".repeat(64),
        source: "offline test fixture".into(),
        source_date: None,
        license: "ODbL-1.0".into(),
        attribution: "© OpenStreetMap contributors".into(),
        conversion_format_version: 1,
    });
    draft.alignment_provenance.push(AlignmentProvenance {
        alignment_id: a,
        start_point_index: 0,
        end_point_index: 1,
        source: AlignmentProvenanceInput {
            source_key: key,
            method: ProvenanceMethod::OsmSegmentCopyV1,
            osm_way_id: "123".into(),
            mode: ReferenceMode::Rail,
            start_segment_index: 0,
            start_fraction: 0.1,
            end_segment_index: 0,
            end_fraction: 0.9,
        },
    });
    World::new(draft).unwrap()
}

#[test]
fn provenance_roundtrip_and_corrupt_metadata_are_strict() {
    let world = provenance_world();
    let bytes = encode(&world).unwrap();
    assert_eq!(decode(&bytes).unwrap(), world);
    let good: Value = serde_json::from_slice(&bytes).unwrap();
    for (pointer, v) in [
        ("/world/alignment_provenance/0/alignment_id", json!("999")),
        (
            "/world/alignment_provenance/0/source/source_key",
            json!("missing"),
        ),
        (
            "/world/alignment_provenance/0/source/start_fraction",
            json!(2),
        ),
        ("/world/source_records/0/graph_sha256", json!("invalid")),
        ("/world/source_records/0/attribution", json!("wrong")),
    ] {
        let mut bad = good.clone();
        *bad.pointer_mut(pointer).unwrap() = v;
        assert!(
            decode(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "{pointer}"
        );
    }
    for pointer in [
        "/world/source_records/0",
        "/world/alignment_provenance/0",
        "/world/alignment_provenance/0/source",
    ] {
        let mut bad = good.clone();
        bad.pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), json!(true));
        assert!(
            decode(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "{pointer}"
        );
    }
}

#[test]
fn schema_seven_source_ranges_migrate_and_extended_ranges_roundtrip() {
    let mut world = provenance_world();
    let mut old: Value = serde_json::from_slice(&encode(&world).unwrap()).unwrap();
    old["schema_version"] = json!(7);
    for p in old["world"]["alignment_provenance"].as_array_mut().unwrap() {
        p.as_object_mut().unwrap().remove("start_point_index");
        p.as_object_mut().unwrap().remove("end_point_index");
    }
    assert_eq!(decode(&serde_json::to_vec(&old).unwrap()).unwrap(), world);
    let mut v6 = old.clone();
    v6["schema_version"] = json!(6);
    for a in v6["world"]["alignments"].as_array_mut().unwrap() {
        a.as_object_mut().unwrap().remove("track_type");
    }
    assert_eq!(decode(&serde_json::to_vec(&v6).unwrap()).unwrap(), world);
    old["world"]["alignment_provenance"][0]["start_point_index"] = json!(0);
    assert!(decode(&serde_json::to_vec(&old).unwrap()).is_err());
    let a = world.snapshot().alignments[0].clone();
    let end = *a.points.last().unwrap();
    let prev = a.points[a.points.len() - 2];
    world
        .extend_alignment(
            a.owner,
            a.id,
            AlignmentEndpoint::End,
            vec![
                end,
                GeoPoint {
                    latitude: 2.0 * end.latitude - prev.latitude,
                    longitude: 2.0 * end.longitude - prev.longitude,
                },
            ],
        )
        .unwrap();
    assert_eq!(decode(&encode(&world).unwrap()).unwrap(), world);
}

#[test]
fn schema_seven_keeps_optional_alignment_arrays_but_rejects_wrong_types() {
    let world = World::default();
    let mut old: Value = serde_json::from_slice(&encode(&world).unwrap()).unwrap();
    old["schema_version"] = json!(7);
    old["world"].as_object_mut().unwrap().remove("alignments");
    old["world"]
        .as_object_mut()
        .unwrap()
        .remove("station_placements");
    assert_eq!(decode(&serde_json::to_vec(&old).unwrap()).unwrap(), world);
    for invalid in [Value::Null, json!({}), json!("invalid")] {
        old["world"]["alignments"] = invalid;
        assert!(decode(&serde_json::to_vec(&old).unwrap()).is_err());
    }
}
