use rosenfu_core::world::*;
fn point(lon: f64) -> GeoPoint {
    GeoPoint {
        latitude: 35.0,
        longitude: lon,
    }
}
#[test]
fn both_ends_preserve_stations_geometry_distances_and_identity() {
    for endpoint in [AlignmentEndpoint::Start, AlignmentEndpoint::End] {
        let mut w = World::default();
        let p = w.add_player("P".into()).unwrap();
        let c = w.add_company(p, "C".into()).unwrap();
        let a = w
            .build_alignment(c, "A".into(), vec![point(139.0), point(139.1)])
            .unwrap();
        for position in [0.2, 0.8] {
            w.place_station_on_alignment(c, "S".into(), a, position, 100.0, 10.0)
                .unwrap();
        }
        let before = w.snapshot().clone();
        let points = if endpoint == AlignmentEndpoint::Start {
            vec![point(139.0), point(138.9)]
        } else {
            vec![point(139.1), point(139.2)]
        };
        w.extend_alignment(c, a, endpoint, points).unwrap();
        let after = w.snapshot();
        assert_eq!(after.stations, before.stations);
        assert_eq!(after.next_id, before.next_id);
        assert_eq!(after.alignments.len(), 1);
        assert_eq!(after.alignments[0].id, a);
        let old_gap = (before.station_placements[1].position
            - before.station_placements[0].position)
            * before.alignments[0].length_m();
        let gap = (after.station_placements[1].position - after.station_placements[0].position)
            * after.alignments[0].length_m();
        assert!((gap - old_gap).abs() < 1e-6);
        for (placement, station) in after.station_placements.iter().zip(&after.stations) {
            let actual = after.alignments[0].point_at(placement.position);
            assert!((actual.longitude - station.longitude).abs() < 1e-10);
        }
        let stable = after.clone();
        for bad in [
            vec![point(140.0), point(140.1)],
            vec![point(139.1)],
            vec![point(139.1), point(f64::NAN)],
        ] {
            assert!(w
                .extend_alignment(c, a, AlignmentEndpoint::End, bad)
                .is_err());
            assert_eq!(w.snapshot(), &stable);
        }
    }
}

#[test]
fn backward_and_right_angle_joins_are_atomic_errors() {
    let mut world = World::default();
    let player = world.add_player("P".into()).unwrap();
    let owner = world.add_company(player, "C".into()).unwrap();
    let id = world
        .build_alignment(owner, "A".into(), vec![point(139.0), point(139.1)])
        .unwrap();
    let before = world.snapshot().clone();
    for endpoint in [AlignmentEndpoint::Start, AlignmentEndpoint::End] {
        let anchor = if endpoint == AlignmentEndpoint::Start {
            point(139.0)
        } else {
            point(139.1)
        };
        let backward = if endpoint == AlignmentEndpoint::Start {
            point(139.1)
        } else {
            point(139.0)
        };
        for next in [
            backward,
            GeoPoint {
                latitude: 35.01,
                longitude: anchor.longitude,
            },
        ] {
            assert!(world
                .extend_alignment(owner, id, endpoint, vec![anchor, next])
                .is_err());
            assert_eq!(world.snapshot(), &before);
        }
    }
}
