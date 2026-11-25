//! Exact installed witness projection. One one-way route may coexist
//! with bounded classic OFM routes on the witnessed archive only.
use super::*;
use crate::ofm_evidence::{
    raw_sha256, replay_installed_westbound_seam, road_grid_lon_lat, BusPathBinding,
    InstalledWestboundSeamRecipe, WestboundSeamTraversal, BUS_GRAPH_RULE,
};
use crate::world::{InfrastructureLevel, World};

fn traversal(
    world: &WorldDraft,
    recipe: &InstalledWestboundSeamRecipe,
) -> Result<WestboundSeamTraversal, RoadTransportError> {
    replay_installed_westbound_seam(&world.runtime_road_evidence, recipe)
        .map_err(|_| AccessForbidden)
}

fn project(
    world: &WorldDraft,
    owner: CompanyId,
    id: &str,
    name: &str,
    recipe: &InstalledWestboundSeamRecipe,
    metadata: BTreeMap<&str, &BusRoute>,
) -> Result<RoadTransport, RoadTransportError> {
    if !label(id)
        || !label(name)
        || world.ofm_gameplay_bus_routes.contains_key(id)
        || world.ofm_gameplay_bus_routes.len() > 1
        || !world.road_transport.tram_sections.is_empty()
    {
        return Err(AccessForbidden);
    }
    let proof = traversal(world, recipe)?;
    if !world
        .ofm_gameplay_bus_routes
        .values()
        .all(|binding| supported_legacy(binding, &proof))
    {
        return Err(AccessForbidden);
    }
    let mut graph = super::ofm::project_world_routes(world, metadata)?;
    let [east, west] = proof.atoms_in_order();
    let namespace = raw_sha256(recipe.catalog_key.as_bytes());
    let nodes: Vec<_> = proof
        .grid_points()
        .iter()
        .enumerate()
        .map(|(i, &[x, y])| {
            let [longitude, latitude] = road_grid_lon_lat(
                f64::from(east.tile.x) + f64::from(x) / 4096.,
                f64::from(east.tile.y) + f64::from(y) / 4096.,
            );
            RoadNode {
                id: format!("ofm-seam:node:{namespace}:{i}"),
                point: GeoPoint {
                    longitude,
                    latitude,
                },
            }
        })
        .collect();
    let edges: Vec<_> = [east, west]
        .into_iter()
        .enumerate()
        .map(|(i, atom)| {
            let digest = raw_sha256(
                format!(
                    "ofm-seam-atom-v1\0{}\0{}\0{}\0{}",
                    atom.tile.canonical_url,
                    atom.tile.raw_sha256,
                    atom.fragment_id,
                    atom.raw_ordinal
                )
                .as_bytes(),
            );
            RoadEdge {
                id: format!("ofm-seam:edge:{digest}"),
                from: nodes[i].id.clone(),
                to: nodes[i + 1].id.clone(),
                source: atom.fragment_id.clone(),
                revision: atom.tile.revision.clone(),
                geometry: vec![nodes[i].point, nodes[i + 1].point],
                bus_access: Access::Allowed,
                tram_access: Access::Forbidden,
                level: Some(InfrastructureLevel::Ground),
            }
        })
        .collect();
    let bus_stops: Vec<_> = ["start", "end"]
        .into_iter()
        .enumerate()
        .map(|(i, label)| BusStop {
            id: format!("ofm-seam:stop:{namespace}:{label}"),
            owner,
            name: format!("{id} {label}"),
            edge_id: edges[i].id.clone(),
            position: i as f64,
            location_kind: BusStopLocationKind::PlayerCreated,
        })
        .collect();
    let route = BusRoute {
        id: id.to_owned(),
        owner,
        name: name.to_owned(),
        edges: edges.iter().map(|edge| edge.id.clone()).collect(),
        stops: bus_stops
            .iter()
            .enumerate()
            .map(|(i, stop)| BusStopOccurrence {
                stop_id: stop.id.clone(),
                edge_index: i,
            })
            .collect(),
    };
    let insertion = graph
        .bus_routes
        .partition_point(|other| other.id < route.id);
    let mixed = !graph.bus_routes.is_empty();
    let stop_offset: usize = graph.bus_routes[..insertion]
        .iter()
        .map(|other| other.stops.len())
        .sum();
    graph.bus_routes.insert(insertion, route);
    graph.bus_stops.splice(stop_offset..stop_offset, bus_stops);
    graph.nodes.extend(nodes);
    graph.edges.extend(edges);
    if mixed {
        graph.nodes.sort_unstable_by(|a, b| a.id.cmp(&b.id));
        graph.edges.sort_unstable_by(|a, b| a.id.cmp(&b.id));
    }
    graph.validate(world)?;
    Ok(graph)
}

