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
    region: String,
    origin: String,
    attribution: String,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StopPosition {
    pub edge_id: String,
    pub position: f64,
}

/// Authoritative editor request: expected route is an atomic compare, not a replacement graph.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PilotBusEdit {
    pub route_id: String,
    pub name: String,
    pub expected_route: Option<BusRoute>,
    pub from: StopPosition,
    pub to: StopPosition,
}

#[derive(Serialize)]
pub struct NativeRoadRegion {
    id: String,
    region: String,
    origin: String,
    revision: String,
    attribution: String,
    bus_stop_positions: Vec<StopPosition>,
}

struct Approved {
    graph: RoadTransport,
    source: String,
    region: NativeRoadRegion,
    transitions: Vec<Movement>,
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
            || !label(&asset.region)
            || !label(&asset.origin)
            || !label(&asset.attribution)
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
            region: NativeRoadRegion {
                id: asset.asset_id,
                region: asset.region,
                origin: asset.origin,
                revision: asset.revision,
                attribution: asset.attribution,
                bus_stop_positions: asset.bus_stop_positions,
            },
            transitions: asset.transitions,
        };
        approved.gate(&approved.graph, &WorldDraft::default())?;
        let edges = indexed(&approved.graph.edges, |edge| &edge.id)?;
        let mut seen = BTreeSet::new();
        for stop in &approved.region.bus_stop_positions {
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
            &self.region.revision,
            &rules,
        )?;
        let allowed: BTreeSet<_> = self
            .region
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

