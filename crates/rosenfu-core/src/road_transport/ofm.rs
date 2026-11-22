use super::*;
use crate::ofm_evidence::{BusPath, BusPathBinding, BusPattern, MAX_PINNED_BINDINGS};
use crate::world::InfrastructureLevel;

const OFM_SOURCE: &str = "openfreemap_gameplay_bus";
const MAX_EDGES_PER_PATH: usize = 4096; // existing RoadTransport per-route edge limit
const MAX_TOTAL_ROUTE_EDGES: usize = 32_768;
pub(crate) const MAX_ORDINARY_ROUTES: usize = 64;

fn merge<T: PartialEq>(
    union: &mut BTreeMap<String, T>,
    key: String,
    item: T,
) -> Result<(), RoadTransportError> {
    use std::collections::btree_map::Entry;
    match union.entry(key) {
        Entry::Vacant(e) => {
            e.insert(item);
            Ok(())
        }
        Entry::Occupied(e) if e.get() == &item => Ok(()),
        _ => Err(InvalidData),
    }
}

fn project_route(
    path: &BusPath,
    route: &BusRoute,
    nodes: &mut BTreeMap<String, RoadNode>,
    edges: &mut BTreeMap<String, RoadEdge>,
    stops: &mut Vec<BusStop>,
) -> Result<BusRoute, RoadTransportError> {
    if !label(&route.id)
        || route.id.len() > 160
        || !label(&route.name)
        || path.steps.len() + 1 != path.points.len()
        || path.points_grid.len() != path.points.len()
        || !(1..=MAX_EDGES_PER_PATH).contains(&path.steps.len())
    {
        return Err(InvalidData);
    }
    let mut ids = Vec::with_capacity(path.steps.len());
    for i in 0..path.steps.len() {
        let id = path.source_atom_id(i).ok_or(InvalidData)?;
        let revision = path.source_atom_raw_sha(i).ok_or(InvalidData)?.to_owned();
        let point = |j: usize| GeoPoint {
            longitude: path.points[j][0],
            latitude: path.points[j][1],
        };
        let from = path.points_grid[i].id();
        let to = path.points_grid[i + 1].id();
        if from == to {
            return Err(InvalidData);
        }
        merge(
            nodes,
            from.clone(),
            RoadNode {
                id: from.clone(),
                point: point(i),
            },
        )?;
        merge(
            nodes,
            to.clone(),
            RoadNode {
                id: to.clone(),
                point: point(i + 1),
            },
        )?;
        merge(
            edges,
            id.clone(),
            RoadEdge {
                id: id.clone(),
                from,
                to,
                source: OFM_SOURCE.into(),
                revision,
                geometry: vec![point(i), point(i + 1)],
                bus_access: Access::Allowed,
                tram_access: Access::Unknown,
                level: Some(InfrastructureLevel::Ground),
            },
        )?;
        ids.push(id);
    }
    let mut occurrences = vec![];
    for (name, edge_index, position) in [("起点", 0, 0.), ("終点", ids.len() - 1, 1.)] {
        let id = format!("ofm-bus:{}:{name}", route.id);
        stops.push(BusStop {
            id: id.clone(),
            owner: route.owner,
            name: name.into(),
            edge_id: ids[edge_index].clone(),
            position,
            location_kind: BusStopLocationKind::PlayerCreated,
        });
        occurrences.push(BusStopOccurrence {
            stop_id: id,
            edge_index,
        });
    }
    Ok(BusRoute {
        id: route.id.clone(),
        owner: route.owner,
        name: route.name.clone(),
        edges: ids,
        stops: occurrences,
    })
}

