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
fn pair(
    na: u8,
    nb: u8,
    ae: AlignmentEndpoint,
    be: AlignmentEndpoint,
) -> (World, CompanyId, AlignmentEndpointRef, AlignmentEndpointRef) {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let mut ap = vec![point(139.), point(139.1)];
    let mut bp = vec![point(139.1), point(139.2)];
    if ae == AlignmentEndpoint::Start {
        ap.reverse()
    };
    if be == AlignmentEndpoint::End {
        bp.reverse()
    };
    let a = w.build_alignment(c, "A".into(), ap).unwrap();
    let b = w.build_alignment(c, "B".into(), bp).unwrap();
    let mut d = w.snapshot().clone();
    d.alignments
        .iter_mut()
        .find(|x| x.id == a)
        .unwrap()
        .infrastructure = vec![SectionInfrastructure {
        track_count: na,
        level: InfrastructureLevel::Ground,
    }];
    d.alignments
        .iter_mut()
        .find(|x| x.id == b)
        .unwrap()
        .infrastructure = vec![SectionInfrastructure {
        track_count: nb,
        level: InfrastructureLevel::Ground,
    }];
    (World::new(d).unwrap(), c, end(a, ae), end(b, be))
}
#[test]
fn y_points_have_two_bidirectional_paths_and_one_shared_resource() {
    for ae in [AlignmentEndpoint::Start, AlignmentEndpoint::End] {
        for be in [AlignmentEndpoint::Start, AlignmentEndpoint::End] {
            let (mut w, c, a, b) = pair(1, 2, ae, be);
            assert!(w.alignment_lane_movements().is_empty()); // Coincidence is not connectivity.
            w.connect_alignment_endpoints(c, a, b).unwrap();
            let paths = w.alignment_lane_movements();
            assert_eq!(paths.len(), 4);
            let resource = paths[0].point.unwrap();
            for path in &paths {
                assert_eq!(path.point, Some(resource));
                assert!(paths.iter().any(|r| r.from == path.to && r.to == path.from));
            }
            for lane in 0..2 {
                assert!(paths.iter().any(|p| p.from.endpoint == b
                    && p.from.lane_index == lane
                    && p.to.endpoint == a
                    && p.to.lane_index == 0));
            }
            let raw = serde_json::to_string(w.snapshot()).unwrap();
            let restored = World::new(serde_json::from_str(&raw).unwrap()).unwrap();
            assert_eq!(restored.alignment_lane_movements(), paths);
            let mut reverse = w.snapshot().clone();
            for link in &mut reverse.alignment_connections {
                std::mem::swap(&mut link.a, &mut link.b);
            }
            assert_eq!(
                World::new(reverse).unwrap().alignment_lane_movements(),
                paths
            );
        }
    }
}
#[test]
fn equal_counts_continue_without_crossovers_and_four_to_two_pairs_lanes() {
    for na in [1, 2, 4] {
        for nb in [1, 2, 4] {
            let (mut w, c, a, b) = pair(na, nb, AlignmentEndpoint::End, AlignmentEndpoint::Start);
            w.connect_alignment_endpoints(c, a, b).unwrap();
            let paths = w.alignment_lane_movements();
            assert_eq!(paths.len(), usize::from(na.max(nb)) * 2);
            for p in paths {
                assert_eq!(p.point.is_some(), na != nb);
                if na == nb {
                    assert_eq!(p.from.lane_index, p.to.lane_index);
                }
            }
        }
    }
}
#[test]
fn different_levels_never_connect_and_changing_connected_level_is_invalid() {
    let (mut w, c, a, b) = pair(1, 2, AlignmentEndpoint::End, AlignmentEndpoint::Start);
    let before = w.snapshot().clone();
    let mut raised = before.clone();
    raised
        .alignments
        .iter_mut()
        .find(|v| v.id == b.alignment_id)
        .unwrap()
        .infrastructure[0]
        .level = InfrastructureLevel::Elevated;
    let mut raised = World::new(raised).unwrap();
    let old = raised.snapshot().clone();
    assert_eq!(
        raised.connect_alignment_endpoints(c, a, b),
        Err(WorldError::InvalidAlignmentConnection)
    );
    assert_eq!(raised.snapshot(), &old);
    w.connect_alignment_endpoints(c, a, b).unwrap();
    let mut d = w.snapshot().clone();
    d.alignments
        .iter_mut()
        .find(|v| v.id == b.alignment_id)
        .unwrap()
        .infrastructure[0]
        .level = InfrastructureLevel::Underground;
    assert!(World::new(d).is_err());
    assert_eq!(w.alignment_lane_movements().len(), 4);
    assert_eq!(
        World::new(before).unwrap().alignment_lane_movements().len(),
        0
    ); // Undo snapshot removes derived points.
}

