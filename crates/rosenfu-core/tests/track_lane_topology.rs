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