fn outward_evidence_error(error: crate::ofm_evidence::Error) -> RoadTransportError {
    match error {
        crate::ofm_evidence::Error::NoBusLeg(index) => UnreachableBusLeg(index),
        _ => StaleSnapshot,
    }
}
fn project_world_routes(
    world: &WorldDraft,
    metadata: BTreeMap<&str, &BusRoute>,
) -> Result<RoadTransport, RoadTransportError> {
    let bindings = &world.ofm_gameplay_bus_routes;
    if bindings.len() != metadata.len() || bindings.len() > MAX_ORDINARY_ROUTES {
        return Err(InvalidData);
    }
    // Separate cap on cold route-solver work, even for a malformed save
    // requesting many long proofs, and aggregate materialized occurrences.
    let mut total_bindings = 0usize;
    for proof in bindings.values() {
        let directions = if proof.pattern.is_some() { 2 } else { 1 };
        total_bindings = total_bindings
            .checked_add(directions * proof.tile_keys().map_err(|_| InvalidData)?.len())
            .ok_or(InvalidData)?;
        if total_bindings > MAX_PINNED_BINDINGS {
            return Err(InvalidData);
        }
    }
    let (mut nodes, mut edges) = (BTreeMap::new(), BTreeMap::new());
    let (mut routes, mut stops) = (Vec::new(), Vec::new());
    let mut total_edges = 0;
    for (id, binding) in bindings {
        let route = metadata.get(id.as_str()).ok_or(InvalidData)?;
        let path = world
            .runtime_road_evidence
            .stored_gameplay_bus_path(binding)
            .map_err(outward_evidence_error)?
            .ok_or(DisconnectedPath)?;
        total_edges += path.steps.len();
        if binding.pattern == Some(BusPattern::OpenRoundTrip) {
            let returned = world
                .checked_gameplay_bus_return(&path)?
                .ok_or(MissingReturnPath)?;
            total_edges += returned.steps.len();
        }
        if total_edges > MAX_TOTAL_ROUTE_EDGES {
            return Err(InvalidData);
        }
        routes.push(project_route(
            &path, route, &mut nodes, &mut edges, &mut stops,
        )?);
    }
    Ok(RoadTransport {
        nodes: nodes.into_values().collect(),
        edges: edges.into_values().collect(),
        bus_stops: stops,
        bus_routes: routes,
        tram_sections: world.road_transport.tram_sections.clone(),
        ..RoadTransport::default()
    })
}

fn same_point(a: GeoPoint, b: GeoPoint) -> bool {
    (a.longitude - b.longitude).abs() <= 1e-10 && (a.latitude - b.latitude).abs() <= 1e-10
}

/// Preserve previously validated geometry at unchanged atoms on a different
/// libm/platform. New edges sharing a node use the same canonical persisted
/// coordinates, so adding an unrelated route never changes a live Bus spec.
fn retain_existing_points(
    old: &RoadTransport,
    new: &mut RoadTransport,
) -> Result<(), RoadTransportError> {
    let prior = indexed(&old.nodes, |n| &n.id)?;
    for n in &mut new.nodes {
        if let Some(before) = prior.get(n.id.as_str()) {
            if !same_point(n.point, before.point) {
                return Err(InvalidData);
            }
            n.point = before.point;
        }
    }
    let points = indexed(&new.nodes, |n| &n.id)?;
    for e in &mut new.edges {
        e.geometry[0] = points.get(e.from.as_str()).ok_or(InvalidData)?.point;
        e.geometry[1] = points.get(e.to.as_str()).ok_or(InvalidData)?.point;
    }
    Ok(())
}

impl RoadTransport {
    pub(super) fn validate_ofm_gameplay_bus(
        &self,
        world: &WorldDraft,
    ) -> Result<(), RoadTransportError> {
        let metadata = indexed(&self.bus_routes, |x| &x.id)?;
        let mut expected = project_world_routes(world, metadata)?;
        // Tolerate only sub-millimetre libm latitude variation; runtime uses
        // original persisted geometry after verifying the exact wire identities.
        if expected.nodes.len() != self.nodes.len() || expected.edges.len() != self.edges.len() {
            return Err(InvalidData);
        }
        for (got, want) in self.nodes.iter().zip(&mut expected.nodes) {
            if !same_point(got.point, want.point) {
                return Err(InvalidData);
            }
            want.point = got.point;
        }
        for (got, want) in self.edges.iter().zip(&mut expected.edges) {
            if got.geometry.len() != want.geometry.len()
                || !got
                    .geometry
                    .iter()
                    .zip(&want.geometry)
                    .all(|(&a, &b)| same_point(a, b))
            {
                return Err(InvalidData);
            }
            want.geometry.clone_from(&got.geometry);
        }
        if self != &expected {
            return Err(InvalidData);
        }
        self.validate(world)
    }
}

impl WorldDraft {
    pub fn gameplay_bus_return_path(
        &self,
        request: &BusPathBinding,
    ) -> Result<Option<std::sync::Arc<BusPath>>, RoadTransportError> {
        let outbound = self
            .runtime_road_evidence
            .stored_gameplay_bus_path(request)
            .map_err(outward_evidence_error)?
            .ok_or(DisconnectedPath)?;
        self.checked_gameplay_bus_return(&outbound)
    }

