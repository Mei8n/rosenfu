use rosenfu_core::world::*;
fn point(x: f64, y: f64) -> GeoPoint {
    GeoPoint {
        latitude: 35. + y / 111_000.,
        longitude: 139. + x / 91_000.,
    }
}
fn line(
    world: &mut World,
    c: CompanyId,
    points: Vec<GeoPoint>,
    name: &str,
    length: f64,
) -> StationId {
    let id = world.build_alignment(c, "線路".into(), points).unwrap();
    world
        .place_station_on_alignment(c, name.into(), id, 0.5, length, 12.)
        .unwrap()
}
#[test]
fn crossing_platforms_share_identity_and_name_without_changing_routes() {
    let mut world = World::default();
    let p = world.add_player("P".into()).unwrap();
    let c = world.add_company(p, "C".into()).unwrap();
    let a = line(
        &mut world,
        c,
        vec![point(-500., 0.), point(500., 0.)],
        "中央",
        100.,
    );
    let before = world.snapshot().clone();
    let b = line(
        &mut world,
        c,
        vec![point(0., -500.), point(0., 500.)],
        "仮名",
        100.,
    );
    assert_eq!(
        world.snapshot().station_groups,
        vec![StationGroup {
            station_id: a,
            members: vec![a, b]
        }]
    );
    assert_eq!(world.snapshot().station_group_id(b), a);
    assert_eq!(world.snapshot().stations[0], before.stations[0]);
    assert_eq!(world.snapshot().stations[1].name, "中央");
    assert_eq!(world.snapshot().routes, before.routes);
    assert_eq!(world.snapshot().service_paths, before.service_paths);
    let mut draft = world.snapshot().clone();
    draft.rename_station_group(b, "共通名".into()).unwrap();
    world.replace(draft).unwrap();
    assert!(world.snapshot().stations.iter().all(|s| s.name == "共通名"));
}
#[test]
fn bridge_merges_existing_groups_but_nearby_and_other_owner_do_not_merge() {
    let mut world = World::default();
    let p = world.add_player("P".into()).unwrap();
    let c = world.add_company(p, "C".into()).unwrap();
    let a = line(
        &mut world,
        c,
        vec![point(-500., 0.), point(500., 0.)],
        "A",
        100.,
    );
    let b = line(
        &mut world,
        c,
        vec![point(0., -500.), point(0., 500.)],
        "B",
        100.,
    );
    let d = line(
        &mut world,
        c,
        vec![point(-500., 150.), point(500., 150.)],
        "D",
        100.,
    );
    let e = line(
        &mut world,
        c,
        vec![point(0., -350.), point(0., 650.)],
        "E",
        100.,
    );
    assert_eq!(world.snapshot().station_groups.len(), 2);
    let f = line(
        &mut world,
        c,
        vec![point(0., -500.), point(0., 650.)],
        "F",
        300.,
    );
    assert_eq!(
        world.snapshot().station_groups,
        vec![StationGroup {
            station_id: a,
            members: vec![a, b, d, e, f]
        }]
    );
    let nearby = line(
        &mut world,
        c,
        vec![point(-500., 300.), point(500., 300.)],
        "別駅",
        100.,
    );
    assert_eq!(world.snapshot().station_group_id(nearby), nearby);
    let other = world.add_company(p, "他社".into()).unwrap();
    let foreign = line(
        &mut world,
        other,
        vec![point(-500., 0.), point(500., 0.)],
        "他社駅",
        100.,
    );
    assert_eq!(world.snapshot().station_group_id(foreign), foreign);
}
#[test]
fn malformed_groups_rejected_without_breaking_old_ungrouped_worlds() {
    let mut world = World::default();
    let p = world.add_player("P".into()).unwrap();
    let c = world.add_company(p, "C".into()).unwrap();
    let a = line(
        &mut world,
        c,
        vec![point(-500., 0.), point(500., 0.)],
        "駅",
        100.,
    );
    let b = line(
        &mut world,
        c,
        vec![point(0., -500.), point(0., 500.)],
        "駅",
        100.,
    );
    let original = world.snapshot().clone();
    for members in [vec![a], vec![b, a], vec![a, a], vec![a, StationId(999)]] {
        let mut bad = original.clone();
        bad.station_groups[0].members = members;
        assert!(World::new(bad).is_err());
    }
    let mut bad = original.clone();
    bad.station_groups.push(bad.station_groups[0].clone());
    assert!(World::new(bad).is_err());
    let mut bad = original.clone();
    bad.stations[1].name = "別名".into();
    assert!(World::new(bad).is_err());
    let mut legacy = original;
    legacy.station_groups.clear();
    legacy.stations[1].name = "別名".into();
    assert!(World::new(legacy).is_ok());
}
#[test]
fn grouping_keeps_existing_route_service_platform_references_and_occurrences() {
    let mut world = World::default();
    let p = world.add_player("P".into()).unwrap();
    let c = world.add_company(p, "C".into()).unwrap();
    let a = line(
        &mut world,
        c,
        vec![point(-500., 0.), point(500., 0.)],
        "中央",
        100.,
    );
    let alignment = world.snapshot().station_placements[0].alignment_id;
    let end = world
        .place_station_on_alignment(c, "終点".into(), alignment, 0.85, 100., 12.)
        .unwrap();
    let segments = vec![RouteSegment::Alignment {
        alignment_id: alignment,
        from: a,
        to: end,
    }];
    let route = world
        .add_planned_route(c, "路線".into(), a, segments.clone(), vec![a, end])
        .unwrap();
    let service = world
        .add_service_path(
            c,
            "系統".into(),
            a,
            segments,
            vec![
                StationOccurrenceRef::Start,
                StationOccurrenceRef::OnLeg {
                    leg_key: 1,
                    station_id: end,
                },
            ],
        )
        .unwrap();
    let before = world.snapshot().clone();
    let occurrences = world.service_path_occurrences(service).unwrap();
    let b = line(
        &mut world,
        c,
        vec![point(0., -500.), point(0., 500.)],
        "仮名",
        100.,
    );
    assert_eq!(world.snapshot().station_group_id(b), a);
    assert_eq!(world.snapshot().routes, before.routes);
    assert_eq!(world.snapshot().route_paths, before.route_paths);
    assert_eq!(world.snapshot().services, before.services);
    assert_eq!(world.snapshot().service_paths, before.service_paths);
    assert_eq!(world.route_path_stations(route).unwrap(), vec![a, end]);
    assert_eq!(
        world.service_path_occurrences(service).unwrap(),
        occurrences
    );
    assert_eq!(
        world.snapshot().station_placements[..2],
        before.station_placements
    );

    // Replacement still uses the physical platform (not the shared group ID).
    let b_alignment = world
        .snapshot()
        .station_placements
        .iter()
        .find(|p| p.station_id == b)
        .unwrap()
        .alignment_id;
    let b_end = world
        .place_station_on_alignment(c, "別線終点".into(), b_alignment, 0.85, 100., 12.)
        .unwrap();
    let replacement = vec![RouteSegment::Alignment {
        alignment_id: b_alignment,
        from: b,
        to: b_end,
    }];
    let expected = world.snapshot().service_paths[0].clone();
    let groups = world.snapshot().station_groups.clone();
    let preview = world
        .preview_service_path_replacement(c, service, b, replacement.clone())
        .unwrap();
    assert_eq!(preview.occurrences[0].station_id, b);
    assert_ne!(b, world.snapshot().station_group_id(b));
    let stops = preview
        .occurrences
        .into_iter()
        .map(|o| o.reference)
        .collect();
    world
        .replace_service_path(
            c,
            service,
            expected,
            "別線系統".into(),
            b,
            replacement,
            stops,
        )
        .unwrap();
    assert_eq!(world.snapshot().services[0].start, b);
    assert_eq!(world.snapshot().station_groups, groups);
    assert_eq!(
        world.service_path_occurrences(service).unwrap()[0].station_id,
        b
    );
    assert_eq!(World::new(world.snapshot().clone()).unwrap(), world);
}

