use rosenfu_core::world::World;
use rosenfu_storage::{
    backup_path, decode, encode, load_world, save_world, SaveOutcome, StorageError, MAX_SAVE_BYTES,
};
use std::fs;

fn sample() -> World {
    let mut world = World::default();
    let player = world.add_player("Player".into()).unwrap();
    let company = world.add_company(player, "Railway".into()).unwrap();
    let line = world
        .build_alignment(
            company,
            "Line".into(),
            vec![
                rosenfu_core::world::GeoPoint {
                    latitude: 35.125,
                    longitude: 139.25,
                },
                rosenfu_core::world::GeoPoint {
                    latitude: 35.5,
                    longitude: 139.75,
                },
            ],
        )
        .unwrap();
    let a = world
        .place_station_on_alignment(company, "A".into(), line, 0.1, 20., 12.)
        .unwrap();
    let b = world
        .place_station_on_alignment(company, "B".into(), line, 0.9, 20., 12.)
        .unwrap();
    world
        .add_planned_route(
            company,
            "Main".into(),
            a,
            vec![rosenfu_core::world::RouteSegment::Alignment {
                alignment_id: line,
                from: a,
                to: b,
            }],
            vec![a, b],
        )
        .unwrap();
    world
}

#[test]
fn json_roundtrip_preserves_graph_and_allocator() {
    let source = sample();
    let mut restored = decode(&encode(&source).unwrap()).unwrap();
    assert_eq!(restored, source);
    let next = restored.add_player("Next player".into()).unwrap();
    assert_eq!(next.0, source.snapshot().next_id);
    let json: serde_json::Value = serde_json::from_slice(&encode(&source).unwrap()).unwrap();
    assert!(json["world"]["players"][0]["id"].is_string());
    assert!(json["world"]["next_id"].is_string());
}

#[test]
fn unsupported_version_is_reported_before_decoding_world() {
    // Schema 27 is the default writer, not the highest supported version.
    for version in [0, 25, 31, u64::MAX] {
        let bytes = format!(
            r#"{{"format":"rosenfu-world","schema_version":{version},"world":"not a world"}}"#
        );
        assert!(
            matches!(decode(bytes.as_bytes()), Err(StorageError::UnsupportedVersion(v)) if v == version)
        );
    }
}

#[test]
fn mixed_schema_header_reaches_world_validation_only_in_compatible_readers() {
    let result = decode(br#"{"format":"rosenfu-world","schema_version":28,"world":"not a world"}"#);
    #[cfg(feature = "standard-periodic-rail")]
    assert!(matches!(result, Err(StorageError::InvalidHeader)));
    #[cfg(not(feature = "standard-periodic-rail"))]
    assert!(matches!(result, Err(StorageError::UnsupportedVersion(28))));
}

#[test]
fn malformed_format_header_and_json_are_rejected() {
    assert!(matches!(decode(b"{"), Err(StorageError::Json(_))));
    assert!(matches!(decode(b"{}"), Err(StorageError::InvalidHeader)));
    assert!(matches!(
        decode(br#"{"format":"other","schema_version":1}"#),
        Err(StorageError::InvalidFormat)
    ));
    assert!(matches!(
        decode(br#"{"format":"rosenfu-world","schema_version":"1"}"#),
        Err(StorageError::InvalidHeader)
    ));
}

#[test]
fn invalid_world_reference_is_not_loaded() {
    let mut json: serde_json::Value = serde_json::from_slice(&encode(&sample()).unwrap()).unwrap();
    json["world"]["companies"][0]["owner"] = serde_json::json!("99999");
    assert!(matches!(
        decode(&serde_json::to_vec(&json).unwrap()),
        Err(StorageError::InvalidWorld(_))
    ));
}

#[test]
fn unknown_envelope_fields_are_rejected() {
    let mut json: serde_json::Value = serde_json::from_slice(&encode(&sample()).unwrap()).unwrap();
    json["unknown"] = serde_json::json!(true);
    assert!(matches!(
        decode(&serde_json::to_vec(&json).unwrap()),
        Err(StorageError::Json(_))
    ));
}

#[test]
fn new_save_and_load_work_without_creating_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.rosenfu.json");
    let world = sample();
    assert_eq!(save_world(&path, &world).unwrap(), SaveOutcome::Created);
    assert_eq!(load_world(&path).unwrap(), world);
    assert!(!backup_path(&path).exists());
}

#[test]
fn replacement_preserves_previous_bytes_and_rotates_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.rosenfu.json");
    let first = sample();
    save_world(&path, &first).unwrap();
    let first_bytes = fs::read(&path).unwrap();
    let mut second = first.clone();
    second.add_player("Second".into()).unwrap();
    assert_eq!(
        save_world(&path, &second).unwrap(),
        SaveOutcome::Updated {
            backup: backup_path(&path)
        }
    );
    assert_eq!(fs::read(backup_path(&path)).unwrap(), first_bytes);
    assert_eq!(load_world(&path).unwrap(), second);
    let second_bytes = fs::read(&path).unwrap();
    save_world(&path, &first).unwrap();
    assert_eq!(fs::read(backup_path(&path)).unwrap(), second_bytes);
    assert_eq!(load_world(backup_path(&path)).unwrap(), second);
    assert_eq!(load_world(&path).unwrap(), first);
}

#[test]
fn corrupt_existing_save_and_backup_remain_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.rosenfu.json");
    let backup = backup_path(&path);
    fs::write(&path, b"corrupt original").unwrap();
    fs::write(&backup, b"previous recovery data").unwrap();
    assert!(save_world(&path, &sample()).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"corrupt original");
    assert_eq!(fs::read(&backup).unwrap(), b"previous recovery data");
}

