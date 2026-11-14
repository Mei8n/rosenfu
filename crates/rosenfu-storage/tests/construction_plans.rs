use rosenfu_core::world::{ConstructionPlans, World, WorldDraft};
use rosenfu_storage::{backup_path, decode, encode, load_world, save_world};
use serde_json::json;

fn plans() -> ConstructionPlans {
    serde_json::from_value(json!({
        "version":1,"plans":[{"id":1,"name":"一期"},{"id":2,"name":"二期"}],
        "parkedPaths":[{"id":3,"planId":2,"name":"二期","points":[{"latitude":35.0,"longitude":139.0}],"redo":[],"line":[],"extension":null,"designSpeedKmh":120,"trackType":"normal","roadFollowing":false}],
        "activePathId":1,"activePlanId":1,"draftPoints":[{"latitude":35.0,"longitude":139.0}],"redoPoints":[],"draftExtension":null,
        "trackType":"normal","designSpeedKmh":120,"roadFollowing":false,"draftFinished":false,
        "plannedStations":[{"id":"planned:1","pathId":3,"name":"新駅","point":{"latitude":35.0,"longitude":139.0},"lengthM":100}]
    })).unwrap()
}
#[test]
fn plans_roundtrip_with_multiple_plans_paths_and_stations() {
    let mut draft = WorldDraft::default();
    draft.construction_plans = Some(plans());
    let world = World::new(draft).unwrap();
    assert_eq!(decode(&encode(&world).unwrap()).unwrap(), world);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.json");
    let previous = World::default();
    save_world(&path, &previous).unwrap();
    save_world(&path, &world).unwrap();
    assert_eq!(load_world(&path).unwrap(), world);
    assert_eq!(load_world(backup_path(&path)).unwrap(), previous);
}
#[test]
fn schema_nine_without_plans_remains_readable() {
    let mut value: serde_json::Value =
        serde_json::from_slice(&encode(&World::default()).unwrap()).unwrap();
    value["schema_version"] = json!(9);
    value["world"]
        .as_object_mut()
        .unwrap()
        .remove("construction_plans");
    assert!(decode(&serde_json::to_vec(&value).unwrap())
        .unwrap()
        .snapshot()
        .construction_plans
        .is_none());
}
#[test]
fn invalid_plan_payload_is_rejected_without_overwriting_old_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.json");
    save_world(&path, &World::default()).unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut draft = WorldDraft::default();
    let mut plans = plans();
    plans.active_plan_id = 999;
    draft.construction_plans = Some(plans);
    assert!(World::new(draft).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let mut value: serde_json::Value = serde_json::from_slice(&before).unwrap();
    value["world"]["construction_plans"] = json!({"version":999});
    assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
}
