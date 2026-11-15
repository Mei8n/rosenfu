#![allow(dead_code)]
use rosenfu_core::world::*;
pub fn line(n: usize) -> (World, CompanyId, AlignmentId, Vec<StationId>) {
    let mut w = World::default();
    let p = w.add_player("Player".into()).unwrap();
    let c = w.add_company(p, "Railway".into()).unwrap();
    let a = w
        .build_alignment(
            c,
            "Line".into(),
            vec![
                GeoPoint {
                    latitude: 35.,
                    longitude: 139.,
                },
                GeoPoint {
                    latitude: 35.,
                    longitude: 139.1,
                },
            ],
        )
        .unwrap();
    let s = (0..n)
        .map(|i| {
            w.place_station_on_alignment(
                c,
                format!("S{i}"),
                a,
                (i + 1) as f64 / (n + 1) as f64,
                20.,
                10.,
            )
            .unwrap()
        })
        .collect();
    (w, c, a, s)
}
pub fn segment(a: AlignmentId, from: StationId, to: StationId) -> RouteSegment {
    RouteSegment::Alignment {
        alignment_id: a,
        from,
        to,
    }
}
pub fn stops(w: &World, start: StationId, segments: &[RouteSegment]) -> Vec<StationOccurrenceRef> {
    w.preview_service_path(start, segments.to_vec())
        .unwrap()
        .occurrences
        .into_iter()
        .map(|o| o.reference)
        .collect()
}
pub fn service(
    w: &mut World,
    c: CompanyId,
    start: StationId,
    segments: Vec<RouteSegment>,
) -> ServiceId {
    let refs = stops(w, start, &segments);
    w.add_service_path(c, "Service".into(), start, segments, refs)
        .unwrap()
}
