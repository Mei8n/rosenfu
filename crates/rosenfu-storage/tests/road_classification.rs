use rosenfu_core::world::*;
use rosenfu_storage::{decode, encode};
#[test]
fn classified_paths_and_unclassified_legacy_roads_roundtrip_without_relabelling() {
    let mut world = World::default();
    let road = RoadWay {
        id: "ofm:fixture:14:14551:6465:0:0".into(),
        classification: Some(RoadClassification {
            class: "path".into(),
            subclass: Some("steps".into()),
            access: Some("private".into()),
            oneway: Some("-1".into()),
        }),
        coverage: None,
        points: vec![
            GeoPoint {
                longitude: 139.7,
                latitude: 35.67,
            },
            GeoPoint {
                longitude: 139.71,
                latitude: 35.68,
            },
        ],
        layer: None,
        level: None,
        bridge: None,
        tunnel: None,
    };
    let mut unknown_direction = road.clone();
    unknown_direction.id = "unknown-direction/1".into();
    unknown_direction.classification.as_mut().unwrap().oneway = None;
    let mut legacy = road.clone();
    legacy.id = "legacy/1".into();
    legacy.classification = None;
    world
        .add_road_snapshot(RoadSnapshot {
            revision: "fixture".into(),
            source: "fixture".into(),
            source_url: "https://example.org/roads".into(),
            source_sha256: "a".repeat(64),
            attribution: "Synthetic fixture".into(),
            license: "CC0".into(),
            coverage_verified: false,
            road_coverage: vec![CoverageBounds {
                west: 139.6,
                east: 139.8,
                south: 35.6,
                north: 35.8,
            }],
            roads: vec![road, legacy, unknown_direction],
        })
        .unwrap();
    let loaded = decode(&encode(&world).unwrap()).unwrap();
    assert_eq!(loaded, world);
    let roads = &loaded.snapshot().road_snapshots[0].roads;
    assert_eq!(
        roads[0]
            .classification
            .as_ref()
            .unwrap()
            .subclass
            .as_deref(),
        Some("steps")
    );
    assert_eq!(
        roads[0].classification.as_ref().unwrap().oneway.as_deref(),
        Some("-1")
    );
    assert!(roads[1].classification.is_none());
    assert!(roads[2].classification.as_ref().unwrap().oneway.is_none());
}
