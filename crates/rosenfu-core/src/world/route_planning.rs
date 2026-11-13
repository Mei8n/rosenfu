//! Read-only route planning. Never allocates IDs, constructs tracks, or selects an
//! ambiguous path implicitly. The returned plans are not persisted world data.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

const MAX_CANDIDATES: usize = 32;
const MAX_STEPS: usize = 100_000;
const MAX_HOPS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RouteSegment {
    Track {
        track_id: TrackId,
    },
    /// A virtual edge between adjacent stations on one alignment, not a Track.
    Alignment {
        alignment_id: AlignmentId,
        from: StationId,
        to: StationId,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RouteCandidate {
    pub stations: Vec<StationId>,
    pub segments: Vec<RouteSegment>,
    /// False means existing save_route cannot consume this candidate.
    pub track_only: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteSearchStatus {
    NoPath,
    Unique,
    Ambiguous,
    Incomplete,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RouteCandidates {
    pub status: RouteSearchStatus,
    pub candidates: Vec<RouteCandidate>,
    pub truncated: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RouteMembershipPreview {
    pub path: RouteCandidate,
    /// Membership only; exclusions do not delete infrastructure or set stops.
    pub member_stations: Vec<StationId>,
    pub excluded_stations: Vec<StationId>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutePlanningError {
    MissingStation,
    SameEndpoints,
    EmptyPath,
    InvalidSegment,
    RepeatedStation,
    InvalidExclusion,
    PathTooLong,
}
impl std::fmt::Display for RoutePlanningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RoutePlanningError {}

#[derive(Clone)]
struct Edge {
    to: StationId,
    segment: RouteSegment,
}
type Graph = BTreeMap<StationId, Vec<Edge>>;

fn graph(data: &WorldDraft) -> Graph {
    let mut graph: Graph = data.stations.iter().map(|s| (s.id, vec![])).collect();
    // Sort independently of save order, to make limited searches deterministic.
    let mut tracks: Vec<_> = data.tracks.iter().collect();
    tracks.sort_by_key(|t| t.id);
    for t in tracks {
        graph.get_mut(&t.from).unwrap().push(Edge {
            to: t.to,
            segment: RouteSegment::Track { track_id: t.id },
        });
        graph.get_mut(&t.to).unwrap().push(Edge {
            to: t.from,
            segment: RouteSegment::Track { track_id: t.id },
        });
    }
    // Group once rather than scanning every placement for every alignment.
    // BTreeMap retains alignment-ID order independently of save order.
    let mut by_alignment: BTreeMap<AlignmentId, Vec<&StationPlacement>> = BTreeMap::new();
    for placement in &data.station_placements {
        by_alignment
            .entry(placement.alignment_id)
            .or_default()
            .push(placement);
    }
    for (alignment_id, placements) in &mut by_alignment {
        placements.sort_by(|a, b| {
            a.position
                .total_cmp(&b.position)
                .then(a.station_id.cmp(&b.station_id))
        });
        for (index, pair) in placements.windows(2).enumerate() {
            // A coincident group disconnects both boundaries as well as its
            // interior. Since positions are sorted, only immediate neighbours
            // are needed to establish whether both endpoints are unique.
            if pair[0].position == pair[1].position
                || (index > 0 && placements[index - 1].position == pair[0].position)
                || (index + 2 < placements.len()
                    && placements[index + 2].position == pair[1].position)
            {
                continue;
            }
            let from = pair[0].station_id;
            let to = pair[1].station_id;
            graph.get_mut(&from).unwrap().push(Edge {
                to,
                segment: RouteSegment::Alignment {
                    alignment_id: *alignment_id,
                    from,
                    to,
                },
            });
            graph.get_mut(&to).unwrap().push(Edge {
                to: from,
                segment: RouteSegment::Alignment {
                    alignment_id: *alignment_id,
                    from: to,
                    to: from,
                },
            });
        }
    }
    graph
}
fn candidate(stations: Vec<StationId>, segments: Vec<RouteSegment>) -> RouteCandidate {
    let track_only = segments
        .iter()
        .all(|s| matches!(s, RouteSegment::Track { .. }));
    RouteCandidate {
        stations,
        segments,
        track_only,
    }
}
impl World {
    /// Enumerate station-simple paths, not a shortest-path recommendation.
    /// Cycles/repeated stations require explicit future editing support.
    /// `incomplete` MUST NOT be interpreted as uniqueness or no connectivity.
    pub fn route_candidates(
        &self,
        start: StationId,
        end: StationId,
    ) -> Result<RouteCandidates, RoutePlanningError> {
        let graph = graph(self.snapshot());
        if !graph.contains_key(&start) || !graph.contains_key(&end) {
            return Err(RoutePlanningError::MissingStation);
        }
        if start == end {
            return Err(RoutePlanningError::SameEndpoints);
        }
        let mut stations = vec![start];
        let mut segments = vec![];
        let mut cursors = vec![0usize];
        let mut visited = BTreeSet::from([start]);
        let mut candidates = vec![];
        let mut truncated = false;
        let mut steps = 0;
        while let Some(&current) = stations.last() {
            if steps >= MAX_STEPS {
                truncated = true;
                break;
            }
            steps += 1;
            let edges = &graph[&current];
            let cursor = cursors.last_mut().unwrap();
            if *cursor == edges.len() {
                visited.remove(&stations.pop().unwrap());
                cursors.pop();
                segments.pop();
                continue;
            }
            let edge = &edges[*cursor];
            *cursor += 1;
            if visited.contains(&edge.to) {
                continue;
            }
            if segments.len() >= MAX_HOPS {
                truncated = true;
                continue;
            }
            stations.push(edge.to);
            segments.push(edge.segment.clone());
            if edge.to == end {
                candidates.push(candidate(stations.clone(), segments.clone()));
                stations.pop();
                segments.pop();
                if candidates.len() >= MAX_CANDIDATES {
                    truncated = true;
                    break;
                }
            } else {
                visited.insert(edge.to);
                cursors.push(0);
            }
        }
        let status = if truncated {
            RouteSearchStatus::Incomplete
        } else {
            match candidates.len() {
                0 => RouteSearchStatus::NoPath,
                1 => RouteSearchStatus::Unique,
                _ => RouteSearchStatus::Ambiguous,
            }
        };
        Ok(RouteCandidates {
            status,
            candidates,
            truncated,
        })
    }
    /// Validate a selected path against the CURRENT world and preview membership.
    /// No writes occur, including when a stale selection or exclusion is rejected.
    pub fn preview_route_membership(
        &self,
        start: StationId,
        segments: Vec<RouteSegment>,
        excluded: Vec<StationId>,
    ) -> Result<RouteMembershipPreview, RoutePlanningError> {
        if segments.is_empty() {
            return Err(RoutePlanningError::EmptyPath);
        }
        if segments.len() > MAX_HOPS {
            return Err(RoutePlanningError::PathTooLong);
        }
        if !self.data.stations.iter().any(|s| s.id == start) {
            return Err(RoutePlanningError::MissingStation);
        }
        let stations = super::route_paths::resolve_path(self.snapshot(), start, &segments)?;
        let seen: BTreeSet<_> = stations.iter().copied().collect();
        let current = *stations.last().ok_or(RoutePlanningError::EmptyPath)?;
        let excluded_set: BTreeSet<_> = excluded.iter().copied().collect();
        if excluded_set.len() != excluded.len()
            || excluded_set
                .iter()
                .any(|id| *id == start || *id == current || !seen.contains(id))
        {
            return Err(RoutePlanningError::InvalidExclusion);
        }
        let member_stations = stations
            .iter()
            .copied()
            .filter(|id| !excluded_set.contains(id))
            .collect();
        // Canonical path order, not click order.
        let excluded_stations = stations
            .iter()
            .copied()
            .filter(|id| excluded_set.contains(id))
            .collect();
        Ok(RouteMembershipPreview {
            path: candidate(stations, segments),
            member_stations,
            excluded_stations,
        })
    }
}
