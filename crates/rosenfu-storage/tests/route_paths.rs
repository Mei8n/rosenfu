use rosenfu_core::world::*;
use rosenfu_storage::{backup_path, decode, encode, load_world, save_world};
use serde_json::{json, Value};
use std::fs;
fn sample() -> (World, AlignmentId, CompanyId, Vec<StationId>) {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let a = w
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
    let s = [0.1, 0.5, 0.9]
        .map(|pos| {
            w.place_station_on_alignment(c, "S".into(), a, pos, 100.0, 10.0)
                .unwrap()
        })
        .to_vec();
    (w, a, c, s)
}
#[test]
fn current_schema_roundtrip_preserves_alignment_membership_and_future_insertions() {
    let (mut w, a, c, s) = sample();
    w.add_planned_route(
        c,
        "Route".into(),
        s[0],
        vec![RouteSegment::Alignment {
            alignment_id: a,
            from: s[0],
            to: s[2],
        }],
        vec![s[0], s[2]],
    )
    .unwrap();
    w.place_station_on_alignment(c, "New".into(), a, 0.3, 100.0, 10.0)
        .unwrap();
    let bytes = encode(&w).unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["schema_version"], 8);
    assert_eq!(
        value["world"]["route_paths"][0]["member_stations"],
        json!([s[0], s[2]])
    );
    assert_eq!(decode(&bytes).unwrap(), w);
}
#[test]
fn old_schemas_migrate_without_mutation_and_preserve_bytes_on_save() {
    let (w, _, _, _) = sample();
    for version in [2, 3] {
        let mut old: Value = serde_json::from_slice(&encode(&w).unwrap()).unwrap();
        old["schema_version"] = json!(version);
        for alignment in old["world"]["alignments"].as_array_mut().unwrap() {
            alignment.as_object_mut().unwrap().remove("track_type");
        }
        old["world"]
            .as_object_mut()
            .unwrap()
            .remove("source_records");
        old["world"]
            .as_object_mut()
            .unwrap()
            .remove("alignment_provenance");
        old["world"].as_object_mut().unwrap().remove("route_paths");
        old["world"]
            .as_object_mut()
            .unwrap()
            .remove("service_paths");
        let bytes = serde_json::to_vec_pretty(&old).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("old.json");
        fs::write(&file, &bytes).unwrap();
        let restored = load_world(&file).unwrap();
        assert_eq!(restored, w);
        assert_eq!(fs::read(&file).unwrap(), bytes);
        save_world(&file, &restored).unwrap();
        assert_eq!(fs::read(backup_path(&file)).unwrap(), bytes);
        let new: Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
        assert_eq!(new["schema_version"], 8);
    }
}
#[test]
fn legacy_files_cannot_smuggle_new_fields_and_current_requires_paths() {
    let (w, _, _, _) = sample();
    let current: Value = serde_json::from_slice(&encode(&w).unwrap()).unwrap();
    for version in [2, 3] {
        let mut bad = current.clone();
        bad["schema_version"] = json!(version);
        assert!(decode(&serde_json::to_vec(&bad).unwrap()).is_err());
        bad["world"].as_object_mut().unwrap().remove("route_paths");
        bad["world"]["unknown"] = json!(true);
        assert!(decode(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
    let mut v1: Value =
        serde_json::from_slice(include_bytes!("fixtures/schema-1-world.json")).unwrap();
    v1["world"]["route_paths"] = json!([]);
    assert!(decode(&serde_json::to_vec(&v1).unwrap()).is_err());
    let mut bad = current;
    bad["world"].as_object_mut().unwrap().remove("route_paths");
    assert!(decode(&serde_json::to_vec(&bad).unwrap()).is_err());
}
#[test]
fn invalid_new_path_preserves_existing_file_and_backup() {
    let (mut w, a, c, s) = sample();
    w.add_planned_route(
        c,
        "R".into(),
        s[0],
        vec![RouteSegment::Alignment {
            alignment_id: a,
            from: s[0],
            to: s[2],
        }],
        s.clone(),
    )
    .unwrap();
    let mut bad: Value = serde_json::from_slice(&encode(&w).unwrap()).unwrap();
    bad["world"]["route_paths"][0]["member_stations"] = json!([s[0], "999", s[2]]);
    let bytes = serde_json::to_vec(&bad).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.json");
    fs::write(&path, &bytes).unwrap();
    fs::write(backup_path(&path), b"sentinel").unwrap();
    assert!(save_world(&path, &w).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(fs::read(backup_path(&path)).unwrap(), b"sentinel");
}
