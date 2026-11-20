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
    assert_eq!(value["schema_version"], 27);
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
        .replace("\"schema_version\": 27", "\"schema_version\": 24");
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
#[test]
fn parked_and_built_seam_paths_roundtrip_complete_envelope_and_fail_atomically() {
    use rosenfu_core::ofm_evidence::{reconstruct_path, PathPick};
    use rosenfu_core::world::TrackType;
    let prefix = vec![
        26, 56, 10, 14, 116, 114, 97, 110, 115, 112, 111, 114, 116, 97, 116, 105, 111, 110, 120, 2,
        40, 128, 32, 18, 17, 18, 2, 0, 0, 24, 2, 34, 9, 9,
    ];
    let suffix = vec![
        10, 144, 3, 0, 26, 5, 99, 108, 97, 115, 115, 34, 7, 10, 5, 109, 105, 110, 111, 114,
    ];
    let west = [prefix.clone(), vec![192, 62, 160, 31], suffix.clone()].concat();
    let east = [prefix, vec![191, 1, 162, 31], suffix].concat();
    let revision = "20261101_080013_pt";
    let bindings: Vec<_> = [&west, &east]
        .iter()
        .enumerate()
        .map(|(i, raw)| TileBinding {
            canonical_url: format!(
                "https://tiles.openfreemap.org/planet/{revision}/14/{}/6451.pbf",
                14552 + i
            ),
            revision: revision.into(),
            z: 14,
            x: 14552 + i as u32,
            y: 6451,
            raw_sha256: raw_sha256(raw),
            decoder_rule: DECODER_RULE.into(),
        })
        .collect();
    let raw: Vec<_> = [west, east]
        .into_iter()
        .map(|bytes| RawEvidence {
            sha256: raw_sha256(&bytes),
            bytes,
        })
        .collect();
    let store = EvidenceStore::from_parts(raw, bindings).unwrap();
    let start = PathPick {
        fragment_id: format!("ofm:{revision}:14:14552:6451:0:0"),
        segment_index: 0,
        fraction: 0.,
    };
    let end = PathPick {
        fragment_id: format!("ofm:{revision}:14:14553:6451:0:0"),
        segment_index: 0,
        fraction: 1.,
    };
    let keys = store
        .bindings()
        .map(|b| b.canonical_url.clone())
        .collect::<Vec<_>>();
    let path = reconstruct_path(&store, &keys, &start, &end)
        .unwrap()
        .descriptor;
    let mut world = World::default();
    let player = world.add_player("p".into()).unwrap();
    let owner = world.add_company(player, "c".into()).unwrap();
    let plans=serde_json::from_value(serde_json::json!({"version":1,"plans":[{"id":1,"name":"seam"}],"parkedPaths":[{"id":2,"planId":1,"name":"seam","points":[],"redo":[],"line":[],"extension":null,"trackType":"normal","designSpeedKmh":120,"roadFollowing":true,"ofmPath":path}],"activePathId":1,"activePlanId":1,"draftPoints":[],"redoPoints":[],"draftExtension":null,"trackType":"normal","designSpeedKmh":120,"roadFollowing":true,"plannedStations":[],"draftFinished":false})).unwrap();
    world
        .stage_ofm_construction_plans(
            plans,
            store
                .blobs()
                .map(|(sha, raw)| RawEvidence {
                    sha256: sha.into(),
                    bytes: raw.into(),
                })
                .collect(),
            store.bindings().cloned().collect(),
        )
        .unwrap();
    let parked = world.clone();
    assert_eq!(decode(&encode(&world).unwrap()).unwrap(), world);
    world
        .build_alignment_from_road_path(owner, "built".into(), TrackType::Normal, path)
        .unwrap();
    let prior_rails = world.snapshot().alignments.clone();
    let prior_proofs = world.snapshot().road_path_provenance.clone();
    let parked_plans = world.snapshot().construction_plans.clone().unwrap();
    let template = store.bindings().next().unwrap().clone();
    let distant: Vec<_> = (5000..5016)
        .map(|x| TileBinding {
            canonical_url: format!(
                "https://tiles.openfreemap.org/planet/{revision}/14/{x}/6000.pbf"
            ),
            x,
            y: 6000,
            ..template.clone()
        })
        .collect();
    world
        .stage_ofm_construction_plans(parked_plans, vec![], distant)
        .unwrap();
    let mut reopened = decode(&encode(&world).unwrap()).unwrap();
    assert_eq!(reopened, world);
    assert_eq!(
        reopened.snapshot().runtime_road_evidence.bindings().count(),
        18
    );
    // Force a full world validation on an unrelated edit after an offline reopen.
    reopened.add_player("offline edit".into()).unwrap();
    assert_eq!(reopened.snapshot().alignments, prior_rails);
    assert_eq!(reopened.snapshot().road_path_provenance, prior_proofs);
    let mut value: serde_json::Value = serde_json::from_slice(&encode(&world).unwrap()).unwrap();
    value["world"]["road_path_provenance"][0]["to_position"] = serde_json::json!(0.4);
    assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
    assert_eq!(decode(&encode(&parked).unwrap()).unwrap(), parked);
}

