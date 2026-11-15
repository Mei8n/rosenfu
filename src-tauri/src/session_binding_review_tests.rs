use super::WorldEditor;
use rosenfu_core::world::*;
#[test]
fn source_rebinding_failure_and_stale_expected_path_preserve_entire_world() {
    let mut e = WorldEditor::new();
    let a = e.add_station("A".into(), 35., 139.).unwrap().stations[0].id;
    let b = e.add_station("B".into(), 35., 139.01).unwrap().stations[1].id;
    let c = e.add_station("C".into(), 35., 139.02).unwrap().stations[2].id;
    let ab = e.add_track(a, b).unwrap().tracks[0].id;
    let bc = e.add_track(b, c).unwrap().tracks[1].id;
    let r = e.save_route(None, "R".into(), a, vec![ab]).unwrap().routes[0].id;
    let other = e
        .save_route(None, "Other".into(), b, vec![bc])
        .unwrap()
        .routes[1]
        .id;
    let segments = vec![RouteSegment::Track { track_id: ab }];
    let preview = e.preview_service_path(a, segments.clone()).unwrap();
    let stops = preview
        .occurrences
        .iter()
        .map(|o| o.reference.clone())
        .collect();
    let initial = e
        .create_service_path_with_source_route("S".into(), a, segments.clone(), stops, Some(r))
        .unwrap();
    let id = initial.services[0].id;
    let expected = initial.service_paths[0].clone();
    let new_segments = vec![RouteSegment::Track { track_id: bc }];
    let preview = e
        .preview_service_path_replacement(id, b, new_segments.clone())
        .unwrap();
    let stops = preview
        .occurrences
        .iter()
        .map(|o| o.reference.clone())
        .collect::<Vec<_>>();
    let mut stale = expected.clone();
    stale.next_leg_key += 1;
    assert!(e
        .replace_service_path_with_source_route(
            id,
            stale,
            "S".into(),
            b,
            new_segments.clone(),
            stops.clone(),
            Some(other)
        )
        .is_err());
    assert_eq!(e.snapshot(), initial);
    assert!(e
        .replace_service_path_with_source_route(
            id,
            expected.clone(),
            "S".into(),
            b,
            new_segments.clone(),
            stops.clone(),
            Some(r)
        )
        .is_err());
    assert_eq!(e.snapshot(), initial);
    let updated = e
        .replace_service_path_with_source_route(
            id,
            expected,
            "S".into(),
            b,
            new_segments,
            stops,
            Some(other),
        )
        .unwrap();
    assert_eq!(
        updated.service_route_bindings[0].binding,
        ServiceRouteBindingKind::Route { route_id: other }
    );
    assert_eq!(updated.services[0].id, id);
}
