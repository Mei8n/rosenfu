use super::*;
use crate::ofm_evidence::{
    raw_sha256, BusInteriorStop, BusPath, BusPathBinding, BusPattern, BUS_STOP_DENOMINATOR,
    MAX_PINNED_BINDINGS,
};
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

const MAX_INTERIOR_STOPS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct StopPosition {
    atom: usize,
    fraction_q: u32,
}

#[derive(Debug, Clone)]
struct ResolvedStop<'a> {
    data: &'a crate::ofm_evidence::BusInteriorStop,
    outward: StopPosition,
    back: Option<StopPosition>,
}

fn interior_stop_id(route_id: &str, local_id: &str) -> String {
    let input = format!("ofm-bus-interior-stop-v1\0{route_id}\0{local_id}");
    format!("ofm-bus:stop:{}", raw_sha256(input.as_bytes()))
}

fn resolve_interior_stops<'a>(
    binding: &'a BusPathBinding,
    outward: &BusPath,
    returned: Option<&BusPath>,
) -> Result<Vec<ResolvedStop<'a>>, RoadTransportError> {
    if binding.interior_stops.is_empty() {
        return Ok(vec![]);
    }
    if binding.interior_stops.len() > MAX_INTERIOR_STOPS {
        return Err(InvalidData);
    }
    let keys = outward.stop_occurrence_keys();
    if keys.len() != outward.steps.len() {
        return Err(InvalidData);
    }
    let mut occurrences = BTreeMap::new();
    for (index, key) in keys.into_iter().enumerate() {
        if occurrences.insert(key, index).is_some() {
            return Err(AmbiguousBusStopOccurrence);
        }
    }
    let mut reverse_atoms: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    if let Some(returned) = returned {
        for i in 0..returned.steps.len() {
            reverse_atoms
                .entry(returned.physical_atom_id(i).ok_or(InvalidData)?)
                .or_default()
                .push(i);
        }
    }
    let mut ids = BTreeSet::new();
    let mut positions = BTreeSet::new();
    let mut stops = Vec::with_capacity(binding.interior_stops.len());
    for data in &binding.interior_stops {
        if data.id.is_empty()
            || data.id.len() > 80
            || !data
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || !ids.insert(data.id.as_str())
            || !label(&data.name)
            || !(1..BUS_STOP_DENOMINATOR).contains(&data.fraction_q)
        {
            return Err(InvalidData);
        }
        let atom = *occurrences
            .get(&data.occurrence_key)
            .ok_or(MissingBusStopOccurrence)?;
        let position = StopPosition {
            atom,
            fraction_q: data.fraction_q,
        };
        if !positions.insert(position) {
            return Err(InvalidStopOrder);
        }
        let back = if let Some(returned) = returned {
            let key = outward.physical_atom_id(atom).ok_or(InvalidData)?;
            let candidates = reverse_atoms.get(&key).map_or(&[][..], Vec::as_slice);
            let (from, to) = outward.control_pair(atom);
            let reversed = (
                BusPath::on_return_control(to),
                BusPath::on_return_control(from),
            );
            let mut matching = candidates
                .iter()
                .copied()
                .filter(|&i| returned.control_pair(i) == reversed);
            let first = matching.next();
            let selected = if first.is_some() && matching.next().is_none() {
                first
            } else if candidates.len() == 1 {
                candidates.first().copied()
            } else {
                None
            };
            selected.map(|index| {
                let outward_step = outward.steps[atom];
                let return_step = returned.steps[index];
                let same =
                    (outward_step.from < outward_step.to) == (return_step.from < return_step.to);
                StopPosition {
                    atom: index,
                    fraction_q: if same {
                        data.fraction_q
                    } else {
                        BUS_STOP_DENOMINATOR - data.fraction_q
                    },
                }
            })
        } else {
            None
        };
        stops.push(ResolvedStop {
            data,
            outward: position,
            back,
        });
    }
    let mut counts = BTreeMap::new();
    for p in stops.iter().filter_map(|s| s.back) {
        *counts.entry(p).or_insert(0) += 1;
    }
    for s in &mut stops {
        // Two authored outward occurrences must not silently become the same
        // return stop occurrence. Neither is announced on the ambiguous leg.
        if s.back.is_some_and(|p| counts[&p] > 1) {
            s.back = None;
        }
    }
    stops.sort_by_key(|s| s.outward);
    Ok(stops)
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameplayBusInteriorStopView {
    pub id: String,
    pub stop_id: String,
    pub name: String,
    pub point: GeoPoint,
    pub serves_return: bool,
}