fn supported_legacy(binding: &BusPathBinding, proof: &WestboundSeamTraversal) -> bool {
    // No crossed-version archive guesses, implicit reverse traversals, or
    // alternate structure/stop graphs. Later modes may need more resources.
    binding.rule == BUS_GRAPH_RULE
        && binding.grade_policy.is_none()
        && binding.respect_oneway
        && binding.pattern.is_none()
        && !binding.has_fractional_stops()
        && binding.additional_tiles.is_empty()
        && binding.end_tile_key.is_none()
        && binding.via.is_empty()
        && binding.interior_stops.is_empty()
        && proof.atoms_in_order().iter().any(|atom| {
            atom.tile.canonical_url == binding.tile_key
                && atom.tile.raw_sha256 == binding.tile_raw_sha256
        })
}

pub(super) fn validate_installation(
    roads: &RoadTransport,
    world: &WorldDraft,
) -> Result<(), RoadTransportError> {
    if world.ofm_installed_seam_bus_routes.len() != 1 {
        return Err(AccessForbidden);
    }
    let mut metadata = indexed(&roads.bus_routes, |route| &route.id)?;
    let (id, recipe) = world
        .ofm_installed_seam_bus_routes
        .first_key_value()
        .ok_or(InvalidData)?;
    let route = metadata.remove(id.as_str()).ok_or(InvalidData)?;
    let mut expected = project(world, route.owner, id, &route.name, recipe, metadata)?;
    // The cross-platform libm rendering coordinates are not the proof. Bound
    // rounding variance before comparing topology and all other wire fields.
    let matches = |a: GeoPoint, b: GeoPoint| {
        (a.longitude - b.longitude).abs() <= 1e-10 && (a.latitude - b.latitude).abs() <= 1e-10
    };
    if expected.nodes.len() != roads.nodes.len() || expected.edges.len() != roads.edges.len() {
        return Err(InvalidData);
    }
    for (want, got) in expected.nodes.iter_mut().zip(&roads.nodes) {
        if !matches(want.point, got.point) {
            return Err(InvalidData);
        }
        want.point = got.point;
    }
    for (want, got) in expected.edges.iter_mut().zip(&roads.edges) {
        if want.geometry.len() != got.geometry.len()
            || !want
                .geometry
                .iter()
                .zip(&got.geometry)
                .all(|(&a, &b)| matches(a, b))
        {
            return Err(InvalidData);
        }
        want.geometry.clone_from(&got.geometry);
    }
    if expected != *roads {
        return Err(InvalidData);
    }
    roads.validate(world)
}

pub(crate) fn operation_fragment_resources(
    world: &WorldDraft,
    id: &str,
) -> Result<Vec<String>, RoadTransportError> {
    let Some((seam_id, recipe)) = world.ofm_installed_seam_bus_routes.first_key_value() else {
        return Ok(vec![]);
    };
    let proof = traversal(world, recipe)?;
    if seam_id == id {
        return Ok(proof.fragment_resources().into());
    }
    let legacy = world
        .ofm_gameplay_bus_routes
        .get(id)
        .ok_or(AccessForbidden)?;
    if !supported_legacy(legacy, &proof) {
        return Err(AccessForbidden);
    }
    let path = world
        .runtime_road_evidence
        .stored_gameplay_bus_path(legacy)
        .map_err(|_| InvalidData)?
        .ok_or(DisconnectedPath)?;
    // Reserving a applicable fragment on every leg is conservative but never
    // assumes identity between a seam atom and an old clipped edge.
    Ok(proof.legacy_fragment_resources(&path))
}

impl World {
    /// Return a validated detached candidate. Only an already retained proof in
    /// the installed catalog can grant construction; no external geometry or
    /// digest enters here. UI authorization and distinct storage are separate.
    pub fn candidate_with_installed_seam_bus(
        &self,
        owner: CompanyId,
        id: String,
        name: String,
        recipe: InstalledWestboundSeamRecipe,
    ) -> Result<Self, RoadTransportError> {
        if self.operation().is_some()
            || (self.snapshot().ofm_gameplay_bus_routes.is_empty()
                && self.snapshot().road_transport != RoadTransport::default())
            || !self.snapshot().ofm_installed_seam_bus_routes.is_empty()
        {
            return Err(AccessForbidden);
        }
        let mut draft = self.snapshot().clone();
        let metadata = indexed(&draft.road_transport.bus_routes, |route| &route.id)?;
        let mut projected = project(&draft, owner, &id, &name, &recipe, metadata)?;
        super::ofm::retain_existing_points(&draft.road_transport, &mut projected)?;
        draft.road_transport = projected;
        draft.ofm_installed_seam_bus_routes.insert(id, recipe);
        let mut candidate = self.clone();
        candidate.replace(draft).map_err(|_| InvalidData)?;
        Ok(candidate)
    }
}