#[test]
fn unsupported_existing_save_is_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.rosenfu.json");
    let bytes = br#"{"format":"rosenfu-world","schema_version":999,"world":{}}"#;
    fs::write(&path, bytes).unwrap();
    assert!(matches!(
        save_world(&path, &sample()),
        Err(StorageError::UnsupportedVersion(999))
    ));
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(!backup_path(&path).exists());
}

#[test]
fn backup_failure_does_not_modify_original() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.rosenfu.json");
    let mut world = sample();
    save_world(&path, &world).unwrap();
    let before = fs::read(&path).unwrap();
    fs::create_dir(backup_path(&path)).unwrap();
    world.add_player("Another".into()).unwrap();
    assert!(matches!(
        save_world(&path, &world),
        Err(StorageError::NotRegularFile(_))
    ));
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn read_size_is_bounded_and_missing_files_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.rosenfu.json");
    assert!(matches!(load_world(&path), Err(StorageError::Io(_))));
    let bytes = vec![b' '; MAX_SAVE_BYTES + 1];
    assert!(matches!(decode(&bytes), Err(StorageError::TooLarge)));
    fs::write(&path, &bytes).unwrap();
    assert!(matches!(load_world(&path), Err(StorageError::TooLarge)));
    assert!(matches!(
        save_world(&path, &sample()),
        Err(StorageError::TooLarge)
    ));
    assert_eq!(fs::metadata(&path).unwrap().len(), bytes.len() as u64);
}

#[test]
fn exact_disk_budget_and_trailing_bytes_apply_to_load_and_backup() {
    use rosenfu_storage::{file_version, load_world_with_version, save_world_if_version};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bounded.json");
    let backup = backup_path(&path);
    let world = sample();
    let mut bytes = encode(&world).unwrap();
    bytes.resize(MAX_SAVE_BYTES, b' ');
    fs::write(&path, &bytes).unwrap();
    let (loaded, exact_version) = load_world_with_version(&path).unwrap();
    assert_eq!(loaded, world);
    assert_eq!(exact_version, file_version(&path).unwrap());

    // Backup publication accepts whitespace at the cap and preserves exact
    // source bytes, not a normalized reserialization of the now-known world.
    let (_, normalized_version) = save_world_if_version(&path, &world, &exact_version).unwrap();
    assert_ne!(normalized_version, exact_version);
    assert_eq!(fs::read(&backup).unwrap(), bytes);
    assert_eq!(load_world(&backup).unwrap(), world);

    bytes.push(b' ');
    fs::write(&path, &bytes).unwrap();
    let oversized_version = file_version(&path).unwrap();
    assert!(matches!(
        load_world_with_version(&path),
        Err(StorageError::TooLarge)
    ));
    assert!(matches!(
        save_world_if_version(&path, &world, &oversized_version),
        Err(StorageError::TooLarge)
    ));
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(file_version(&backup).unwrap(), exact_version);

    let invalid = [encode(&world).unwrap(), b"x".to_vec()].concat();
    fs::write(&path, &invalid).unwrap();
    let invalid_version = file_version(&path).unwrap();
    assert!(matches!(
        load_world_with_version(&path),
        Err(StorageError::Json(_))
    ));
    assert!(matches!(
        save_world_if_version(&path, &world, &invalid_version),
        Err(StorageError::Json(_))
    ));
    assert_eq!(fs::read(&path).unwrap(), invalid);
    assert_eq!(file_version(&backup).unwrap(), exact_version);
}

