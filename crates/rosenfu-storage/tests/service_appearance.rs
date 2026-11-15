use rosenfu_core::world::*;
use rosenfu_storage::{decode, encode, SCHEMA_VERSION};
fn world() -> World {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let line = w
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
    let a = w
        .place_station_on_alignment(c, "A".into(), line, 0.1, 20., 12.)
        .unwrap();
    let b = w
        .place_station_on_alignment(c, "B".into(), line, 0.9, 20., 12.)
        .unwrap();
    let segment = RouteSegment::Alignment {
        alignment_id: line,
        from: a,
        to: b,
    };
    w.add_planned_route(c, "R".into(), a, vec![segment.clone()], vec![a, b])
        .unwrap();
    let preview = w.preview_service_path(a, vec![segment.clone()]).unwrap();
    w.add_service_path(
        c,
        "S".into(),
        a,
        vec![segment],
        preview
            .occurrences
            .into_iter()
            .map(|o| o.reference)
            .collect(),
    )
    .unwrap();
    w
}
#[test]
fn schema19_preserves_color_and_schema18_loads_without_guessing_color() {
    let mut w = world();
    let before = w.snapshot();
    let c = before.companies[0].id;
    let service = before.services[0].id;
    let mut old: serde_json::Value = serde_json::from_slice(&encode(&w).unwrap()).unwrap();
    old["schema_version"] = 18.into();
    assert_eq!(
        decode(&serde_json::to_vec(&old).unwrap())
            .unwrap()
            .snapshot(),
        before
    );
    w.set_service_color(c, service, "#ab12ef".into()).unwrap();
    let encoded = encode(&w).unwrap();
    assert_eq!(SCHEMA_VERSION, 19);
    assert_eq!(decode(&encoded).unwrap().snapshot(), w.snapshot());
    let mut disguised: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    disguised["schema_version"] = 18.into();
    assert!(decode(&serde_json::to_vec(&disguised).unwrap()).is_err());
}
#[test]
fn invalid_duplicate_or_dangling_color_rows_are_rejected() {
    let mut w = world();
    let c = w.snapshot().companies[0].id;
    let s = w.snapshot().services[0].id;
    for color in ["red", "#fff", "#1234567", "#12xx34", "123456"] {
        let before = w.snapshot().clone();
        assert!(w.set_service_color(c, s, color.into()).is_err());
        assert_eq!(w.snapshot(), &before);
    }
    w.set_service_color(c, s, "#123456".into()).unwrap();
    let mut draft = w.snapshot().clone();
    draft
        .service_appearances
        .push(draft.service_appearances[0].clone());
    assert!(World::new(draft).is_err());
    let mut draft = w.snapshot().clone();
    draft.service_appearances[0].service_id = ServiceId(u64::MAX);
    assert!(World::new(draft).is_err());
}
