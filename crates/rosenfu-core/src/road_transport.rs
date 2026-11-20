//! Road transport is separate from railway topology and display-only road evidence.
//! Only explicitly verified directed edges may carry a service. Coincident points
//! never create connectivity. The acquisition adapter must retain unknown access.
use crate::world::{AlignmentId, CompanyId, GeoPoint, TrackType, WorldDraft};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

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
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BusStop {
    pub id: String,
    pub owner: CompanyId,
    pub name: String,
    pub edge_id: String,
    pub position: f64,
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
    pub alignment_id: AlignmentId,
    pub from_position: f64,
    pub to_position: f64,
    pub placement: TramPlacement,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct RoadTransport {
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
    pub fn validate(&self, world: &WorldDraft) -> Result<(), RoadTransportError> {
        if self.nodes.len() > 100_000
            || self.edges.len() > 200_000
            || self.bus_stops.len() > 10_000
            || self.bus_routes.len() > 10_000
            || self.tram_sections.len() > 10_000
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
            let mut prior = None;
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
                if prior.is_some_and(|p| p >= current) {
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
            if a.track_type != TrackType::Tram
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
            }
        }
        Ok(())
    }
    /// Shortest verified directed road path by geographic polyline length.
    /// Unknown/forbidden edges are excluded, never treated as usable roads.
    /// A bounded search returns an explicit error rather than a partial path.
    pub fn shortest_path(
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
                edge.tram_access
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
            }],
            bus_stops: vec![
                BusStop {
                    id: "s1".into(),
                    owner: CompanyId(1),
                    name: "Start".into(),
                    edge_id: "ab".into(),
                    position: 0.1,
                },
                BusStop {
                    id: "s2".into(),
                    owner: CompanyId(1),
                    name: "End".into(),
                    edge_id: "ab".into(),
                    position: 0.9,
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
