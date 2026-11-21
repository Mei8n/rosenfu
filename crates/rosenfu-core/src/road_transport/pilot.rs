//! Positive movement gate for an independently reviewed road graph.
use super::*;
use std::{cmp::Ordering, collections::BinaryHeap};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PilotRoadMode {
    Bus,
    Tram,
}

/// Positive proof for a single directed movement. No row means not permitted.
/// Importers must exclude all segments involved in unsupported restrictions;
/// this two-edge gate does not evaluate time conditions or via-way rules.
pub struct PilotTransition<'a> {
    pub incoming: &'a str,
    pub outgoing: &'a str,
    pub mode: PilotRoadMode,
}

const MAX_EDGES: usize = 4096;
const MAX_TRANSITIONS: usize = 8192;
const MAX_PATH: usize = 4096;

pub struct PilotRoad<'a> {
    graph: &'a RoadTransport,
    edges: BTreeMap<&'a str, &'a RoadEdge>,
    transitions: BTreeMap<(PilotRoadMode, &'a str), BTreeSet<&'a str>>,
}

impl<'a> PilotRoad<'a> {
    /// Compare the persisted graph against **independently trusted** pinned
    /// native data before a graph can be used for path search or restored routes
    /// can be accepted. The caller must pin and authenticate expected bytes
    /// outside the save; neither a save's graph nor values received by IPC can
    /// serve as the trusted argument. The snapshot is canonical and ordered;
    /// user-created bus stops, bus paths and tram sections are not graph data.
    pub fn from_pinned_graph(
        graph: &'a RoadTransport,
        world: &WorldDraft,
        pinned_graph: &RoadTransport,
        source: &str,
        revision: &str,
        rules: &[PilotTransition<'a>],
    ) -> Result<Self, RoadTransportError> {
        if graph.nodes != pinned_graph.nodes
            || graph.edges != pinned_graph.edges
            || graph.evidence != pinned_graph.evidence
            || !pinned_graph.bus_stops.is_empty()
            || !pinned_graph.bus_routes.is_empty()
            || !pinned_graph.tram_sections.is_empty()
        {
            return Err(StaleSnapshot);
        }
        Self::checked(graph, world, source, revision, rules)
    }

    /// Caller supplies the expected source/revision from a trusted pinned
    /// manifest, never from a save/map string. This checks graph consistency
    /// and positive movements; it does not authenticate the source or rules.
    fn checked(
        graph: &'a RoadTransport,
        world: &WorldDraft,
        source: &str,
        revision: &str,
        rules: &[PilotTransition<'a>],
    ) -> Result<Self, RoadTransportError> {
        if !label(source)
            || !label(revision)
            || graph.edges.len() > MAX_EDGES
            || graph.nodes.len() > 2 * MAX_EDGES
            || rules.len() > MAX_TRANSITIONS
        {
            return Err(InvalidData);
        }
        let edges = indexed(&graph.edges, |edge| &edge.id)?;
        if graph
            .edges
            .iter()
            .any(|edge| edge.source != source || edge.revision != revision)
        {
            return Err(StaleSnapshot);
        }
        let mut transitions = BTreeMap::<_, BTreeSet<_>>::new();
        for rule in rules {
            let &incoming = edges.get(rule.incoming).ok_or(MissingReference)?;
            let &outgoing = edges.get(rule.outgoing).ok_or(MissingReference)?;
            if incoming.to != outgoing.from {
                return Err(DisconnectedPath);
            }
            // The first pilot excludes U-turns even if supplied as a rule.
            if incoming.from == outgoing.to
                || !Self::edge_is_allowed(incoming, rule.mode)
                || !Self::edge_is_allowed(outgoing, rule.mode)
            {
                return Err(AccessForbidden);
            }
            if !transitions
                .entry((rule.mode, rule.incoming))
                .or_default()
                .insert(rule.outgoing)
            {
                return Err(DuplicateId);
            }
        }
        graph.validate(world)?;
        let gate = Self {
            graph,
            edges,
            transitions,
        };
        for stop in &graph.bus_stops {
            if !Self::edge_is_allowed(gate.edges[stop.edge_id.as_str()], PilotRoadMode::Bus) {
                return Err(AccessForbidden);
            }
        }
        for route in &graph.bus_routes {
            gate.validate_path(PilotRoadMode::Bus, None, None, &route.edges)?;
        }
        for section in &graph.tram_sections {
            if let TramPlacement::SharedRoad { edges } = &section.placement {
                gate.validate_path(PilotRoadMode::Tram, None, None, edges)?;
            }
        }
        Ok(gate)
    }

    fn edge_is_allowed(edge: &RoadEdge, mode: PilotRoadMode) -> bool {
        edge.level == Some(crate::world::InfrastructureLevel::Ground)
            && match mode {
                PilotRoadMode::Bus => edge.bus_access,
                PilotRoadMode::Tram => edge.tram_access,
            } == Access::Allowed
    }

    fn permits(&self, mode: PilotRoadMode, incoming: &str, outgoing: &str) -> bool {
        self.transitions
            .get(&(mode, incoming))
            .is_some_and(|approved| approved.contains(outgoing))
    }

    /// Validate the full path (not only edited edges) using the same positive
    /// transition table as route search. Future save/edit adapters must check
    /// both modes before committing the candidate graph.
    pub fn validate_path(
        &self,
        mode: PilotRoadMode,
        from: Option<&str>,
        to: Option<&str>,
        ids: &[String],
    ) -> Result<(), RoadTransportError> {
        if ids.is_empty() || ids.len() > MAX_PATH {
            return Err(InvalidData);
        }
        let mut prev: Option<&RoadEdge> = None;
        for id in ids {
            let &edge = self.edges.get(id.as_str()).ok_or(MissingReference)?;
            if !Self::edge_is_allowed(edge, mode) {
                return Err(AccessForbidden);
            }
            if let Some(p) = prev {
                if p.to != edge.from {
                    return Err(DisconnectedPath);
                }
                if !self.permits(mode, &p.id, id) {
                    return Err(AccessForbidden);
                }
            } else if from.is_some_and(|from| edge.from != from) {
                return Err(DisconnectedPath);
            }
            prev = Some(edge);
        }
        if to.is_some_and(|to| prev.is_none_or(|edge| edge.to != to)) {
            return Err(DisconnectedPath);
        }
        Ok(())
    }

    /// State is the incoming directed edge, not the node. Arriving at a junction
    /// via a different predecessor may allow a different next movement.
    pub fn shortest_path(
        &self,
        mode: PilotRoadMode,
        from: &str,
        to: &str,
    ) -> Result<Vec<String>, RoadTransportError> {
        if from == to
            || !self.graph.nodes.iter().any(|node| node.id == from)
            || !self.graph.nodes.iter().any(|node| node.id == to)
        {
            return Err(InvalidData);
        }

        #[derive(PartialEq)]
        struct Entry<'a>(f64, &'a str);
        impl Eq for Entry<'_> {}
        impl Ord for Entry<'_> {
            fn cmp(&self, other: &Self) -> Ordering {
                other.0.total_cmp(&self.0).then_with(|| other.1.cmp(self.1))
            }
        }
        impl PartialOrd for Entry<'_> {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        let lengths: BTreeMap<_, f64> = self
            .edges
            .iter()
            .filter(|(_, edge)| Self::edge_is_allowed(edge, mode))
            .map(|(&id, edge)| {
                let length = edge
                    .geometry
                    .windows(2)
                    .map(|p| distance_m(p[0], p[1]))
                    .sum();
                (id, length)
            })
            .collect();
        let mut queue = BinaryHeap::new();
        let mut shortest = BTreeMap::new();
        for &id in lengths.keys() {
            if self.edges[id].from == from {
                shortest.insert(id, (lengths[id], None));
                queue.push(Entry(lengths[id], id));
            }
        }
        while let Some(Entry(cost, id)) = queue.pop() {
            if cost > shortest[id].0 {
                continue;
            }
            if self.edges[id].to == to {
                let mut cursor = Some(id);
                let mut path = Vec::new();
                while let Some(current) = cursor {
                    path.push(current.to_owned());
                    cursor = shortest[current].1;
                }
                path.reverse();
                self.validate_path(mode, Some(from), Some(to), &path)?;
                return Ok(path);
            }
            for &out in self.transitions.get(&(mode, id)).into_iter().flatten() {
                let next = cost + lengths[out];
                if shortest.get(out).is_none_or(|&(old, _)| next < old) {
                    shortest.insert(out, (next, Some(id)));
                    queue.push(Entry(next, out));
                }
            }
        }
        Err(DisconnectedPath)
    }
}