#[test]
fn smooth_ramp_uses_maximum_not_average_grade_and_is_atomic() {
    for kind in [TrackType::Normal, TrackType::HighSpeed, TrackType::Tram] {
        let profile = VerticalProfile {
            start_tangent_m: 0.0,
            end_tangent_m: 0.0,
            start_height_m: 0.,
            end_height_m: 6.,
        };
        let limit = kind.maximum_construction_gradient_per_mille();
        let required = 9000.0 / limit;
        assert!(profile.is_valid(required, kind));
        assert!(!profile.is_valid(required - 0.001, kind));
        assert!(profile.is_valid(required + 0.001, kind));
        assert_eq!(profile.height_at(0.), 0.);
        assert_eq!(profile.height_at(1.), 6.);
        assert_eq!(profile.height_at(0.5), 3.);
        assert!(!profile.is_valid(f64::NAN, kind));
        let (mut w, c, _, _) = pair(1, 1, AlignmentEndpoint::End, AlignmentEndpoint::Start);
        let before = w.snapshot().clone();
        assert!(w
            .build_alignment_ramp(
                c,
                "short".into(),
                vec![point(140.), point(140.0001)],
                kind,
                InfrastructureLevel::Ground,
                InfrastructureLevel::Elevated
            )
            .is_err());
        assert_eq!(w.snapshot(), &before);
        let id = w
            .build_alignment_ramp(
                c,
                "ramp".into(),
                vec![point(140.), point(140.01)],
                kind,
                InfrastructureLevel::Ground,
                InfrastructureLevel::Elevated,
            )
            .unwrap();
        let ramp = w.snapshot().alignments.iter().find(|a| a.id == id).unwrap();
        assert_eq!(ramp.endpoint_height_m(AlignmentEndpoint::Start), 0.);
        assert_eq!(ramp.endpoint_height_m(AlignmentEndpoint::End), 6.);
        assert_eq!(
            ramp.effective_design_speed_kmh(),
            kind.default_design_speed_kmh()
        );
        let saved = serde_json::to_string(w.snapshot()).unwrap();
        assert_eq!(
            World::new(serde_json::from_str(&saved).unwrap())
                .unwrap()
                .snapshot(),
            w.snapshot()
        );
        // Stations require a flat platform section, not a whole sloping profile.
        assert!(w
            .place_station_on_alignment(c, "slope station".into(), id, 0.5, 20., 10.)
            .is_err());
    }
}

#[test]
fn ramp_links_use_both_endpoint_actual_heights_not_the_section_category() {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let low = w
        .build_alignment(c, "low".into(), vec![point(139.), point(139.01)])
        .unwrap();
    let ramp = w
        .build_alignment_ramp(
            c,
            "ramp".into(),
            vec![point(139.01), point(139.02)],
            TrackType::Normal,
            InfrastructureLevel::Ground,
            InfrastructureLevel::Elevated,
        )
        .unwrap();
    let high = w
        .build_alignment(c, "high".into(), vec![point(139.02), point(139.03)])
        .unwrap();
    w.connect_alignment_endpoints(
        c,
        end(low, AlignmentEndpoint::End),
        end(ramp, AlignmentEndpoint::Start),
    )
    .unwrap();
    let before = w.snapshot().clone();
    assert!(w
        .connect_alignment_endpoints(
            c,
            end(ramp, AlignmentEndpoint::End),
            end(high, AlignmentEndpoint::Start)
        )
        .is_err());
    assert_eq!(w.snapshot(), &before);
    let mut d = w.snapshot().clone();
    d.alignments
        .iter_mut()
        .find(|a| a.id == high)
        .unwrap()
        .infrastructure = vec![SectionInfrastructure {
        track_count: 1,
        level: InfrastructureLevel::Elevated,
    }];
    w = World::new(d).unwrap();
    w.connect_alignment_endpoints(
        c,
        end(ramp, AlignmentEndpoint::End),
        end(high, AlignmentEndpoint::Start),
    )
    .unwrap();
}

