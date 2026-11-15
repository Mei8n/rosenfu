use rosenfu_core::world::{GeoPoint, World, WorldError};
fn point(x: f64, y: f64) -> GeoPoint {
    GeoPoint {
        latitude: 35.0 + y / 111_000.0,
        longitude: 139.0 + x / 91_000.0,
    }
}
#[test]
fn same_alignment_overlap_is_atomic_but_separate_platforms_are_allowed() {
    let mut world = World::default();
    let p = world.add_player("P".into()).unwrap();
    let c = world.add_company(p, "C".into()).unwrap();
    let id = world
        .build_alignment(c, "線路".into(), vec![point(0., 0.), point(1000., 0.)])
        .unwrap();
    world
        .place_station_on_alignment(c, "駅".into(), id, 0.5, 100., 12.)
        .unwrap();
    let before = world.clone();
    for position in [0.5, 0.52, 0.48] {
        assert_eq!(
            world.place_station_on_alignment(c, "重複".into(), id, position, 100., 12.),
            Err(WorldError::StationFootprintOverlap)
        );
        assert_eq!(world, before);
    }
    world
        .place_station_on_alignment(c, "別駅".into(), id, 0.8, 100., 12.)
        .unwrap();
    let nearby = world
        .build_alignment(c, "隣線".into(), vec![point(0., 20.), point(1000., 20.)])
        .unwrap();
    world
        .place_station_on_alignment(c, "隣接".into(), nearby, 0.5, 100., 12.)
        .unwrap();
    let crossing = world
        .build_alignment(
            c,
            "交差線".into(),
            vec![point(500., -500.), point(500., 500.)],
        )
        .unwrap();
    // Cross-alignment station grouping remains a separate explicit feature.
    world
        .place_station_on_alignment(c, "交差".into(), crossing, 0.5, 100., 12.)
        .unwrap();
}
#[test]
fn curved_alignment_overlap_and_clamped_ends_are_guarded_without_load_migration() {
    let mut world = World::default();
    let p = world.add_player("P".into()).unwrap();
    let c = world.add_company(p, "C".into()).unwrap();
    let id = world
        .build_alignment(
            c,
            "曲線".into(),
            vec![point(0., 0.), point(200., 100.), point(500., 0.)],
        )
        .unwrap();
    world
        .place_station_on_alignment(c, "駅".into(), id, 0., 100., 12.)
        .unwrap();
    let before = world.clone();
    assert_eq!(
        world.place_station_on_alignment(c, "重複".into(), id, 0.01, 100., 12.),
        Err(WorldError::StationFootprintOverlap)
    );
    assert_eq!(world, before);
    world
        .place_station_on_alignment(c, "遠方".into(), id, 1., 100., 12.)
        .unwrap();
    let mut legacy = world.snapshot().clone();
    legacy.station_placements[1].position = legacy.station_placements[0].position;
    legacy.stations[1].latitude = legacy.stations[0].latitude;
    legacy.stations[1].longitude = legacy.stations[0].longitude;
    assert!(legacy.validate().is_ok());
    world.replace(legacy).unwrap();
}
