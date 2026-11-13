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
fn inspect_migrates_legacy_in_memory_and_counts_current_services() {
    use rosenfu_core::world::{CompanyId, StationId, TrackId};
    let legacy = include_bytes!("../../rosenfu-storage/tests/fixtures/schema-1-world.json");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.json");
    std::fs::write(&path, legacy).unwrap();
    let inspect = || {
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
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };
    assert_eq!(inspect()["services"], 0);
    assert_eq!(std::fs::read(&path).unwrap(), legacy);
    assert!(!rosenfu_storage::backup_path(&path).exists());
    let mut world = rosenfu_storage::load_world(&path).unwrap();
    world
        .add_service(
            CompanyId(2),
            "Express".into(),
            StationId(3),
            vec![TrackId(6), TrackId(7)],
            vec![0, 2],
        )
        .unwrap();
    rosenfu_storage::save_world(&path, &world).unwrap();
    let result = inspect();
    assert_eq!(result["services"], 1);
    assert_eq!(result["stations"], 3);
    assert_eq!(result["next_id"], "11");
}
