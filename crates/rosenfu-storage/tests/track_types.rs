use rosenfu_core::world::{GeoPoint, TrackType, World};
use rosenfu_storage::{decode, encode};
use serde_json::{json, Value};

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