#[test]
fn whitespace_only_external_change_breaks_exact_save_version() {
    use rosenfu_storage::{file_version, load_world_with_version, save_world_if_version};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("changed.json");
    let world = sample();
    save_world(&path, &world).unwrap();
    save_world(&path, &world).unwrap();
    let backup = backup_path(&path);
    let before_backup = fs::read(&backup).unwrap();
    let (loaded, baseline) = load_world_with_version(&path).unwrap();
    let changed = [fs::read(&path).unwrap(), b" \n".to_vec()].concat();
    fs::write(&path, &changed).unwrap();
    let (edited, version) = load_world_with_version(&path).unwrap();
    assert_eq!(loaded, edited);
    assert_ne!(baseline, version);
    assert_eq!(version, file_version(&path).unwrap());
    assert!(matches!(
        save_world_if_version(&path, &loaded, &baseline),
        Err(StorageError::FileChanged)
    ));
    assert_eq!(fs::read(&path).unwrap(), changed);
    assert_eq!(fs::read(&backup).unwrap(), before_backup);
}

#[test]
fn missing_parent_does_not_create_partial_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing").join("world.rosenfu.json");
    assert!(matches!(
        save_world(&path, &sample()),
        Err(StorageError::Io(_))
    ));
    assert!(!path.exists());
}

#[test]
fn create_only_api_never_replaces_existing_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.rosenfu.json");
    let first = sample();
    rosenfu_storage::create_world(&path, &first).unwrap();
    let before = fs::read(&path).unwrap();
    assert!(rosenfu_storage::create_world(&path, &World::default()).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(!backup_path(&path).exists());
}

#[test]
fn versioned_save_rejects_stale_session_without_touching_backup() {
    use rosenfu_storage::{file_version, save_world_if_version, FileVersion};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("versioned.json");
    let mut first = sample();
    let (_, baseline) = save_world_if_version(&path, &first, &FileVersion::Missing).unwrap();
    first.add_player("newer".into()).unwrap();
    let (_, newer) = save_world_if_version(&path, &first, &baseline).unwrap();
    let backup = fs::read(backup_path(&path)).unwrap();
    assert!(matches!(
        save_world_if_version(&path, &sample(), &baseline),
        Err(StorageError::FileChanged)
    ));
    assert_eq!(file_version(&path).unwrap(), newer);
    assert_eq!(fs::read(backup_path(&path)).unwrap(), backup);
    fs::remove_file(&path).unwrap();
    assert!(matches!(
        save_world_if_version(&path, &first, &newer),
        Err(StorageError::FileChanged)
    ));
    assert!(!path.exists());
}

#[test]
fn versions_fingerprint_corrupt_and_oversized_files_without_decoding() {
    use rosenfu_storage::{file_version, FileVersion};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.json");
    assert_eq!(file_version(&path).unwrap(), FileVersion::Missing);
    fs::write(&path, b"bad").unwrap();
    let first = file_version(&path).unwrap();
    assert!(matches!(first, FileVersion::Present(_)));
    fs::write(&path, vec![b'x'; MAX_SAVE_BYTES + 1]).unwrap();
    assert_ne!(file_version(&path).unwrap(), first);
}

#[test]
fn load_baseline_matches_exact_loaded_bytes() {
    use rosenfu_storage::{file_version, load_world_with_version};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("save.json");
    save_world(&path, &sample()).unwrap();
    let (world, version) = load_world_with_version(&path).unwrap();
    assert_eq!(world, sample());
    assert_eq!(version, file_version(&path).unwrap());
}

