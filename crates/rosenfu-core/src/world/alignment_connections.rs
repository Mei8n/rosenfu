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
        let mut occupied = std::collections::BTreeMap::new();
        let mut pairs = BTreeSet::new();
        for connection in &self.alignment_connections {
            if connection.a == connection.b {
                return Err(invalid);
            }
            let key = |end: AlignmentEndpointRef| {
                (end.alignment_id, end.endpoint == AlignmentEndpoint::End)
            };
            let pair = if key(connection.a) < key(connection.b) {
                (key(connection.a), key(connection.b))
            } else {
                (key(connection.b), key(connection.a))
            };
            if !pairs.insert(pair) {
                return Err(invalid);
            }
            let mut ends = Vec::with_capacity(2);
            for end in [connection.a, connection.b] {
                // At most three physical ports meet at a supported turnout.
                let key = (end.alignment_id, end.endpoint == AlignmentEndpoint::End);
                let degree = occupied.entry(key).or_insert(0);
                *degree += 1;
                if *degree > 2 {
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
            // Connections require equal actual elevation, not merely equal map coordinates.
            if a.owner != b.owner
                || a.track_type != b.track_type
                || point != other
                || (a.endpoint_height_m(connection.a.endpoint)
                    - b.endpoint_height_m(connection.b.endpoint))
                .abs()
                    > 1e-6
            {
                return Err(invalid);
            }
            if (a.endpoint_inward_grade(connection.a.endpoint)
                + b.endpoint_inward_grade(connection.b.endpoint))
            .abs()
                > 1e-6
            {
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
        // Reject unsupported multi-way cliques/cycles rather than silently choosing
        // a movement. One turnout consists of a stem and two branches.
        for initial in occupied.keys() {
            let mut ports = BTreeSet::from([*initial]);
            loop {
                let before = ports.len();
                for (a, b) in &pairs {
                    if ports.contains(a) || ports.contains(b) {
                        ports.insert(*a);
                        ports.insert(*b);
                    }
                }
                if ports.len() > 3 {
                    return Err(invalid);
                }
                if ports.len() == before {
                    break;
                }
            }
        }
        Ok(())
    }
}
impl World {
    /// Connect exact, free endpoints without merging or renumbering infrastructure.
    /// Validation rejects accidental overlap, cusps and foreign ownership; a three-port turnout is supported.
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
    let ring_return = current_id == first.alignment_id
        && ((connections[0].a.endpoint == AlignmentEndpoint::Start
            && start == 1.0
            && last.position > first.position)
            || (connections[0].a.endpoint == AlignmentEndpoint::End
                && start == 0.0
                && last.position < first.position));
    if current_id != last.alignment_id
        || (!seen.insert(current_id) && !ring_return)
        || start == last.position
    {
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
) -> (Vec<(StationId, StationId, Vec<AlignmentConnection>)>, bool) {
    let mut edges = Vec::new();
    let mut truncated = false;
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
            let mut pending = vec![(
                vec![first.clone()],
                first.b,
                BTreeSet::from([first.a.alignment_id]),
            )];
            let mut searched = 0usize;
            while let Some((chain, incoming, mut seen)) = pending.pop() {
                searched += 1;
                if chain.len() > 256 || searched > 4096 {
                    truncated = true;
                    if searched > 4096 {
                        break;
                    }
                    continue;
                }
                if let Some(target) = nearest(incoming) {
                    if resolve_corridor(data, source.station_id, target.station_id, &chain).is_ok()
                    {
                        edges.push((source.station_id, target.station_id, chain));
                    }
                    continue;
                }
                if !seen.insert(incoming.alignment_id) {
                    continue;
                }
                let exit = AlignmentEndpointRef {
                    alignment_id: incoming.alignment_id,
                    endpoint: if incoming.endpoint == AlignmentEndpoint::Start {
                        AlignmentEndpoint::End
                    } else {
                        AlignmentEndpoint::Start
                    },
                };
                let mut next: Vec<_> = data
                    .alignment_connections
                    .iter()
                    .filter_map(|link| {
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
                    })
                    .collect();
                next.sort_by_key(|link| {
                    (
                        link.b.alignment_id,
                        link.b.endpoint == AlignmentEndpoint::End,
                    )
                });
                for link in next.into_iter().rev() {
                    let mut extended = chain.clone();
                    extended.push(link.clone());
                    pending.push((extended, link.b, seen.clone()));
                }
            }
        }
    }
    // Save order and endpoint-pair storage orientation must not change which
    // candidate survives bounded search.
    edges.sort_by_key(|(from, to, chain)| {
        (
            *from,
            *to,
            chain
                .iter()
                .map(|link| {
                    (
                        link.a.alignment_id,
                        link.a.endpoint == AlignmentEndpoint::End,
                        link.b.alignment_id,
                        link.b.endpoint == AlignmentEndpoint::End,
                    )
                })
                .collect::<Vec<_>>(),
        )
    });
    (edges, truncated)
}
