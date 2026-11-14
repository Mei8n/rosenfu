use rosenfu_core::world::*;
use rosenfu_storage::{decode, encode};
#[test]
fn connection_and_corridor_route_roundtrip_without_id_changes() {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let point = |x| GeoPoint {
        latitude: 35.,
        longitude: x,
    };
    let a = w
        .build_alignment(c, "A".into(), vec![point(139.), point(139.1)])
        .unwrap();
    let b = w
        .build_alignment(c, "B".into(), vec![point(139.1), point(139.2)])
        .unwrap();
    let s = w
        .place_station_on_alignment(c, "S".into(), a, 0.5, 100., 10.)
        .unwrap();
    let t = w
        .place_station_on_alignment(c, "T".into(), b, 0.5, 100., 10.)
        .unwrap();
    w.connect_alignment_endpoints(
        c,
        AlignmentEndpointRef {
            alignment_id: a,
            endpoint: AlignmentEndpoint::End,
        },
        AlignmentEndpointRef {
            alignment_id: b,
            endpoint: AlignmentEndpoint::Start,
        },
    )
    .unwrap();
    let candidate = w.route_candidates(s, t).unwrap().candidates.remove(0);
    w.add_planned_route(c, "R".into(), s, candidate.segments, vec![s, t])
        .unwrap();
    assert_eq!(decode(&encode(&w).unwrap()).unwrap(), w);
}
