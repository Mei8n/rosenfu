//! These abstract graph cases retain search safety coverage after Track removal.
use super::*;
fn graph_fixture(n: u64, edges: &[(u64, u64)]) -> Graph {
    let mut g: Graph = (0..n).map(|id| (StationId(id + 1), vec![])).collect();
    for (index, &(a, b)) in edges.iter().enumerate() {
        let a = StationId(a + 1);
        let b = StationId(b + 1);
        for (from, to) in [(a, b), (b, a)] {
            g.get_mut(&from).unwrap().push(Edge {
                to,
                segment: RouteSegment::Alignment {
                    alignment_id: AlignmentId(index as u64 + 1),
                    from,
                    to,
                },
            });
        }
    }
    g
}
fn search(g: &Graph, start: u64, end: u64, via: &[u64]) -> RouteCandidates {
    enumerate_candidates(
        g,
        false,
        StationId(start + 1),
        StationId(end + 1),
        &via.iter().map(|i| StationId(i + 1)).collect::<Vec<_>>(),
    )
    .unwrap()
}
#[test]
fn cycles_parallel_edges_and_ambiguity_are_preserved() {
    let g = graph_fixture(3, &[(0, 1), (0, 1), (1, 2), (2, 0)]);
    let r = search(&g, 0, 2, &[]);
    assert_eq!(r.candidates.len(), 3);
    assert_eq!(r.status, RouteSearchStatus::Ambiguous);
    assert!(r
        .candidates
        .iter()
        .all(|c| c.stations.iter().collect::<BTreeSet<_>>().len() == c.stations.len()));
}
#[test]
fn ordered_waypoints_match_full_oracle_across_branches_and_repeated_calls() {
    let g = graph_fixture(
        7,
        &[
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 6),
            (1, 4),
            (4, 2),
            (0, 5),
            (5, 3),
            (5, 2),
        ],
    );
    let all = search(&g, 0, 6, &[]);
    assert!(!all.truncated);
    for via in [vec![2, 3], vec![4, 2], vec![3, 2], vec![1], vec![]] {
        let expected: Vec<_> = all
            .candidates
            .iter()
            .filter(|c| {
                let positions: Option<Vec<_>> = via
                    .iter()
                    .map(|id| c.stations.iter().position(|s| *s == StationId(id + 1)))
                    .collect();
                positions.is_some_and(|p| p.windows(2).all(|pair| pair[0] < pair[1]))
            })
            .cloned()
            .collect();
        for _ in 0..3 {
            let r = search(&g, 0, 6, &via);
            assert!(!r.truncated);
            assert_eq!(r.candidates, expected);
        }
    }
}
#[test]
fn incomplete_budget_never_claims_unique_or_shortest() {
    let g = graph_fixture(2, &vec![(0, 1); 40]);
    let r = search(&g, 0, 1, &[]);
    assert_eq!(r.status, RouteSearchStatus::Incomplete);
    assert_eq!(r.candidates.len(), 32);
    assert_eq!(r.shortest_candidate_index, None);
    let mut edges = vec![(0, 1); 40];
    edges.extend([(1, 2), (2, 3)]);
    let g = graph_fixture(4, &edges);
    let r = search(&g, 0, 3, &[1, 2]);
    assert_eq!(r.status, RouteSearchStatus::Incomplete);
    assert_eq!(r.candidates.len(), 32);
}
#[test]
fn ordered_constraint_is_applied_before_candidate_limit() {
    let mut edges = vec![(0, 2); 40];
    edges.extend([(0, 1), (1, 2)]);
    let g = graph_fixture(3, &edges);
    assert!(search(&g, 0, 2, &[]).truncated);
    let r = search(&g, 0, 2, &[1]);
    assert_eq!(r.status, RouteSearchStatus::Unique);
    assert_eq!(
        r.candidates[0].stations,
        vec![StationId(1), StationId(2), StationId(3)]
    );
}
#[test]
fn graph_truncation_and_hop_limit_remain_incomplete() {
    let g = graph_fixture(2, &[(0, 1)]);
    assert_eq!(
        enumerate_candidates(&g, true, StationId(1), StationId(2), &[])
            .unwrap()
            .status,
        RouteSearchStatus::Incomplete
    );
    let edges: Vec<_> = (0..257).map(|i| (i, i + 1)).collect();
    let g = graph_fixture(258, &edges);
    assert_eq!(
        search(&g, 0, 257, &[]).status,
        RouteSearchStatus::Incomplete
    );
}
#[test]
fn physical_distance_uses_curved_alignment_span() {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let a = w
        .build_alignment(
            c,
            "A".into(),
            vec![
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.0,
                },
                GeoPoint {
                    latitude: 35.01,
                    longitude: 139.05,
                },
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.1,
                },
            ],
        )
        .unwrap();
    let from = w
        .place_station_on_alignment(c, "A".into(), a, 0.2, 100.0, 10.0)
        .unwrap();
    let to = w
        .place_station_on_alignment(c, "B".into(), a, 0.7, 100.0, 10.0)
        .unwrap();
    let length = w.snapshot().alignments[0].length_m();
    let d = segment_distance(
        w.snapshot(),
        &RouteSegment::Alignment {
            alignment_id: a,
            from,
            to,
        },
    )
    .unwrap();
    assert!((d - length * 0.5).abs() < 0.0001);
    let result = w.route_candidates(from, to).unwrap();
    assert_eq!(result.shortest_candidate_index, Some(0));
}
