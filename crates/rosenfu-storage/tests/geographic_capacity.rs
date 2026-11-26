//! Repeatable, offline #197 acceptance. All MVT features are invented. The
//! canonical URL format is used only as a test identifier, never for fetching.
use rosenfu_core::ofm_evidence::{
    raw_sha256, reconstruct_path, validate_path_descriptor, Error, EvidenceStore, PathDescriptor,
    PathPick, RawEvidence, TileBinding, DECODER_RULE, MAX_PINNED_BINDINGS, MAX_TILES,
};
use rosenfu_core::world::{ConstructionPlans, TrackType, World, WorldDraft, WorldError};
use rosenfu_storage::{
    backup_path, decode, encode, file_version, load_world, load_world_with_version, save_world,
    save_world_if_version, StorageError, MAX_SAVE_BYTES,
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

#[test]
fn archive_weighted_limit_and_scope_are_independent_of_unique_saved_bytes() {
    const MIB: usize = 1024 * 1024;
    // Valid MVT with 1,024 same-size dense road features. Pad an ignored
    // protobuf field so the exact byte boundary is independent of the fixture.
    let dense = |seed| {
        let mut raw = synthetic_mvt(1024, seed);
        bytes(9, &vec![0; MIB - raw.len() - 4], &mut raw);
        assert_eq!(raw.len(), MIB);
        RawEvidence {
            sha256: raw_sha256(&raw),
            bytes: raw,
        }
    };
    let raw = || vec![dense(1), dense(2)];
    let digests = raw().into_iter().map(|r| r.sha256).collect::<Vec<_>>();
    assert_ne!(digests[0], digests[1]);
    // Four separated synthetic strips. Each distinct coordinate has its own
    // canonical binding although 32 bindings share each raw blob.
    let bindings: Vec<_> = [(14624, 6016), (14552, 6451), (14358, 6506), (14126, 6567)]
        .into_iter()
        .enumerate()
        .flat_map(|(region, (x, y))| {
            let hash = &digests[region % 2];
            (0..16).map(move |i| bound(REV, x + i, y, hash))
        })
        .collect();
    assert_eq!(bindings.len(), 64);
    let start = Instant::now();
    let store = EvidenceStore::from_archive_parts(raw(), bindings.clone()).unwrap();
    let admit = start.elapsed();
    assert_eq!(store.blobs().map(|(_, b)| b.len()).sum::<usize>(), 2 * MIB);
    assert_eq!(store.bindings().count(), 64);
    let weighted = store
        .bindings()
        .map(|b| store.raw(&b.raw_sha256).unwrap().len())
        .sum::<usize>();
    assert_eq!(weighted, 64 * MIB);
    let mut world = World::new(WorldDraft::default().with_road_evidence(store).unwrap()).unwrap();
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("weighted-geography.json");
    save_world(&file, &world).unwrap();
    let staged = fs::read(&file).unwrap();
    world
        .add_player("Independent save generation".into())
        .unwrap();
    save_world(&file, &world).unwrap();
    let saved = fs::read(&file).unwrap();
    assert_ne!(saved, staged);
    let wire: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    assert_eq!(
        wire["road_evidence"]["raw_blobs"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        wire["road_evidence"]["bindings"].as_array().unwrap().len(),
        64
    );
    assert!(saved.len() < MAX_SAVE_BYTES);
    let start = Instant::now();
    let (mut opened, version) = load_world_with_version(&file).unwrap();
    let reopen = start.elapsed();
    assert_eq!(opened, world);
    let archive = &opened.snapshot().runtime_road_evidence;
    for region in bindings.chunks(16) {
        let scope = archive.scope(&keys(&region[..8])).unwrap();
        assert_eq!(
            scope.decoded(&region[7].canonical_url).unwrap().lines.len(),
            1_024
        );
        assert!(matches!(
            archive.scope(&keys(&region[..9])),
            Err(Error::Limit)
        ));
    }
    let extra = bound(REV, 14150, 6567, &digests[0]);
    let mut too_many_weighted = bindings;
    too_many_weighted.push(extra.clone());
    assert!(too_many_weighted.len() < MAX_PINNED_BINDINGS);
    assert!(matches!(
        EvidenceStore::from_archive_parts(raw(), too_many_weighted),
        Err(Error::Limit)
    ));
    let seam: serde_json::Value = serde_json::from_str(include_str!(
        "../../rosenfu-core/tests/fixtures/ofm_session_synthetic.json"
    ))
    .unwrap();
    let mut plans: ConstructionPlans = serde_json::from_value(seam["plans"].clone()).unwrap();
    plans.parked_paths.clear();
    let mut valid = opened.clone();
    valid
        .stage_ofm_construction_plans(plans.clone(), vec![], vec![])
        .unwrap();
    assert_eq!(
        opened.stage_ofm_construction_plans(plans, vec![], vec![extra]),
        Err(WorldError::InvalidRoadEvidence)
    );
    assert_eq!(opened, world);
    assert_eq!(file_version(&file).unwrap(), version);
    assert_eq!(fs::read(&file).unwrap(), saved);
    let backup = backup_path(&file);
    assert_eq!(fs::read(&backup).unwrap(), staged);
    assert_ne!(load_world_with_version(&backup).unwrap().0, world);
    eprintln!("synthetic binding weight: 64 keys, {} unique raw bytes, {weighted} bytes weighted, {} save bytes, restore archive {admit:?}, file reopen {reopen:?}", 2 * MIB, saved.len());
}

// A real core service on explicitly invented rail geometry. The long train
// path is independent of the distant synthetic provider-format road tiles.
fn add_synthetic_long_service(world: &mut World) {
    use rosenfu_core::operation::{Timetable, Vehicle};
    use rosenfu_core::world::{GeoPoint, OperationServiceConfig, RouteSegment};

    let player = world.add_player("Long service fixture".into()).unwrap();
    let owner = world
        .add_company(player, "Synthetic operator".into())
        .unwrap();
    let points = (0..=64)
        .map(|i| GeoPoint {
            latitude: 35.,
            longitude: 138.9 + f64::from(i) / 64.,
        })
        .collect();
    let alignment_id = world
        .build_alignment(owner, "Synthetic long route".into(), points)
        .unwrap();
    assert!(world
        .snapshot()
        .alignments
        .iter()
        .any(|line| line.id == alignment_id && line.length_m() > 90_000.));
    let from = world
        .place_station_on_alignment(owner, "West".into(), alignment_id, 0.05, 100., 10.)
        .unwrap();
    let to = world
        .place_station_on_alignment(owner, "East".into(), alignment_id, 0.95, 100., 10.)
        .unwrap();
    let segments = vec![RouteSegment::Alignment {
        alignment_id,
        from,
        to,
    }];
    let stops = world
        .preview_service_path(from, segments.clone())
        .unwrap()
        .occurrences
        .into_iter()
        .map(|stop| stop.reference)
        .collect();
    let service_id = world
        .add_service_path(
            owner,
            "Synthetic long service".into(),
            from,
            segments,
            stops,
        )
        .unwrap();
    world
        .configure_operation(vec![OperationServiceConfig {
            service_id,
            timetable: Timetable {
                first_departure_ms: 0,
                last_departure_ms: 0,
                headway_ms: 60_000,
            },
            vehicle: Vehicle {
                max_speed_kmh: 100.,
                acceleration_mps2: 1.,
                braking_mps2: 1.,
                length_m: 80.,
            },
            dwell_ms: 30_000,
        }])
        .unwrap();
    let runtime = &mut world.operation_mut().unwrap().runtime;
    runtime.advance_to(1_000, 100).unwrap();
    assert_eq!(runtime.trains().count(), 1);
}

#[test]
fn geographic_file_reads_preserve_original_versions_and_require_canonical_evidence() {
    // Distinct geographic scopes with a shared immutable blob. Adjust the
    // ignored protobuf fields so that the source needs exactly two '=' pads.
    let mut raw = synthetic_mvt(128, 0);
    while raw.len() % 3 != 1 {
        raw.extend([8, 0]);
    }
    let sha = raw_sha256(&raw);
    let bindings = vec![bound(REV, 14624, 6016, &sha), bound(REV, 14552, 6451, &sha)];
    let store = EvidenceStore::from_archive_parts(
        vec![RawEvidence {
            bytes: raw,
            sha256: sha.clone(),
        }],
        bindings.clone(),
    )
    .unwrap();
    let mut world = World::new(WorldDraft::default().with_road_evidence(store).unwrap()).unwrap();
    add_synthetic_long_service(&mut world);
    let original_operation = world.operation_save();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dense-two-regions.json");
    save_world(&path, &world).unwrap();
    let canonical = fs::read_to_string(&path).unwrap();
    let original = file_version(&path).unwrap();
    let compat = canonical.replacen(r#""format":"#, r#""format": null, "format":"#, 1);
    for (i, mut wire) in [canonical.clone(), compat].into_iter().enumerate() {
        // Distinguish exact file version from the same restored/canonical world.
        wire.push_str(if i == 0 { "\r\n " } else { "   " });
        fs::write(&path, wire.as_bytes()).unwrap();
        let version = file_version(&path).unwrap();
        let (loaded, observed) = load_world_with_version(&path).unwrap();
        assert_eq!(observed, version);
        assert_ne!(observed, original);
        assert_eq!(loaded, world);
        assert_eq!(loaded.operation_save(), original_operation);
        assert_eq!(load_world(&path).unwrap(), world);
        assert_eq!(decode(wire.as_bytes()).unwrap(), world);
        assert_eq!(encode(&loaded).unwrap(), canonical.as_bytes());
        for b in &bindings {
            let scope = loaded
                .snapshot()
                .runtime_road_evidence
                .scope(&keys(std::slice::from_ref(b)))
                .unwrap();
            assert_eq!(scope.decoded(&b.canonical_url).unwrap().lines.len(), 128);
        }
    }

    let mut wire: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    let base64 = &wire["road_evidence"]["raw_blobs"][0]["base64"];
    let mut spare_bits = base64.as_str().unwrap().as_bytes().to_vec();
    let end = spare_bits.len();
    let alpha = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let sextet = alpha
        .iter()
        .position(|&c| c == spare_bits[end - 3])
        .unwrap();
    assert_eq!(sextet % 16, 0);
    spare_bits[end - 3] = alpha[sextet + 1];
    let mut wrong_padding = base64.as_str().unwrap().as_bytes().to_vec();
    wrong_padding[end - 2] = b'A';
    for noncanonical in [spare_bits, wrong_padding] {
        wire["road_evidence"]["raw_blobs"][0]["base64"] =
            String::from_utf8(noncanonical).unwrap().into();
        let bytes = serde_json::to_vec(&wire).unwrap();
        fs::write(&path, &bytes).unwrap();
        for result in [
            decode(&bytes),
            load_world(&path),
            load_world_with_version(&path).map(|(world, _)| world),
        ] {
            assert!(matches!(
                result,
                Err(StorageError::InvalidWorld(WorldError::InvalidRoadEvidence))
            ));
        }
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

/// Generate only. Measure a separate process that opens these exact bytes;
/// measuring this generator would also count build/serialize memory.
#[test]
#[ignore = "explicit benchmark fixture output path required"]
fn prepare_dense_and_long_geographic_reopen_fixture() {
    const MIB: usize = 1024 * 1024;
    let path = std::env::var_os("ROSENFU_GEOGRAPHIC_REOPEN_FIXTURE")
        .expect("set an explicit nonexistent output path in a scratch directory");
    let mut raw = vec![];
    let mut bindings = vec![];
    // Four separated geographic regions, two independent dense blobs each.
    for (region, (x, y)) in [(14624, 6016), (14552, 6451), (14358, 6506), (14126, 6567)]
        .into_iter()
        .enumerate()
    {
        for i in 0..2 {
            let mut tile = synthetic_mvt(1_024, (region as u32) * 2 + i);
            bytes(9, &vec![0; MIB - tile.len() - 4], &mut tile);
            assert_eq!(tile.len(), MIB);
            let sha256 = raw_sha256(&tile);
            bindings.push(bound(REV, x + i, y, &sha256));
            raw.push(RawEvidence {
                bytes: tile,
                sha256,
            });
        }
    }
    let store = EvidenceStore::from_archive_parts(raw, bindings).unwrap();
    let mut world = World::new(WorldDraft::default().with_road_evidence(store).unwrap()).unwrap();
    add_synthetic_long_service(&mut world);
    rosenfu_storage::create_world(&path, &world).unwrap();
    let file_bytes = fs::metadata(&path).unwrap().len();
    assert!(((10 * MIB) as u64..=MAX_SAVE_BYTES as u64).contains(&file_bytes));
    eprintln!(
        "synthetic dense+long fixture: 8 distinct MVT blobs, 1,024 lines each, {file_bytes} save bytes"
    );
}
