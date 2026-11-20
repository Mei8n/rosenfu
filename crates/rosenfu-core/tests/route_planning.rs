use rosenfu_core::world::*;
fn setup(n: usize) -> (World, CompanyId, Vec<StationId>) {
    let mut w = World::default();
    let p = w.add_player("p".into()).unwrap();
    let c = w.add_company(p, "c".into()).unwrap();
    let s = (0..n)
        .map(|i| {
            w.add_station(c, format!("S{i}"), 35.0, 139.0 + i as f64 * 0.001)
                .unwrap()
        })
        .collect();
    (w, c, s)
}
#[test]
fn disconnected_invalid_and_same_endpoints_are_distinguished() {
    let (w, _, s) = setup(2);
    assert_eq!(
        w.route_candidates(s[0], s[1]).unwrap().status,
        RouteSearchStatus::NoPath
    );
    assert_eq!(
        w.route_candidates(s[0], s[0]).unwrap_err(),
        RoutePlanningError::SameEndpoints
    );
    assert_eq!(
        w.route_candidates(s[0], StationId(999)).unwrap_err(),
        RoutePlanningError::MissingStation
    );
}
#[test]
fn alignment_candidates_include_intermediate_stations_without_building_tracks() {
    let (mut w, c, _) = setup(0);
    let a = w
        .build_alignment(
            c,
            "a".into(),
            vec![
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.0,
                },
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.1,
                },
            ],
        )
        .unwrap();
    let mut s = vec![];
    for position in [0.1, 0.5, 0.9] {
        s.push(
            w.place_station_on_alignment(c, "s".into(), a, position, 100.0, 10.0)
                .unwrap(),
        );
    }
    let before = w.snapshot().clone();
    let r = w.route_candidates(s[0], s[2]).unwrap();
    assert_eq!(r.status, RouteSearchStatus::Unique);
    assert_eq!(r.candidates[0].stations, s);
    let reverse = w.route_candidates(s[2], s[0]).unwrap();
    let plan = w
        .preview_route_membership(s[2], reverse.candidates[0].segments.clone(), vec![s[1]])
        .unwrap();
    assert_eq!(plan.member_stations, vec![s[2], s[0]]);
    assert_eq!(w.snapshot(), &before);
}
#[test]
fn geometric_crossings_do_not_connect_alignments() {
    let (mut w, c, _) = setup(0);
    let mut s = vec![];
    for name in ["a", "b"] {
        let a = w
            .build_alignment(
                c,
                name.into(),
                vec![
                    GeoPoint {
                        latitude: 35.0,
                        longitude: 139.0,
                    },
                    GeoPoint {
                        latitude: 35.0,
                        longitude: 139.1,
                    },
                ],
            )
            .unwrap();
        for pos in [0.2, 0.8] {
            s.push(
                w.place_station_on_alignment(c, "s".into(), a, pos, 100.0, 10.0)
                    .unwrap(),
            );
        }
    }
    assert_eq!(
        w.route_candidates(s[0], s[3]).unwrap().status,
        RouteSearchStatus::NoPath
    );
}

