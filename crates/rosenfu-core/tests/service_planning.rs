use rosenfu_core::world::*;
fn sample() -> (World, CompanyId, Vec<StationId>, Vec<TrackId>) {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let s = (0..3)
        .map(|i| {
            w.add_station(c, format!("S{i}"), 35.0, 139.0 + f64::from(i) * 0.01)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let t = vec![
        w.add_track(c, s[0], s[1]).unwrap(),
        w.add_track(c, s[1], s[2]).unwrap(),
    ];
    (w, c, s, t)
}
#[test]
fn forward_reverse_partial_and_repeated_occurrences_are_explicit() {
    let (mut w, c, s, t) = sample();
    let r = w
        .add_route(c, "R".into(), s[0], vec![t[0], t[1], t[1], t[0]])
        .unwrap();
    let before = w.clone();
    let p = w.preview_service_from_route(r, 4, 1).unwrap();
    assert_eq!(p.tracks, vec![t[0], t[1], t[1]]);
    assert_eq!(
        p.stations
            .iter()
            .map(|s| s.route_occurrence)
            .collect::<Vec<_>>(),
        vec![4, 3, 2, 1]
    );
    assert_eq!(
        p.stations.iter().map(|s| s.occurrence).collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
    assert_eq!(p.start, s[0]);
    assert!(p.requires_stop_selection);
    assert_eq!(w.preview_service_from_route(r, 0, 2).unwrap().tracks, t);
    assert_eq!(
        w.preview_service_from_route(r, 1, 2).unwrap().tracks,
        vec![t[1]]
    );
    assert_eq!(w, before);
}
#[test]
fn exclusions_are_membership_not_stop_decisions() {
    let (mut w, c, s, t) = sample();
    let r = w
        .add_planned_route(
            c,
            "R".into(),
            s[0],
            t.iter()
                .map(|id| RouteSegment::Track { track_id: *id })
                .collect(),
            vec![s[0], s[2]],
        )
        .unwrap();
    let before = w.clone();
    let p = w.preview_service_from_route(r, 0, 2).unwrap();
    assert_eq!(
        p.stations
            .iter()
            .map(|s| s.route_member)
            .collect::<Vec<_>>(),
        vec![true, false, true]
    );
    assert!(p.requires_stop_selection);
    let json = serde_json::to_value(p).unwrap();
    assert!(json.get("stops").is_none());
    assert_eq!(w, before);
}
#[test]
fn invalid_inputs_are_read_only() {
    let (mut w, c, s, t) = sample();
    let r = w.add_route(c, "R".into(), s[0], t).unwrap();
    let before = w.clone();
    assert_eq!(
        w.preview_service_from_route(RouteId(999), 0, 1),
        Err(ServicePlanningError::MissingRoute)
    );
    assert_eq!(
        w.preview_service_from_route(r, 0, 0),
        Err(ServicePlanningError::EmptySection)
    );
    assert_eq!(
        w.preview_service_from_route(r, 0, u32::MAX),
        Err(ServicePlanningError::InvalidOccurrence)
    );
    assert_eq!(w, before);
}
#[test]
fn alignment_sections_are_unsupported_but_track_only_subsection_is_available() {
    let (mut w, c, s, t) = sample();
    let a = w
        .build_alignment(
            c,
            "A".into(),
            vec![
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.1,
                },
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.2,
                },
            ],
        )
        .unwrap();
    let x = w
        .place_station_on_alignment(c, "X".into(), a, 0.1, 100.0, 10.0)
        .unwrap();
    let y = w
        .place_station_on_alignment(c, "Y".into(), a, 0.9, 100.0, 10.0)
        .unwrap();
    let connector = w.add_track(c, s[2], x).unwrap();
    let r = w
        .add_planned_route(
            c,
            "R".into(),
            s[0],
            vec![
                RouteSegment::Track { track_id: t[0] },
                RouteSegment::Track { track_id: t[1] },
                RouteSegment::Track {
                    track_id: connector,
                },
                RouteSegment::Alignment {
                    alignment_id: a,
                    from: x,
                    to: y,
                },
            ],
            vec![s[0], y],
        )
        .unwrap();
    let before = w.clone();
    assert_eq!(
        w.preview_service_from_route(r, 0, 4),
        Err(ServicePlanningError::AlignmentUnsupported)
    );
    assert_eq!(
        w.preview_service_from_route(r, 4, 2),
        Err(ServicePlanningError::AlignmentUnsupported)
    );
    assert_eq!(w.preview_service_from_route(r, 0, 2).unwrap().tracks, t);
    assert_eq!(w, before);
}

#[test]
fn mixed_alignment_prefix_preserves_track_subsection_occurrence_offsets() {
    let (mut w, c, s, t) = sample();
    let alignment = w
        .build_alignment(
            c,
            "Prefix".into(),
            vec![
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.1,
                },
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.2,
                },
            ],
        )
        .unwrap();
    let a = w
        .place_station_on_alignment(c, "A".into(), alignment, 0.1, 100.0, 10.0)
        .unwrap();
    let middle = w
        .place_station_on_alignment(c, "Middle".into(), alignment, 0.5, 100.0, 10.0)
        .unwrap();
    let b = w
        .place_station_on_alignment(c, "B".into(), alignment, 0.9, 100.0, 10.0)
        .unwrap();
    let connector = w.add_track(c, b, s[0]).unwrap();
    let r = w
        .add_planned_route(
            c,
            "Mixed".into(),
            a,
            vec![
                RouteSegment::Alignment {
                    alignment_id: alignment,
                    from: a,
                    to: b,
                },
                RouteSegment::Track {
                    track_id: connector,
                },
                RouteSegment::Track { track_id: t[0] },
                RouteSegment::Track { track_id: t[1] },
            ],
            vec![a, middle, b, s[2]],
        )
        .unwrap();
    let before = w.clone();
    let forward = w.preview_service_from_route(r, 2, 5).unwrap();
    assert_eq!(forward.tracks, vec![connector, t[0], t[1]]);
    assert_eq!(
        forward
            .stations
            .iter()
            .map(|x| x.route_occurrence)
            .collect::<Vec<_>>(),
        vec![2, 3, 4, 5]
    );
    let reverse = w.preview_service_from_route(r, 5, 2).unwrap();
    assert_eq!(reverse.tracks, vec![t[1], t[0], connector]);
    assert_eq!(
        reverse
            .stations
            .iter()
            .map(|x| (
                x.station_id,
                x.occurrence,
                x.route_occurrence,
                x.route_member
            ))
            .collect::<Vec<_>>(),
        vec![
            (s[2], 0, 5, true),
            (s[1], 1, 4, false),
            (s[0], 2, 3, false),
            (b, 3, 2, true)
        ]
    );
    assert!(reverse.requires_stop_selection);
    assert_eq!(w, before);
}