impl BusPath {
    fn stop_point(&self, p: StopPosition) -> Result<GeoPoint, RoadTransportError> {
        let [a, b] = self.points.get(p.atom..=p.atom + 1).ok_or(InvalidData)? else {
            return Err(InvalidData);
        };
        let q = f64::from(p.fraction_q) / f64::from(BUS_STOP_DENOMINATOR);
        Ok(GeoPoint {
            longitude: a[0] + (b[0] - a[0]) * q,
            latitude: a[1] + (b[1] - a[1]) * q,
        })
    }
}

const AUTO_BOARDING_GAP_UM: u128 = 2_000_000_000;
const AUTO_MARGIN_UM: u128 = 5_000_000;

fn auto_stop_suggestions(
    binding: &BusPathBinding,
    path: &BusPath,
) -> Result<Vec<BusInteriorStop>, RoadTransportError> {
    let placed = resolve_interior_stops(binding, path, None)?;
    let capacity = MAX_INTERIOR_STOPS.saturating_sub(placed.len());
    if capacity == 0 || path.steps.is_empty() {
        return Ok(vec![]);
    }
    if path.steps.len() != path.atom_operation_micrometres.len() {
        return Err(InvalidData);
    }
    let mut offset = Vec::with_capacity(path.steps.len() + 1);
    offset.push(0_u128);
    for &um in &path.atom_operation_micrometres {
        offset.push(offset.last().ok_or(InvalidData)? + u128::from(um));
    }
    let total = *offset.last().ok_or(InvalidData)?;
    let mut fixed = vec![0];
    let mut physical_seen = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let physical_position = |atom: usize, q: u32| -> Result<_, RoadTransportError> {
        let step = path.steps.get(atom).ok_or(InvalidData)?;
        let direction_agnostic_q = if step.from < step.to {
            q
        } else {
            BUS_STOP_DENOMINATOR - q
        };
        Ok((
            path.physical_atom_id(atom).ok_or(InvalidData)?,
            direction_agnostic_q,
        ))
    };
    for stop in placed {
        let StopPosition { atom, fraction_q } = stop.outward;
        let base = offset[atom]
            + u128::from(path.atom_operation_micrometres[atom]) * u128::from(fraction_q)
                / u128::from(BUS_STOP_DENOMINATOR);
        fixed.push(base);
        physical_seen.insert(physical_position(atom, fraction_q)?);
        ids.insert(stop.data.id.as_str());
    }
    fixed.push(total);
    let count = |gap: u128, threshold: u128| gap.saturating_sub(1) / threshold;
    let requested: u128 = fixed
        .windows(2)
        .map(|p| count(p[1].saturating_sub(p[0]), AUTO_BOARDING_GAP_UM))
        .sum();
    if requested == 0 {
        return Ok(vec![]);
    }
    // Preserve all hand positions. On exceptionally long routes, widen the
    // target uniformly rather than truncate suggestions at one end.
    let threshold = if requested > capacity as u128 {
        AUTO_BOARDING_GAP_UM.max(total / (capacity as u128 + 1) + 1)
    } else {
        AUTO_BOARDING_GAP_UM
    };
    let keys = path.stop_occurrence_keys();
    if keys.len() != path.steps.len() {
        return Err(InvalidData);
    }
    let mut answer = vec![];
    for gap in fixed.windows(2) {
        let [a, b] = [gap[0], gap[1]];
        let n = count(b.saturating_sub(a), threshold);
        for i in 1..=n {
            let target = a + (b - a) * i / (n + 1);
            // Exact vertex ideals choose the following atom's interior,
            // never a node/terminus/turn itself. Margin is limited by length.
            let atom = offset.partition_point(|&o| o <= target).saturating_sub(1);
            let Some(&length) = path.atom_operation_micrometres.get(atom) else {
                continue;
            };
            let length = u128::from(length);
            if length < 4 {
                continue;
            }
            let margin = AUTO_MARGIN_UM.min(length / 4);
            let q = ((target - offset[atom]).clamp(margin, length - margin)
                * u128::from(BUS_STOP_DENOMINATOR)
                + length / 2)
                / length;
            let q = (q as u32).clamp(1, BUS_STOP_DENOMINATOR - 1);
            let placed = offset[atom] + length * u128::from(q) / u128::from(BUS_STOP_DENOMINATOR);
            if placed <= a || placed >= b || !physical_seen.insert(physical_position(atom, q)?) {
                continue;
            }
            let key = &keys[atom];
            let identity = format!("ofm-bus-auto-passenger-stop-v1\0{key}\0{q}");
            let id = format!("auto_{}", raw_sha256(identity.as_bytes()));
            if ids.contains(id.as_str()) {
                continue;
            }
            answer.push(BusInteriorStop {
                id,
                name: format!("停留所{}", answer.len() + 1),
                occurrence_key: key.clone(),
                fraction_q: q,
            });
        }
    }
    if answer.len() > capacity {
        return Err(InvalidData);
    }
    Ok(answer)
}