#[cfg(test)]
mod tests {
    use super::{PilotRoadMode::*, *};
    use crate::world::{Company, InfrastructureLevel, PlayerId};

    fn fixture() -> RoadTransport {
        let mut graph = RoadTransport::default();
        for (id, lat, lon) in [
            ("a", 35.0, 135.0),
            ("j", 35.0, 135.001),
            ("d", 35.0, 135.002),
            ("b", 35.001, 135.0),
            ("other-level", 35.0, 135.001),
        ] {
            graph.nodes.push(RoadNode {
                id: id.into(),
                point: GeoPoint {
                    latitude: lat,
                    longitude: lon,
                },
            });
        }
        for (id, from, to) in [
            ("aj", "a", "j"),
            ("ab", "a", "b"),
            ("bj", "b", "j"),
            ("jd", "j", "d"),
            ("ja", "j", "a"),
            ("other", "other-level", "d"),
        ] {
            let point = |id| graph.nodes.iter().find(|node| node.id == id).unwrap().point;
            graph.edges.push(RoadEdge {
                id: id.into(),
                from: from.into(),
                to: to.into(),
                source: "review-fixture".into(),
                revision: "test-1".into(),
                geometry: vec![point(from), point(to)],
                bus_access: Access::Allowed,
                tram_access: Access::Allowed,
                level: Some(InfrastructureLevel::Ground),
            });
        }
        graph.edges.last_mut().unwrap().level = None;
        graph
    }

