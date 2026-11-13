use rosenfu_core::world::{GeoPoint, TrackType, World};
use rosenfu_storage::{backup_path, decode, encode, load_world, save_world};
use serde_json::{json, Value};
use std::fs;

fn sample() -> World {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    w.build_alignment(
        c,
        "A".into(),
        vec![
            GeoPoint {
                latitude: 35.0,
                longitude: 139.0,
            },
            GeoPoint {
                latitude: 35.01,
                longitude: 139.0,
            },
        ],
    )
    .unwrap();
    w
}
fn old(version: u64) -> Value {
    let mut value: Value = serde_json::from_slice(&encode(&sample()).unwrap()).unwrap();
    value["schema_version"] = json!(version);
    for a in value["world"]["alignments"].as_array_mut().unwrap() {
        a.as_object_mut().unwrap().remove("track_type");
    }
    let w = value["world"].as_object_mut().unwrap();
    if version < 6 {
        w.remove("source_records");
        w.remove("alignment_provenance");
    }
    if version < 5 {
        w.remove("service_paths");
    }
    if version < 4 {
        w.remove("route_paths");
    }
    value
}
#[test]
fn schemas_two_through_six_default_old_alignments_to_normal_and_preserve_original_backup() {
    for version in 2..=6 {
        let old = old(version);
        let bytes = serde_json::to_vec_pretty(&old).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.json");
        fs::write(&path, &bytes).unwrap();
        let restored = load_world(&path).unwrap();
        assert_eq!(
            restored.snapshot().alignments[0].track_type,
            TrackType::Normal
        );
        assert_eq!(restored, sample());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        save_world(&path, &restored).unwrap();
        assert_eq!(fs::read(backup_path(&path)).unwrap(), bytes);
        assert_eq!(load_world(&path).unwrap(), restored);
    }
}
#[test]
fn old_headers_reject_new_kind_and_current_requires_known_kind() {
    for version in 2..=6 {
        let mut value = old(version);
        value["world"]["alignments"][0]["track_type"] = json!("normal");
        assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    let value: Value = serde_json::from_slice(&encode(&sample()).unwrap()).unwrap();
    for kind in [Value::Null, json!("unknown"), json!(1), json!({})] {
        let mut bad = value.clone();
        bad["world"]["alignments"][0]["track_type"] = kind;
        assert!(decode(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
    let mut missing = value;
    missing["world"]["alignments"][0]
        .as_object_mut()
        .unwrap()
        .remove("track_type");
    assert!(decode(&serde_json::to_vec(&missing).unwrap()).is_err());
}
#[test]
fn all_kinds_roundtrip_without_changing_ids_or_geometry() {
    let base = sample();
    for (wire, kind) in [
        ("normal", TrackType::Normal),
        ("high_speed", TrackType::HighSpeed),
        ("tram", TrackType::Tram),
    ] {
        let mut value: Value = serde_json::from_slice(&encode(&base).unwrap()).unwrap();
        value["world"]["alignments"][0]["track_type"] = json!(wire);
        let restored = decode(&serde_json::to_vec(&value).unwrap()).unwrap();
        let a = &restored.snapshot().alignments[0];
        assert_eq!(a.track_type, kind);
        assert_eq!(a.id, base.snapshot().alignments[0].id);
        assert_eq!(a.points, base.snapshot().alignments[0].points);
        assert_eq!(decode(&encode(&restored).unwrap()).unwrap(), restored);
    }
}