/// An unsplit circuit needs a real interior boarding point. Place it near
/// half the measured lap, on an exact route atom (never on a nearby road).
fn loop_fallback(
    binding: &BusPathBinding,
    path: &BusPath,
) -> Result<Option<StopPosition>, RoadTransportError> {
    if binding.pattern != Some(BusPattern::ClosedOneWay) || !binding.interior_stops.is_empty() {
        return Ok(None);
    }
    let total: u128 = path
        .atom_operation_micrometres
        .iter()
        .map(|&n| u128::from(n))
        .sum();
    let target = total / 2;
    let mut offset = 0;
    for (atom, &length) in path.atom_operation_micrometres.iter().enumerate() {
        let length = u128::from(length);
        let end = offset + length;
        if target < end && length > 0 {
            let den = u128::from(BUS_STOP_DENOMINATOR);
            let q = ((target - offset) * den + length / 2) / length;
            return Ok(Some(StopPosition {
                atom,
                fraction_q: (q as u32).clamp(1, BUS_STOP_DENOMINATOR - 1),
            }));
        }
        offset = end;
    }
    Err(InvalidStopOrder)
}

fn loop_fallback_id(route_id: &str) -> String {
    format!("ofm-bus:{route_id}:環状中間")
}

fn closed_legs(
    path: &BusPath,
    route_id: &str,
    binding: &BusPathBinding,
) -> Result<Vec<BusLeg>, RoadTransportError> {
    let mut middle: Vec<_> = resolve_interior_stops(binding, path, None)?
        .iter()
        .map(|s| (s.outward, interior_stop_id(route_id, &s.data.id)))
        .collect();
    if let Some(fallback) = loop_fallback(binding, path)? {
        middle.push((fallback, loop_fallback_id(route_id)));
    }
    let seam_stop = format!("ofm-bus:{route_id}:起点");
    metric_legs(path, &seam_stop, &seam_stop, middle)
}

