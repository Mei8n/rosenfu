use rosenfu_core::{
    ofm_evidence::{
        find_gameplay_bus_path, BusVertex, EvidenceStore, InstalledWestboundSeamRecipe,
        RawEvidence, TileBinding,
    },
    operation::{OperationRuntime, Timetable, TrainState, Vehicle},
    world::{BusOperationConfig, World},
};
use rosenfu_storage::{
    backup_path, decode, encode, load_world, save_world, SaveOutcome, StorageError,
};
#[derive(serde::Deserialize)]
struct Fixture {
    tiles: Vec<TileBinding>,
    blobs: Vec<Vec<u8>>,
    key: String,
}

#[test]
fn installed_release_admission_full_file_roundtrip_and_overwrite_refusal() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("generated_installed_seam_world.json")).unwrap();
    let raw: Vec<_> = fixture
        .blobs
        .into_iter()
        .zip(&fixture.tiles)
        .map(|(bytes, t)| RawEvidence {
            sha256: t.raw_sha256.clone(),
            bytes,
        })
        .collect();
    let evidence = EvidenceStore::from_archive_parts(raw, fixture.tiles).unwrap();
    let mut world = World::default();
    let player = world.add_player("player".into()).unwrap();
    let owner = world.add_company(player, "operator".into()).unwrap();
    let legacy = encode(&world).unwrap();
    assert_eq!(encode(&decode(&legacy).unwrap()).unwrap(), legacy);
    let staged = World::new(
        world
            .snapshot()
            .clone()
            .with_road_evidence(evidence)
            .unwrap(),
    )
    .unwrap();
    let archive = &staged.snapshot().runtime_road_evidence;
    let east = archive
        .bindings()
        .find(|t| t.x == 14553)
        .unwrap()
        .canonical_url
        .clone();
    let east_only = archive.scope(std::slice::from_ref(&east)).unwrap();
    let vertex = |vertex| BusVertex {
        feature: 0,
        line: 0,
        vertex,
    };
    let ordinary_path = find_gameplay_bus_path(&east_only, &east, vertex(0), vertex(1), true)
        .unwrap()
        .unwrap();
    let ordinary = staged
        .candidate_with_gameplay_ofm_bus(
            owner,
            "A".into(),
            "Ordinary".into(),
            ordinary_path.binding,
            vec![],
        )
        .unwrap();
    let legacy_before = encode(&ordinary).unwrap();
    assert_eq!(
        encode(&decode(&legacy_before).unwrap()).unwrap(),
        legacy_before
    );
    let mut seam = ordinary
        .candidate_with_installed_seam_bus(
            owner,
            "B".into(),
            "Bus".into(),
            InstalledWestboundSeamRecipe {
                version: 1,
                catalog_key: fixture.key,
            },
        )
        .unwrap();
    let config = |id: &str| BusOperationConfig {
        route_id: id.into(),
        timetable: Timetable {
            first_departure_ms: 0,
            last_departure_ms: 0,
            headway_ms: 1000,
        },
        vehicle: Vehicle {
            max_speed_kmh: 35.,
            acceleration_mps2: 1.,
            braking_mps2: 1.,
            length_m: 10.,
        },
        dwell_ms: 0,
        assumed_speed_kmh: 35.,
        turnaround_ms: None,
    };
    assert_eq!(encode(&ordinary).unwrap(), legacy_before);
    let old_spec = ordinary.compile_bus_operation(&config("A")).unwrap();
    let mixed_spec = seam.compile_bus_operation(&config("A")).unwrap();
    let new_spec = seam.compile_bus_operation(&config("B")).unwrap();
    let old_resources = &old_spec.legs[0].sections[0].resources;
    let mixed_resources = &mixed_spec.legs[0].sections[0].resources;
    let new_resources = &new_spec.legs[0].sections[0].resources;
    assert!(mixed_resources.starts_with(old_resources));
    let lock = mixed_resources.last().unwrap();
    assert!(lock.starts_with("ofm-bus:fragment-lock:") && new_resources.contains(lock));
    assert_eq!(mixed_resources.len(), old_resources.len() + 1);
    assert_eq!(
        mixed_spec.legs[0].sections[0].length_m,
        old_spec.legs[0].sections[0].length_m
    );
    seam.configure_all_operation(vec![], vec![config("A"), config("B")])
        .unwrap();
    seam.operation_mut()
        .unwrap()
        .runtime
        .advance_to(1001, 20)
        .unwrap();
    assert_eq!(
        seam.operation()
            .unwrap()
            .runtime
            .trains()
            .filter(|t| matches!(t.state, TrainState::Waiting { .. }))
            .count(),
        1
    );
    let trains: Vec<_> = seam.operation().unwrap().runtime.trains().collect();
    assert!(trains
        .iter()
        .any(|t| t.key.service_id == "bus:A" && matches!(t.state, TrainState::Moving { .. })));
    assert!(trains
        .iter()
        .any(|t| t.key.service_id == "bus:B" && matches!(t.state, TrainState::Waiting { .. })));
    let checkpoint = encode(&seam).unwrap();
    // The runner reuses only this synthetic checkpoint in a separately
    // built, unmodified release catalog. Never bake the QA tiles there.
    std::fs::write(
        std::env::var_os("ROSENFU_QA_SEAM_CHECKPOINT").unwrap(),
        &checkpoint,
    )
    .unwrap();
    let mut wire: serde_json::Value = serde_json::from_slice(&checkpoint).unwrap();
    assert_eq!(wire["schema_version"], 36);
    assert_eq!(encode(&decode(&checkpoint).unwrap()).unwrap(), checkpoint);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.json");
    assert_eq!(save_world(&path, &seam).unwrap(), SaveOutcome::Created);
    assert_eq!(std::fs::read(&path).unwrap(), checkpoint);
    let mut reopened = load_world(&path).unwrap();
    let finish = |w: &mut World| {
        w.operation_mut()
            .unwrap()
            .runtime
            .advance_to(120_000, 50)
            .unwrap()
    };
    let arrivals = finish(&mut seam);
    assert_eq!(finish(&mut reopened), arrivals);
    assert_eq!(
        arrivals
            .arrivals
            .iter()
            .map(|a| a.train.service_id.as_str())
            .collect::<Vec<_>>(),
        ["bus:A", "bus:B"]
    );
    assert!(arrivals.arrivals[1].delay_ms > 0);
    let finished = encode(&reopened).unwrap();
    assert_ne!(finished, checkpoint);
    assert!(matches!(
        save_world(&path, &reopened).unwrap(),
        SaveOutcome::Updated { .. }
    ));
    assert_eq!(std::fs::read(backup_path(&path)).unwrap(), checkpoint);
    assert_eq!(encode(&load_world(&path).unwrap()).unwrap(), finished);

    let mut cheated = reopened.clone();
    let mut op = cheated.operation().unwrap().runtime.save();
    op.services[0].legs[0].sections[0].resources.pop();
    cheated.operation_mut().unwrap().runtime = OperationRuntime::restore(op).unwrap();
    assert!(matches!(
        save_world(&path, &cheated),
        Err(StorageError::UnsupportedVersion(36))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), finished);
    assert_eq!(std::fs::read(backup_path(&path)).unwrap(), checkpoint);

    wire["world"]["ofm_installed_seam_bus_routes"]["B"]["catalogKey"] =
        format!("ofm-westbound-v1:{}", "0".repeat(64)).into();
    let untrusted = serde_json::to_vec(&wire).unwrap();
    std::fs::write(&path, &untrusted).unwrap();
    assert!(matches!(
        load_world(&path),
        Err(StorageError::UnsupportedVersion(36))
    ));
    assert!(matches!(
        save_world(&path, &world),
        Err(StorageError::UnsupportedVersion(36))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), untrusted);
    assert_eq!(std::fs::read(backup_path(&path)).unwrap(), checkpoint);
}

#[test]
fn unmodified_release_rejects_the_checkpoint_without_overwrite() {
    let checkpoint =
        std::fs::read(std::env::var_os("ROSENFU_QA_SEAM_CHECKPOINT").unwrap()).unwrap();
    assert!(matches!(
        decode(&checkpoint),
        Err(StorageError::UnsupportedVersion(36))
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.json");
    std::fs::write(&path, &checkpoint).unwrap();
    assert!(matches!(
        load_world(&path),
        Err(StorageError::UnsupportedVersion(36))
    ));
    assert!(matches!(
        save_world(&path, &World::default()),
        Err(StorageError::UnsupportedVersion(36))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), checkpoint);
    assert!(!backup_path(&path).exists());
}
