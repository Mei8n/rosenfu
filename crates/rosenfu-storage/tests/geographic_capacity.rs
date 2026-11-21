//! Repeatable, offline #197 acceptance. All MVT features are invented. The
//! canonical URL format is used only as a test identifier, never for fetching.
use rosenfu_core::ofm_evidence::{
    raw_sha256, reconstruct_path, validate_path_descriptor, Error, EvidenceStore, PathDescriptor,
    PathPick, RawEvidence, TileBinding, DECODER_RULE, MAX_PINNED_BINDINGS, MAX_TILES,
};
use rosenfu_core::world::{ConstructionPlans, TrackType, World, WorldDraft, WorldError};
use rosenfu_storage::{
    backup_path, encode, file_version, load_world_with_version, save_world, save_world_if_version,
    StorageError, MAX_SAVE_BYTES,
};
use std::{collections::BTreeMap, fs, time::Instant};

const REV: &str = "test_geo_capacity_a";
const NEXT: &str = "test_geo_capacity_b";

// Building a synthetic MVT locally keeps hundreds of retained tiles in CI without
// checking in a large fixture or requiring a provider, licensing or network.
fn var(mut n: u32, out: &mut Vec<u8>) {
    while n >= 128 {
        out.push((n as u8 & 0x7f) | 0x80);
        n >>= 7;
    }
    out.push(n as u8);
}

fn bytes(field: u32, payload: &[u8], out: &mut Vec<u8>) {
    var((field << 3) | 2, out);
    var(payload.len() as u32, out);
    out.extend_from_slice(payload);
}

fn number(field: u32, value: u32, out: &mut Vec<u8>) {
    var(field << 3, out);
    var(value, out);
}

fn synthetic_mvt(features: u32, seed: u32) -> Vec<u8> {
    let mut layer = vec![];
    bytes(1, b"transportation", &mut layer);
    number(15, 2, &mut layer);
    number(5, 4096, &mut layer);
    for i in 0..features {
        let mut geometry = vec![];
        // Only positive coordinates and deltas are needed by this fixture.
        for n in [9, 2 * (32 + seed), 2 * (32 + i * 3), 10, 4000, 0] {
            var(n, &mut geometry);
        }
        let mut feature = vec![];
        bytes(2, &[0, 0], &mut feature);
        number(3, 2, &mut feature);
        bytes(4, &geometry, &mut feature);
        bytes(2, &feature, &mut layer);
    }
    bytes(3, b"class", &mut layer);
    let mut value = vec![];
    bytes(1, b"minor", &mut value);
    bytes(4, &value, &mut layer);
    let mut tile = vec![];
    bytes(3, &layer, &mut tile);
    tile
}

fn bound(revision: &str, x: u32, y: u32, sha: &str) -> TileBinding {
    TileBinding {
        canonical_url: format!("https://tiles.openfreemap.org/planet/{revision}/14/{x}/{y}.pbf"),
        revision: revision.into(),
        z: 14,
        x,
        y,
        raw_sha256: sha.into(),
        decoder_rule: DECODER_RULE.into(),
    }
}

fn keys(bindings: &[TileBinding]) -> Vec<String> {
    bindings.iter().map(|b| b.canonical_url.clone()).collect()
}

fn paired_path(archive: &EvidenceStore, bindings: &[TileBinding]) -> PathDescriptor {
    let wanted = keys(bindings);
    let scope = archive.scope(&wanted).unwrap();
    let pick = |binding: &TileBinding, fraction| PathPick {
        fragment_id: format!("ofm:{REV}:14:{}:{}:0:0", binding.x, binding.y),
        segment_index: 0,
        fraction,
    };
    let path = reconstruct_path(
        &scope,
        &wanted,
        &pick(&bindings[0], 0.),
        &pick(&bindings[1], 1.),
    )
    .unwrap();
    assert_eq!(path.descriptor.joins.len(), 1);
    path.descriptor
}