#[test]
fn separate_regions_keep_pinned_bytes_and_restore_only_bounded_scopes() {
    use rosenfu_core::ofm_evidence::{Error, MAX_PINNED_BINDINGS, MAX_TILES};

    let seed = evidence_world();
    let data = seed.snapshot();
    let store = &data.runtime_road_evidence;
    let root = store.bindings().next().unwrap().clone();
    let raw = store.raw(&root.raw_sha256).unwrap().to_vec();
    let bound_at = |x, y| TileBinding {
        canonical_url: format!("https://tiles.openfreemap.org/planet/test/14/{x}/{y}.pbf"),
        x,
        y,
        ..root.clone()
    };
    // Two geographically separated groups. Same synthetic raw tile across
    // coordinates intentionally exercises byte reuse without merging provenance.
    let bounds: Vec<_> = [(8000, 6000), (14000, 6400)]
        .into_iter()
        .flat_map(|(x, y)| (0..MAX_TILES as u32).map(move |dx| (x + dx, y)))
        .map(|(x, y)| bound_at(x, y))
        .collect();
    let keys: Vec<_> = bounds.iter().map(|b| b.canonical_url.clone()).collect();
    let mut world = World::default();
    let plans: rosenfu_core::world::ConstructionPlans = serde_json::from_value(serde_json::json!({
        "version": 1, "plans": [{"id": 1, "name": "fixture"}], "parkedPaths": [],
        "activePathId": 1, "activePlanId": 1, "draftPoints": [], "redoPoints": [],
        "draftExtension": null, "trackType": "normal", "designSpeedKmh": 120,
        "roadFollowing": false, "plannedStations": [], "draftFinished": false
    }))
    .unwrap();
    for (i, region) in [&bounds[..MAX_TILES], &bounds[MAX_TILES..]]
        .into_iter()
        .enumerate()
    {
        let raw_input = if i == 0 {
            vec![RawEvidence {
                sha256: root.raw_sha256.clone(),
                bytes: raw.clone(),
            }]
        } else {
            vec![]
        };
        world
            .stage_ofm_construction_plans(plans.clone(), raw_input, region.to_vec())
            .unwrap();
    }
    let before = encode(&world).unwrap();
    let mut data: serde_json::Value = serde_json::from_slice(&before).unwrap();
    assert_eq!(
        data["road_evidence"]["raw_blobs"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        data["road_evidence"]["bindings"].as_array().unwrap().len(),
        32
    );
    let mut false_legacy = data.clone();
    false_legacy["schema_version"] = serde_json::json!(26);
    false_legacy
        .as_object_mut()
        .unwrap()
        .remove("fixed_od_scenario");
    assert!(decode(&serde_json::to_vec(&false_legacy).unwrap()).is_err());
    let restored = decode(&before).unwrap();
    assert_eq!(encode(&restored).unwrap(), before);
    let store = &restored.snapshot().runtime_road_evidence;
    assert_eq!(store.raw(&root.raw_sha256).unwrap(), raw);
    assert!(matches!(store.scope(&keys), Err(Error::Limit)));
    let first = store.scope(&keys[..MAX_TILES]).unwrap();
    let second = store.scope(&keys[MAX_TILES..]).unwrap();
    let a = first.decoded(&keys[0]).unwrap();
    let b = second.decoded(&keys[MAX_TILES]).unwrap();
    let repeated = store.scope(&keys[MAX_TILES..]).unwrap();
    assert!(std::ptr::eq(b, repeated.decoded(&keys[MAX_TILES]).unwrap()));
    let rebuild_after_eviction = store.scope(&keys[..MAX_TILES]).unwrap();
    assert_eq!(a, rebuild_after_eviction.decoded(&keys[0]).unwrap());
    assert!(!std::ptr::eq(
        a,
        rebuild_after_eviction.decoded(&keys[0]).unwrap()
    ));
    assert_ne!(a.canonical_sha256, b.canonical_sha256);
    assert!(first.decoded(&keys[MAX_TILES]).is_err());
    assert!(second.decoded(&keys[0]).is_err());
    let identical = world.clone();
    assert!(world
        .stage_ofm_construction_plans(
            plans,
            vec![],
            (0..MAX_TILES as u32 + 1).map(|x| bound_at(x, 42)).collect()
        )
        .is_err());
    assert_eq!(world, identical);
    // Persistence must not accumulate unbounded restore work from an input DTO.
    let b = data["road_evidence"]["bindings"][0].clone();
    data["road_evidence"]["bindings"] = serde_json::json!(vec![b; MAX_PINNED_BINDINGS + 1]);
    assert!(decode(&serde_json::to_vec(&data).unwrap()).is_err());
}
