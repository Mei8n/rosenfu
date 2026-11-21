//! Native trust boundary: only release-pinned bytes can authorize real road graph use.
use super::*;
use sha2::{Digest, Sha256};

const SOURCE_PREFIX: &str = "native-pilot:";
const MAX_ASSET_BYTES: usize = 4 * 1024 * 1024;

struct Pin<'a> {
    id: &'a str,
    sha256: [u8; 32],
    bytes: &'a [u8],
}

// Intentionally empty until a real corridor and reuse rights are explicitly reviewed.
// Never populate this from a save, IPC, filesystem lookup, network, or test fixture.
const PINS: &[Pin<'static>] = &[];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Asset {
    format: u32,
    asset_id: String,
    revision: String,
    nodes: Vec<RoadNode>,
    edges: Vec<RoadEdge>,
    transitions: Vec<Movement>,
    bus_stop_positions: Vec<StopPosition>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Movement {
    incoming: String,
    outgoing: String,
    mode: PilotRoadMode,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StopPosition {
    edge_id: String,
    position: f64,
}

struct Approved {
    graph: RoadTransport,
    source: String,
    revision: String,
    transitions: Vec<Movement>,
    bus_stop_positions: Vec<StopPosition>,
}

impl Pin<'_> {
    fn authenticate(&self) -> Result<Approved, RoadTransportError> {
        let id = self.id;
        if id.is_empty()
            || id.len() > 64
            || !id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || self.bytes.len() > MAX_ASSET_BYTES
            || <[u8; 32]>::from(Sha256::digest(self.bytes)) != self.sha256
        {
            return Err(StaleSnapshot);
        }
        let asset: Asset = serde_json::from_slice(self.bytes).map_err(|_| InvalidData)?;
        if asset.format != 1
            || asset.asset_id != id
            || asset.nodes.is_empty()
            || asset.edges.is_empty()
            || !asset.nodes.windows(2).all(|p| p[0].id < p[1].id)
            || !asset.edges.windows(2).all(|p| p[0].id < p[1].id)
            || asset.nodes.iter().any(|n| n.id.starts_with("qa:"))
            || asset.edges.iter().any(|e| e.id.starts_with("qa:"))
            || asset.bus_stop_positions.len() > 4096
        {
            return Err(InvalidData);
        }
        let approved = Approved {
            graph: RoadTransport {
                nodes: asset.nodes,
                edges: asset.edges,
                ..RoadTransport::default()
            },
            source: format!("{SOURCE_PREFIX}{id}"),
            revision: asset.revision,
            transitions: asset.transitions,
            bus_stop_positions: asset.bus_stop_positions,
        };
        approved.gate(&approved.graph, &WorldDraft::default())?;
        let edges = indexed(&approved.graph.edges, |edge| &edge.id)?;
        let mut seen = BTreeSet::new();
        for stop in &approved.bus_stop_positions {
            if !position(stop.position) {
                return Err(InvalidData);
            }
            let edge = edges.get(stop.edge_id.as_str()).ok_or(MissingReference)?;
            if edge.level != Some(crate::world::InfrastructureLevel::Ground)
                || edge.bus_access != Access::Allowed
            {
                return Err(AccessForbidden);
            }
            if !seen.insert((&stop.edge_id, position_key(stop.position))) {
                return Err(DuplicateId);
            }
        }
        Ok(approved)
    }
}

// These are already finite fractions; normalize negative zero to match f64 equality.
fn position_key(fraction: f64) -> u64 {
    if fraction == 0.0 {
        0
    } else {
        fraction.to_bits()
    }
}

impl Approved {
    fn gate<'a>(
        &'a self,
        graph: &'a RoadTransport,
        world: &WorldDraft,
    ) -> Result<PilotRoad<'a>, RoadTransportError> {
        let rules: Vec<_> = self
            .transitions
            .iter()
            .map(|r| PilotTransition {
                incoming: &r.incoming,
                outgoing: &r.outgoing,
                mode: r.mode,
            })
            .collect();
        let gate = PilotRoad::from_pinned_graph(
            graph,
            world,
            &self.graph,
            &self.source,
            &self.revision,
            &rules,
        )?;
        let allowed: BTreeSet<_> = self
            .bus_stop_positions
            .iter()
            .map(|stop| (stop.edge_id.as_str(), position_key(stop.position)))
            .collect();
        if graph
            .bus_stops
            .iter()
            .any(|stop| !allowed.contains(&(stop.edge_id.as_str(), position_key(stop.position))))
        {
            return Err(AccessForbidden);
        }
        Ok(gate)
    }
}