#[test]
fn coincident_groups_disconnect_both_boundaries_without_affecting_unique_runs() {
    for positions in [
        vec![0.1, 0.2, 0.3, 0.4],
        vec![0.1, 0.1, 0.2, 0.3],
        vec![0.1, 0.2, 0.3, 0.3],
        vec![0.1, 0.2, 0.3, 0.3, 0.4, 0.5],
        vec![0.1, 0.2, 0.3, 0.3, 0.3, 0.4, 0.5],
        vec![0.1, 0.1, 0.1],
    ] {
        let (mut world, company, _) = setup(0);
        let alignment = world
            .build_alignment(
                company,
                "alignment".into(),
                vec![
                    GeoPoint {
                        latitude: 35.0,
                        longitude: 139.0,
                    },
                    GeoPoint {
                        latitude: 35.0,
                        longitude: 139.1,
                    },
                ],
            )
            .unwrap();
        let stations: Vec<_> = positions
            .iter()
            .map(|&position| {
                // Construct a legacy coincident-placement fixture. New station
                // placement rejects overlaps, but old snapshots remain valid.
                let point = world
                    .snapshot()
                    .alignments
                    .iter()
                    .find(|a| a.id == alignment)
                    .unwrap()
                    .point_at(position);
                world
                    .add_station(company, "station".into(), point.latitude, point.longitude)
                    .unwrap()
            })
            .collect();
        // Neither save order nor traversal direction may change connectivity.
        let mut draft = world.snapshot().clone();
        draft.station_placements = stations
            .iter()
            .zip(&positions)
            .map(|(&station_id, &position)| StationPlacement {
                station_id,
                alignment_id: alignment,
                position,
                length_m: 100.0,
                width_m: 10.0,
            })
            .collect();
        draft.station_placements.reverse();
        world.replace(draft).unwrap();
        let before = world.snapshot().clone();
        for from in 0..stations.len() {
            for to in 0..stations.len() {
                if from == to {
                    continue;
                }
                // Reference semantics: each endpoint of every intervening edge
                // must occur exactly once in the complete alignment.
                let connected = positions[from.min(to)..=from.max(to)]
                    .iter()
                    .all(|position| {
                        positions.iter().filter(|other| *other == position).count() == 1
                    });
                let result = world
                    .route_candidates(stations[from], stations[to])
                    .unwrap();
                assert_eq!(
                    result.status,
                    if connected {
                        RouteSearchStatus::Unique
                    } else {
                        RouteSearchStatus::NoPath
                    },
                    "positions={positions:?}, from={from}, to={to}"
                );
                if connected {
                    let mut expected = stations[from.min(to)..=from.max(to)].to_vec();
                    if from > to {
                        expected.reverse();
                    }
                    assert_eq!(result.candidates[0].stations, expected);
                } else {
                    assert!(result.candidates.is_empty());
                }
            }
        }
        assert_eq!(world.snapshot(), &before);
    }
}

#[test]
fn waypoint_validation_rejects_missing_or_repeated_selected_stations() {
    let (w, _, s) = setup(3);
    for via in [vec![s[0]], vec![s[2]], vec![s[1], s[1]]] {
        assert_eq!(
            w.route_candidates_via(s[0], s[2], &via).unwrap_err(),
            RoutePlanningError::RepeatedStation
        );
    }
    assert_eq!(
        w.route_candidates_via(s[0], s[2], &[StationId(999)])
            .unwrap_err(),
        RoutePlanningError::MissingStation
    );
}

#[test]
fn membership_validation_and_stale_alignment_paths_are_atomic() {
    let (mut w, c, _) = setup(0);
    let a = w
        .build_alignment(
            c,
            "a".into(),
            vec![
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.0,
                },
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.1,
                },
            ],
        )
        .unwrap();
    let s: Vec<_> = [0.1, 0.5, 0.9]
        .into_iter()
        .map(|p| {
            w.place_station_on_alignment(c, "s".into(), a, p, 100.0, 10.0)
                .unwrap()
        })
        .collect();
    let path = vec![RouteSegment::Alignment {
        alignment_id: a,
        from: s[0],
        to: s[2],
    }];
    let before = w.snapshot().clone();
    for excluded in [
        vec![s[0]],
        vec![s[2]],
        vec![s[1], s[1]],
        vec![StationId(999)],
    ] {
        assert!(w
            .preview_route_membership(s[0], path.clone(), excluded)
            .is_err());
    }
    let cyclic = vec![
        RouteSegment::Alignment {
            alignment_id: a,
            from: s[0],
            to: s[1],
        },
        RouteSegment::Alignment {
            alignment_id: a,
            from: s[1],
            to: s[0],
        },
    ];
    assert_eq!(
        w.preview_route_membership(s[0], cyclic, vec![])
            .unwrap_err(),
        RoutePlanningError::RepeatedStation
    );
    assert!(w
        .preview_route_membership(
            s[0],
            vec![RouteSegment::Alignment {
                alignment_id: AlignmentId(999),
                from: s[0],
                to: s[2]
            }],
            vec![]
        )
        .is_err());
    let preview = w.preview_route_membership(s[0], path, vec![s[1]]).unwrap();
    assert_eq!(preview.member_stations, vec![s[0], s[2]]);
    assert_eq!(w.snapshot(), &before);
}
