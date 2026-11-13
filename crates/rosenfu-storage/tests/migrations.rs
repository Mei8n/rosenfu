use rosenfu_core::world::{CompanyId, StationId, TrackId};
use rosenfu_storage::{
    backup_path, decode, encode, load_world, save_world, StorageError, SCHEMA_VERSION,
};
use std::fs;

const LEGACY: &[u8] = include_bytes!("fixtures/schema-1-world.json");

#[test]
fn fixed_schema_one_fixture_migrates_without_changing_graph_or_allocator() {
    let mut world = decode(LEGACY).unwrap();
    let data = world.snapshot();
    assert_eq!(data.players.len(), 1);
    assert_eq!(data.companies.len(), 1);
    assert_eq!(data.stations.len(), 3);
    assert_eq!(data.stations[0].latitude, 35.125);
    assert_eq!(data.tracks.len(), 2);
    assert_eq!(data.routes[0].tracks, vec![TrackId(6), TrackId(7)]);
    assert!(data.services.is_empty());
    assert_eq!(data.next_id, 10);
    assert_eq!(world.add_player("After migration".into()).unwrap().0, 10);
}

#[test]
fn current_schema_roundtrip_retains_services_and_stop_occurrences() {
    let mut source = decode(LEGACY).unwrap();
    source
        .add_service(
            CompanyId(2),
            "Express".into(),
            StationId(3),
            vec![TrackId(6), TrackId(7)],
            vec![0, 2],
        )
        .unwrap();
    let bytes = encode(&source).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(SCHEMA_VERSION, 8);
    assert_eq!(json["schema_version"], 8);
    assert_eq!(json["world"]["services"][0]["id"], "10");
    assert_eq!(
        json["world"]["services"][0]["stops"],
        serde_json::json!([0, 2])
    );
    assert_eq!(decode(&bytes).unwrap(), source);
}

#[test]
fn loading_old_save_is_read_only_and_explicit_save_preserves_original_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.rosenfu.json");
    let backup = backup_path(&path);
    fs::write(&path, LEGACY).unwrap();
    fs::write(&backup, b"existing backup sentinel").unwrap();
    let world = load_world(&path).unwrap();
    assert_eq!(fs::read(&path).unwrap(), LEGACY);
    assert_eq!(fs::read(&backup).unwrap(), b"existing backup sentinel");
    save_world(&path, &world).unwrap();
    assert_eq!(fs::read(&backup).unwrap(), LEGACY);
    let current: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(current["schema_version"], 8);
    assert_eq!(current["world"]["services"], serde_json::json!([]));
    assert_eq!(load_world(&path).unwrap(), world);
}

#[test]
fn schema_one_rejects_services_and_unknown_fields_at_every_legacy_boundary() {
    let legacy: serde_json::Value = serde_json::from_slice(LEGACY).unwrap();
    for pointer in [
        "",
        "/world",
        "/world/players/0",
        "/world/companies/0",
        "/world/stations/0",
        "/world/tracks/0",
        "/world/routes/0",
    ] {
        let mut altered = legacy.clone();
        altered
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), serde_json::json!(true));
        assert!(
            matches!(
                decode(&serde_json::to_vec(&altered).unwrap()),
                Err(StorageError::Json(_))
            ),
            "{pointer}"
        );
    }
    let mut disguised = legacy;
    disguised["world"]["services"] = serde_json::json!([]);
    assert!(matches!(
        decode(&serde_json::to_vec(&disguised).unwrap()),
        Err(StorageError::Json(_))
    ));
}

#[test]
fn schema_two_requires_services_and_schema_one_requires_all_legacy_fields() {
    let mut legacy: serde_json::Value = serde_json::from_slice(LEGACY).unwrap();
    legacy["schema_version"] = serde_json::json!(2);
    assert!(matches!(
        decode(&serde_json::to_vec(&legacy).unwrap()),
        Err(StorageError::Json(_))
    ));
    legacy["schema_version"] = serde_json::json!(1);
    legacy["world"].as_object_mut().unwrap().remove("routes");
    assert!(matches!(
        decode(&serde_json::to_vec(&legacy).unwrap()),
        Err(StorageError::Json(_))
    ));
}

#[test]
fn failed_legacy_migration_preserves_original_and_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.rosenfu.json");
    let backup = backup_path(&path);
    let good = decode(LEGACY).unwrap();
    let legacy: serde_json::Value = serde_json::from_slice(LEGACY).unwrap();
    for kind in [
        "bad-reference",
        "bad-counter",
        "disguised-new-data",
        "unknown-field",
    ] {
        let mut altered = legacy.clone();
        match kind {
            "bad-reference" => altered["world"]["companies"][0]["owner"] = serde_json::json!("999"),
            "bad-counter" => altered["world"]["next_id"] = serde_json::json!("1"),
            "disguised-new-data" => altered["world"]["services"] = serde_json::json!([]),
            _ => altered["world"]["future_data"] = serde_json::json!(123),
        }
        let bytes = serde_json::to_vec(&altered).unwrap();
        fs::write(&path, &bytes).unwrap();
        fs::write(&backup, b"keep recovery bytes").unwrap();
        assert!(load_world(&path).is_err(), "{kind}");
        assert!(save_world(&path, &good).is_err(), "{kind}");
        assert_eq!(fs::read(&path).unwrap(), bytes, "{kind}");
        assert_eq!(fs::read(&backup).unwrap(), b"keep recovery bytes", "{kind}");
    }
}
