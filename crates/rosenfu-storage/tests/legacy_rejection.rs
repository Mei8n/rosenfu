use rosenfu_core::world::World;
use rosenfu_storage::{
    backup_path, decode, encode, load_world, save_world, StorageError, SCHEMA_VERSION,
};
use serde_json::json;
use std::fs;

#[test]
fn every_old_schema_is_rejected_before_world_parsing_without_touching_files() {
    assert_eq!(SCHEMA_VERSION, 18);
    assert!(matches!(
        decode(include_bytes!("fixtures/schema-1-world.json")),
        Err(StorageError::UnsupportedVersion(1))
    ));
    let dir = tempfile::tempdir().unwrap();
    for version in 1..SCHEMA_VERSION {
        let bytes = serde_json::to_vec(
            &json!({"format":"rosenfu-world", "schema_version":version, "world":"legacy contents"}),
        )
        .unwrap();
        let path = dir.path().join(format!("old-{version}.json"));
        fs::write(&path, &bytes).unwrap();
        fs::write(backup_path(&path), b"original backup").unwrap();
        let error = load_world(&path).unwrap_err();
        assert!(matches!(error, StorageError::UnsupportedVersion(v) if v == version));
        assert!(error.to_string().contains("旧データは移行せず"));
        assert!(save_world(&path, &World::default()).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(fs::read(backup_path(&path)).unwrap(), b"original backup");
    }
}

#[test]
fn old_track_shapes_cannot_be_disguised_as_current_schema() {
    let current: serde_json::Value =
        serde_json::from_slice(&encode(&World::default()).unwrap()).unwrap();
    for (field, content) in [
        ("tracks", json!([])),
        ("tracks", json!([{"id":"3","from":"1","to":"2"}])),
        ("routes", json!([{"tracks":[]}])),
        ("services", json!([{"stops":[]}])),
        (
            "route_paths",
            json!([{"segments":[{"kind":"track","track_id":"3"}]}]),
        ),
        (
            "service_paths",
            json!([{"legs":[{"segment":{"kind":"track","track_id":"3"}}]}]),
        ),
    ] {
        let mut value = current.clone();
        value["world"][field] = content;
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(
            matches!(decode(&bytes), Err(StorageError::LegacyFormat)),
            "{field}"
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("disguised.json");
        fs::write(&path, &bytes).unwrap();
        fs::write(backup_path(&path), b"backup").unwrap();
        assert!(load_world(&path).is_err());
        assert!(save_world(&path, &World::default()).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(fs::read(backup_path(&path)).unwrap(), b"backup");
    }
}

#[test]
fn current_requires_all_authoritative_path_arrays() {
    let current: serde_json::Value =
        serde_json::from_slice(&encode(&World::default()).unwrap()).unwrap();
    for field in [
        "route_paths",
        "service_paths",
        "service_route_bindings",
        "source_records",
        "alignment_provenance",
    ] {
        for replacement in [None, Some(json!(null)), Some(json!({}))] {
            let mut invalid = current.clone();
            match replacement {
                Some(v) => invalid["world"][field] = v,
                None => {
                    invalid["world"].as_object_mut().unwrap().remove(field);
                }
            }
            assert!(
                matches!(
                    decode(&serde_json::to_vec(&invalid).unwrap()),
                    Err(StorageError::InvalidHeader)
                ),
                "{field}"
            );
        }
    }
}

#[test]
fn explicit_legacy_replacement_preserves_exact_source_and_existing_backup() {
    use rosenfu_storage::{file_version, replace_legacy_world_if_version, SaveOutcome};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.json");
    let old = include_bytes!("fixtures/schema-1-world.json");
    fs::write(&path, old).unwrap();
    fs::write(backup_path(&path), b"existing rolling backup").unwrap();
    let approved = file_version(&path).unwrap();
    let world = World::default();
    let (outcome, version) = replace_legacy_world_if_version(&path, &world, &approved).unwrap();
    let SaveOutcome::Updated { backup } = outcome else {
        panic!("missing legacy backup")
    };
    assert_eq!(fs::read(&backup).unwrap(), old);
    assert_eq!(
        fs::read(backup_path(&path)).unwrap(),
        b"existing rolling backup"
    );
    assert_eq!(load_world(&path).unwrap(), world);
    assert_eq!(file_version(&path).unwrap(), version);
    assert!(replace_legacy_world_if_version(&path, &world, &approved).is_err());
    assert_eq!(fs::read(backup).unwrap(), old);
}

#[test]
fn explicit_legacy_replacement_rejects_stale_future_corrupt_and_backup_failure() {
    use rosenfu_storage::{file_version, replace_legacy_world_if_version};
    use sha2::{Digest, Sha256};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.json");
    let old = include_bytes!("fixtures/schema-1-world.json");
    fs::write(&path, old).unwrap();
    let approved = file_version(&path).unwrap();
    let world = World::default();
    for bad in [
        b"not json".to_vec(),
        br#"{"format":"rosenfu-world","schema_version":999,"world":{}}"#.to_vec(),
        encode(&world).unwrap(),
    ] {
        fs::write(&path, &bad).unwrap();
        assert!(replace_legacy_world_if_version(&path, &world, &approved).is_err());
        let current = file_version(&path).unwrap();
        assert!(replace_legacy_world_if_version(&path, &world, &current).is_err());
        assert_eq!(fs::read(&path).unwrap(), bad);
    }
    fs::write(&path, old).unwrap();
    let mut name = path.as_os_str().to_os_string();
    name.push(format!(".legacy-{:x}.bak", Sha256::digest(old)));
    let backup = std::path::PathBuf::from(name);
    fs::create_dir(&backup).unwrap();
    assert!(replace_legacy_world_if_version(&path, &world, &approved).is_err());
    assert_eq!(fs::read(&path).unwrap(), old);
    fs::remove_dir(&backup).unwrap();
    fs::write(&backup, b"unrelated bytes").unwrap();
    assert!(replace_legacy_world_if_version(&path, &world, &approved).is_err());
    assert_eq!(fs::read(&path).unwrap(), old);
    assert_eq!(fs::read(backup).unwrap(), b"unrelated bytes");
}