#[test]
fn explicit_three_port_turnout_reaches_both_branches_without_fake_u_turn() {
    let (mut w, c, a, b) = pair(1, 1, AlignmentEndpoint::End, AlignmentEndpoint::Start);
    let branch = w
        .build_alignment(
            c,
            "branch".into(),
            vec![
                point(139.1),
                point(139.101),
                GeoPoint {
                    latitude: 35.01,
                    longitude: 139.2,
                },
            ],
        )
        .unwrap();
    let branch = end(branch, AlignmentEndpoint::Start);
    let s = w
        .place_station_on_alignment(c, "stem".into(), a.alignment_id, 0.5, 100., 10.)
        .unwrap();
    let t = w
        .place_station_on_alignment(c, "through".into(), b.alignment_id, 0.5, 100., 10.)
        .unwrap();
    let u = w
        .place_station_on_alignment(c, "branch".into(), branch.alignment_id, 0.5, 100., 10.)
        .unwrap();
    w.connect_alignment_endpoints(c, a, b).unwrap();
    w.connect_alignment_endpoints(c, a, branch).unwrap();
    for (from, to) in [(s, t), (t, s), (s, u), (u, s)] {
        let search = w.route_candidates(from, to).unwrap();
        assert_eq!(search.status, RouteSearchStatus::Unique);
        assert!(w
            .preview_service_path(from, search.candidates[0].segments.clone())
            .is_ok());
    }
    assert!(w
        .route_candidates(t, u)
        .unwrap()
        .candidates
        .iter()
        .all(|candidate| candidate.stations.contains(&s)));
    let movements = w.alignment_lane_movements();
    assert_eq!(movements.len(), 4);
    assert!(movements
        .iter()
        .all(|m| m.point == movements[0].point && m.point.is_some()));
    let raw = serde_json::to_string(w.snapshot()).unwrap();
    assert_eq!(
        World::new(serde_json::from_str(&raw).unwrap())
            .unwrap()
            .snapshot(),
        w.snapshot()
    );
    let before = w.snapshot().clone();
    assert!(w.connect_alignment_endpoints(c, b, branch).is_err());
    assert_eq!(w.snapshot(), &before);
}

#[test]
fn splitting_existing_anchor_preserves_crossing_route_service_and_station_ids() {
    let mut w = World::default();
    let p = w.add_player("p".into()).unwrap();
    let c = w.add_company(p, "c".into()).unwrap();
    let a = w
        .build_alignment(
            c,
            "A".into(),
            vec![point(139.), point(139.01), point(139.02)],
        )
        .unwrap();
    let mut draft = w.snapshot().clone();
    draft.alignments[0].anchor_indices = Some(vec![0, 1, 2]);
    w.replace(draft).unwrap();
    let s = w
        .place_station_on_alignment(c, "S".into(), a, 0.2, 20., 10.)
        .unwrap();
    let t = w
        .place_station_on_alignment(c, "T".into(), a, 0.8, 20., 10.)
        .unwrap();
    let segments = w
        .route_candidates(s, t)
        .unwrap()
        .candidates
        .remove(0)
        .segments;
    let r = w
        .add_planned_route(c, "R".into(), s, segments.clone(), vec![s, t])
        .unwrap();
    let preview = w.preview_service_path(s, segments.clone()).unwrap();
    let service = w
        .add_service_path(
            c,
            "service".into(),
            s,
            segments,
            preview
                .occurrences
                .iter()
                .map(|o| o.reference.clone())
                .collect(),
        )
        .unwrap();
    w.set_service_route_binding(c, service, ServiceRouteBindingKind::Route { route_id: r })
        .unwrap();
    let before = w.snapshot().clone();
    let suffix = w.split_alignment_at_anchor(c, a, 1).unwrap();
    assert_ne!(a, suffix);
    assert_eq!(w.snapshot().stations, before.stations);
    assert_eq!(w.snapshot().services, before.services);
    assert_eq!(
        w.snapshot().service_paths[0].stops,
        before.service_paths[0].stops
    );
    assert_eq!(
        w.snapshot().service_paths[0].legs[0].key,
        before.service_paths[0].legs[0].key
    );
    assert_eq!(w.snapshot().route_paths[0].member_stations, vec![s, t]);
    let branch = w
        .build_alignment(
            c,
            "branch".into(),
            vec![
                point(139.01),
                point(139.011),
                GeoPoint {
                    latitude: 35.01,
                    longitude: 139.02,
                },
            ],
        )
        .unwrap();
    w.connect_alignment_endpoints(
        c,
        end(a, AlignmentEndpoint::End),
        end(branch, AlignmentEndpoint::Start),
    )
    .unwrap();
    let u = w
        .place_station_on_alignment(c, "U".into(), branch, 0.8, 20., 10.)
        .unwrap();
    assert!(!w.route_candidates(s, u).unwrap().candidates.is_empty());
    assert_eq!(
        w.route_candidates(s, t).unwrap().candidates[0].stations,
        vec![s, t]
    );
    assert!(World::new(w.snapshot().clone()).is_ok());
}

