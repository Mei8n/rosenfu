use rosenfu_core::world::{FixedOdScenario, World};
use rosenfu_storage::{decode, load_world, save_world, StorageError};
use serde_json::{json, Value};
use std::fs;

fn scenario() -> FixedOdScenario {
    FixedOdScenario {
        origin_station_id: "1".into(),
        destination_station_id: "bus:qa:2".into(),
        trips: Some(0.0),
        alternative_seconds: Some(125.0),
    }
}

#[test]
fn reads_prior_schema_without_scenario_or_file_rewrite() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.json");
    // Captured from the previous CLI writer (fc1dfe2), not by tweaking v27 output.
    let bytes = include_bytes!("fixtures/schema-26-world.json");
    let mut old: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(old["schema_version"], 26);
    fs::write(&path, bytes).unwrap();
    assert!(load_world(&path).unwrap().fixed_od_scenario().is_none());
    assert_eq!(fs::read(&path).unwrap(), bytes.as_slice());
    old["fixed_od_scenario"] = serde_json::to_value(scenario()).unwrap();
    assert!(matches!(
        decode(&serde_json::to_vec(&old).unwrap()),
        Err(StorageError::InvalidHeader)
    ));
}

#[test]
fn legacy_schema_operation_remains_on_runtime_format_two() {
    let mut old: Value =
        serde_json::from_slice(include_bytes!("fixtures/schema-26-world.json")).unwrap();
    old["operation"] = json!({
        "configs": [], "bus_configs": [], "invalidated": false, "pending_target": null,
        "runtime": {
            "format": 2, "started": false, "now_ms": 0, "paused": false, "speed": 1,
            "services": [], "trains": [], "next_departures": []
        }
    });
    assert_eq!(
        decode(&serde_json::to_vec(&old).unwrap())
            .unwrap()
            .operation_save()
            .unwrap()
            .runtime
            .format,
        2
    );
    for format in [1, 3, 4, 99] {
        old["operation"]["runtime"]["format"] = json!(format);
        assert!(matches!(
            decode(&serde_json::to_vec(&old).unwrap()),
            Err(StorageError::InvalidHeader)
        ));
    }
}

#[test]
fn scenario_is_one_independent_input_no_results_and_orphan_can_load() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.json");
    let mut world = World::default();
    world.restore_fixed_od_scenario(Some(scenario())).unwrap();
    save_world(&path, &world).unwrap();
    let bytes = fs::read(&path).unwrap();
    assert_eq!(load_world(&path).unwrap(), world);
    assert!(!world
        .fixed_od_scenario()
        .unwrap()
        .endpoints_exist(world.snapshot()));
    let mut value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["fixed_od_scenario"].as_object().unwrap().len(), 4);
    value["fixed_od_scenario"]["origin_station_id"] = json!("01");
    assert!(matches!(
        decode(&serde_json::to_vec(&value).unwrap()),
        Err(StorageError::InvalidHeader)
    ));
    assert_eq!(fs::read(&path).unwrap(), bytes);
}
