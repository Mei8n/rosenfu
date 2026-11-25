//! Exact installed witness projection. This first gate admits only an
//! isolated, one-way route; it never rewrites or extends the legacy road graph.
use super::*;
use crate::ofm_evidence::{
    raw_sha256, replay_installed_westbound_seam, road_grid_lon_lat, InstalledWestboundSeamRecipe,
    WestboundSeamTraversal,
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
) -> Result<RoadTransport, RoadTransportError> {
    if !label(id) || !label(name) || !world.ofm_gameplay_bus_routes.is_empty() {
        return Err(AccessForbidden);
    }
    let proof = traversal(world, recipe)?;
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
    let graph = RoadTransport {
        nodes,
        edges,
        bus_stops,
        bus_routes: vec![route],
        ..RoadTransport::default()
    };
    graph.validate(world)?;
    Ok(graph)
}

pub(super) fn validate_installation(
    roads: &RoadTransport,
    world: &WorldDraft,
) -> Result<(), RoadTransportError> {
    let [route] = roads.bus_routes.as_slice() else {
        return Err(AccessForbidden);
    };
    if world.ofm_installed_seam_bus_routes.len() != 1 || !world.ofm_gameplay_bus_routes.is_empty() {
        return Err(AccessForbidden);
    }
    let recipe = world
        .ofm_installed_seam_bus_routes
        .get(&route.id)
        .ok_or(AccessForbidden)?;
    let mut expected = project(world, route.owner, &route.id, &route.name, recipe)?;
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
    let Some(recipe) = world.ofm_installed_seam_bus_routes.get(id) else {
        return Ok(vec![]);
    };
    Ok(traversal(world, recipe)?.fragment_resources().into())
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
            || self.snapshot().road_transport != RoadTransport::default()
            || !self.snapshot().ofm_installed_seam_bus_routes.is_empty()
        {
            return Err(AccessForbidden);
        }
        let mut draft = self.snapshot().clone();
        draft.road_transport = project(&draft, owner, &id, &name, &recipe)?;
        draft.ofm_installed_seam_bus_routes.insert(id, recipe);
        let mut candidate = self.clone();
        candidate.replace(draft).map_err(|_| InvalidData)?;
        Ok(candidate)
    }
}
