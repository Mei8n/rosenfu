//! Explicit endpoint connectivity. Coincident coordinates alone never imply a link.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentEndpointRef {
    pub alignment_id: AlignmentId,
    pub endpoint: AlignmentEndpoint,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentConnection {
    pub a: AlignmentEndpointRef,
    pub b: AlignmentEndpointRef,
}

impl WorldDraft {
    pub(super) fn validate_alignment_connections(&self) -> Result<(), WorldError> {
        let invalid = WorldError::InvalidAlignmentConnection;
        let mut occupied = BTreeSet::new();
        for connection in &self.alignment_connections {
            if connection.a.alignment_id == connection.b.alignment_id {
                return Err(invalid);
            }
            let mut ends = Vec::with_capacity(2);
            for end in [connection.a, connection.b] {
                // Degree one per endpoint: no implicit junction/branch creation.
                if !occupied.insert((end.alignment_id, end.endpoint == AlignmentEndpoint::End)) {
                    return Err(invalid);
                }
                let alignment = self
                    .alignments
                    .iter()
                    .find(|a| a.id == end.alignment_id)
                    .ok_or(WorldError::MissingReference("connection.alignment"))?;
                let (point, inside) = match end.endpoint {
                    AlignmentEndpoint::Start => (alignment.points[0], alignment.points[1]),
                    AlignmentEndpoint::End => (
                        alignment.points[alignment.points.len() - 1],
                        alignment.points[alignment.points.len() - 2],
                    ),
                };
                ends.push((alignment, point, inside));
            }
            let (a, point, ai) = ends[0];
            let (b, other, bi) = ends[1];
            if a.owner != b.owner || a.track_type != b.track_type || point != other {
                return Err(invalid);
            }
            // Speed caps need not match: each original alignment keeps its own cap.
            let vector = |p: GeoPoint| {
                (
                    (p.longitude - point.longitude) * point.latitude.to_radians().cos(),
                    p.latitude - point.latitude,
                )
            };
            let av = vector(ai);
            let bv = vector(bi);
            let norm = av.0.hypot(av.1) * bv.0.hypot(bv.1);
            if !norm.is_finite()
                || norm <= 0.0
                || (av.0 * bv.0 + av.1 * bv.1) / norm > -5_f64.to_radians().cos()
            {
                return Err(invalid);
            }
        }
        Ok(())
    }
}
impl World {
    /// Connect exact, free endpoints without merging or renumbering infrastructure.
    /// Validation rejects accidental overlap, cusps, foreign ownership and branches.
    pub fn connect_alignment_endpoints(
        &mut self,
        owner: CompanyId,
        a: AlignmentEndpointRef,
        b: AlignmentEndpointRef,
    ) -> Result<(), WorldError> {
        if [a, b].iter().any(|end| {
            !self
                .data
                .alignments
                .iter()
                .any(|alignment| alignment.id == end.alignment_id && alignment.owner == owner)
        }) {
            return Err(WorldError::InvalidAlignmentConnection);
        }
        let mut candidate = self.data.clone();
        candidate
            .alignment_connections
            .push(AlignmentConnection { a, b });
        candidate.validate()?;
        // No geometry, placements, routes, service keys or source records change.
        self.data = candidate;
        Ok(())
    }
}