fn project_route(
    path: &BusPath,
    binding: &BusPathBinding,
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
        let from = path.node_id_at(i).ok_or(InvalidData)?;
        let to = path.node_id_at(i + 1).ok_or(InvalidData)?;
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
                level: path
                    .atom_is_ground(i)
                    .then_some(InfrastructureLevel::Ground),
            },
        )?;
        ids.push(id);
    }
    let mut occurrences = vec![];
    let mut add = |id: String, name: String, index: usize, fraction_q: u32| {
        stops.push(BusStop {
            id: id.clone(),
            owner: route.owner,
            name,
            edge_id: ids[index].clone(),
            position: f64::from(fraction_q) / f64::from(BUS_STOP_DENOMINATOR),
            location_kind: BusStopLocationKind::PlayerCreated,
        });
        occurrences.push(BusStopOccurrence {
            stop_id: id,
            edge_index: index,
        });
    };
    add(format!("ofm-bus:{}:起点", route.id), "起点".into(), 0, 0);
    for stop in resolve_interior_stops(binding, path, None)? {
        add(
            interior_stop_id(&route.id, &stop.data.id),
            stop.data.name.clone(),
            stop.outward.atom,
            stop.outward.fraction_q,
        );
    }
    if binding.pattern == Some(BusPattern::ClosedOneWay) {
        if let Some(p) = loop_fallback(binding, path)? {
            add(
                loop_fallback_id(&route.id),
                "中間停留所".into(),
                p.atom,
                p.fraction_q,
            );
        }
        // The lap closes at the first logical stop, not a second BusStop.
        // Compile both positive legs now as well as during operation restore.
        closed_legs(path, &route.id, binding)?;
    } else {
        add(
            format!("ofm-bus:{}:終点", route.id),
            "終点".into(),
            ids.len() - 1,
            BUS_STOP_DENOMINATOR,
        );
    }
    Ok(BusRoute {
        id: route.id.clone(),
        owner: route.owner,
        name: route.name.clone(),
        edges: ids,
        stops: occurrences,
    })
}

fn metric_legs(
    path: &BusPath,
    from: &str,
    to: &str,
    middle: Vec<(StopPosition, String)>,
) -> Result<Vec<BusLeg>, RoadTransportError> {
    if path.steps.is_empty() || path.steps.len() != path.atom_operation_micrometres.len() {
        return Err(InvalidData);
    }
    let mut points = vec![(
        StopPosition {
            atom: 0,
            fraction_q: 0,
        },
        from.to_owned(),
    )];
    points.extend(middle);
    points.push((
        StopPosition {
            atom: path.steps.len() - 1,
            fraction_q: BUS_STOP_DENOMINATOR,
        },
        to.to_owned(),
    ));
    let mut cumulative = vec![0u128];
    for &um in &path.atom_operation_micrometres {
        cumulative.push(cumulative.last().ok_or(InvalidData)? + u128::from(um));
    }
    let mm = |p: StopPosition| {
        let um = cumulative[p.atom]
            + u128::from(path.atom_operation_micrometres[p.atom]) * u128::from(p.fraction_q)
                / u128::from(BUS_STOP_DENOMINATOR);
        (um + 500) / 1000
    };
    points
        .windows(2)
        .map(|w| {
            let [(p, a), (q, b)] = w else {
                return Err(InvalidData);
            };
            let len = mm(*q)
                .checked_sub(mm(*p))
                .filter(|&n| n > 0)
                .ok_or(InvalidStopOrder)?;
            let traversed_edges = (p.atom..=q.atom)
                .map(|i| path.source_atom_id(i).ok_or(InvalidData))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(BusLeg {
                traversed_edges,
                from_stop_id: a.clone(),
                to_stop_id: b.clone(),
                length_m: len as f64 / 1000.,
            })
        })
        .collect()
}

