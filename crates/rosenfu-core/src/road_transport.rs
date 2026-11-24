//! Road transport is separate from railway topology and display-only road evidence.
//! Only explicitly verified directed edges may carry a service. Coincident points
//! never create connectivity. The acquisition adapter must retain unknown access.
use crate::world::{AlignmentId, CompanyId, GeoPoint, TrackType, WorldDraft};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
mod native;
mod ofm;
pub use ofm::GameplayBusInteriorStopView;
mod pilot;
pub use native::{
    import_native_pilot, native_road_regions, NativeRoadRegion, PilotBusEdit, PilotBusWaypoint,
    StopPosition,
};
pub use pilot::{BusPathVia, PilotRoadMode, MAX_BUS_WAYPOINTS};
use pilot::{PilotRoad, PilotTransition};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    Allowed,
    Forbidden,
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoadNode {
    pub id: String,
    pub point: GeoPoint,
}
/// One directed traversal; reverse travel needs its own explicitly permitted edge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoadEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub source: String,
    pub revision: String,
    pub geometry: Vec<GeoPoint>,
    pub bus_access: Access,
    pub tram_access: Access,
    #[serde(default)]
    pub level: Option<crate::world::InfrastructureLevel>,
}
/// Legacy/approved references stay distinct from fictional player construction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BusStopLocationKind {
    /// Historical wire default. In native regions this is an exact listed reference.
    #[default]
    ReviewedReference,
    /// A fictional stop. Its verified road edge is real; the stop need not be.
    PlayerCreated,
}
impl BusStopLocationKind {
    fn is_reviewed_reference(&self) -> bool {
        *self == Self::ReviewedReference
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BusStop {
    pub id: String,
    pub owner: CompanyId,
    pub name: String,
    pub edge_id: String,
    pub position: f64,
    #[serde(
        default,
        skip_serializing_if = "BusStopLocationKind::is_reviewed_reference"
    )]
    pub location_kind: BusStopLocationKind,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BusRoute {
    pub id: String,
    pub owner: CompanyId,
    pub name: String,
    pub edges: Vec<String>,
    /// A stop occurrence refers to an edge occurrence, not merely its identifier.
    pub stops: Vec<BusStopOccurrence>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BusStopOccurrence {
    pub stop_id: String,
    pub edge_index: usize,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TramPlacement {
    Dedicated,
    SharedRoad { edges: Vec<String> },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TramSection {
    /// Frozen physical reference. Structural rail edits must detach/rebind explicitly.
    pub alignment_geometry: Vec<GeoPoint>,
    pub alignment_id: AlignmentId,
    pub from_position: f64,
    pub to_position: f64,
    pub placement: TramPlacement,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct RoadTransport {
    #[serde(default)]
    pub evidence: Vec<crate::world::RoadSnapshot>,
    pub nodes: Vec<RoadNode>,
    pub edges: Vec<RoadEdge>,
    pub bus_stops: Vec<BusStop>,
    pub bus_routes: Vec<BusRoute>,
    pub tram_sections: Vec<TramSection>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoadTransportError {
    InvalidData,
    DuplicateId,
    MissingReference,
    DisconnectedPath,
    MissingReturnPath,
    InvalidBusCircuit,
    UnreachableBusLeg(usize),
    MissingBusStopOccurrence,
    AmbiguousBusStopOccurrence,
    AccessUnknown,
    AccessForbidden,
    WrongOwner,
    StaleSnapshot,
    InvalidStopOrder,
    InvalidTramSection,
}
use RoadTransportError::*;
fn label(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 200 && !s.chars().any(char::is_control)
}
fn position(p: f64) -> bool {
    p.is_finite() && (0.0..=1.0).contains(&p)
}
fn point(p: GeoPoint) -> bool {
    p.latitude.is_finite()
        && p.longitude.is_finite()
        && (-90.0..=90.0).contains(&p.latitude)
        && (-180.0..=180.0).contains(&p.longitude)
}
fn indexed<'a, T>(
    xs: &'a [T],
    id: impl Fn(&'a T) -> &'a str,
) -> Result<BTreeMap<&'a str, &'a T>, RoadTransportError> {
    let mut out = BTreeMap::new();
    for x in xs {
        let key = id(x);
        if !label(key) {
            return Err(InvalidData);
        }
        if out.insert(key, x).is_some() {
            return Err(DuplicateId);
        }
    }
    Ok(out)
}
impl RoadTransport {
    /// Preserve real source geometry as evidence, NOT a certified routable network.
    /// Tile seam hypotheses and coincident vertices never become junctions here.
    pub fn from_road_evidence(
        snapshot: &crate::world::RoadSnapshot,
    ) -> Result<Self, RoadTransportError> {
        snapshot.validate().map_err(|_| InvalidData)?;
        if snapshot.roads.len() > 50_000 {
            return Err(InvalidData);
        }
        let mut result = Self::default();
        result.evidence.push(snapshot.clone());
        for (index, way) in snapshot.roads.iter().enumerate() {
            let start = format!("evidence:{index}:start");
            let end = format!("evidence:{index}:end");
            result.nodes.push(RoadNode {
                id: start.clone(),
                point: way.points[0],
            });
            result.nodes.push(RoadNode {
                id: end.clone(),
                point: *way.points.last().ok_or(InvalidData)?,
            });
            let forbidden = way.classification.as_ref().is_some_and(|c| {
                matches!(
                    c.class.as_str(),
                    "path" | "pedestrian" | "steps" | "cycleway"
                ) || c.access.as_deref() == Some("no")
            });
            let permission = if forbidden {
                Access::Forbidden
            } else {
                Access::Unknown
            };
            let one = way
                .classification
                .as_ref()
                .and_then(|c| c.oneway.as_deref());
            for reverse in [false, true] {
                if (reverse && matches!(one, Some("yes" | "1" | "true")))
                    || (!reverse && one == Some("-1"))
                {
                    continue;
                }
                let mut geometry = way.points.clone();
                if reverse {
                    geometry.reverse();
                }
                result.edges.push(RoadEdge {
                    id: format!(
                        "evidence:{index}:{}",
                        if reverse { "reverse" } else { "forward" }
                    ),
                    from: if reverse { end.clone() } else { start.clone() },
                    to: if reverse { start.clone() } else { end.clone() },
                    source: snapshot.source.clone(),
                    revision: snapshot.revision.clone(),
                    geometry,
                    bus_access: permission,
                    tram_access: Access::Unknown,
                    level: None,
                });
            }
        }
        result.validate(&WorldDraft::default())?;
        Ok(result)
    }
    pub fn validate(&self, world: &WorldDraft) -> Result<(), RoadTransportError> {
        if self.nodes.len() > 100_000
            || self.edges.len() > 200_000
            || self.bus_stops.len() > 10_000
            || self.bus_routes.len() > 10_000
            || self.tram_sections.len() > 10_000
        {
            return Err(InvalidData);
        }
        for evidence in &self.evidence {
            evidence.validate().map_err(|_| InvalidData)?;
        }
        if self.edges.iter().map(|e| e.geometry.len()).sum::<usize>() > 500_000
            || self.evidence.len() > 32
        {
            return Err(InvalidData);
        }
        let nodes = indexed(&self.nodes, |x| &x.id)?;
        let edges = indexed(&self.edges, |x| &x.id)?;
        let stops = indexed(&self.bus_stops, |x| &x.id)?;
        indexed(&self.bus_routes, |x| &x.id)?;
        let companies: BTreeSet<_> = world.companies.iter().map(|c| c.id).collect();
        for n in &self.nodes {
            if !point(n.point) {
                return Err(InvalidData);
            }
        }
        for e in &self.edges {
            let a = nodes.get(e.from.as_str()).ok_or(MissingReference)?;
            let b = nodes.get(e.to.as_str()).ok_or(MissingReference)?;
            if !label(&e.source)
                || !label(&e.revision)
                || e.from == e.to
                || e.geometry.len() < 2
                || e.geometry.len() > 10_000
                || e.geometry.first() != Some(&a.point)
                || e.geometry.last() != Some(&b.point)
                || e.geometry.iter().any(|p| !point(*p))
                || e.geometry.windows(2).any(|p| p[0] == p[1])
            {
                return Err(InvalidData);
            }
        }
        if self.edges.iter().any(|edge| {
            let length: f64 = edge
                .geometry
                .windows(2)
                .map(|pair| distance_m(pair[0], pair[1]))
                .sum();
            !length.is_finite() || length <= 0.0
        }) {
            return Err(InvalidData);
        }
        let path = |ids: &[String], tram: bool| -> Result<(), RoadTransportError> {
            if ids.is_empty() || ids.len() > 4096 {
                return Err(InvalidData);
            }
            let mut previous: Option<&RoadEdge> = None;
            for id in ids {
                let e = *edges.get(id.as_str()).ok_or(MissingReference)?;
                match if tram { e.tram_access } else { e.bus_access } {
                    Access::Unknown => return Err(AccessUnknown),
                    Access::Forbidden => return Err(AccessForbidden),
                    Access::Allowed => {}
                }
                if tram && e.level != Some(crate::world::InfrastructureLevel::Ground) {
                    return Err(InvalidTramSection);
                }
                if previous.is_some_and(|p| p.to != e.from) {
                    return Err(DisconnectedPath);
                }
                previous = Some(e);
            }
            Ok(())
        };
        for s in &self.bus_stops {
            if !companies.contains(&s.owner) || !label(&s.name) || !position(s.position) {
                return Err(InvalidData);
            }
            let e = edges.get(s.edge_id.as_str()).ok_or(MissingReference)?;
            match e.bus_access {
                Access::Unknown => return Err(AccessUnknown),
                Access::Forbidden => return Err(AccessForbidden),
                Access::Allowed => {}
            }
        }
        for r in &self.bus_routes {
            if !companies.contains(&r.owner)
                || !label(&r.name)
                || r.stops.len() < 2
                || r.stops.len() > 4096
            {
                return Err(InvalidData);
            }
            path(&r.edges, false)?;
            let mut prior: Option<(usize, f64)> = None;
            for occurrence in &r.stops {
                let s = stops
                    .get(occurrence.stop_id.as_str())
                    .ok_or(MissingReference)?;
                if s.owner != r.owner {
                    return Err(WrongOwner);
                }
                if r.edges.get(occurrence.edge_index) != Some(&s.edge_id) {
                    return Err(MissingReference);
                }
                let current = (occurrence.edge_index, s.position);
                if prior.is_some_and(|p| {
                    p >= current || (p.0 + 1 == current.0 && p.1 == 1.0 && current.1 == 0.0)
                }) {
                    return Err(InvalidStopOrder);
                }
                prior = Some(current);
            }
        }
        let mut ranges: BTreeMap<AlignmentId, Vec<(f64, f64)>> = BTreeMap::new();
        for s in &self.tram_sections {
            let a = world
                .alignments
                .iter()
                .find(|a| a.id == s.alignment_id)
                .ok_or(MissingReference)?;
            if a.points != s.alignment_geometry
                || a.track_type != TrackType::Tram
                || !position(s.from_position)
                || !position(s.to_position)
                || s.from_position >= s.to_position
            {
                return Err(InvalidTramSection);
            }
            let spans = ranges.entry(s.alignment_id).or_default();
            if spans
                .iter()
                .any(|(from, to)| s.from_position < *to && s.to_position > *from)
            {
                return Err(InvalidTramSection);
            }
            spans.push((s.from_position, s.to_position));
            if let TramPlacement::SharedRoad { edges: ids } = &s.placement {
                path(ids, true)?;
                // Deliberately narrow until a verified lane/grade adapter exists:
                // only an entire ground alignment with exact directed sample identity.
                if s.from_position != 0.0
                    || s.to_position != 1.0
                    || ids.iter().any(|id| {
                        edges[id.as_str()].level != Some(crate::world::InfrastructureLevel::Ground)
                    })
                {
                    return Err(InvalidTramSection);
                }
                let mut road_points: Vec<GeoPoint> = ids
                    .iter()
                    .flat_map(|id| edges[id.as_str()].geometry.iter().copied())
                    .collect();
                road_points.dedup();
                if road_points != a.points
                    || a.infrastructure
                        .iter()
                        .any(|i| i.level != crate::world::InfrastructureLevel::Ground)
                {
                    return Err(InvalidTramSection);
                }
                for profile in a.vertical_profile.iter().chain(a.section_profiles.iter()) {
                    if profile.start_height_m != 0.0
                        || profile.end_height_m != 0.0
                        || profile.start_tangent_m != 0.0
                        || profile.end_tangent_m != 0.0
                    {
                        return Err(InvalidTramSection);
                    }
                }
            }
        }
        Ok(())
    }
    /// Synthetic QA path by geographic polyline length. Real roads must use the native pilot gate.
    /// Unknown/forbidden edges are excluded, never treated as usable roads.
    /// A bounded search returns an explicit error rather than a partial path.
    fn shortest_path(
        &self,
        world: &WorldDraft,
        from: &str,
        to: &str,
        tram: bool,
    ) -> Result<Vec<String>, RoadTransportError> {
        let graph = Self {
            nodes: self.nodes.clone(),
            edges: self.edges.clone(),
            ..Self::default()
        };
        graph.validate(world)?;
        if from == to
            || !self.nodes.iter().any(|n| n.id == from)
            || !self.nodes.iter().any(|n| n.id == to)
        {
            return Err(InvalidData);
        }
        #[derive(PartialEq)]
        struct Entry(f64, String);
        impl Eq for Entry {}
        impl Ord for Entry {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                other
                    .0
                    .total_cmp(&self.0)
                    .then_with(|| other.1.cmp(&self.1))
            }
        }
        impl PartialOrd for Entry {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
        let mut outgoing: BTreeMap<&str, Vec<&RoadEdge>> = BTreeMap::new();
        for edge in &self.edges {
            if (if tram {
                if edge.level == Some(crate::world::InfrastructureLevel::Ground) {
                    edge.tram_access
                } else {
                    Access::Unknown
                }
            } else {
                edge.bus_access
            }) == Access::Allowed
            {
                outgoing.entry(&edge.from).or_default().push(edge);
            }
        }
        let mut queue = std::collections::BinaryHeap::new();
        let mut distances: BTreeMap<String, f64> = BTreeMap::new();
        let mut previous: BTreeMap<String, (&str, &str)> = BTreeMap::new();
        distances.insert(from.into(), 0.0);
        queue.push(Entry(0.0, from.into()));
        while let Some(Entry(cost, node)) = queue.pop() {
            if cost > distances[&node] {
                continue;
            }
            if node == to {
                let mut result = Vec::new();
                let mut cursor = to;
                while cursor != from {
                    let (parent, edge) = previous.get(cursor).ok_or(DisconnectedPath)?;
                    result.push((*edge).to_owned());
                    cursor = parent;
                    if result.len() > 4096 {
                        return Err(InvalidData);
                    }
                }
                result.reverse();
                return Ok(result);
            }
            for edge in outgoing.get(node.as_str()).into_iter().flatten() {
                let length: f64 = edge
                    .geometry
                    .windows(2)
                    .map(|p| distance_m(p[0], p[1]))
                    .sum();
                let next = cost + length;
                if distances.get(&edge.to).is_none_or(|old| next < *old) {
                    distances.insert(edge.to.clone(), next);
                    previous.insert(edge.to.clone(), (&edge.from, &edge.id));
                    queue.push(Entry(next, edge.to.clone()));
                }
            }
        }
        Err(DisconnectedPath)
    }
    /// Distances between successive stop occurrences, clipping the first/last edge.
    /// Does not invent a speed limit, timetable, vehicle or road capacity.
    pub fn bus_legs(
        &self,
        world: &WorldDraft,
        route_id: &str,
    ) -> Result<Vec<BusLeg>, RoadTransportError> {
        self.validate(world)?;
        let route = self
            .bus_routes
            .iter()
            .find(|r| r.id == route_id)
            .ok_or(MissingReference)?;
        if let Some(proof) = world
            .ofm_gameplay_bus_routes
            .get(route_id)
            .filter(|proof| proof.has_fractional_stops())
        {
            if !std::ptr::eq(self, &world.road_transport) && self != &world.road_transport {
                return Err(StaleSnapshot);
            }
            return world.gameplay_bus_fractional_legs(route, proof);
        }
        if let Some(proof) = world
            .ofm_gameplay_bus_routes
            .get(route_id)
            .filter(|proof| proof.pattern == Some(crate::ofm_evidence::BusPattern::ClosedOneWay))
        {
            if !std::ptr::eq(self, &world.road_transport) && self != &world.road_transport {
                return Err(StaleSnapshot);
            }
            return world.gameplay_bus_closed_legs(route_id, proof);
        }
        let edges = indexed(&self.edges, |e| &e.id)?;
        let stops = indexed(&self.bus_stops, |s| &s.id)?;
        route
            .stops
            .windows(2)
            .map(|pair| {
                let first = stops[pair[0].stop_id.as_str()];
                let last = stops[pair[1].stop_id.as_str()];
                let mut length_m = 0.0;
                let mut traversed_edges = Vec::new();
                for index in pair[0].edge_index..=pair[1].edge_index {
                    let start = if index == pair[0].edge_index {
                        first.position
                    } else {
                        0.0
                    };
                    let end = if index == pair[1].edge_index {
                        last.position
                    } else {
                        1.0
                    };
                    if end <= start {
                        continue;
                    }
                    let edge = edges[route.edges[index].as_str()];
                    traversed_edges.push(edge.id.clone());
                    length_m += edge
                        .geometry
                        .windows(2)
                        .map(|p| distance_m(p[0], p[1]))
                        .sum::<f64>()
                        * (end - start);
                }
                if length_m <= 0.0 || !length_m.is_finite() {
                    return Err(InvalidStopOrder);
                }
                Ok(BusLeg {
                    traversed_edges,
                    from_stop_id: first.id.clone(),
                    to_stop_id: last.id.clone(),
                    length_m,
                })
            })
            .collect()
    }
    /// Caller must authorize the requested changes. Rejection never mutates state.
    /// History and file persistence belong to the session/world transaction layer.
    pub fn replace_checked(
        &mut self,
        expected: &Self,
        candidate: Self,
        world: &WorldDraft,
    ) -> Result<(), RoadTransportError> {
        if self != expected {
            return Err(StaleSnapshot);
        }
        candidate.validate(world)?;
        *self = candidate;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{Company, PlayerId};
    fn fixture() -> (WorldDraft, RoadTransport) {
        let mut world = WorldDraft::default();
        world.companies.push(Company {
            id: CompanyId(1),
            owner: PlayerId(2),
            name: "Bus company".into(),
        });
        let a = GeoPoint {
            latitude: 35.0,
            longitude: 135.0,
        };
        let b = GeoPoint {
            latitude: 35.001,
            longitude: 135.0,
        };
        let data = RoadTransport {
            evidence: vec![],
            nodes: vec![
                RoadNode {
                    id: "a".into(),
                    point: a,
                },
                RoadNode {
                    id: "b".into(),
                    point: b,
                },
            ],
            edges: vec![RoadEdge {
                id: "ab".into(),
                from: "a".into(),
                to: "b".into(),
                source: "verified-fixture".into(),
                revision: "1".into(),
                geometry: vec![a, b],
                bus_access: Access::Allowed,
                tram_access: Access::Unknown,
                level: None,
            }],
            bus_stops: vec![
                BusStop {
                    id: "s1".into(),
                    owner: CompanyId(1),
                    name: "Start".into(),
                    edge_id: "ab".into(),
                    position: 0.1,
                    location_kind: Default::default(),
                },
                BusStop {
                    id: "s2".into(),
                    owner: CompanyId(1),
                    name: "End".into(),
                    edge_id: "ab".into(),
                    position: 0.9,
                    location_kind: Default::default(),
                },
            ],
            bus_routes: vec![BusRoute {
                id: "route".into(),
                owner: CompanyId(1),
                name: "Bus".into(),
                edges: vec!["ab".into()],
                stops: vec![
                    BusStopOccurrence {
                        stop_id: "s1".into(),
                        edge_index: 0,
                    },
                    BusStopOccurrence {
                        stop_id: "s2".into(),
                        edge_index: 0,
                    },
                ],
            }],
            tram_sections: vec![],
        };
        (world, data)
    }
    #[test]
    fn tram_must_match_road_geometry_not_just_share_an_id() {
        let (mut w, mut d) = fixture();
        d.edges[0].tram_access = Access::Allowed;
        d.edges[0].level = Some(crate::world::InfrastructureLevel::Ground);
        w.alignments.push(crate::world::Alignment {
            id: AlignmentId(5),
            owner: CompanyId(1),
            name: "Tram".into(),
            points: d.edges[0].geometry.clone(),
            anchor_indices: None,
            infrastructure: vec![],
            vertical_profile: None,
            section_profiles: vec![],
            track_type: TrackType::Tram,
            design_speed_kmh: None,
        });
        d.tram_sections.push(TramSection {
            alignment_geometry: w.alignments[0].points.clone(),
            alignment_id: AlignmentId(5),
            from_position: 0.0,
            to_position: 1.0,
            placement: TramPlacement::SharedRoad {
                edges: vec!["ab".into()],
            },
        });
        assert_eq!(d.validate(&w), Ok(()));
        let mut snapshot = w.clone();
        snapshot.road_transport = RoadTransport::synthetic_qa_graph();
        snapshot.alignments[0].points = snapshot.road_transport.edges[0].geometry.clone();
        let mut section = d.tram_sections[0].clone();
        section.alignment_geometry = snapshot.alignments[0].points.clone();
        section.placement = TramPlacement::SharedRoad {
            edges: vec![snapshot.road_transport.edges[0].id.clone()],
        };
        snapshot.road_transport.tram_sections.push(section);
        snapshot.players.push(crate::world::Player {
            id: PlayerId(2),
            name: "Player".into(),
        });
        snapshot.next_id = 6;
        let mut physical = crate::world::World::new(snapshot.clone()).unwrap();
        let end = *snapshot.alignments[0].points.last().unwrap();
        assert!(physical
            .extend_alignment(
                CompanyId(1),
                AlignmentId(5),
                crate::world::AlignmentEndpoint::End,
                vec![
                    end,
                    GeoPoint {
                        latitude: 35.002,
                        longitude: 135.0
                    }
                ]
            )
            .is_err());
        assert_eq!(physical.snapshot(), &snapshot);
        d.edges[0].level = None;
        assert_eq!(d.validate(&w), Err(InvalidTramSection));
        d.edges[0].level = Some(crate::world::InfrastructureLevel::Ground);
        let original = d.edges[0].geometry.clone();
        d.edges[0].geometry.insert(
            1,
            GeoPoint {
                latitude: 35.1,
                longitude: 135.1,
            },
        );
        assert_eq!(d.validate(&w), Err(InvalidTramSection));
        d.edges[0].geometry = original;
        w.alignments[0].vertical_profile = Some(crate::world::VerticalProfile {
            start_height_m: 0.,
            end_height_m: 0.,
            start_tangent_m: 0.01,
            end_tangent_m: 0.,
        });
        assert_eq!(d.validate(&w), Err(InvalidTramSection));
        w.alignments[0].vertical_profile = None;
        w.alignments[0].points[0].longitude += 0.01;
        assert_eq!(d.validate(&w), Err(InvalidTramSection));
        w.alignments[0].points.clear();
        w.road_transport = d;
        let decoded: WorldDraft =
            serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        assert!(std::panic::catch_unwind(|| decoded.validate())
            .unwrap()
            .is_err());
    }
    #[test]
    fn zero_distance_boundary_stops_are_rejected_at_validation() {
        let (w, mut d) = fixture();
        let mut reverse = d.edges[0].clone();
        reverse.id = "ba".into();
        reverse.from = "b".into();
        reverse.to = "a".into();
        reverse.geometry.reverse();
        d.edges.push(reverse);
        d.bus_routes[0].edges.push("ba".into());
        d.bus_stops[0].position = 1.;
        d.bus_stops[1].edge_id = "ba".into();
        d.bus_stops[1].position = 0.;
        d.bus_routes[0].stops[1].edge_index = 1;
        assert_eq!(d.validate(&w), Err(InvalidStopOrder));
    }
    #[test]
    fn directed_search_excludes_unknown_and_reverse() {
        let (w, mut d) = fixture();
        assert_eq!(d.shortest_path(&w, "a", "b", false), Ok(vec!["ab".into()]));
        assert_eq!(d.shortest_path(&w, "b", "a", false), Err(DisconnectedPath));
        d.edges[0].bus_access = Access::Unknown;
        assert_eq!(d.shortest_path(&w, "a", "b", false), Err(DisconnectedPath));
    }
    #[test]
    fn serialization_preserves_verified_graph_and_route() {
        let (w, d) = fixture();
        assert_eq!(d.validate(&w), Ok(()));
        let restored: RoadTransport =
            serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        assert_eq!(d, restored);
        let legs = d.bus_legs(&w, "route").unwrap();
        assert_eq!(legs.len(), 1);
        assert!((legs[0].length_m - 88.956).abs() < 0.1);
        assert_eq!(restored.validate(&w), Ok(()));
    }
    #[test]
    fn unknown_and_prohibited_are_not_routable() {
        let (w, mut d) = fixture();
        d.edges[0].bus_access = Access::Unknown;
        assert_eq!(d.validate(&w), Err(AccessUnknown));
        d.edges[0].bus_access = Access::Forbidden;
        assert_eq!(d.validate(&w), Err(AccessForbidden));
    }
    #[test]
    fn reverse_path_requires_explicit_reverse_edge() {
        let (w, mut d) = fixture();
        d.bus_routes[0].edges = vec!["ba".into()];
        assert_eq!(d.validate(&w), Err(MissingReference));
    }
    #[test]
    fn stop_order_and_ownership_are_validated() {
        let (w, mut d) = fixture();
        d.bus_routes[0].stops.reverse();
        assert_eq!(d.validate(&w), Err(InvalidStopOrder));
    }
    #[test]
    fn same_coordinate_does_not_connect_different_nodes() {
        let (w, mut d) = fixture();
        let mut e = d.edges[0].clone();
        d.nodes.push(RoadNode {
            id: "bridge-b".into(),
            point: d.nodes[1].point,
        });
        e.id = "ba".into();
        e.from = "bridge-b".into();
        e.to = "a".into();
        e.geometry.reverse();
        d.edges.push(e);
        d.bus_routes[0].edges.push("ba".into());
        assert_eq!(d.validate(&w), Err(DisconnectedPath));
    }
    #[test]
    fn deletion_of_referenced_edge_is_atomic_and_stale_changes_rejected() {
        let (w, mut d) = fixture();
        let before = d.clone();
        let mut candidate = d.clone();
        candidate.edges.clear();
        assert_eq!(
            d.replace_checked(&before, candidate, &w),
            Err(MissingReference)
        );
        assert_eq!(d, before);
        let mut stale = before.clone();
        stale.bus_routes.clear();
        assert_eq!(
            d.replace_checked(&stale, RoadTransport::default(), &w),
            Err(StaleSnapshot)
        );
        assert_eq!(d, before);
    }
    #[test]
    fn boundary_only_edges_do_not_reserve_adjacent_bus_corridor() {
        let (w, mut d) = fixture();
        let b = d.nodes[1].point;
        let c = GeoPoint {
            latitude: 35.002,
            longitude: 135.,
        };
        d.nodes.push(RoadNode {
            id: "c".into(),
            point: c,
        });
        let mut edge = d.edges[0].clone();
        edge.id = "bc".into();
        edge.from = "b".into();
        edge.to = "c".into();
        edge.geometry = vec![b, c];
        d.edges.push(edge);
        d.bus_stops[0].position = 0.;
        d.bus_stops[1].position = 1.;
        for (id, position) in [("s3", 0.), ("s4", 1.)] {
            d.bus_stops.push(BusStop {
                id: id.into(),
                owner: CompanyId(1),
                name: id.into(),
                edge_id: "bc".into(),
                position,
                location_kind: Default::default(),
            });
        }
        d.bus_routes[0].edges = vec!["ab".into(), "bc".into()];
        d.bus_routes[0].stops = vec![
            BusStopOccurrence {
                stop_id: "s1".into(),
                edge_index: 0,
            },
            BusStopOccurrence {
                stop_id: "s3".into(),
                edge_index: 1,
            },
        ];
        let mut second = d.bus_routes[0].clone();
        second.id = "second".into();
        second.stops = vec![
            BusStopOccurrence {
                stop_id: "s2".into(),
                edge_index: 0,
            },
            BusStopOccurrence {
                stop_id: "s4".into(),
                edge_index: 1,
            },
        ];
        d.bus_routes.push(second);
        let first = d.bus_legs(&w, "route").unwrap();
        let second = d.bus_legs(&w, "second").unwrap();
        assert_eq!(first[0].traversed_edges, vec!["ab"]);
        assert_eq!(second[0].traversed_edges, vec!["bc"]);
        assert!(first[0]
            .traversed_edges
            .iter()
            .all(|e| !second[0].traversed_edges.contains(e)));
    }
    #[test]
    fn invalid_stop_position_rejected() {
        let (w, mut d) = fixture();
        d.bus_stops[0].position = f64::NAN;
        assert_eq!(d.validate(&w), Err(InvalidData));
    }
}

fn distance_m(a: GeoPoint, b: GeoPoint) -> f64 {
    let lat = (b.latitude - a.latitude).to_radians();
    let lon = (b.longitude - a.longitude).to_radians();
    let h = (lat / 2.0).sin().powi(2)
        + a.latitude.to_radians().cos() * b.latitude.to_radians().cos() * (lon / 2.0).sin().powi(2);
    2.0 * 6_371_000.0 * h.clamp(0.0, 1.0).sqrt().asin()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BusLeg {
    /// Only positive-distance edge occurrences, excludes boundary-only edges.
    pub traversed_edges: Vec<String>,
    pub from_stop_id: String,
    pub to_stop_id: String,
    pub length_m: f64,
}
