//! Persisted physical route path and independent station membership.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutePath {
    pub route_id: RouteId,
    pub segments: Vec<RouteSegment>,
    pub member_stations: Vec<StationId>,
}

/// Resolve explicit infrastructure spans without relying on preview adjacency.
/// A station inserted into a saved alignment span does not invalidate that span.
pub(super) fn resolve_path(
    data: &WorldDraft,
    start: StationId,
    segments: &[RouteSegment],
) -> Result<Vec<StationId>, RoutePlanningError> {
    if segments.is_empty() || segments.len() > 256 || !data.stations.iter().any(|s| s.id == start) {
        return Err(RoutePlanningError::InvalidSegment);
    }
    let mut stations = vec![start];
    let mut seen = BTreeSet::from([start]);
    let mut current = start;
    for segment in segments {
        let next: Vec<StationId> = match *segment {
            RouteSegment::Track { track_id } => {
                let t = data
                    .tracks
                    .iter()
                    .find(|t| t.id == track_id)
                    .ok_or(RoutePlanningError::InvalidSegment)?;
                vec![if t.from == current {
                    t.to
                } else if t.to == current {
                    t.from
                } else {
                    return Err(RoutePlanningError::InvalidSegment);
                }]
            }
            RouteSegment::Alignment {
                alignment_id,
                from,
                to,
            } => {
                if current != from || from == to {
                    return Err(RoutePlanningError::InvalidSegment);
                }
                let placements: Vec<_> = data
                    .station_placements
                    .iter()
                    .filter(|p| p.alignment_id == alignment_id)
                    .collect();
                let a = placements
                    .iter()
                    .find(|p| p.station_id == from)
                    .ok_or(RoutePlanningError::InvalidSegment)?;
                let b = placements
                    .iter()
                    .find(|p| p.station_id == to)
                    .ok_or(RoutePlanningError::InvalidSegment)?;
                if a.position == b.position {
                    return Err(RoutePlanningError::InvalidSegment);
                }
                let mut span: Vec<_> = placements
                    .iter()
                    .filter(|p| {
                        p.position >= a.position.min(b.position)
                            && p.position <= a.position.max(b.position)
                    })
                    .collect();
                span.sort_by(|a, b| a.position.total_cmp(&b.position));
                if span.windows(2).any(|p| p[0].position == p[1].position) {
                    return Err(RoutePlanningError::InvalidSegment);
                }
                if a.position > b.position {
                    span.reverse();
                }
                span.into_iter().skip(1).map(|p| p.station_id).collect()
            }
        };
        for id in next {
            if !seen.insert(id) {
                return Err(RoutePlanningError::RepeatedStation);
            }
            stations.push(id);
            current = id;
        }
    }
    Ok(stations)
}
impl WorldDraft {
    pub(super) fn validate_route_paths(&self) -> Result<(), WorldError> {
        let mut ids = BTreeSet::new();
        for path in &self.route_paths {
            if !ids.insert(path.route_id) {
                return Err(WorldError::InvalidRoutePath);
            }
            let route = self
                .routes
                .iter()
                .find(|r| r.id == path.route_id)
                .ok_or(WorldError::InvalidRoutePath)?;
            // Exactly one physical-path source is authoritative.
            if !route.tracks.is_empty() {
                return Err(WorldError::InvalidRoutePath);
            }
            let stations = resolve_path(self, route.start, &path.segments)
                .map_err(|_| WorldError::InvalidRoutePath)?;
            let members = &path.member_stations;
            if members.first() != stations.first()
                || members.last() != stations.last()
                || members.len() < 2
            {
                return Err(WorldError::InvalidRoutePath);
            }
            let mut cursor = 0;
            for id in members {
                let offset = stations[cursor..]
                    .iter()
                    .position(|s| s == id)
                    .ok_or(WorldError::InvalidRoutePath)?;
                cursor += offset + 1;
            }
        }
        Ok(())
    }
}
impl World {
    /// Commit an explicitly selected path and explicit station membership.
    /// No infrastructure or services are created or modified.
    pub fn add_planned_route(
        &mut self,
        owner: CompanyId,
        name: String,
        start: StationId,
        segments: Vec<RouteSegment>,
        member_stations: Vec<StationId>,
    ) -> Result<RouteId, WorldError> {
        self.insert(|data, id| {
            let route_id = RouteId(id);
            data.routes.push(Route {
                id: route_id,
                owner,
                name,
                start,
                tracks: vec![],
            });
            data.route_paths.push(RoutePath {
                route_id,
                segments,
                member_stations,
            });
        })
        .map(RouteId)
    }
    pub fn route_path_stations(&self, route_id: RouteId) -> Result<Vec<StationId>, WorldError> {
        let route = self
            .data
            .routes
            .iter()
            .find(|r| r.id == route_id)
            .ok_or(WorldError::MissingReference("route"))?;
        if let Some(path) = self
            .data
            .route_paths
            .iter()
            .find(|p| p.route_id == route_id)
        {
            resolve_path(&self.data, route.start, &path.segments)
                .map_err(|_| WorldError::InvalidRoutePath)
        } else {
            // Legacy routes can revisit stations; preserve their occurrence order.
            let mut current = route.start;
            let mut stations = vec![current];
            for id in &route.tracks {
                let t = self
                    .data
                    .tracks
                    .iter()
                    .find(|t| t.id == *id)
                    .ok_or(WorldError::DisconnectedRoute)?;
                current = if t.from == current {
                    t.to
                } else if t.to == current {
                    t.from
                } else {
                    return Err(WorldError::DisconnectedRoute);
                };
                stations.push(current);
            }
            Ok(stations)
        }
    }
}
