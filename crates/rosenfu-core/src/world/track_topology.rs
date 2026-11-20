//! Deterministic individual-track wiring at explicit corridor endpoint joins.
//! Coordinates alone, station groups and map crossings never create movements.
use super::*;

/// Lane index is measured left-to-right looking in the alignment's start-to-end
/// direction. The tuple identifies a port without allocating unstable vector IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackLanePort {
    pub endpoint: AlignmentEndpointRef,
    pub lane_index: u8,
}

/// Shared point resource. Both directions and all converging lanes occupy it.
/// This describes conflicts; train dispatch/locking is not yet simulated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackPointResource {
    pub a: AlignmentEndpointRef,
    pub b: AlignmentEndpointRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackMovement {
    pub from: TrackLanePort,
    pub to: TrackLanePort,
    /// Equal-count straight continuations do not require a merging point.
    pub point: Option<TrackPointResource>,
}

fn endpoint_key(end: AlignmentEndpointRef) -> (AlignmentId, bool) {
    (end.alignment_id, end.endpoint == AlignmentEndpoint::End)
}

impl World {
    /// Expand explicit corridor joins into bidirectional lane movements.
    /// 2-to-1 yields two Y-point paths, not one disconnected decorative line.
    /// 4-to-2 maps adjacent pairs; equal counts preserve physical lateral order.
    /// No crossing/all-to-all paths are invented. This is endpoint wiring only:
    /// interior branching, lane-selective service routing and point geometry are
    /// separate features, not implied by this topology view.
    pub fn alignment_lane_movements(&self) -> Vec<TrackMovement> {
        let mut links: Vec<_> = self
            .data
            .alignment_connections
            .iter()
            .map(|link| {
                if endpoint_key(link.a) < endpoint_key(link.b) {
                    (link.a, link.b)
                } else {
                    (link.b, link.a)
                }
            })
            .collect();
        links.sort_by_key(|(a, b)| (endpoint_key(*a), endpoint_key(*b)));
        let mut result = Vec::new();
        for (a, b) in links.iter().copied() {
            let count = |end: AlignmentEndpointRef| {
                let alignment = self
                    .data
                    .alignments
                    .iter()
                    .find(|v| v.id == end.alignment_id)
                    .expect("validated connection alignment");
                alignment
                    .section_infrastructure(match end.endpoint {
                        AlignmentEndpoint::Start => 0,
                        AlignmentEndpoint::End => alignment.section_count() - 1,
                    })
                    .expect("validated section")
                    .track_count
            };
            let (na, nb) = (count(a), count(b));
            let width = na.max(nb);
            let mut ports = vec![a, b];
            for (x, y) in &links {
                if ports.contains(x) || ports.contains(y) {
                    if !ports.contains(x) {
                        ports.push(*x);
                    }
                    if !ports.contains(y) {
                        ports.push(*y);
                    }
                }
            }
            ports.sort_by_key(|port| endpoint_key(*port));
            let point = (na != nb || ports.len() > 2).then_some(TrackPointResource {
                a: ports[0],
                b: ports[1],
            });
            for lane in 0..width {
                let ai = lane * na / width;
                let bi = lane * nb / width;
                let bi = if a.endpoint == b.endpoint {
                    nb - 1 - bi
                } else {
                    bi
                };
                let from = TrackLanePort {
                    endpoint: a,
                    lane_index: ai,
                };
                let to = TrackLanePort {
                    endpoint: b,
                    lane_index: bi,
                };
                result.push(TrackMovement { from, to, point });
                result.push(TrackMovement {
                    from: to,
                    to: from,
                    point,
                });
            }
        }
        result
    }
}

/// A lane at one side of an anchor-delimited section within an alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackSectionPort {
    pub alignment_id: AlignmentId,
    pub section_index: usize,
    pub lane_index: u8,
    pub side: AlignmentEndpoint,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternalTrackPoint {
    pub alignment_id: AlignmentId,
    pub anchor_index: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternalTrackMovement {
    pub from: TrackSectionPort,
    pub to: TrackSectionPort,
    pub point: Option<InternalTrackPoint>,
}
impl World {
    /// Internal section boundaries retain the same bidirectional physical lane
    /// correspondence as endpoint joins. They are not inferred map crossings.
    pub fn internal_lane_movements(&self) -> Vec<InternalTrackMovement> {
        let mut result = Vec::new();
        for a in &self.data.alignments {
            for boundary in 1..a.section_count() {
                let na = a.section_infrastructure(boundary - 1).unwrap().track_count;
                let nb = a.section_infrastructure(boundary).unwrap().track_count;
                let width = na.max(nb);
                let point = (na != nb).then_some(InternalTrackPoint {
                    alignment_id: a.id,
                    anchor_index: boundary,
                });
                for lane in 0..width {
                    let from = TrackSectionPort {
                        alignment_id: a.id,
                        section_index: boundary - 1,
                        lane_index: lane * na / width,
                        side: AlignmentEndpoint::End,
                    };
                    let to = TrackSectionPort {
                        alignment_id: a.id,
                        section_index: boundary,
                        lane_index: lane * nb / width,
                        side: AlignmentEndpoint::Start,
                    };
                    result.push(InternalTrackMovement { from, to, point });
                    result.push(InternalTrackMovement {
                        from: to,
                        to: from,
                        point,
                    });
                }
            }
        }
        result
    }
}
