use rosenfu_core::world::*;
#[test]
fn source_rebinding_failure_and_stale_expected_path_preserve_entire_world() {
    let (mut e, line, [a, b, c]) = super::tests::alignment_fixture();
    let span = |from, to| super::tests::span(line, from, to);
    let r = e
        .save_planned_route(None, "R".into(), a, vec![span(a, b)], vec![])
        .unwrap()
        .routes[0]
        .id;
    let other = e
        .save_planned_route(None, "Other".into(), b, vec![span(b, c)], vec![])
        .unwrap()
        .routes[1]
        .id;
    let segments = vec![span(a, b)];
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
    let new_segments = vec![span(b, c)];
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