#[test]
fn two_threads_sharing_baseline_cannot_both_update() {
    use rosenfu_storage::{file_version, save_world_if_version};
    use std::sync::{Arc, Barrier};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("shared.json");
    save_world(&path, &sample()).unwrap();
    let version = file_version(&path).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|i| {
            let (path, version, barrier) = (path.clone(), version.clone(), barrier.clone());
            std::thread::spawn(move || {
                let mut world = sample();
                world.add_player(format!("writer {i}")).unwrap();
                barrier.wait();
                save_world_if_version(path, &world, &version).is_ok()
            })
        })
        .collect();
    assert_eq!(
        handles
            .into_iter()
            .filter_map(|h| h.join().ok())
            .filter(|ok| *ok)
            .count(),
        1
    );
}

#[test]
fn explicit_replacement_requires_baseline_and_preserves_backup() {
    use rosenfu_storage::{file_version, replace_world_if_version};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.json");
    fs::write(&path, b"broken").unwrap();
    fs::write(backup_path(&path), b"preserved recovery copy").unwrap();
    let baseline = file_version(&path).unwrap();
    assert!(save_world(&path, &sample()).is_err());
    replace_world_if_version(&path, &sample(), &baseline).unwrap();
    assert_eq!(load_world(&path).unwrap(), sample());
    assert_eq!(
        fs::read(backup_path(&path)).unwrap(),
        b"preserved recovery copy"
    );
    assert!(matches!(
        replace_world_if_version(&path, &sample(), &baseline),
        Err(StorageError::FileChanged)
    ));
}

#[test]
fn compact_world_that_exceeds_pretty_limit_loads_and_can_repair_exact_baseline() {
    use rosenfu_core::world::{Alignment, AlignmentId, GeoPoint, TrackType};
    use rosenfu_storage::{ensure_world_saveable, file_version, replace_world_if_version};
    let mut world = World::default();
    let player = world.add_player("p".into()).unwrap();
    let owner = world.add_company(player, "c".into()).unwrap();
    let points: Vec<_> = (0..4096)
        .map(|i| GeoPoint {
            latitude: 35.0 + i as f64 * 0.000001,
            longitude: 139.1234567890123,
        })
        .collect();
    let mut draft = world.snapshot().clone();
    for i in 0..40 {
        draft.alignments.push(Alignment {
            anchor_indices: None,
            infrastructure: Vec::new(),
            vertical_profile: None,
            section_profiles: Vec::new(),
            id: AlignmentId(3 + i),
            owner,
            name: "a".into(),
            points: points.clone(),
            track_type: TrackType::Normal,
            design_speed_kmh: None,
        });
    }
    draft.next_id = 43;
    let large = World::new(draft).unwrap();
    ensure_world_saveable(&large).unwrap();
    let compact = serde_json::to_vec(&serde_json::json!({
        "format": "rosenfu-world", "schema_version": rosenfu_storage::SCHEMA_VERSION, "world": large.snapshot(),
        "road_evidence": {"raw_blobs": [], "bindings": []}
    }))
    .unwrap();
    assert!(compact.len() < MAX_SAVE_BYTES);
    assert_eq!(decode(&compact).unwrap(), large);
    let bytes = encode(&large).unwrap();
    assert!(bytes.starts_with(b"{\"format\":"));
    assert!(bytes.len() < MAX_SAVE_BYTES);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.json");
    fs::write(&path, b"broken").unwrap();
    fs::write(backup_path(&path), b"retained recovery copy").unwrap();
    let baseline = file_version(&path).unwrap();
    let (outcome, updated) = replace_world_if_version(&path, &large, &baseline).unwrap();
    assert_eq!(outcome, SaveOutcome::Replaced);
    assert_eq!(file_version(&path).unwrap(), updated);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(load_world(&path).unwrap(), large);
    assert_eq!(
        fs::read(backup_path(&path)).unwrap(),
        b"retained recovery copy"
    );
    assert!(matches!(
        replace_world_if_version(&path, &large, &baseline),
        Err(StorageError::FileChanged)
    ));
}

#[test]
fn held_sidecar_lock_rejects_writer_without_modifying_save() {
    use rosenfu_storage::{file_version, save_world_if_version};
    use std::fs::OpenOptions;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("locked.json");
    save_world(&path, &sample()).unwrap();
    let baseline = file_version(&path).unwrap();
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(dir.path().join("locked.json.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert!(save_world_if_version(&path, &sample(), &baseline).is_err());
    assert_eq!(file_version(&path).unwrap(), baseline);
    assert!(!backup_path(&path).exists());
}