#[test]
fn bounded_geographic_archive_reopens_offline_without_losing_routes_or_revisions() {
    const REGIONS: [(&str, u32, u32, u32); 4] = [
        ("Sapporo", 14624, 6016, 16),
        ("Tokyo", 14552, 6451, 1024),
        ("Osaka", 14358, 6506, 256),
        ("Fukuoka", 14126, 6567, 64),
    ];
    // Use established two distinct synthetic seam fragments only for ALPHA QA.
    let seam: serde_json::Value = serde_json::from_str(include_str!(
        "../../rosenfu-core/tests/fixtures/ofm_session_synthetic.json"
    ))
    .unwrap();
    let seam_bindings: Vec<TileBinding> =
        serde_json::from_value(seam["tileBindings"].clone()).unwrap();
    let seam_raw = seam["rawEvidence"].as_array().unwrap();
    let mut raw = BTreeMap::<String, Vec<u8>>::new();
    let mut expected = BTreeMap::new();
    let mut bindings = vec![];
    let mut push = |revision: &str, x, y, bytes: Vec<u8>, count| {
        let hash = raw_sha256(&bytes);
        let binding = bound(revision, x, y, &hash);
        raw.entry(hash).or_insert(bytes);
        assert!(expected
            .insert(binding.canonical_url.clone(), count)
            .is_none());
        bindings.push(binding);
    };
    let seam_bytes = |offset: usize| {
        let sha = &seam_bindings[offset].raw_sha256;
        let blob = seam_raw.iter().find(|v| v["sha256"] == *sha).unwrap();
        let bytes: Vec<u8> = serde_json::from_value(blob["bytes"].clone()).unwrap();
        assert_eq!(raw_sha256(&bytes), *sha);
        bytes
    };
    for (region, x, y, density) in REGIONS {
        for i in 0..16 {
            let (bytes, density) = match i {
                0 | 1 => (seam_bytes(i as usize), 1),
                // Unbuilt second seam to prove editing after the fresh reopen.
                4 | 5 if region == "Tokyo" => (seam_bytes(i as usize - 4), 1),
                _ => (synthetic_mvt(density, i), density),
            };
            push(REV, x + i % 4, y + i / 4, bytes, density);
        }
    }
    // A 192-tile geographic strip is load/retention stress, NOT a continuous road.
    for i in 0..192 {
        push(REV, 14360 + i, 6490, synthetic_mvt(32, i), 32);
    }
    assert_eq!(bindings.len(), MAX_PINNED_BINDINGS);
    let proofs = || {
        raw.iter()
            .map(|(hash, bytes)| RawEvidence {
                sha256: hash.clone(),
                bytes: bytes.clone(),
            })
            .collect()
    };
    let start = Instant::now();
    let store = EvidenceStore::from_archive_parts(proofs(), bindings.clone()).unwrap();
    let restore_archive = start.elapsed();
    let descriptors = store.descriptors();
    let paths: Vec<PathDescriptor> = bindings[..REGIONS.len() * 16]
        .as_chunks::<16>()
        .0
        .iter()
        .map(|region| paired_path(&store, &region[..2]))
        .collect();
    let mut world = World::new(WorldDraft::default().with_road_evidence(store).unwrap()).unwrap();
    let player = world
        .add_player("Synthetic #197 acceptance".into())
        .unwrap();
    let owner = world
        .add_company(player, "Example operator".into())
        .unwrap();
    let mut plans: ConstructionPlans = serde_json::from_value(seam["plans"].clone()).unwrap();
    plans.parked_paths[0].ofm_path = Some(paths[0].clone());
    world
        .stage_ofm_construction_plans(plans.clone(), vec![], vec![])
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("bounded-geography.json");
    save_world(&file, &world).unwrap();
    let staged = fs::read(&file).unwrap();
    for ((region, ..), path) in REGIONS.into_iter().zip(&paths) {
        world
            .build_alignment_from_road_path(
                owner,
                format!("Synthetic {region}"),
                TrackType::Normal,
                path.clone(),
            )
            .unwrap();
    }
    assert_eq!(world.snapshot().road_path_provenance.len(), REGIONS.len());
    assert_eq!(world.snapshot().road_evidence_descriptors, descriptors);
    save_world(&file, &world).unwrap();
    let saved = fs::read(&file).unwrap();
    let backup = backup_path(&file);
    assert_eq!(fs::read(&backup).unwrap(), staged);
    let reopen = Instant::now();
    let (mut loaded, version) = load_world_with_version(&file).unwrap();
    let reopen = reopen.elapsed();
    assert_eq!(loaded, world);
    assert_eq!(encode(&loaded).unwrap(), saved);

    let archive = &loaded.snapshot().runtime_road_evidence;
    let read_scopes = Instant::now();
    for _ in 0..2 {
        for chunk in bindings.chunks(MAX_TILES) {
            let scope = archive.scope(&keys(chunk)).unwrap();
            for b in chunk {
                let tile = scope.decoded(&b.canonical_url).unwrap();
                assert_eq!(tile.lines.len(), expected[&b.canonical_url] as usize);
                assert_eq!(scope.raw(&b.raw_sha256).unwrap(), raw[&b.raw_sha256]);
            }
        }
    }
    let read_scopes = read_scopes.elapsed();
    for desc in &paths {
        let scope = archive.scope(&desc.required_binding_keys).unwrap();
        validate_path_descriptor(&scope, desc).unwrap();
    }
    // Construct a previously unbuilt second Tokyo seam from saved evidence.
    // Keep this separate so failures can compare the exact original save.
    let offline_path = paired_path(archive, &bindings[20..22]);
    let mut resumed = loaded.clone();
    resumed
        .build_alignment_from_road_path(
            owner,
            "Offline test route".into(),
            TrackType::Normal,
            offline_path,
        )
        .unwrap();
    assert_eq!(
        resumed.snapshot().road_path_provenance.len(),
        paths.len() + 1
    );
    assert_eq!(resumed.snapshot().road_evidence_descriptors, descriptors);

    // A valid new-revision key at the same coordinates does not authorize
    // using it where committed geometry requires the old pinned revision.
    let tokyo = &bindings[16];
    let new_version = bound(NEXT, tokyo.x, tokyo.y, &tokyo.raw_sha256);
    let replacement_only = EvidenceStore::from_parts(
        vec![RawEvidence {
            sha256: new_version.raw_sha256.clone(),
            bytes: raw[&new_version.raw_sha256].clone(),
        }],
        vec![new_version.clone()],
    )
    .unwrap();
    assert_eq!(
        replacement_only
            .decoded(&new_version.canonical_url)
            .unwrap()
            .lines
            .len(),
        1
    );
    assert!(matches!(
        archive.scope(&keys(std::slice::from_ref(&new_version))),
        Err(Error::Binding)
    ));
    let mut mixed = bindings.clone();
    mixed[16] = new_version;
    assert_eq!(mixed.len(), MAX_PINNED_BINDINGS);
    assert!(matches!(
        EvidenceStore::from_archive_parts(proofs(), mixed),
        Err(Error::Binding)
    ));
    assert!(matches!(
        archive.scope(&keys(&bindings[..MAX_TILES + 1])),
        Err(Error::Limit)
    ));
    let overflow = bound(REV, 1000, 6000, &bindings[2].raw_sha256);
    let mut too_many = bindings;
    too_many.push(overflow.clone());
    assert!(matches!(
        EvidenceStore::from_archive_parts(proofs(), too_many),
        Err(Error::Limit)
    ));
    assert!(loaded
        .stage_ofm_construction_plans(plans, vec![], vec![overflow])
        .is_err());
    assert_eq!(loaded, world);
    assert_eq!(fs::read(&file).unwrap(), saved);
    assert_eq!(fs::read(&backup).unwrap(), staged);

    // Distinguish conflicting geography from ordinary client/file CAS conflicts.
    // Even with the CURRENT on-disk version, mixed geography cannot be accepted
    // or rotate the verified older backup.
    let mut wrong: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    let old: TileBinding =
        serde_json::from_value(wrong["road_evidence"]["bindings"][0].clone()).unwrap();
    wrong["road_evidence"]["bindings"][0] =
        serde_json::to_value(bound(NEXT, old.x, old.y, &old.raw_sha256)).unwrap();
    let wrong = serde_json::to_vec(&wrong).unwrap();
    assert!(wrong.len() < MAX_SAVE_BYTES);
    fs::write(&file, &wrong).unwrap();
    let overwritten = file_version(&file).unwrap();
    assert_ne!(version, overwritten);
    assert!(matches!(
        load_world_with_version(&file),
        Err(StorageError::InvalidWorld(WorldError::InvalidRoadEvidence))
    ));
    assert!(matches!(
        save_world_if_version(&file, &world, &overwritten),
        Err(StorageError::InvalidWorld(WorldError::InvalidRoadEvidence))
    ));
    assert_eq!(fs::read(&file).unwrap(), wrong);
    assert_eq!(fs::read(&backup).unwrap(), staged);

    // Measurements are an observation, never machine-dependent pass criteria.
    eprintln!(
        "synthetic #197: bindings={} unique_raw_n={} unique_raw_bytes={} saved_bytes={} four_local_paths={} archive_restore={restore_archive:?} file_reopen={reopen:?} two_scope_passes={read_scopes:?}",
        MAX_PINNED_BINDINGS, raw.len(), raw.values().map(Vec::len).sum::<usize>(), saved.len(), paths.len(),
    );
}
