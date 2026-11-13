use rosenfu_core::world::{GeoPoint, World};
#[test]
fn malformed_alignment_never_changes_world_or_allocator() {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let before = w.clone();
    for points in [
        vec![],
        vec![GeoPoint {
            latitude: 35.0,
            longitude: 139.0,
        }],
        vec![
            GeoPoint {
                latitude: 35.0,
                longitude: 139.0
            };
            2
        ],
        vec![
            GeoPoint {
                latitude: f64::NAN,
                longitude: 139.0,
            },
            GeoPoint {
                latitude: 35.0,
                longitude: 139.0,
            },
        ],
    ] {
        assert!(w.build_alignment(c, "L".into(), points).is_err());
        assert_eq!(w, before);
    }
}
#[test]
fn lengths_are_in_metres_and_resize_preserves_station_identity() {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let a = w
        .build_alignment(
            c,
            "L".into(),
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
    let s = w
        .place_station_on_alignment(c, "S".into(), a, 1.0, 200.0, 20.0)
        .unwrap();
    let next = w.snapshot().next_id;
    w.update_station_placement(s, 300.0, 30.0).unwrap();
    assert_eq!(w.snapshot().next_id, next);
    assert_eq!(w.snapshot().stations[0].id, s);
    assert_eq!(w.snapshot().station_placements[0].length_m, 300.0);
    w.snapshot().validate().unwrap();
}