#[test]
fn exact_non_anchor_split_preserves_shape_and_rejects_platform_cut_atomically() {
    let (mut w, c, a, _) = pair(2, 2, AlignmentEndpoint::End, AlignmentEndpoint::Start);
    let station = w
        .place_station_on_alignment(c, "centre".into(), a.alignment_id, 0.5, 100., 10.)
        .unwrap();
    let before = w.snapshot().clone();
    assert!(w
        .split_alignment_at_position(c, a.alignment_id, 0, 0.5)
        .is_err());
    assert_eq!(w.snapshot(), &before);
    let suffix = w
        .split_alignment_at_position(c, a.alignment_id, 0, 0.3)
        .unwrap();
    let prefix = w
        .snapshot()
        .alignments
        .iter()
        .find(|v| v.id == a.alignment_id)
        .unwrap();
    assert_eq!(prefix.points.last().unwrap(), &point(139.03));
    assert_eq!(w.snapshot().stations, before.stations);
    assert_eq!(
        w.snapshot()
            .station_placements
            .iter()
            .find(|p| p.station_id == station)
            .unwrap()
            .alignment_id,
        suffix
    );
    assert_eq!(prefix.infrastructure[0].track_count, 2);
    let saved = serde_json::to_string(w.snapshot()).unwrap();
    assert_eq!(
        World::new(serde_json::from_str(&saved).unwrap())
            .unwrap()
            .snapshot(),
        w.snapshot()
    );
}

#[test]
fn ramp_clip_preserves_height_and_gradient_everywhere() {
    let p = VerticalProfile {
        start_height_m: 0.,
        end_height_m: 6.,
        start_tangent_m: 0.,
        end_tangent_m: 0.,
    };
    let clipped = p.clip(0.2, 0.8).unwrap();
    for i in 0..101 {
        let t = i as f64 / 100.;
        assert!((clipped.height_at(t) - p.height_at(0.2 + t * 0.6)).abs() < 1e-10);
        assert!(
            (clipped.tangent_at(t) / (300. * 0.6) - p.tangent_at(0.2 + t * 0.6) / 300.).abs()
                < 1e-12
        );
    }
    assert!(clipped.is_valid(180., TrackType::HighSpeed));
    assert!(!clipped.is_valid(179.999, TrackType::HighSpeed));
    assert!(!VerticalProfile {
        start_tangent_m: f64::NAN,
        ..p
    }
    .is_valid(1000., TrackType::Normal));
    let clipped_again = clipped.clip(0.25, 0.75).unwrap();
    assert!((clipped_again.height_at(0.5) - p.height_at(0.5)).abs() < 1e-10);
}