    fn checked_gameplay_bus_return(
        &self,
        outbound: &BusPath,
    ) -> Result<Option<std::sync::Arc<BusPath>>, RoadTransportError> {
        let mut reversed = outbound
            .binding
            .swapped_endpoints()
            .map_err(|_| StaleSnapshot)?;
        // Strict direction: geometry controls author the outbound shape only.
        // Requiring them again could send an otherwise simple return around
        // an entire extra loop to reach a one-way waypoint in reverse order.
        // There are not yet intermediate passenger stop obligations on return.
        if reversed.respect_oneway {
            reversed.via.clear();
        }
        let result = if reversed.pattern.is_some() {
            self.runtime_road_evidence
                .stored_gameplay_bus_path(&reversed)
        } else {
            self.runtime_road_evidence.gameplay_bus_path(&reversed)
        };
        let returned = match result {
            Err(crate::ofm_evidence::Error::NoBusLeg(_)) => None,
            Err(_) => return Err(StaleSnapshot),
            Ok(found) => found,
        };
        if returned.as_ref().is_some_and(|path| {
            outbound.points_grid.first() != path.points_grid.last()
                || outbound.points_grid.last() != path.points_grid.first()
                || path.steps.len() > MAX_EDGES_PER_PATH
                || (outbound.binding.pattern.is_some()
                    && (outbound.operation_length_mm == 0 || path.operation_length_mm == 0))
        }) {
            return Err(InvalidData);
        }
        Ok(returned)
    }

    /// Admit only addressed raw context through the shared immutable archive.
    /// No unrelated hints, graph data, or source bytes can enter via this edit.
    pub(crate) fn admit_gameplay_ofm_bus_tiles(
        mut self,
        request: &BusPathBinding,
        cached: Vec<(
            crate::ofm_evidence::RawEvidence,
            crate::ofm_evidence::TileBinding,
        )>,
    ) -> Result<Self, RoadTransportError> {
        let keys = request.tile_keys().map_err(|_| StaleSnapshot)?;
        if cached.len() > keys.len() {
            return Err(StaleSnapshot);
        }
        for (raw, binding) in &cached {
            let digest = if request.tile_key == binding.canonical_url {
                &request.tile_raw_sha256
            } else {
                &request
                    .additional_tiles
                    .iter()
                    .find(|p| p.tile_key == binding.canonical_url)
                    .ok_or(StaleSnapshot)?
                    .tile_raw_sha256
            };
            if digest != &binding.raw_sha256 || digest != &raw.sha256 {
                return Err(StaleSnapshot);
            }
        }
        if cached.is_empty() {
            return Ok(self);
        }
        let (raw, bindings) = cached.into_iter().unzip();
        let store = self
            .runtime_road_evidence
            .with_pinned_additions(raw, bindings)
            .map_err(|_| StaleSnapshot)?;
        self.road_evidence_descriptors = store.descriptors();
        self.runtime_road_evidence = store;
        Ok(self)
    }

    /// Admit one independent route without replacing another route's metadata.
    pub(crate) fn put_ofm_gameplay_bus(
        mut self,
        owner: CompanyId,
        id: String,
        name: String,
        binding: BusPathBinding,
    ) -> Result<Self, RoadTransportError> {
        let road = &self.road_transport;
        if self.ofm_gameplay_bus_routes.is_empty()
            && (!road.nodes.is_empty()
                || !road.edges.is_empty()
                || !road.evidence.is_empty()
                || !road.bus_routes.is_empty()
                || !road.bus_stops.is_empty())
        {
            return Err(AccessForbidden);
        }
        if road
            .bus_routes
            .iter()
            .any(|r| r.id == id && r.owner != owner)
        {
            return Err(WrongOwner);
        }
        // Only change metadata of the edited route. Re-project the deterministic
        // union from verified paths; unused edges/stops never survive.
        let updated = BusRoute {
            id: id.clone(),
            owner,
            name,
            edges: vec![],
            stops: vec![],
        };
        let mut metadata = indexed(&road.bus_routes, |r| &r.id)?;
        metadata.insert(id.as_str(), &updated);
        self.ofm_gameplay_bus_routes.insert(id.clone(), binding);
        let mut projected = project_world_routes(&self, metadata)?;
        retain_existing_points(&self.road_transport, &mut projected)?;
        self.road_transport = projected;
        Ok(self)
    }

    pub fn remove_ofm_gameplay_bus(
        mut self,
        owner: CompanyId,
        id: &str,
    ) -> Result<Self, RoadTransportError> {
        if !self.ofm_gameplay_bus_routes.contains_key(id)
            || !self
                .road_transport
                .bus_routes
                .iter()
                .any(|r| r.id == id && r.owner == owner)
        {
            return Err(WrongOwner);
        }
        let mut metadata = indexed(&self.road_transport.bus_routes, |r| &r.id)?;
        metadata.remove(id);
        self.ofm_gameplay_bus_routes.remove(id);
        let mut projected = project_world_routes(&self, metadata)?;
        retain_existing_points(&self.road_transport, &mut projected)?;
        self.road_transport = projected;
        Ok(self)
    }
}