#[test]
fn different_height_crossing_platforms_group_without_physical_track_connection() {
    for level in [
        InfrastructureLevel::Elevated,
        InfrastructureLevel::Elevated3,
        InfrastructureLevel::Underground,
        InfrastructureLevel::Underground3,
    ] {
        let mut world = World::default();
        let player = world.add_player("P".into()).unwrap();
        let company = world.add_company(player, "C".into()).unwrap();
        let ground_station = line(
            &mut world,
            company,
            vec![point(-500., 0.), point(500., 0.)],
            "中央",
            100.,
        );
        let ground_alignment = world.snapshot().station_placements[0].alignment_id;
        let raised = world
            .build_alignment_ramp(
                company,
                "別階層".into(),
                vec![point(0., -500.), point(0., 500.)],
                TrackType::Normal,
                level,
                level,
            )
            .unwrap();
        let before = world.snapshot().clone();
        let raised_station = world
            .place_station_on_alignment(company, "仮名".into(), raised, 0.5, 100., 12.)
            .unwrap();
        let draft = world.snapshot();
        assert_eq!(
            draft.station_groups,
            vec![StationGroup {
                station_id: ground_station,
                members: vec![ground_station, raised_station],
            }]
        );
        assert_eq!(draft.station_group_id(raised_station), ground_station);
        assert!(draft.stations.iter().all(|station| station.name == "中央"));
        let ground = draft
            .alignments
            .iter()
            .find(|a| a.id == ground_alignment)
            .unwrap();
        let upper = draft.alignments.iter().find(|a| a.id == raised).unwrap();
        assert_eq!(ground.height_at_position_m(0.5), Some(0.));
        assert_eq!(upper.height_at_position_m(0.5), Some(level.height_m()));
        assert!(ground.is_flat_over_position(0.45, 0.55));
        assert!(upper.is_flat_over_position(0.45, 0.55));
        assert_eq!(draft.alignments, before.alignments);
        assert!(draft.alignment_connections.is_empty());
        assert!(world.alignment_lane_movements().is_empty());
        assert_eq!(draft.routes, before.routes);
        assert_eq!(draft.service_paths, before.service_paths);
        let saved = serde_json::to_string(draft).unwrap();
        let restored = World::new(serde_json::from_str(&saved).unwrap()).unwrap();
        assert_eq!(restored.snapshot(), draft);
        assert!(restored.snapshot().alignment_connections.is_empty());
        assert!(restored.alignment_lane_movements().is_empty());
    }
}
