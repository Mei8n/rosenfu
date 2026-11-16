use rosenfu_core::world::*;
use rosenfu_storage::{decode, encode};
fn snapshot(revision: &str, x: u32) -> RoadSnapshot {
    let url = format!("https://tiles.openfreemap.org/planet/{revision}/14/{x}/6465.pbf");
    RoadSnapshot {
        revision: revision.into(),
        source: url.clone(),
        source_url: url,
        source_sha256: "a".repeat(64),
        attribution: "© OpenStreetMap contributors".into(),
        license: "ODbL-1.0".into(),
        coverage_verified: false,
        road_coverage: vec![CoverageBounds {
            west: 139.6,
            east: 139.8,
            south: 35.6,
            north: 35.8,
        }],
        roads: vec![],
    }
}
#[test]
fn saved_ofm_revision_is_retained_and_mixed_or_mislabelled_save_is_rejected() {
    let mut world = World::default();
    world.add_road_snapshot(snapshot("old", 14551)).unwrap();
    let saved = encode(&world).unwrap();
    let mut loaded = decode(&saved).unwrap();
    assert!(loaded.add_road_snapshot(snapshot("new", 14552)).is_err());
    assert_eq!(encode(&loaded).unwrap(), saved);
    let mut value: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    value["world"]["road_snapshots"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::to_value(snapshot("new", 14552)).unwrap());
    assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
    let mut bad = world.snapshot().clone();
    bad.road_snapshots[0].revision = "wrong".into();
    assert!(World::new(bad).is_err());
}