fn outward_evidence_error(error: crate::ofm_evidence::Error) -> RoadTransportError {
    match error {
        crate::ofm_evidence::Error::NoBusLeg(index) => UnreachableBusLeg(index),
        crate::ofm_evidence::Error::InvalidBusCircuit => InvalidBusCircuit,
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
        let directions = if proof.pattern == Some(BusPattern::OpenRoundTrip) {
            2
        } else {
            1
        };
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
            &path, binding, route, &mut nodes, &mut edges, &mut stops,
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
    /// Edges/routes are sorted by ID in the validated ordinary OFM
    /// projection. Stop rows are appended in occurrence order per route.
    fn gameplay_bus_leg_span(
        &self,
        route_id: &str,
        leg: usize,
        from_id: &str,
        to_id: &str,
    ) -> Option<(&[String], usize, f64, f64)> {
        if self.bus_routes.len() > MAX_ORDINARY_ROUTES || self.edges.len() > MAX_TOTAL_ROUTE_EDGES {
            return None;
        }
        let index = self
            .bus_routes
            .binary_search_by(|r| r.id.as_str().cmp(route_id))
            .ok()?;
        let route = &self.bus_routes[index];
        let next = leg.checked_add(1)?;
        let from = route.stops.get(leg)?;
        let to = route.stops.get(next)?;
        if from.stop_id != from_id || to.stop_id != to_id {
            return None;
        }
        let edge_ids = route.edges.get(from.edge_index..=to.edge_index)?;
        let offset = self.bus_routes[..index]
            .iter()
            .map(|r| r.stops.len())
            .sum::<usize>();
        let a = self.bus_stops.get(offset.checked_add(leg)?)?;
        let z = self.bus_stops.get(offset.checked_add(next)?)?;
        if a.id != from_id
            || z.id != to_id
            || &a.edge_id != edge_ids.first()?
            || &z.edge_id != edge_ids.last()?
        {
            return None;
        }
        Some((edge_ids, from.edge_index, a.position, z.position))
    }

    fn gameplay_bus_edge(&self, edge_id: &str) -> Option<&RoadEdge> {
        let index = self
            .edges
            .binary_search_by(|e| e.id.as_str().cmp(edge_id))
            .ok()?;
        self.edges.get(index)
    }

    pub(crate) fn gameplay_bus_leg_chord(
        &self,
        route_id: &str,
        leg: usize,
        from_id: &str,
        to_id: &str,
        edge_id: &str,
    ) -> Option<(&RoadEdge, f64, f64)> {
        let (edges, _, from, to) = self.gameplay_bus_leg_span(route_id, leg, from_id, to_id)?;
        let [only] = edges else { return None };
        if only != edge_id {
            return None;
        }
        Some((self.gameplay_bus_edge(edge_id)?, from, to))
    }

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
    /// Project one part of a short, validated outward bus leg. Bus scheduling
    /// uses integer millimetres at these same absolute atom cuts; a haversine
    /// approximation could choose the wrong displayed edge near a junction.
    pub(crate) fn gameplay_bus_live_outward_chord(
        &self,
        route_id: &str,
        leg_index: usize,
        leg: &crate::operation::RunLeg,
        section: &crate::operation::Section,
        fraction: f64,
    ) -> Option<(&RoadEdge, f64)> {
        const MAX_OUTWARD_ATOMS: usize = 16;
        let binding = self.ofm_gameplay_bus_routes.get(route_id)?;
        if binding.pattern != Some(BusPattern::OpenRoundTrip) {
            return None;
        }
        let (edges, first, from, to) = self.road_transport.gameplay_bus_leg_span(
            route_id,
            leg_index,
            leg.from.strip_prefix("bus:")?,
            leg.to.strip_prefix("bus:")?,
        )?;
        if !(2..=MAX_OUTWARD_ATOMS).contains(&edges.len())
            || section.resources.len() != edges.len()
            || !section
                .resources
                .iter()
                .zip(edges)
                .all(|(resource, edge)| resource.strip_prefix("road:edge:") == Some(edge))
        {
            return None;
        }
        let path = self
            .runtime_road_evidence
            .peek_stored_gameplay_bus_path(binding)?;
        let lengths = &path.atom_operation_micrometres;
        if lengths.len() > MAX_OUTWARD_ATOMS || lengths.len() != path.steps.len() {
            return None;
        }
        let last = first.checked_add(edges.len() - 1)?;
        // Convert the same saved fractions projected from integer stop cuts.
        let q = |v: f64| {
            if !(0.0..=1.0).contains(&v) {
                return None;
            }
            let q = (v * f64::from(BUS_STOP_DENOMINATOR)).round() as u32;
            (f64::from(q) / f64::from(BUS_STOP_DENOMINATOR) == v).then_some(q)
        };
        let (from_q, to_q) = (q(from)?, q(to)?);
        let mut abs = [0u128; MAX_OUTWARD_ATOMS + 1];
        for (i, &length) in lengths.iter().enumerate() {
            abs[i + 1] = abs[i] + u128::from(length);
        }
        let cut = |i: usize, fraction_q: u32| {
            Some(
                (abs.get(i)?
                    + u128::from(*lengths.get(i)?) * u128::from(fraction_q)
                        / u128::from(BUS_STOP_DENOMINATOR)
                    + 500)
                    / 1000,
            )
        };
        let start = cut(first, from_q)?;
        let end = cut(last, to_q)?;
        let mm = end.checked_sub(start)?;
        if mm == 0
            || mm as f64 / 1000. != section.length_m
            || !(first..=last).all(|i| path.atom_is_ground(i))
        {
            return None;
        }
        let travelled = fraction * mm as f64;
        for (index, edge) in edges.iter().enumerate() {
            let atom = first + index;
            let a = if atom == first { from_q } else { 0 };
            let z = if atom == last {
                to_q
            } else {
                BUS_STOP_DENOMINATOR
            };
            let lower = cut(atom, a)?.checked_sub(start)?;
            let upper = cut(atom, z)?.checked_sub(start)?;
            if travelled <= upper as f64 && upper > lower {
                let progress = ((travelled - lower as f64) / (upper - lower) as f64).clamp(0., 1.);
                let position =
                    (f64::from(a) + f64::from(z - a) * progress) / f64::from(BUS_STOP_DENOMINATOR);
                return Some((self.road_transport.gameplay_bus_edge(edge)?, position));
            }
        }
        None
    }

    /// The last direct return leg is proved by the compiled service plus
    /// its retained independently solved path. Outward road/stop rows do
    /// not describe a return detour. Missing proof is a display omission.
    pub(crate) fn gameplay_bus_live_direct_return_chord(
        &self,
        route_id: &str,
        leg_index: usize,
        spec: &crate::operation::ServiceRunSpec,
        section: &crate::operation::Section,
        fraction: f64,
    ) -> Option<(GeoPoint, GeoPoint, f64)> {
        const MAX_RETURN_ATOMS: usize = 16;
        let binding = self.ofm_gameplay_bus_routes.get(route_id)?;
        if binding.pattern != Some(BusPattern::OpenRoundTrip)
            || !(1..=MAX_RETURN_ATOMS).contains(&section.resources.len())
            || leg_index.checked_add(1)? != spec.legs.len()
        {
            return None;
        }
        let roads = &self.road_transport;
        let index = roads
            .bus_routes
            .binary_search_by(|r| r.id.as_str().cmp(route_id))
            .ok()?;
        let route = roads.bus_routes.get(index)?;
        let leg = spec.legs.get(leg_index)?;
        // The number of outward intervals comes from its exact stop
        // occurrences. Reject *all* return legs when a stop serves return.
        if leg_index == 0
            || leg_index.checked_add(1)? != route.stops.len()
            || leg.from.strip_prefix("bus:")? != route.stops.last()?.stop_id
            || leg.to.strip_prefix("bus:")? != route.stops.first()?.stop_id
        {
            return None;
        }

        let outward = self
            .runtime_road_evidence
            .peek_stored_gameplay_bus_path(binding)?;
        let return_binding = Self::gameplay_bus_return_binding(&outward).ok()?;
        let path = self
            .runtime_road_evidence
            .peek_stored_gameplay_bus_path(&return_binding)?;
        let lengths = &path.atom_operation_micrometres;
        if !Self::gameplay_bus_return_is_valid(&outward, &path)
            || lengths.len() != section.resources.len()
            || lengths.len() != path.steps.len()
            || path.points.len() != lengths.len().checked_add(1)?
            || lengths.iter().any(|&length| length == 0)
            || !(0..lengths.len()).all(|i| path.atom_is_ground(i))
            || !section.resources.iter().enumerate().all(|(i, resource)| {
                resource
                    .strip_prefix("road:edge:")
                    .is_some_and(|id| path.source_atom_id(i).as_deref() == Some(id))
            })
        {
            return None;
        }
        let total_um = lengths.iter().map(|&n| u128::from(n)).sum::<u128>();
        let total_mm = (total_um + 500) / 1000;
        if total_mm == 0
            || total_mm != u128::from(path.operation_length_mm)
            || total_mm as f64 / 1000. != section.length_m
        {
            return None;
        }
        let distance_mm = fraction * total_mm as f64;
        let mut prefix_um = 0;
        for (i, &len) in lengths.iter().enumerate() {
            let lo = (prefix_um + 500) / 1000;
            prefix_um += u128::from(len);
            let hi = (prefix_um + 500) / 1000;
            if hi > lo && distance_mm <= hi as f64 {
                let position = ((distance_mm - lo as f64) / (hi - lo) as f64).clamp(0., 1.);
                let point = |i: usize| {
                    let &[longitude, latitude] = path.points.get(i)?;
                    Some(GeoPoint {
                        latitude,
                        longitude,
                    })
                };
                let (start, end) = (point(i)?, point(i + 1)?);
                return (start != end).then_some((start, end, position));
            }
        }
        None
    }

    /// Generate addressed candidate stops in existing manual gaps. This is
    /// read only; the ordinary route Confirm is the only persistence path.
    pub fn gameplay_bus_auto_stop_suggestions(
        &self,
        binding: &BusPathBinding,
    ) -> Result<Vec<BusInteriorStop>, RoadTransportError> {
        let path = self
            .runtime_road_evidence
            .stored_gameplay_bus_path(binding)
            .map_err(outward_evidence_error)?
            .ok_or(DisconnectedPath)?;
        auto_stop_suggestions(binding, &path)
    }
    pub fn gameplay_bus_interior_stop_views(
        &self,
        route_id: &str,
        binding: &BusPathBinding,
        returned: Option<&BusPath>,
    ) -> Result<Vec<GameplayBusInteriorStopView>, RoadTransportError> {
        let outward = self
            .runtime_road_evidence
            .stored_gameplay_bus_path(binding)
            .map_err(outward_evidence_error)?
            .ok_or(DisconnectedPath)?;
        resolve_interior_stops(binding, &outward, returned)?
            .into_iter()
            .map(|stop| {
                Ok(GameplayBusInteriorStopView {
                    id: stop.data.id.clone(),
                    stop_id: interior_stop_id(route_id, &stop.data.id),
                    name: stop.data.name.clone(),
                    point: outward.stop_point(stop.outward)?,
                    serves_return: stop.back.is_some(),
                })
            })
            .collect()
    }

    pub(super) fn gameplay_bus_closed_legs(
        &self,
        route_id: &str,
        binding: &BusPathBinding,
    ) -> Result<Vec<BusLeg>, RoadTransportError> {
        let path = self
            .runtime_road_evidence
            .stored_gameplay_bus_path(binding)
            .map_err(outward_evidence_error)?
            .ok_or(DisconnectedPath)?;
        closed_legs(&path, route_id, binding)
    }

    pub fn gameplay_bus_loop_fallback_view(
        &self,
        route_id: &str,
        binding: &BusPathBinding,
    ) -> Result<Option<GameplayBusInteriorStopView>, RoadTransportError> {
        if binding.pattern != Some(BusPattern::ClosedOneWay) || !binding.interior_stops.is_empty() {
            return Ok(None);
        }
        let path = self
            .runtime_road_evidence
            .stored_gameplay_bus_path(binding)
            .map_err(outward_evidence_error)?
            .ok_or(DisconnectedPath)?;
        loop_fallback(binding, &path)?
            .map(|p| {
                Ok(GameplayBusInteriorStopView {
                    id: "loop_fallback".into(),
                    stop_id: loop_fallback_id(route_id),
                    name: "中間停留所".into(),
                    point: path.stop_point(p)?,
                    serves_return: false,
                })
            })
            .transpose()
    }

    /// Deterministic native outbound/return legs using the same authored stop
    /// IDs as the existing RoadTransport projection and OD manager.
    pub(crate) fn gameplay_bus_roundtrip_legs(
        &self,
        route_id: &str,
        binding: &BusPathBinding,
    ) -> Result<(Vec<BusLeg>, usize), RoadTransportError> {
        let outward = self
            .runtime_road_evidence
            .stored_gameplay_bus_path(binding)
            .map_err(outward_evidence_error)?
            .ok_or(DisconnectedPath)?;
        let returned = self
            .checked_gameplay_bus_return(&outward)?
            .ok_or(MissingReturnPath)?;
        let stops = resolve_interior_stops(binding, &outward, Some(&returned))?;
        let a = format!("ofm-bus:{route_id}:起点");
        let z = format!("ofm-bus:{route_id}:終点");
        let mut back: Vec<_> = stops
            .iter()
            .filter_map(|s| s.back.map(|p| (p, interior_stop_id(route_id, &s.data.id))))
            .collect();
        back.sort_by_key(|(p, _)| *p);
        let forward = stops
            .iter()
            .map(|s| (s.outward, interior_stop_id(route_id, &s.data.id)))
            .collect();
        let mut legs = metric_legs(&outward, &a, &z, forward)?;
        let terminal_arrival = legs.len() - 1;
        legs.extend(metric_legs(&returned, &z, &a, back)?);
        Ok((legs, terminal_arrival))
    }

    pub fn gameplay_bus_return_path(
        &self,
        request: &BusPathBinding,
    ) -> Result<Option<std::sync::Arc<BusPath>>, RoadTransportError> {
        if request.pattern == Some(BusPattern::ClosedOneWay) {
            return Ok(None);
        }
        let outbound = self
            .runtime_road_evidence
            .stored_gameplay_bus_path(request)
            .map_err(outward_evidence_error)?
            .ok_or(DisconnectedPath)?;
        self.checked_gameplay_bus_return(&outbound)
    }

    fn gameplay_bus_return_binding(
        outward: &BusPath,
    ) -> Result<BusPathBinding, RoadTransportError> {
        let mut reversed = outward
            .binding
            .swapped_endpoints()
            .map_err(|_| StaleSnapshot)?;
        // For strict one-way travel, a forward shaping control can be
        // impossible to visit on the independent return. It is not a stop.
        if reversed.respect_oneway {
            reversed.via.clear();
        }
        Ok(reversed)
    }

    fn gameplay_bus_return_is_valid(outward: &BusPath, path: &BusPath) -> bool {
        outward.points_grid.first() == path.points_grid.last()
            && outward.points_grid.last() == path.points_grid.first()
            && path.steps.len() <= MAX_EDGES_PER_PATH
            && (outward.binding.pattern.is_none()
                || (outward.operation_length_mm != 0 && path.operation_length_mm != 0))
    }

    fn checked_gameplay_bus_return(
        &self,
        outbound: &BusPath,
    ) -> Result<Option<std::sync::Arc<BusPath>>, RoadTransportError> {
        let reversed = Self::gameplay_bus_return_binding(outbound)?;
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
        if returned
            .as_ref()
            .is_some_and(|path| !Self::gameplay_bus_return_is_valid(outbound, path))
        {
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

    /// One local authoring preview from the existing immutable road-tile input.
    /// This consumes a detached draft: the real world does not gain a Bus
    /// route, new authority or retained data merely by looking at a guide.
    pub fn authored_tram_road_guide(
        self,
        request: &crate::ofm_evidence::AuthoredTramGuide,
        cached: Vec<(
            crate::ofm_evidence::RawEvidence,
            crate::ofm_evidence::TileBinding,
        )>,
    ) -> Result<Option<Vec<[f64; 2]>>, RoadTransportError> {
        let staged = self.admit_gameplay_ofm_bus_tiles(&request.evidence_request(), cached)?;
        crate::ofm_evidence::reconstruct_authored_tram_guide(&staged.runtime_road_evidence, request)
            .map_err(outward_evidence_error)
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