    fn rule<'a>(incoming: &'a str, outgoing: &'a str, mode: PilotRoadMode) -> PilotTransition<'a> {
        PilotTransition {
            incoming,
            outgoing,
            mode,
        }
    }

    fn gate<'a>(
        graph: &'a RoadTransport,
        rules: &[PilotTransition<'a>],
    ) -> Result<PilotRoad<'a>, RoadTransportError> {
        PilotRoad::from_pinned_graph(
            graph,
            &WorldDraft::default(),
            graph,
            "review-fixture",
            "test-1",
            rules,
        )
    }

    fn ids(path: &[&str]) -> Vec<String> {
        path.iter().map(|s| (*s).into()).collect()
    }

    #[test]
    fn different_approach_can_be_legal_and_modes_do_not_leak() {
        let graph = fixture();
        let gate = gate(
            &graph,
            &[
                rule("ab", "bj", Bus),
                rule("bj", "jd", Bus),
                rule("aj", "jd", Tram),
            ],
        )
        .unwrap();
        assert_eq!(
            gate.shortest_path(Bus, "a", "d"),
            Ok(ids(&["ab", "bj", "jd"]))
        );
        assert_eq!(gate.shortest_path(Tram, "a", "d"), Ok(ids(&["aj", "jd"])));
        assert_eq!(gate.shortest_path(Bus, "d", "a"), Err(DisconnectedPath));
        assert_eq!(
            gate.validate_path(Bus, Some("a"), Some("d"), &ids(&["aj", "jd"])),
            Err(AccessForbidden)
        );
        assert_eq!(
            gate.validate_path(Tram, Some("b"), Some("d"), &ids(&["aj", "jd"])),
            Err(DisconnectedPath)
        );
    }

    #[test]
    fn stale_unsupported_duplicate_and_disconnected_rules_never_expand_access() {
        let mut graph = fixture();
        assert_eq!(
            gate(&graph, &[rule("aj", "other", Bus)]).err(),
            Some(DisconnectedPath)
        );
        assert_eq!(
            gate(&graph, &[rule("aj", "ja", Bus)]).err(),
            Some(AccessForbidden)
        );
        assert_eq!(
            gate(&graph, &[rule("ab", "bj", Bus), rule("ab", "bj", Bus)]).err(),
            Some(DuplicateId)
        );
        assert_eq!(
            gate(&graph, &[rule("aj", "missing", Bus)]).err(),
            Some(MissingReference)
        );
        assert_eq!(
            gate(&graph, &[])
                .unwrap()
                .shortest_path(Bus, "other-level", "d"),
            Err(DisconnectedPath)
        );
        graph.edges[0].tram_access = Access::Unknown;
        assert_eq!(
            gate(&graph, &[rule("aj", "jd", Tram)]).err(),
            Some(AccessForbidden)
        );
        graph.edges[0].revision = "stale".into();
        assert_eq!(gate(&graph, &[]).err(), Some(StaleSnapshot));
    }

    #[test]
    fn old_generic_validator_acceptance_does_not_authorize_saved_pilot_path() {
        let pinned = fixture();
        let mut graph = pinned.clone();
        let mut world = WorldDraft::default();
        world.companies.push(Company {
            id: CompanyId(1),
            owner: PlayerId(2),
            name: "pilot".into(),
        });
        for (id, edge_id) in [("s", "aj"), ("t", "jd")] {
            graph.bus_stops.push(BusStop {
                id: id.into(),
                owner: CompanyId(1),
                name: id.into(),
                edge_id: edge_id.into(),
                position: 0.5,
            });
        }
        graph.bus_routes.push(BusRoute {
            id: "r".into(),
            owner: CompanyId(1),
            name: "r".into(),
            edges: ids(&["aj", "jd"]),
            stops: vec![
                BusStopOccurrence {
                    stop_id: "s".into(),
                    edge_index: 0,
                },
                BusStopOccurrence {
                    stop_id: "t".into(),
                    edge_index: 1,
                },
            ],
        });
        let graph: RoadTransport =
            serde_json::from_str(&serde_json::to_string(&graph).unwrap()).unwrap();
        assert_eq!(graph.validate(&world), Ok(()));
        assert_eq!(
            PilotRoad::from_pinned_graph(
                &graph,
                &world,
                &pinned,
                "review-fixture",
                "test-1",
                &[rule("aj", "jd", Tram)]
            )
            .err(),
            Some(AccessForbidden)
        );
        assert!(PilotRoad::from_pinned_graph(
            &graph,
            &world,
            &pinned,
            "review-fixture",
            "test-1",
            &[rule("aj", "jd", Bus)]
        )
        .is_ok());
    }
    #[test]
    fn matching_tram_geometry_is_not_permission_to_turn() {
        let pinned = fixture();
        let mut graph = pinned.clone();
        let mut world = WorldDraft::default();
        let edges = ids(&["aj", "jd"]);
        let points: Vec<GeoPoint> = graph.nodes[..3].iter().map(|n| n.point).collect();
        world.alignments.push(crate::world::Alignment {
            id: AlignmentId(5),
            owner: CompanyId(1),
            name: "Tram".into(),
            points: points.clone(),
            anchor_indices: None,
            infrastructure: vec![],
            vertical_profile: None,
            section_profiles: vec![],
            track_type: TrackType::Tram,
            design_speed_kmh: None,
        });
        graph.tram_sections.push(TramSection {
            alignment_id: AlignmentId(5),
            alignment_geometry: points,
            from_position: 0.0,
            to_position: 1.0,
            placement: TramPlacement::SharedRoad { edges },
        });
        let graph: RoadTransport =
            serde_json::from_str(&serde_json::to_string(&graph).unwrap()).unwrap();
        assert_eq!(graph.validate(&world), Ok(()));
        let check = |mode| {
            PilotRoad::from_pinned_graph(
                &graph,
                &world,
                &pinned,
                "review-fixture",
                "test-1",
                &[rule("aj", "jd", mode)],
            )
        };
        assert_eq!(check(Bus).err(), Some(AccessForbidden));
        assert!(check(Tram).is_ok());
    }

    #[test]
    fn pinned_graph_identity_is_not_just_source_and_revision_strings() {
        let mut pinned = fixture();
        pinned.edges[0].bus_access = Access::Unknown;
        let restored = pinned.clone();
        let world = WorldDraft::default();
        let admissible =
            |g| PilotRoad::from_pinned_graph(g, &world, &pinned, "review-fixture", "test-1", &[]);
        assert_eq!(
            admissible(&restored).unwrap().shortest_path(Bus, "a", "j"),
            Err(DisconnectedPath)
        );
        assert_eq!(
            admissible(&restored).unwrap().shortest_path(Tram, "a", "j"),
            Ok(ids(&["aj"]))
        );
        let mut widened = pinned.clone();
        widened.edges[0].bus_access = Access::Allowed;
        assert_eq!(admissible(&widened).err(), Some(StaleSnapshot));
        let mut reversed = pinned.clone();
        reversed.edges[0].geometry.reverse();
        assert_eq!(admissible(&reversed).err(), Some(StaleSnapshot));
    }
}
