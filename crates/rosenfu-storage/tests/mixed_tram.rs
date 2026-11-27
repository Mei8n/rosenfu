use rosenfu_core::operation::{Timetable, TrainState, Vehicle};
use rosenfu_core::world::{
    AlignmentConnection,
    AlignmentEndpoint::{End, Start},
    AlignmentEndpointRef, AlignmentId, CompanyId, GeoPoint, OperationServiceConfig, RouteSegment,
    ServiceId, StationId, StationOccurrenceRef, TrackType, World, WorldError,
};
use rosenfu_storage::{decode, encode};

fn p(longitude: f64) -> GeoPoint {
    GeoPoint {
        latitude: 35.,
        longitude,
    }
}
fn fixture() -> (
    World,
    CompanyId,
    [AlignmentId; 3],
    [StationId; 4],
    [AlignmentConnection; 2],
) {
    let mut w = World::default();
    let owner = w.add_player("Test".into()).unwrap();
    let c = w.add_company(owner, "Transit".into()).unwrap();
    let ids = [TrackType::Normal, TrackType::Tram, TrackType::HighSpeed]
        .into_iter()
        .enumerate()
        .map(|(i, kind)| {
            let x = 139. + (i as f64 * 0.01);
            // Reuse the exact same numeric endpoint expression across adjacencies.
            w.build_alignment_typed(
                c,
                format!("{kind:?}"),
                vec![p(x), p(139. + ((i + 1) as f64 * 0.01))],
                kind,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let [rail, tram, fast]: [AlignmentId; 3] = ids.try_into().unwrap();
    let stations = [
        w.place_station_on_alignment(c, "A".into(), rail, 0.2, 40., 8.)
            .unwrap(),
        w.place_station_on_alignment(c, "A2".into(), rail, 0.4, 40., 8.)
            .unwrap(),
        w.place_station_on_alignment(c, "Tram".into(), tram, 0.5, 40., 8.)
            .unwrap(),
        w.place_station_on_alignment(c, "B".into(), fast, 0.7, 40., 8.)
            .unwrap(),
    ];
    let link = |a, b| AlignmentConnection {
        a: AlignmentEndpointRef {
            alignment_id: a,
            endpoint: End,
        },
        b: AlignmentEndpointRef {
            alignment_id: b,
            endpoint: Start,
        },
    };
    let links = [link(rail, tram), link(tram, fast)];
    for l in &links {
        w.connect_alignment_endpoints(c, l.a, l.b).unwrap();
    }
    (w, c, [rail, tram, fast], stations, links)
}
fn config(service_id: ServiceId) -> OperationServiceConfig {
    OperationServiceConfig {
        service_id,
        timetable: Timetable {
            first_departure_ms: 0,
            last_departure_ms: 0,
            headway_ms: 60_000,
        },
        vehicle: Vehicle {
            max_speed_kmh: 130.,
            acceleration_mps2: 1.,
            braking_mps2: 1.,
            length_m: 25.,
        },
        dwell_ms: 20_000,
    }
}
fn selected(
    w: &World,
    from: StationId,
    path: &[RouteSegment],
    skip: Option<StationId>,
) -> Vec<StationOccurrenceRef> {
    w.preview_service_path(from, path.to_vec())
        .unwrap()
        .occurrences
        .into_iter()
        .filter(|o| Some(o.station_id) != skip)
        .map(|o| o.reference)
        .collect()
}
#[test]
fn selected_actual_tram_platform_authorizes_traversal_and_roundtrip() {
    let (mut w, c, [rail, _, fast], [a, a2, t, b], j) = fixture();
    let forward = vec![RouteSegment::ConnectedAlignments {
        from: a,
        to: b,
        connections: j.to_vec(),
    }];
    let skip = selected(&w, a, &forward, Some(t));
    let before = w.snapshot().clone();
    assert_eq!(
        w.add_service_path(
            c,
            "Train forbidden".into(),
            a,
            forward.clone(),
            skip.clone()
        ),
        Err(WorldError::TramTrackRequiresStop)
    );
    assert_eq!(w.snapshot(), &before);
    for backwards in [false, true] {
        let (start, segments) = if backwards {
            (
                b,
                vec![RouteSegment::ConnectedAlignments {
                    from: b,
                    to: a,
                    connections: j
                        .iter()
                        .rev()
                        .map(|l| AlignmentConnection { a: l.b, b: l.a })
                        .collect(),
                }],
            )
        } else {
            (a, forward.clone())
        };
        assert_eq!(
            w.add_service_path(
                c,
                "Pass only".into(),
                start,
                segments.clone(),
                selected(&w, start, &segments, Some(t))
            ),
            Err(WorldError::TramTrackRequiresStop)
        );
        let tram = w
            .add_service_path(
                c,
                "Tram-train".into(),
                start,
                segments.clone(),
                selected(&w, start, &segments, None),
            )
            .unwrap();
        let spec = w.compile_operation(&config(tram)).unwrap();
        assert!(spec
            .legs
            .iter()
            .flat_map(|l| &l.sections)
            .all(|s| s.speed_limit_kmh <= 75.));
        for id in [rail, fast] {
            let key = format!("rail:{}:section:", id.0);
            assert!(spec.legs.iter().flat_map(|l| &l.sections).any(
                |s| s.speed_limit_kmh == 75. && s.resources.iter().any(|r| r.starts_with(&key))
            ));
        }
        let original = encode(&w).unwrap();
        assert_eq!(
            w.update_service_path(
                c,
                tram,
                "Drop only tram stop".into(),
                selected(&w, start, &segments, Some(t))
            ),
            Err(WorldError::TramTrackRequiresStop)
        );
        assert_eq!(encode(&w).unwrap(), original);
    }
    let rail_path = vec![RouteSegment::Alignment {
        alignment_id: rail,
        from: a,
        to: a2,
    }];
    let local = w
        .add_service_path(
            c,
            "Pure train".into(),
            a,
            rail_path.clone(),
            selected(&w, a, &rail_path, None),
        )
        .unwrap();
    let tram = w.snapshot().services[0].id;
    w.configure_operation(vec![config(tram), config(local)])
        .unwrap();
    let rt = &mut w.operation_mut().unwrap().runtime;
    rt.advance_to(1_000, 100).unwrap();
    assert_eq!(
        rt.trains()
            .filter(|t| matches!(t.state, TrainState::Waiting { .. }))
            .count(),
        1
    );
    assert_eq!(
        rt.trains()
            .filter(|t| matches!(t.state, TrainState::Moving { .. }))
            .count(),
        1
    );
    let bytes = encode(&w).unwrap();
    assert_eq!(encode(&decode(&bytes).unwrap()).unwrap(), bytes);

    let mut d = w.snapshot().clone();
    d.service_paths
        .iter_mut()
        .find(|s| s.service_id == tram)
        .unwrap()
        .stops = skip;
    assert_eq!(World::new(d).err(), Some(WorldError::TramTrackRequiresStop));
}

#[test]
fn mixed_joint_rejects_foreign_owner_height_and_platform_at_joint() {
    let (mut w, c, [_, _, fast], _, _) = fixture();
    let at = w
        .snapshot()
        .alignments
        .iter()
        .find(|a| a.id == fast)
        .unwrap()
        .points[1];
    let extend = |w: &mut World, owner, name: &str| {
        w.build_alignment_typed(owner, name.into(), vec![at, p(139.04)], TrackType::Tram)
            .unwrap()
    };
    let player = w.add_player("Foreign".into()).unwrap();
    let foreign = w.add_company(player, "Not connectable".into()).unwrap();
    let a = AlignmentEndpointRef {
        alignment_id: fast,
        endpoint: End,
    };
    let b = |alignment_id| AlignmentEndpointRef {
        alignment_id,
        endpoint: Start,
    };
    let x = extend(&mut w, foreign, "Foreign");
    assert_eq!(
        w.connect_alignment_endpoints(c, a, b(x)),
        Err(WorldError::InvalidAlignmentConnection)
    );
    let x = extend(&mut w, c, "Same owner");
    let mut elevated = w.snapshot().clone();
    elevated
        .alignments
        .iter_mut()
        .find(|l| l.id == x)
        .unwrap()
        .infrastructure = vec![rosenfu_core::world::SectionInfrastructure {
        track_count: 1,
        level: rosenfu_core::world::InfrastructureLevel::Elevated,
    }];
    let mut higher = World::new(elevated).unwrap();
    assert_eq!(
        higher.connect_alignment_endpoints(c, a, b(x)),
        Err(WorldError::InvalidAlignmentConnection)
    );
    let len = w
        .snapshot()
        .alignments
        .iter()
        .find(|l| l.id == x)
        .unwrap()
        .length_m();
    let touching = 40. / (2. * len);
    let mut before_link = w.clone();
    w.place_station_on_alignment(c, "At mixed joint".into(), x, touching, 40., 8.)
        .unwrap();
    assert_eq!(
        w.connect_alignment_endpoints(c, a, b(x)),
        Err(WorldError::StationAtMixedTrackConnection)
    );
    before_link.connect_alignment_endpoints(c, a, b(x)).unwrap();
    assert_eq!(
        before_link.place_station_on_alignment(c, "At mixed joint".into(), x, touching, 40., 8.),
        Err(WorldError::StationAtMixedTrackConnection)
    );
}

#[test]
fn another_tram_platform_in_station_group_cannot_authorize_a_train() {
    let (mut w, c, [rail, _, _], [a, _, t, b], j) = fixture();
    let a_point = w
        .snapshot()
        .alignments
        .iter()
        .find(|l| l.id == rail)
        .unwrap()
        .point_at(0.2);
    let other = w
        .build_alignment_typed(
            c,
            "Unconnected tram".into(),
            vec![
                GeoPoint {
                    latitude: a_point.latitude - 0.004,
                    longitude: a_point.longitude,
                },
                GeoPoint {
                    latitude: a_point.latitude + 0.004,
                    longitude: a_point.longitude,
                },
            ],
            TrackType::Tram,
        )
        .unwrap();
    let tram_stop = w
        .place_station_on_alignment(c, "Other".into(), other, 0.5, 40., 8.)
        .unwrap();
    assert_eq!(w.snapshot().station_group_id(tram_stop), a);
    let paths = vec![RouteSegment::ConnectedAlignments {
        from: a,
        to: b,
        connections: j.to_vec(),
    }];
    assert_eq!(
        w.add_service_path(
            c,
            "Cannot borrow fellow platform".into(),
            a,
            paths.clone(),
            selected(&w, a, &paths, Some(t))
        ),
        Err(WorldError::TramTrackRequiresStop)
    );
}

#[test]
fn previously_valid_pure_tram_service_keeps_dedicated_cap_and_save() {
    let (mut w, c, [_, tram, _], [_, _, a, _], _) = fixture();
    let b = w
        .place_station_on_alignment(c, "Pure B".into(), tram, 0.75, 40., 8.)
        .unwrap();
    let path = vec![RouteSegment::Alignment {
        alignment_id: tram,
        from: a,
        to: b,
    }];
    let id = w
        .add_service_path(
            c,
            "Pure tram".into(),
            a,
            path.clone(),
            selected(&w, a, &path, None),
        )
        .unwrap();
    let spec = w.compile_operation(&config(id)).unwrap();
    assert!(spec
        .legs
        .iter()
        .flat_map(|l| &l.sections)
        .all(|s| s.speed_limit_kmh == 50.));
    w.configure_operation(vec![config(id)]).unwrap();
    let bytes = encode(&w).unwrap();
    assert_eq!(encode(&decode(&bytes).unwrap()).unwrap(), bytes);
}
