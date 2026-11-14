use rosenfu_core::world::*;
fn point(x: f64) -> GeoPoint {
    GeoPoint {
        latitude: 35.,
        longitude: x,
    }
}
fn end(id: AlignmentId, endpoint: AlignmentEndpoint) -> AlignmentEndpointRef {
    AlignmentEndpointRef {
        alignment_id: id,
        endpoint,
    }
}
#[test]
fn all_four_orientations_preserve_ids_stations_and_speed_caps() {
    for ae in [AlignmentEndpoint::Start, AlignmentEndpoint::End] {
        for be in [AlignmentEndpoint::Start, AlignmentEndpoint::End] {
            let mut w = World::default();
            let p = w.add_player("P".into()).unwrap();
            let c = w.add_company(p, "C".into()).unwrap();
            let mut ap = vec![point(139.), point(139.1)];
            let mut bp = vec![point(139.1), point(139.2)];
            if ae == AlignmentEndpoint::Start {
                ap.reverse();
            }
            if be == AlignmentEndpoint::End {
                bp.reverse();
            }
            let a = w.build_alignment(c, "A".into(), ap).unwrap();
            let b = w.build_alignment(c, "B".into(), bp).unwrap();
            w.set_alignment_design_speed(a, 120.).unwrap();
            w.set_alignment_design_speed(b, 80.).unwrap();
            w.place_station_on_alignment(c, "S".into(), a, 0.5, 100., 10.)
                .unwrap();
            let before = w.snapshot().clone();
            w.connect_alignment_endpoints(c, end(a, ae), end(b, be))
                .unwrap();
            let after = w.snapshot().clone();
            assert_eq!(before.alignments, after.alignments);
            assert_eq!(before.station_placements, after.station_placements);
            assert_eq!(before.stations, after.stations);
            assert_eq!(before.next_id, after.next_id);
            assert_eq!(after.alignment_connections.len(), 1);
            assert!(w
                .connect_alignment_endpoints(c, end(b, be), end(a, ae))
                .is_err());
            assert_eq!(w.snapshot(), &after);
            let raw = serde_json::to_string(&after).unwrap();
            let restored: WorldDraft = serde_json::from_str(&raw).unwrap();
            assert_eq!(World::new(restored).unwrap().snapshot(), &after);
            // Extension cannot detach an occupied endpoint.
            let origin = point(139.1);
            let out = point(139.15);
            assert!(w.extend_alignment(c, a, ae, vec![origin, out]).is_err());
            assert_eq!(w.snapshot(), &after);
        }
    }
}
#[test]
fn gap_cusp_foreign_owner_and_self_are_atomic_errors() {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let other = w.add_company(p, "Other".into()).unwrap();
    let a = w
        .build_alignment(c, "A".into(), vec![point(139.), point(139.1)])
        .unwrap();
    let gap = w
        .build_alignment(c, "Gap".into(), vec![point(139.11), point(139.2)])
        .unwrap();
    let cusp = w
        .build_alignment(c, "Cusp".into(), vec![point(139.1), point(139.05)])
        .unwrap();
    let foreign = w
        .build_alignment(other, "Foreign".into(), vec![point(139.1), point(139.2)])
        .unwrap();
    let before = w.snapshot().clone();
    for b in [gap, cusp, foreign, a] {
        assert!(w
            .connect_alignment_endpoints(
                c,
                end(a, AlignmentEndpoint::End),
                end(b, AlignmentEndpoint::Start)
            )
            .is_err());
        assert_eq!(w.snapshot(), &before);
    }
}
