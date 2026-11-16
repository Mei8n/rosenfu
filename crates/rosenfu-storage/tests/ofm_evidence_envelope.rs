use rosenfu_core::{
    ofm_evidence::{raw_sha256, EvidenceStore, RawEvidence, TileBinding, DECODER_RULE},
    world::{World, WorldDraft},
};
use rosenfu_storage::*;
fn evidence_world() -> World {
    let mut layer = vec![10, 14];
    layer.extend(b"transportation");
    layer.extend([120, 2, 40, 128, 32]);
    let mut raw = vec![26, layer.len() as u8];
    raw.extend(layer);
    let hash = raw_sha256(&raw);
    let b = TileBinding {
        canonical_url: "https://tiles.openfreemap.org/planet/test/14/1/2.pbf".into(),
        revision: "test".into(),
        z: 14,
        x: 1,
        y: 2,
        raw_sha256: hash.clone(),
        decoder_rule: DECODER_RULE.into(),
    };
    let store = EvidenceStore::from_parts(
        vec![RawEvidence {
            sha256: hash,
            bytes: raw,
        }],
        vec![b],
    )
    .unwrap();
    World::new(WorldDraft::default().with_road_evidence(store).unwrap()).unwrap()
}
#[test]
fn full_envelope_roundtrips_content_without_runtime_data_in_world_json() {
    let world = evidence_world();
    let bytes = encode(&world).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["schema_version"], 25);
    assert!(value["world"].get("runtime_road_evidence").is_none());
    assert_eq!(
        value["road_evidence"]["raw_blobs"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let restored = decode(&bytes).unwrap();
    assert_eq!(world, restored);
    assert_eq!(encode(&restored).unwrap(), bytes);
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("world.json");
    save_world(&path, &world).unwrap();
    assert_eq!(load_world(&path).unwrap(), world);
    let (versioned, _) = load_world_with_version(&path).unwrap();
    assert_eq!(versioned, world);
}
#[test]
fn missing_modified_and_noncanonical_evidence_reject() {
    let value: serde_json::Value =
        serde_json::from_slice(&encode(&evidence_world()).unwrap()).unwrap();
    for i in 0..7 {
        let mut v = value.clone();
        match i {
            0 => {
                v.as_object_mut().unwrap().remove("road_evidence");
            }
            1 => v["road_evidence"]["raw_blobs"] = serde_json::json!([]),
            2 => {
                v["road_evidence"]["bindings"][0]["raw_sha256"] = serde_json::json!("0".repeat(64))
            }
            3 => {
                v["world"]["road_evidence_descriptors"][0]["canonical_sha256"] =
                    serde_json::json!("0".repeat(64))
            }
            4 => {
                let s = v["road_evidence"]["raw_blobs"][0]["base64"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                v["road_evidence"]["raw_blobs"][0]["base64"] = serde_json::json!(format!("{s}\n"));
            }
            5 => {
                v["road_evidence"]["raw_blobs"][0]["byte_length"] =
                    serde_json::json!(5 * 1024 * 1024 + 1)
            }
            _ => v["road_evidence"]["bindings"][0]["decoder_rule"] = serde_json::json!("future"),
        };
        assert!(
            decode(&serde_json::to_vec(&v).unwrap()).is_err(),
            "mutation {i}"
        );
    }
}
#[test]
fn old24_is_rejected_without_touching_file_or_backup_and_oversize_save_is_atomic() {
    let world = evidence_world();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("world.json");
    save_world(&path, &world).unwrap();
    save_world(&path, &world).unwrap();
    let before = std::fs::read(&path).unwrap();
    let backup = std::fs::read(backup_path(&path)).unwrap();
    let mut large = world.clone();
    large.add_player("x".repeat(MAX_SAVE_BYTES)).unwrap();
    assert!(matches!(
        save_world(&path, &large),
        Err(StorageError::TooLarge)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read(backup_path(&path)).unwrap(), backup);
    let old = String::from_utf8(before)
        .unwrap()
        .replace("\"schema_version\": 25", "\"schema_version\": 24");
    std::fs::write(&path, &old).unwrap();
    assert!(matches!(
        load_world(&path),
        Err(StorageError::UnsupportedVersion(24))
    ));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), old);
    assert_eq!(std::fs::read(backup_path(&path)).unwrap(), backup);
}
#[test]
fn slots_export_import_and_versioned_replacement_use_the_complete_envelope() {
    let root = tempfile::tempdir().unwrap();
    let world = evidence_world();
    let (slot, version) = create_save_slot(root.path(), "evidence", &world).unwrap();
    let path = save_slot_path(root.path(), &slot.id).unwrap();
    let loaded = load_world(&path).unwrap();
    assert_eq!(loaded, world);
    assert!(list_save_slots(root.path()).unwrap()[0].loadable);
    let exported = encode(&loaded).unwrap();
    let imported = decode(&exported).unwrap();
    let destination = root.path().join("import.json");
    create_world(&destination, &imported).unwrap();
    assert_eq!(load_world(&destination).unwrap(), world);
    assert!(save_world_if_version(&path, &imported, &version).is_ok());
    assert_eq!(load_world(&path).unwrap(), world);
}
