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
