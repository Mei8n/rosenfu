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

#[test]
fn schema_ten_path_station_and_schema_eleven_built_station_roundtrip() {
    let legacy = World::new(WorldDraft {
        construction_plans: Some(plans()),
        ..WorldDraft::default()
    })
    .unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&encode(&legacy).unwrap()).unwrap();
    value["schema_version"] = json!(10);
    assert_eq!(
        decode(&serde_json::to_vec(&value).unwrap()).unwrap(),
        legacy
    );
    let station = &mut value["world"]["construction_plans"]["plannedStations"][0];
    station.as_object_mut().unwrap().remove("pathId");
    station["alignmentId"] = json!("999");
    station["planId"] = json!(2);
    value["schema_version"] = json!(11);
    // Stale infrastructure references remain editable drafts, validated at build time.
    let restored = decode(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(decode(&encode(&restored).unwrap()).unwrap(), restored);
    value["world"]["construction_plans"]["plannedStations"][0]["pathId"] = json!(3);
    assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn edited_plan_shape_roundtrips_without_reconstruction() {
    let mut payload = serde_json::to_value(plans()).unwrap();
    let a = json!({"latitude":35.0,"longitude":139.0});
    let b = json!({"latitude":35.001,"longitude":139.002});
    let mid = json!({"latitude":35.0006,"longitude":139.001});
    payload["draftPoints"] = json!([a, b]);
    payload["draftEditedGeometry"] = json!({"version":1,"anchors":[a,b],"line":[a,mid,b]});
    let mut draft = WorldDraft::default();
    draft.construction_plans = Some(serde_json::from_value(payload.clone()).unwrap());
    let world = World::new(draft).unwrap();
    assert_eq!(decode(&encode(&world).unwrap()).unwrap(), world);
    payload["draftEditedGeometry"]["version"] = json!(999);
    let mut invalid = WorldDraft::default();
    invalid.construction_plans = Some(serde_json::from_value(payload).unwrap());
    assert!(World::new(invalid).is_err());
}

#[test]
fn schema_eleven_plan_without_geometry_remains_unchanged() {
    let mut draft = WorldDraft::default();
    draft.construction_plans = Some(plans());
    let world = World::new(draft).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&encode(&world).unwrap()).unwrap();
    value["schema_version"] = json!(11);
    let restored = decode(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(restored, world);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&encode(&restored).unwrap()).unwrap()
            ["schema_version"],
        rosenfu_storage::SCHEMA_VERSION
    );
}

#[test]
fn destination_and_redo_destination_survive_plan_save() {
    let mut payload = serde_json::to_value(plans()).unwrap();
    let a = json!({"latitude":35.0,"longitude":139.0});
    let b = json!({"latitude":35.0,"longitude":139.01});
    payload["draftPoints"] = json!([a, b]);
    payload["draftExtension"] = json!({"alignmentId":"1","endpoint":"end"});
    payload["draftDestination"] = json!({"alignmentId":"2","endpoint":"start"});
    let parked = &mut payload["parkedPaths"][0];
    parked["extension"] = json!({"alignmentId":"3","endpoint":"start"});
    parked["redo"] = json!([b]);
    parked["redoDestination"] = json!({"alignmentId":"4","endpoint":"end"});
    let world = World::new(WorldDraft {
        construction_plans: Some(serde_json::from_value(payload.clone()).unwrap()),
        ..WorldDraft::default()
    })
    .unwrap();
    assert_eq!(decode(&encode(&world).unwrap()).unwrap(), world);
    payload["draftPoints"] = json!([a]);
    payload.as_object_mut().unwrap().remove("draftDestination");
    payload["draftRedoDestination"] = json!({"alignmentId":"2","endpoint":"start"});
    payload["redoPoints"] = json!([b]);
    let world = World::new(WorldDraft {
        construction_plans: Some(serde_json::from_value(payload).unwrap()),
        ..WorldDraft::default()
    })
    .unwrap();
    assert_eq!(decode(&encode(&world).unwrap()).unwrap(), world);
}

#[test]
fn opposite_endpoint_ring_drafts_and_parked_paths_roundtrip() {
    let mut payload = serde_json::to_value(plans()).unwrap();
    let points = json!([{"latitude":35.0,"longitude":139.0},{"latitude":35.0,"longitude":139.01}]);
    payload["draftPoints"] = points.clone();
    payload["draftExtension"] = json!({"alignmentId":"3","endpoint":"end"});
    payload["draftDestination"] = json!({"alignmentId":"3","endpoint":"start"});
    payload["parkedPaths"][0]["points"] = points;
    payload["parkedPaths"][0]["extension"] = payload["draftExtension"].clone();
    payload["parkedPaths"][0]["destination"] = payload["draftDestination"].clone();
    let make = |v| {
        World::new(WorldDraft {
            construction_plans: Some(serde_json::from_value(v).unwrap()),
            ..WorldDraft::default()
        })
    };
    let world = make(payload.clone()).unwrap();
    assert_eq!(decode(&encode(&world).unwrap()).unwrap(), world);
    payload["draftDestination"]["endpoint"] = json!("end");
    assert!(make(payload).is_err());
}