#[test]
fn ramp_split_preserves_cubic_and_connection_grade() {
    let mut w = World::default();
    let p = w.add_player("p".into()).unwrap();
    let c = w.add_company(p, "c".into()).unwrap();
    let id = w
        .build_alignment_ramp(
            c,
            "ramp".into(),
            vec![point(139.), point(139.01)],
            TrackType::Normal,
            InfrastructureLevel::Ground,
            InfrastructureLevel::Elevated,
        )
        .unwrap();
    let old = w.snapshot().alignments[0].clone();
    let suffix = w.split_alignment_at_position(c, id, 0, 0.4).unwrap();
    let left = w.snapshot().alignments.iter().find(|a| a.id == id).unwrap();
    let right = w
        .snapshot()
        .alignments
        .iter()
        .find(|a| a.id == suffix)
        .unwrap();
    for i in 0..11 {
        let t = i as f64 / 10.;
        assert!(
            (left.height_at_position_m(t).unwrap() - old.height_at_position_m(t * 0.4).unwrap())
                .abs()
                < 1e-8
        );
        assert!(
            (right.height_at_position_m(t).unwrap()
                - old.height_at_position_m(0.4 + t * 0.6).unwrap())
            .abs()
                < 1e-8
        );
    }
    assert!(
        (left.endpoint_inward_grade(AlignmentEndpoint::End)
            + right.endpoint_inward_grade(AlignmentEndpoint::Start))
        .abs()
            < 1e-9
    );
    assert!(World::new(w.snapshot().clone()).is_ok());
}

#[test]
fn single_double_y_has_distinct_curved_lane_geometry_and_exact_reverse_paths() {
    let (mut w, c, a, b) = pair(1, 2, AlignmentEndpoint::End, AlignmentEndpoint::Start);
    w.connect_alignment_endpoints(c, a, b).unwrap();
    let paths = w.track_movement_geometries().unwrap();
    assert_eq!(paths.len(), 4);
    for pair in paths.chunks_exact(2) {
        assert_eq!(
            pair[0].points,
            pair[1].points.iter().copied().rev().collect::<Vec<_>>()
        );
        assert!(pair[0]
            .points
            .iter()
            .all(|p| p.latitude.is_finite() && p.longitude.is_finite()));
    }
    assert_eq!(paths[0].points[0], paths[2].points[0]);
    assert_ne!(paths[0].points.last(), paths[2].points.last());
    assert_ne!(paths[0].points[12].latitude, paths[2].points[12].latitude);
    let saved = serde_json::to_string(w.snapshot()).unwrap();
    let restored = World::new(serde_json::from_str(&saved).unwrap()).unwrap();
    assert_eq!(paths, restored.track_movement_geometries().unwrap());
}

#[test]
fn rendered_lanes_join_turnouts_exactly_with_shared_four_metre_spacing() {
    let (mut w, c, a, b) = pair(1, 2, AlignmentEndpoint::End, AlignmentEndpoint::Start);
    w.connect_alignment_endpoints(c, a, b).unwrap();
    let geometry = w.track_render_geometry().unwrap();
    assert_eq!(geometry.lanes.len(), 3);
    for path in &geometry.movements {
        let port = path.movement.from;
        let lane = geometry
            .lanes
            .iter()
            .find(|lane| {
                lane.alignment_id == port.endpoint.alignment_id
                    && lane.lane_index == port.lane_index
            })
            .unwrap();
        let join = if port.endpoint.endpoint == AlignmentEndpoint::Start {
            lane.points[0]
        } else {
            *lane.points.last().unwrap()
        };
        assert_eq!(join, path.points[0]);
    }
    assert_ne!(geometry.lanes[1].points, geometry.lanes[2].points);
}

