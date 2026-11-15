mod common;
use common::*;
use rosenfu_core::world::*;

#[test]
fn service_traverses_both_directions_and_can_skip_intermediate_stations() {
    let (mut w, c, a, s) = line(3);
    let next = w.snapshot().next_id;
    let seg = vec![segment(a, s[0], s[2])];
    let refs = stops(&w, s[0], &seg);
    let id = w
        .add_service_path(
            c,
            "Express".into(),
            s[0],
            seg,
            vec![refs[0].clone(), refs[2].clone()],
        )
        .unwrap();
    assert_eq!(id, ServiceId(next));
    assert_eq!(w.snapshot().next_id, next + 1);
    assert_eq!(w.snapshot().service_paths[0].stops.len(), 2);
    service(&mut w, c, s[2], vec![segment(a, s[2], s[0])]);
}
#[test]
fn repeated_station_occurrences_have_independent_stop_references() {
    let (mut w, c, a, s) = line(3);
    let seg = vec![
        segment(a, s[0], s[1]),
        segment(a, s[1], s[0]),
        segment(a, s[0], s[2]),
    ];
    let refs = stops(&w, s[0], &seg);
    assert_ne!(refs[1], refs[3]);
    let selected = vec![refs[0].clone(), refs[3].clone(), refs[4].clone()];
    w.add_service_path(c, "Loop".into(), s[0], seg, selected.clone())
        .unwrap();
    assert_eq!(w.snapshot().service_paths[0].stops, selected);
}
#[test]
fn invalid_stop_positions_never_mutate_or_allocate() {
    let (mut w, c, a, s) = line(3);
    let seg = vec![segment(a, s[0], s[2])];
    let r = stops(&w, s[0], &seg);
    let before = w.clone();
    for indexes in [
        vec![],
        vec![0],
        vec![2],
        vec![1, 2],
        vec![0, 1],
        vec![0, 2, 1, 2],
        vec![0, 1, 1, 2],
    ] {
        assert!(w
            .add_service_path(
                c,
                "Bad".into(),
                s[0],
                seg.clone(),
                indexes.into_iter().map(|i| r[i].clone()).collect()
            )
            .is_err());
        assert_eq!(w, before);
    }
    assert!(w
        .add_service_path(
            c,
            "Bad".into(),
            s[0],
            seg,
            vec![
                StationOccurrenceRef::Start,
                StationOccurrenceRef::OnLeg {
                    leg_key: 999,
                    station_id: s[2]
                }
            ]
        )
        .is_err());
    assert_eq!(w, before);
}
#[test]
fn missing_references_disconnected_empty_and_unnamed_paths_never_mutate() {
    let (mut w, c, a, s) = line(3);
    let before = w.clone();
    let good = vec![segment(a, s[0], s[2])];
    let r = stops(&w, s[0], &good);
    for (owner, start, segs, name) in [
        (CompanyId(999), s[0], good.clone(), "Bad"),
        (c, StationId(999), good.clone(), "Bad"),
        (c, s[0], vec![segment(AlignmentId(999), s[0], s[2])], "Bad"),
        (c, s[0], vec![segment(a, s[1], s[2])], "Bad"),
        (c, s[0], vec![], "Bad"),
        (c, s[0], good, " "),
    ] {
        assert!(w
            .add_service_path(owner, name.into(), start, segs, r.clone())
            .is_err());
        assert_eq!(w, before);
    }
}
#[test]
fn independent_route_changes_or_deletion_preserve_service_and_equipment() {
    let (mut w, c, a, s) = line(3);
    w.add_planned_route(
        c,
        "Route".into(),
        s[0],
        vec![segment(a, s[0], s[2])],
        s.clone(),
    )
    .unwrap();
    service(&mut w, c, s[0], vec![segment(a, s[0], s[2])]);
    let before = w.snapshot().clone();
    let mut d = before.clone();
    d.routes[0].name = "Changed".into();
    d.route_paths[0].segments = vec![segment(a, s[0], s[1])];
    d.route_paths[0].member_stations = s[..2].to_vec();
    w.replace(d).unwrap();
    let mut d = w.snapshot().clone();
    d.routes.clear();
    d.route_paths.clear();
    w.replace(d).unwrap();
    assert_eq!(w.snapshot().services, before.services);
    assert_eq!(w.snapshot().service_paths, before.service_paths);
    assert_eq!(w.snapshot().alignments, before.alignments);
}
#[test]
fn service_references_protect_equipment_and_edit_failures_are_atomic() {
    let (mut w, c, a, s) = line(3);
    let id = service(&mut w, c, s[0], vec![segment(a, s[0], s[2])]);
    let before = w.clone();
    let mut d = w.snapshot().clone();
    d.alignments.clear();
    assert!(w.replace(d).is_err());
    assert_eq!(w, before);
    let mut d = w.snapshot().clone();
    d.service_paths[0].stops.pop();
    assert!(w.replace(d).is_err());
    assert_eq!(w, before);
    let refs = w.snapshot().service_paths[0].stops.clone();
    w.update_service_path(
        c,
        id,
        "Express".into(),
        vec![refs[0].clone(), refs[2].clone()],
    )
    .unwrap();
    assert_eq!(w.snapshot().services[0].id, id);
    assert_eq!(w.snapshot().next_id, before.snapshot().next_id);
}
#[test]
fn ids_counters_and_existing_only_replacement_include_services() {
    let (mut w, c, a, s) = line(3);
    service(&mut w, c, s[0], vec![segment(a, s[0], s[1])]);
    let before = w.snapshot().clone();
    let mut d = before.clone();
    d.services[0].id = ServiceId(0);
    assert_eq!(d.validate(), Err(WorldError::InvalidId("service")));
    let mut d = before.clone();
    d.services.push(d.services[0].clone());
    assert_eq!(d.validate(), Err(WorldError::InvalidId("service")));
    let mut d = before.clone();
    d.next_id = d.services[0].id.0;
    assert_eq!(d.validate(), Err(WorldError::InvalidCounter));
    let mut d = before.clone();
    d.services.clear();
    d.service_paths.clear();
    w.replace(d).unwrap();
    assert_eq!(w.replace(before.clone()), Err(WorldError::UnallocatedId));
    let id = service(&mut w, c, s[1], vec![segment(a, s[1], s[0])]);
    assert_eq!(id.0, before.next_id);
}
#[test]
fn service_owner_validation_is_not_equipment_usage_authorization() {
    let (mut w, _, a, s) = line(3);
    let other = w
        .add_company(w.snapshot().players[0].id, "Other".into())
        .unwrap();
    service(&mut w, other, s[0], vec![segment(a, s[0], s[1])]);
}
#[test]
fn serialization_preserves_identity_path_and_occurrences() {
    let (mut w, c, a, s) = line(3);
    service(
        &mut w,
        c,
        s[0],
        vec![segment(a, s[0], s[1]), segment(a, s[1], s[0])],
    );
    let json = serde_json::to_value(w.snapshot()).unwrap();
    assert!(json["services"][0]["id"].is_string());
    assert!(json["service_paths"][0]["stops"][1]["leg_key"].is_string());
    assert_eq!(
        World::new(serde_json::from_value(json).unwrap()).unwrap(),
        w
    );
    assert_eq!(
        serde_json::to_string(&ServiceId(9_007_199_254_740_993)).unwrap(),
        "\"9007199254740993\""
    );
}
#[test]
fn required_fields_invalid_references_and_legacy_fields_are_rejected() {
    let (mut w, c, a, s) = line(3);
    service(&mut w, c, s[0], vec![segment(a, s[0], s[2])]);
    w.add_planned_route(c, "R".into(), s[0], vec![segment(a, s[0], s[2])], s)
        .unwrap();
    let original = serde_json::to_value(w.snapshot()).unwrap();
    let mut missing = original.clone();
    missing.as_object_mut().unwrap().remove("services");
    assert!(serde_json::from_value::<WorldDraft>(missing).is_err());
    for field in ["tracks", "future_feature"] {
        let mut d = original.clone();
        d[field] = serde_json::json!([]);
        assert!(serde_json::from_value::<WorldDraft>(d).is_err());
    }
    for collection in [
        "players",
        "companies",
        "stations",
        "alignments",
        "routes",
        "services",
        "route_paths",
        "service_paths",
    ] {
        let mut d = original.clone();
        d[collection][0]["future_feature"] = serde_json::json!(true);
        assert!(
            serde_json::from_value::<WorldDraft>(d).is_err(),
            "{collection}"
        );
    }
    for invalid in [
        serde_json::json!({"kind":"on_leg","leg_key":"0","station_id":"1"}),
        serde_json::json!({"kind":"on_leg","leg_key":1,"station_id":"1"}),
        serde_json::json!({"kind":"unknown"}),
    ] {
        let mut d = original.clone();
        d["service_paths"][0]["stops"][0] = invalid;
        let parsed = serde_json::from_value::<WorldDraft>(d);
        assert!(parsed.is_err() || parsed.unwrap().validate().is_err());
    }
}
