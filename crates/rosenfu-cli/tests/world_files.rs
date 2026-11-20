use std::process::Command;

#[test]
fn new_and_inspect_work_and_new_never_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test world.rosenfu.json");
    let run = |action| {
        Command::new(env!("CARGO_BIN_EXE_rosenfu-cli"))
            .args(["world", action])
            .arg(&path)
            .output()
            .unwrap()
    };
    assert!(run("new").status.success());
    let original = std::fs::read(&path).unwrap();
    let inspected = run("inspect");
    assert!(inspected.status.success());
    let value: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
    assert_eq!(value["stations"], 0);
    assert_eq!(value["services"], 0);
    assert_eq!(value["next_id"], "1");
    assert!(!run("new").status.success());
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn bad_arguments_and_corrupt_files_fail_clearly() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.json");
    std::fs::write(&path, b"broken").unwrap();
    for args in [vec!["world"], vec!["unknown"]] {
        assert!(!Command::new(env!("CARGO_BIN_EXE_rosenfu-cli"))
            .args(args)
            .output()
            .unwrap()
            .status
            .success());
    }
    let output = Command::new(env!("CARGO_BIN_EXE_rosenfu-cli"))
        .args(["world", "inspect"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!output.stderr.is_empty());
    assert!(output.stdout.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), b"broken");
}

#[test]
fn inspect_rejects_legacy_without_touching_the_file() {
    let legacy = include_bytes!("../../rosenfu-storage/tests/fixtures/schema-1-world.json");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.json");
    std::fs::write(&path, legacy).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rosenfu-cli"))
        .args(["world", "inspect"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), legacy);
    assert!(!rosenfu_storage::backup_path(&path).exists());
}

#[test]
fn inspect_counts_physical_services_without_modifying_current_save() {
    use rosenfu_core::world::{GeoPoint, RouteSegment, World};
    let mut world = World::default();
    let player = world.add_player("Player".into()).unwrap();
    let company = world.add_company(player, "Railway".into()).unwrap();
    let alignment = world
        .build_alignment(
            company,
            "Line".into(),
            vec![
                GeoPoint {
                    latitude: 35.,
                    longitude: 139.,
                },
                GeoPoint {
                    latitude: 35.,
                    longitude: 139.1,
                },
            ],
        )
        .unwrap();
    let stations: Vec<_> = (1..=3)
        .map(|i| {
            world
                .place_station_on_alignment(
                    company,
                    format!("S{i}"),
                    alignment,
                    i as f64 / 4.,
                    20.,
                    10.,
                )
                .unwrap()
        })
        .collect();
    let segments = vec![RouteSegment::Alignment {
        alignment_id: alignment,
        from: stations[0],
        to: stations[2],
    }];
    let occurrences = world
        .preview_service_path(stations[0], segments.clone())
        .unwrap()
        .occurrences;
    let stops = vec![
        occurrences.first().unwrap().reference.clone(),
        occurrences.last().unwrap().reference.clone(),
    ];
    world
        .add_service_path(company, "Express".into(), stations[0], segments, stops)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("current.json");
    rosenfu_storage::save_world(&path, &world).unwrap();
    let original = std::fs::read(&path).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rosenfu-cli"))
        .args(["world", "inspect"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["services"], 1);
    assert_eq!(result["stations"], 3);
    assert_eq!(result["alignments"], 1);
    assert_eq!(
        result["next_id"],
        serde_json::Value::String(world.snapshot().next_id.to_string())
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!rosenfu_storage::backup_path(&path).exists());
}