/// Resolve a saved, oriented endpoint corridor. Interior stations added later are
/// included, while the explicit link sequence cannot silently choose another path.
pub(super) fn resolve_corridor(
    data: &WorldDraft,
    from: StationId,
    to: StationId,
    connections: &[AlignmentConnection],
) -> Result<Vec<StationId>, RoutePlanningError> {
    let invalid = RoutePlanningError::InvalidSegment;
    if from == to || connections.is_empty() || connections.len() > 256 {
        return Err(invalid);
    }
    let placement = |id| data.station_placements.iter().find(|p| p.station_id == id);
    let first = placement(from).ok_or(invalid.clone())?;
    let last = placement(to).ok_or(invalid.clone())?;
    let mut current_id = first.alignment_id;
    let mut start = first.position;
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    let mut append = |id: AlignmentId, begin: f64, finish: f64| -> Result<(), RoutePlanningError> {
        let mut stops: Vec<_> = data
            .station_placements
            .iter()
            .filter(|p| {
                p.alignment_id == id
                    && p.position >= begin.min(finish)
                    && p.position <= begin.max(finish)
            })
            .collect();
        stops.sort_by(|a, b| a.position.total_cmp(&b.position));
        if stops
            .windows(2)
            .any(|pair| pair[0].position == pair[1].position)
        {
            return Err(RoutePlanningError::InvalidSegment);
        }
        if begin > finish {
            stops.reverse();
        }
        result.extend(
            stops
                .into_iter()
                .filter(|p| p.station_id != from)
                .map(|p| p.station_id),
        );
        Ok(())
    };
    for link in connections {
        if link.a.alignment_id != current_id
            || !seen.insert(current_id)
            || !data
                .alignment_connections
                .iter()
                .any(|saved| saved == link || (saved.a == link.b && saved.b == link.a))
        {
            return Err(invalid);
        }
        let exit = if link.a.endpoint == AlignmentEndpoint::Start {
            0.0
        } else {
            1.0
        };
        if start == exit {
            return Err(invalid);
        }
        append(current_id, start, exit)?;
        current_id = link.b.alignment_id;
        start = if link.b.endpoint == AlignmentEndpoint::Start {
            0.0
        } else {
            1.0
        };
    }
    if current_id != last.alignment_id || !seen.insert(current_id) || start == last.position {
        return Err(invalid);
    }
    append(current_id, start, last.position)?;
    if result.last() != Some(&to) {
        return Err(invalid);
    }
    Ok(result)
}

/// Adjacent station edges crossing one or more explicitly connected alignments.
/// Empty (station-free) alignments are traversed, never converted to fake stations.
pub(super) fn corridor_edges(
    data: &WorldDraft,
) -> Vec<(StationId, StationId, Vec<AlignmentConnection>)> {
    let mut edges = Vec::new();
    for initial in &data.alignment_connections {
        for first in [
            initial.clone(),
            AlignmentConnection {
                a: initial.b,
                b: initial.a,
            },
        ] {
            let nearest = |end: AlignmentEndpointRef| {
                data.station_placements
                    .iter()
                    .filter(|p| p.alignment_id == end.alignment_id)
                    .min_by(|a, b| {
                        let cmp = a.position.total_cmp(&b.position);
                        if end.endpoint == AlignmentEndpoint::Start {
                            cmp
                        } else {
                            cmp.reverse()
                        }
                    })
            };
            let Some(source) = nearest(first.a) else {
                continue;
            };
            let mut chain = vec![first.clone()];
            let mut incoming = first.b;
            let mut seen = BTreeSet::from([first.a.alignment_id]);
            loop {
                if chain.len() > 256 || !seen.insert(incoming.alignment_id) {
                    break;
                }
                if let Some(target) = nearest(incoming) {
                    if resolve_corridor(data, source.station_id, target.station_id, &chain).is_ok()
                    {
                        edges.push((source.station_id, target.station_id, chain));
                    }
                    break;
                }
                let exit = AlignmentEndpointRef {
                    alignment_id: incoming.alignment_id,
                    endpoint: if incoming.endpoint == AlignmentEndpoint::Start {
                        AlignmentEndpoint::End
                    } else {
                        AlignmentEndpoint::Start
                    },
                };
                let Some(next) = data.alignment_connections.iter().find_map(|link| {
                    if link.a == exit {
                        Some(link.clone())
                    } else if link.b == exit {
                        Some(AlignmentConnection {
                            a: link.b,
                            b: link.a,
                        })
                    } else {
                        None
                    }
                }) else {
                    break;
                };
                incoming = next.b;
                chain.push(next);
            }
        }
    }
    edges
}