impl Approved {
    fn bus_path(
        &self,
        graph: &RoadTransport,
        world: &WorldDraft,
        from: &StopPosition,
        to: &StopPosition,
    ) -> Result<Vec<String>, RoadTransportError> {
        for target in [from, to] {
            if !self
                .region
                .bus_stop_positions
                .iter()
                .any(|s| s.edge_id == target.edge_id && s.position == target.position)
            {
                return Err(AccessForbidden);
            }
        }
        self.gate(graph, world)?.bus_path_between(
            &from.edge_id,
            from.position,
            &to.edge_id,
            to.position,
        )
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
    with_pins(|pins| approved_asset(asset_id, pins)).map(|approved| approved.graph)
}

fn approved_asset(asset_id: &str, pins: &[Pin<'_>]) -> Result<Approved, RoadTransportError> {
    pins.iter()
        .find(|pin| pin.id == asset_id)
        .ok_or(StaleSnapshot)?
        .authenticate()
}

/// Describe only readable, integrity-checked release pins. Empty is not a network error.
pub fn native_road_regions() -> Result<Vec<NativeRoadRegion>, RoadTransportError> {
    with_pins(|pins| {
        pins.iter()
            .map(|pin| Ok(pin.authenticate()?.region))
            .collect()
    })
}

#[cfg(test)]
thread_local! {
    static TEST_PIN: std::cell::RefCell<Option<([u8; 32], Vec<u8>)>> =
        const { std::cell::RefCell::new(None) };
}

fn with_pins<T>(f: impl FnOnce(&[Pin<'_>]) -> T) -> T {
    #[cfg(test)]
    return TEST_PIN.with(|test| {
        let test = test.borrow();
        if let Some((sha256, bytes)) = test.as_ref() {
            return f(&[Pin {
                id: "review-fixture",
                sha256: *sha256,
                bytes,
            }]);
        }
        f(PINS)
    });
    #[cfg(not(test))]
    f(PINS)
}

impl RoadTransport {
    /// Recognize the reserved source namespace, not evidence of authorization.
    pub fn uses_native_pilot_source(&self) -> bool {
        self.edges
            .iter()
            .any(|edge| edge.source.starts_with(SOURCE_PREFIX))
    }

    /// Game-world boundary: non-routable map evidence remains inspectable, but
    /// a graph may assert permission only through an approved asset or the exact QA seed.
    pub(crate) fn validate_world(&self, world: &WorldDraft) -> Result<(), RoadTransportError> {
        if let Some(pinned) = with_pins(|pins| approved_for(self, pins))? {
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
        if let Some(pinned) = with_pins(|pins| approved_for(self, pins))? {
            return pinned.gate(self, world)?.shortest_path(mode, from, to);
        }
        if self.has_qa_graph() {
            return self.shortest_path(world, from, to, mode == PilotRoadMode::Tram);
        }
        Err(AccessForbidden)
    }

    /// Only native shared-road sections need road authority when a rail
    /// service travels opposite the direction of the section's frozen edges.
    pub(crate) fn validate_reverse_native_tram(
        &self,
        world: &WorldDraft,
        reversed: &BTreeSet<AlignmentId>,
    ) -> Result<(), RoadTransportError> {
        if reversed.is_empty() || !self.uses_native_pilot_source() {
            return Ok(());
        }
        let mut paths = self
            .tram_sections
            .iter()
            .filter(|section| reversed.contains(&section.alignment_id))
            .filter_map(|section| match &section.placement {
                TramPlacement::SharedRoad { edges } => Some(edges),
                TramPlacement::Dedicated => None,
            })
            .peekable();
        if paths.peek().is_none() {
            return Ok(());
        }
        let pinned = with_pins(|pins| approved_for(self, pins))?.ok_or(AccessForbidden)?;
        let gate = pinned.gate(self, world)?;
        for path in paths {
            gate.validate_reverse_tram_path(path)?;
        }
        Ok(())
    }

    /// Use the reviewed boarding edge, not adjacent nodes or nearest geometry.
    pub fn native_bus_path(
        &self,
        world: &WorldDraft,
        from: &StopPosition,
        to: &StopPosition,
    ) -> Result<Vec<String>, RoadTransportError> {
        let pinned = with_pins(|pins| approved_for(self, pins))?.ok_or(AccessForbidden)?;
        pinned.bus_path(self, world, from, to)
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
            "region":"Test region", "origin":"Test source", "attribution":"No real data",
            "nodes": nodes, "edges": edges,
            "transitions": [{"mode":"bus", "incoming":"test:0-1", "outgoing":"test:1-2"}],
            "bus_stop_positions": [{"edge_id": "test:0-1", "position": 0.25}, {"edge_id":"test:1-2", "position":0.75}]
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
    fn shared_road_projection_uses_native_gate_without_registering_fixture() {
        let id = "synthetic-shared-road";
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/road-pack/fixtures/shared-road-fixture/native-expected.json"
        ));
        assert_eq!(import_native_pilot(id), Err(StaleSnapshot));
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            "ca71ffe54c0c8b53f741f56fc0ee50bcf16cc26bae16389ec8f8a6a572d213ac"
        );
        let approved = Pin {
            id,
            bytes,
            sha256: Sha256::digest(bytes).into(),
        }
        .authenticate()
        .unwrap();
        let world = WorldDraft::default();
        let stops = &approved.region.bus_stop_positions;
        for (from, to, legs) in [(0, 1, 2), (0, 2, 4)] {
            assert_eq!(
                approved
                    .bus_path(&approved.graph, &world, &stops[from], &stops[to])
                    .unwrap()
                    .len(),
                legs
            );
        }
        assert!(approved
            .bus_path(&approved.graph, &world, &stops[2], &stops[0])
            .is_err());
        let unreviewed_stop = StopPosition {
            edge_id: stops[2].edge_id.clone(),
            position: 0.8,
        };
        assert_eq!(
            approved.bus_path(&approved.graph, &world, &stops[0], &unreviewed_stop),
            Err(AccessForbidden)
        );
        let gate = approved.gate(&approved.graph, &world).unwrap();
        assert!(gate
            .shortest_path(PilotRoadMode::Bus, "osm:1", "osm:7")
            .is_err());
        assert!(gate
            .shortest_path(PilotRoadMode::Tram, "osm:1", "osm:5")
            .is_err());
        assert!(gate
            .shortest_path(PilotRoadMode::Tram, "osm:5", "osm:4")
            .is_ok());

        let mut forged = approved.graph.clone();
        forged.edges[0].tram_access = Access::Allowed;
        assert_eq!(approved.gate(&forged, &world).err(), Some(StaleSnapshot));
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

    #[test]
    fn native_tram_reversal_requires_exact_approved_opposite_edge_for_all_rail_operations() {
        use crate::operation::{OperationError, Timetable, Vehicle};
        use crate::world::{OperationServiceConfig, RouteSegment, World};

        // Try a dedicated control, then one-way, a mismatched opposite
        // carriageway, and an expressly authorized opposite track traversal.
        for (shared, reverse) in [
            (false, None),
            (true, None),
            (true, Some(false)),
            (true, Some(true)),
        ] {
            let mut fixture = asset();
            fixture["transitions"] = serde_json::json!([]);
            fixture["bus_stop_positions"] = serde_json::json!([]);
            let edges = fixture["edges"].as_array_mut().unwrap();
            edges.truncate(1);
            let outbound: RoadEdge = serde_json::from_value(edges[0].clone()).unwrap();
            if let Some(exact) = reverse {
                let mut edge = outbound.clone();
                edge.id = "test:1-0".into();
                (edge.from, edge.to) = (edge.to, edge.from);
                edge.geometry.reverse();
                if !exact {
                    let a = edge.geometry[0];
                    let b = edge.geometry[1];
                    edge.geometry.insert(
                        1,
                        GeoPoint {
                            latitude: (a.latitude + b.latitude) / 2.0 + 0.00001,
                            longitude: (a.longitude + b.longitude) / 2.0,
                        },
                    );
                }
                edges.push(serde_json::to_value(edge).unwrap());
            }
            let bytes = serde_json::to_vec(&fixture).unwrap();
            TEST_PIN.replace(Some((Sha256::digest(&bytes).into(), bytes)));
            struct Clear;
            impl Drop for Clear {
                fn drop(&mut self) {
                    TEST_PIN.replace(None);
                }
            }
            let _clear = Clear;

            let mut world = World::new(WorldDraft::default()).unwrap();
            let player = world.add_player("Player".into()).unwrap();
            let owner = world.add_company(player, "Company".into()).unwrap();
            let mut draft = world.snapshot().clone();
            draft.road_transport = import_native_pilot("review-fixture").unwrap();
            world.replace(draft).unwrap();
            let line = world
                .build_alignment_typed(
                    owner,
                    "Tram".into(),
                    outbound.geometry.clone(),
                    TrackType::Tram,
                )
                .unwrap();
            let mut draft = world.snapshot().clone();
            draft.road_transport.tram_sections.push(TramSection {
                alignment_id: line,
                alignment_geometry: outbound.geometry,
                from_position: 0.0,
                to_position: 1.0,
                placement: if shared {
                    TramPlacement::SharedRoad {
                        edges: vec![outbound.id],
                    }
                } else {
                    TramPlacement::Dedicated
                },
            });
            world.replace(draft).unwrap();
            let a = world
                .place_station_on_alignment(owner, "A".into(), line, 0.2, 40., 8.)
                .unwrap();
            let b = world
                .place_station_on_alignment(owner, "B".into(), line, 0.8, 40., 8.)
                .unwrap();
            let mut add = |from, to| {
                let segments = vec![RouteSegment::Alignment {
                    alignment_id: line,
                    from,
                    to,
                }];
                let stops = world
                    .preview_service_path(from, segments.clone())
                    .unwrap()
                    .occurrences
                    .into_iter()
                    .map(|occurrence| occurrence.reference)
                    .collect();
                world
                    .add_service_path(owner, "Tram service".into(), from, segments, stops)
                    .unwrap()
            };
            let outbound = add(a, b);
            let inbound = add(b, a);
            let config = |service_id| OperationServiceConfig {
                service_id,
                timetable: Timetable {
                    first_departure_ms: 0,
                    last_departure_ms: 0,
                    headway_ms: 60_000,
                },
                vehicle: Vehicle {
                    max_speed_kmh: 35.,
                    acceleration_mps2: 1.,
                    braking_mps2: 1.2,
                    length_m: 12.,
                },
                dwell_ms: 10_000,
            };
            let reverse_allowed = !shared || reverse == Some(true);
            let outward = world.compile_operation(&config(outbound)).unwrap();
            assert_eq!(outward.legs.len(), if reverse_allowed { 2 } else { 1 });
            assert_eq!(outward.legs[0].to, b.0.to_string());
            world.configure_operation(vec![config(outbound)]).unwrap();
            let before = world.snapshot().clone();
            let saved = world.operation_save();
            let result = world.configure_operation(vec![config(inbound)]);
            if reverse_allowed {
                result.unwrap();
                assert_eq!(
                    world.compile_operation(&config(inbound)).unwrap().legs[0].to,
                    a.0.to_string()
                );
            } else {
                assert_eq!(result, Err(OperationError::InvalidPath));
                assert_eq!(world.snapshot(), &before);
                assert_eq!(world.operation_save(), saved);

                // A previously compiled, live reverse spec cannot get through
                // normal save restore by carrying its own rail resources.
                let mut dedicated = World::new(before.clone()).unwrap();
                let mut draft = dedicated.snapshot().clone();
                draft.road_transport.tram_sections[0].placement = TramPlacement::Dedicated;
                dedicated.replace(draft).unwrap();
                dedicated
                    .configure_operation(vec![config(inbound)])
                    .unwrap();
                assert_eq!(
                    world.restore_operation(dedicated.operation_save()),
                    Err(OperationError::InvalidPath)
                );
                assert_eq!(world.snapshot(), &before);
                assert_eq!(world.operation_save(), saved);
            }
        }
    }

    #[test]
    fn injected_registry_drives_world_mutations_and_save_restore_boundary() {
        use crate::world::{World, WorldError};
        let mut fixture = asset();
        fixture["transitions"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "mode":"tram", "incoming":"test:0-1", "outgoing":"test:1-2"
            }));
        let bytes = serde_json::to_vec(&fixture).unwrap();
        TEST_PIN.replace(Some((Sha256::digest(&bytes).into(), bytes)));
        struct Clear;
        impl Drop for Clear {
            fn drop(&mut self) {
                TEST_PIN.replace(None);
            }
        }
        let _cleanup = Clear;
        assert_eq!(native_road_regions().unwrap()[0].region, "Test region");
        assert_eq!(import_native_pilot("../../forged.json"), Err(StaleSnapshot));
        let mut world = World::new(WorldDraft::default()).unwrap();
        let player = world.add_player("test".into()).unwrap();
        let owner = world.add_company(player, "test".into()).unwrap();
        let empty = world.snapshot().clone();
        let mut d = empty.clone();
        d.road_transport = import_native_pilot("review-fixture").unwrap();
        world.replace(d.clone()).unwrap();
        let from = StopPosition {
            edge_id: "test:0-1".into(),
            position: 0.25,
        };
        let to = StopPosition {
            edge_id: "test:1-2".into(),
            position: 0.75,
        };
        let edges = d.road_transport.native_bus_path(&d, &from, &to).unwrap();
        assert_eq!(edges, ["test:0-1", "test:1-2"]);
        assert_eq!(
            d.road_transport.native_bus_path(&d, &to, &from),
            Err(DisconnectedPath)
        );
        assert_eq!(
            d.road_transport.native_bus_path(
                &d,
                &StopPosition {
                    position: 0.5,
                    ..from.clone()
                },
                &to
            ),
            Err(AccessForbidden)
        );
        let tram = d
            .road_transport
            .native_shortest_path(&d, "test:0", "test:2", PilotRoadMode::Tram)
            .unwrap();
        assert_eq!(tram, edges);
        for (k, p) in [from, to].into_iter().enumerate() {
            d.road_transport.bus_stops.push(BusStop {
                id: format!("s{k}"),
                owner,
                name: format!("Stop {k}"),
                edge_id: p.edge_id,
                position: p.position,
            });
        }
        d.road_transport.bus_routes.push(BusRoute {
            id: "r".into(),
            owner,
            name: "Bus name".into(),
            edges,
            stops: vec![
                BusStopOccurrence {
                    stop_id: "s0".into(),
                    edge_index: 0,
                },
                BusStopOccurrence {
                    stop_id: "s1".into(),
                    edge_index: 1,
                },
            ],
        });
        world.replace(d.clone()).unwrap();
        let mut points: Vec<_> = tram
            .iter()
            .flat_map(|id| {
                d.road_transport
                    .edges
                    .iter()
                    .find(|e| e.id == *id)
                    .unwrap()
                    .geometry
                    .clone()
            })
            .collect();
        points.dedup();
        let alignment_id = world
            .build_alignment_typed(owner, "Tram".into(), points.clone(), TrackType::Tram)
            .unwrap();
        let mut changed = world.snapshot().clone();
        let shared = TramPlacement::SharedRoad { edges: tram };
        changed.road_transport.tram_sections.push(TramSection {
            alignment_id,
            alignment_geometry: points,
            from_position: 0.0,
            to_position: 1.0,
            placement: shared.clone(),
        });
        world.replace(changed.clone()).unwrap();
        changed.road_transport.tram_sections[0].placement = TramPlacement::Dedicated;
        world.replace(changed.clone()).unwrap();
        changed.road_transport.tram_sections[0].placement = shared;
        changed.road_transport.bus_routes[0].name = "Renamed".into();
        world.replace(changed.clone()).unwrap();
        let mut bad = changed.clone();
        bad.road_transport.bus_stops[0].position = 0.5;
        assert_eq!(world.replace(bad), Err(WorldError::InvalidRoadTransport));
        assert_eq!(world.snapshot(), &changed);
        let decoded: WorldDraft =
            serde_json::from_slice(&serde_json::to_vec(world.snapshot()).unwrap()).unwrap();
        let mut reopened = World::from_saved_draft(decoded, Default::default()).unwrap();
        let mut removed = reopened.snapshot().clone();
        removed.road_transport.bus_routes.clear();
        removed.road_transport.tram_sections.clear();
        removed.alignments.clear();
        reopened.replace(removed.clone()).unwrap();
        assert!(reopened.snapshot().road_transport.bus_routes.is_empty());
        // Restore the pre-removal snapshot through the same validator used by history.
        let mut restored = World::from_saved_draft(changed.clone(), Default::default()).unwrap();
        assert_eq!(restored.snapshot(), &changed);
        restored.replace(removed).unwrap();
        assert!(restored.snapshot().alignments.is_empty());
        assert!(reopened.snapshot().road_transport.bus_routes.is_empty());
    }
}