#[test]
fn internal_count_boundaries_have_bidirectional_y_paths_and_exact_lane_joins() {
    for na in [1, 2, 4] {
        for nb in [1, 2, 4] {
            let mut w = World::default();
            let p = w.add_player("p".into()).unwrap();
            let c = w.add_company(p, "c".into()).unwrap();
            let id = w
                .build_alignment(
                    c,
                    "line".into(),
                    vec![point(139.), point(139.01), point(139.02)],
                )
                .unwrap();
            let mut d = w.snapshot().clone();
            let line = &mut d.alignments[0];
            line.anchor_indices = Some(vec![0, 1, 2]);
            line.infrastructure = vec![
                SectionInfrastructure {
                    track_count: na,
                    level: InfrastructureLevel::Ground,
                },
                SectionInfrastructure {
                    track_count: nb,
                    level: InfrastructureLevel::Elevated,
                },
            ];
            // Height changes are a continuous profile, never an implicit step.
            assert!(World::new(d.clone()).is_err());
            d.alignments[0].vertical_profile = Some(VerticalProfile {
                start_height_m: 0.,
                end_height_m: 6.,
                start_tangent_m: 0.,
                end_tangent_m: 0.,
            });
            let w = World::new(d).unwrap();
            let wiring = w.internal_lane_movements();
            assert_eq!(wiring.len(), usize::from(na.max(nb)) * 2);
            let geometry = w.track_render_geometry().unwrap();
            assert_eq!(
                geometry.internal_movements.len(),
                if na == nb { 0 } else { wiring.len() }
            );
            for pair in geometry.internal_movements.chunks_exact(2) {
                assert_eq!(
                    pair[0].points,
                    pair[1].points.iter().copied().rev().collect::<Vec<_>>()
                );
                assert_eq!(pair[0].movement.point, pair[1].movement.point);
            }
            for path in &geometry.internal_movements {
                let port = path.movement.from;
                assert_eq!(port.alignment_id, id);
                let lane = geometry
                    .lanes
                    .iter()
                    .find(|l| {
                        l.alignment_id == id
                            && l.section_index == port.section_index
                            && l.lane_index == port.lane_index
                    })
                    .unwrap();
                let join = if port.side == AlignmentEndpoint::Start {
                    lane.points[0]
                } else {
                    *lane.points.last().unwrap()
                };
                assert_eq!(join, path.points[0]);
            }
            let saved = serde_json::to_string(w.snapshot()).unwrap();
            let restored = World::new(serde_json::from_str(&saved).unwrap()).unwrap();
            assert_eq!(restored.track_render_geometry().unwrap(), geometry);
        }
    }
}

#[test]
fn insufficient_y_approach_is_rejected_before_connection_and_preserves_snapshot() {
    let (mut w, c, a, b) = pair(1, 2, AlignmentEndpoint::End, AlignmentEndpoint::Start);
    let mut draft = w.snapshot().clone();
    // Last double-track section is too short for a one-spacing approach.
    let line = draft
        .alignments
        .iter_mut()
        .find(|l| l.id == b.alignment_id)
        .unwrap();
    let end = line.points[1];
    line.points.insert(1, point(139.10001));
    line.anchor_indices = Some(vec![0, 1, 2]);
    line.infrastructure = vec![
        SectionInfrastructure {
            track_count: 2,
            level: InfrastructureLevel::Ground
        };
        2
    ];
    assert_eq!(line.points[2], end);
    w = World::new(draft).unwrap();
    let before = w.snapshot().clone();
    assert_eq!(
        w.connect_alignment_endpoints(c, a, b),
        Err(WorldError::InsufficientTurnoutSpace)
    );
    assert_eq!(w.snapshot(), &before);
    assert!(WorldError::InsufficientTurnoutSpace
        .to_string()
        .contains("必要な空間"));
}

#[test]
fn consecutive_internal_y_trims_leave_each_middle_section_visible() {
    let mut w = World::default();
    let p = w.add_player("p".into()).unwrap();
    let c = w.add_company(p, "c".into()).unwrap();
    w.build_alignment(
        c,
        "four sections".into(),
        (0..5).map(|i| point(139. + i as f64 * 0.001)).collect(),
    )
    .unwrap();
    let mut d = w.snapshot().clone();
    d.alignments[0].anchor_indices = Some(vec![0, 1, 2, 3, 4]);
    d.alignments[0].infrastructure = [1, 2, 4, 1]
        .into_iter()
        .map(|track_count| SectionInfrastructure {
            track_count,
            level: InfrastructureLevel::Ground,
        })
        .collect();
    let w = World::new(d).unwrap();
    let render = w.track_render_geometry().unwrap();
    assert_eq!(render.lanes.len(), 8);
    assert!(
        render
            .lanes
            .iter()
            .all(|lane| lane.points.first().unwrap().longitude
                < lane.points.last().unwrap().longitude)
    );
}