fn approved_for(
    graph: &RoadTransport,
    pins: &[Pin<'_>],
) -> Result<Option<Approved>, RoadTransportError> {
    let Some(asset_id) = graph
        .edges
        .iter()
        .find_map(|e| e.source.strip_prefix(SOURCE_PREFIX))
    else {
        return Ok(None);
    };
    approved_asset(asset_id, pins).map(Some)
}

/// Read a release-embedded and digest-pinned graph. No caller-controlled bytes or paths.
pub fn import_native_pilot(asset_id: &str) -> Result<RoadTransport, RoadTransportError> {
    Ok(approved_asset(asset_id, PINS)?.graph)
}

fn approved_asset(asset_id: &str, pins: &[Pin<'_>]) -> Result<Approved, RoadTransportError> {
    pins.iter()
        .find(|pin| pin.id == asset_id)
        .ok_or(StaleSnapshot)?
        .authenticate()
}

impl RoadTransport {
    /// Game-world boundary: non-routable map evidence remains inspectable, but
    /// a graph may assert permission only through an approved asset or the exact QA seed.
    pub(crate) fn validate_world(&self, world: &WorldDraft) -> Result<(), RoadTransportError> {
        if let Some(pinned) = approved_for(self, PINS)? {
            pinned.gate(self, world)?;
            return Ok(());
        }
        if self
            .edges
            .iter()
            .any(|e| e.bus_access == Access::Allowed || e.tram_access == Access::Allowed)
            && !self.has_qa_graph()
        {
            return Err(AccessForbidden);
        }
        self.validate(world)
    }

    /// Native route selection uses the same movement authority as save/edit validation.
    pub fn native_shortest_path(
        &self,
        world: &WorldDraft,
        from: &str,
        to: &str,
        mode: PilotRoadMode,
    ) -> Result<Vec<String>, RoadTransportError> {
        if let Some(pinned) = approved_for(self, PINS)? {
            return pinned.gate(self, world)?.shortest_path(mode, from, to);
        }
        if self.has_qa_graph() {
            return self.shortest_path(world, from, to, mode == PilotRoadMode::Tram);
        }
        Err(AccessForbidden)
    }

    fn has_qa_graph(&self) -> bool {
        if self.nodes.len() != 3 || self.edges.len() != 4 || !self.evidence.is_empty() {
            return false;
        }
        let expected = Self::synthetic_qa_graph();
        self.nodes == expected.nodes && self.edges == expected.edges
    }

    /// A pre-existing, explicitly synthetic virtual street graph, never an asset pin.
    pub fn synthetic_qa_graph() -> Self {
        let pts = [
            GeoPoint {
                latitude: 35.0,
                longitude: 135.0,
            },
            GeoPoint {
                latitude: 35.0,
                longitude: 135.005,
            },
            GeoPoint {
                latitude: 35.005,
                longitude: 135.005,
            },
        ];
        let nodes = pts
            .iter()
            .enumerate()
            .map(|(i, point)| RoadNode {
                id: format!("qa:{i}"),
                point: *point,
            })
            .collect();
        let edges = [(0, 1), (1, 0), (1, 2), (2, 1)]
            .into_iter()
            .map(|(a, b)| RoadEdge {
                id: format!("qa:{a}-{b}"),
                from: format!("qa:{a}"),
                to: format!("qa:{b}"),
                source: "synthetic-qa-not-real-roads".into(),
                revision: "1".into(),
                geometry: vec![pts[a], pts[b]],
                bus_access: Access::Allowed,
                tram_access: Access::Allowed,
                level: Some(crate::world::InfrastructureLevel::Ground),
            })
            .collect();
        Self {
            nodes,
            edges,
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset() -> serde_json::Value {
        let qa = RoadTransport::synthetic_qa_graph();
        let nodes: Vec<_> = qa
            .nodes
            .into_iter()
            .map(|mut node| {
                node.id = node.id.replacen("qa:", "test:", 1);
                node
            })
            .collect();
        let edges: Vec<_> = qa
            .edges
            .into_iter()
            .enumerate()
            .filter_map(|(i, mut edge)| {
                if i != 0 && i != 2 {
                    return None;
                }
                edge.id = edge.id.replacen("qa:", "test:", 1);
                edge.from = edge.from.replacen("qa:", "test:", 1);
                edge.to = edge.to.replacen("qa:", "test:", 1);
                edge.source = "native-pilot:review-fixture".into();
                Some(edge)
            })
            .collect();
        serde_json::json!({
            "format": 1, "asset_id": "review-fixture", "revision": "1",
            "nodes": nodes, "edges": edges,
            "transitions": [{"mode":"bus", "incoming":"test:0-1", "outgoing":"test:1-2"}],
            "bus_stop_positions": [{"edge_id": "test:0-1", "position": 0.25}]
        })
    }

    fn authenticate_value(data: serde_json::Value) -> Result<Approved, RoadTransportError> {
        let bytes = serde_json::to_vec(&data).unwrap();
        Pin {
            id: "review-fixture",
            sha256: Sha256::digest(&bytes).into(),
            bytes: &bytes,
        }
        .authenticate()
    }

    #[test]
    fn authenticates_original_bytes_and_rejects_even_non_semantic_change() {
        let mut bytes = serde_json::to_vec(&asset()).unwrap();
        let sha256 = Sha256::digest(&bytes).into();
        assert!(Pin {
            id: "review-fixture",
            sha256,
            bytes: &bytes
        }
        .authenticate()
        .is_ok());
        bytes.push(b' ');
        assert_eq!(
            Pin {
                id: "review-fixture",
                sha256,
                bytes: &bytes
            }
            .authenticate()
            .err(),
            Some(StaleSnapshot)
        );
    }

    #[test]
    fn validates_native_manifest_and_even_trusted_input_is_not_automatically_permitted() {
        let mut value = asset();
        value["transitions"][0]["outgoing"] = "unknown".into();
        assert_eq!(authenticate_value(value).err(), Some(MissingReference));
        let mut value = asset();
        value["edges"].as_array_mut().unwrap().swap(0, 1);
        assert_eq!(authenticate_value(value).err(), Some(InvalidData));
        let mut value = asset();
        value["asset_id"] = "different".into();
        assert_eq!(authenticate_value(value).err(), Some(InvalidData));
        let mut value = asset();
        value["edges"][0]["level"] = serde_json::Value::Null;
        assert_eq!(authenticate_value(value).err(), Some(AccessForbidden));
    }

    #[test]
    fn production_registry_is_empty_and_qa_and_inert_evidence_remain_independent() {
        let world = WorldDraft::default();
        assert_eq!(import_native_pilot("review-fixture"), Err(StaleSnapshot));
        let approved = authenticate_value(asset()).unwrap();
        let pinned = approved.graph;
        assert_eq!(pinned.validate_world(&world), Err(StaleSnapshot));

        let mut qa = RoadTransport::synthetic_qa_graph();
        assert!(qa.validate_world(&world).is_ok());
        assert_eq!(
            qa.native_shortest_path(&world, "qa:0", "qa:2", PilotRoadMode::Bus),
            Ok(vec!["qa:0-1".into(), "qa:1-2".into()])
        );
        qa.edges[0].source = "self-asserted".into();
        assert_eq!(qa.validate_world(&world), Err(AccessForbidden));
        for edge in &mut qa.edges {
            edge.bus_access = Access::Unknown;
            edge.tram_access = Access::Forbidden;
        }
        assert!(qa.validate_world(&world).is_ok());
        assert_eq!(
            qa.native_shortest_path(&world, "qa:0", "qa:2", PilotRoadMode::Bus),
            Err(AccessForbidden)
        );
    }

    #[test]
    fn one_pinned_graph_uses_correct_direction_only_and_separate_mode_rules() {
        let world = WorldDraft::default();
        let bytes = serde_json::to_vec(&asset()).unwrap();
        let pins = [Pin {
            id: "review-fixture",
            sha256: Sha256::digest(&bytes).into(),
            bytes: &bytes,
        }];
        let graph = approved_asset("review-fixture", &pins).unwrap().graph;
        let restored: RoadTransport =
            serde_json::from_slice(&serde_json::to_vec(&graph).unwrap()).unwrap();
        let approved = approved_for(&restored, &pins).unwrap().unwrap();
        let gate = approved.gate(&restored, &world).unwrap();
        assert_eq!(
            gate.shortest_path(PilotRoadMode::Bus, "test:0", "test:2"),
            Ok(vec!["test:0-1".into(), "test:1-2".into()])
        );
        assert_eq!(
            gate.shortest_path(PilotRoadMode::Bus, "test:2", "test:0"),
            Err(DisconnectedPath)
        );
        assert_eq!(
            gate.shortest_path(PilotRoadMode::Tram, "test:0", "test:2"),
            Err(DisconnectedPath)
        );
        let mut tampered = restored;
        tampered.edges[0].id.push_str("-new");
        assert_eq!(approved.gate(&tampered, &world).err(), Some(StaleSnapshot));
    }

    #[test]
    fn road_movement_does_not_authorize_arbitrary_boarding_locations() {
        use crate::world::{Company, CompanyId, PlayerId};
        let mut world = WorldDraft::default();
        world.companies.push(Company {
            id: CompanyId(1),
            owner: PlayerId(2),
            name: "test".into(),
        });
        let approved = authenticate_value(asset()).unwrap();
        let mut graph = approved.graph.clone();
        graph.bus_stops.push(BusStop {
            id: "A".into(),
            owner: CompanyId(1),
            name: "Test A".into(),
            edge_id: "test:0-1".into(),
            position: 0.25,
        });
        assert!(approved.gate(&graph, &world).is_ok());
        graph.bus_stops[0].position = 0.26;
        assert_eq!(approved.gate(&graph, &world).err(), Some(AccessForbidden));
    }
}
