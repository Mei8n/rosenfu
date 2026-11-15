//! Per-lane geometric paths for explicit endpoint movements.
use super::*;

/// Gameplay centre spacing, in metres. This is not an engineering standard.
pub const TRACK_LANE_SPACING_M: f64 = 4.0;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrackMovementGeometry {
    pub movement: TrackMovement,
    pub from_position: f64,
    pub to_position: f64,
    /// Cubic lane path, including both joins to parallel offset track geometry.
    pub points: Vec<GeoPoint>,
}

fn offset(point: GeoPoint, dx: f64, dy: f64) -> GeoPoint {
    let radians_per_m = 1.0 / 6_371_000.0;
    GeoPoint {
        latitude: point.latitude + (dy * radians_per_m).to_degrees(),
        longitude: point.longitude
            + (dx * radians_per_m / point.latitude.to_radians().cos()).to_degrees(),
    }
}
fn vector(a: GeoPoint, b: GeoPoint) -> (f64, f64) {
    let y = (b.latitude - a.latitude).to_radians() * 6_371_000.0;
    let x = (b.longitude - a.longitude).to_radians()
        * 6_371_000.0
        * ((a.latitude + b.latitude) / 2.0).to_radians().cos();
    (x, y)
}
impl World {
    /// Return actual curved lane paths, not duplicate centre lines or a Y icon.
    /// Reversed movements use precisely reversed samples of the same curve.
    /// The corridor itself is unchanged; these paths connect individual lanes.
    pub fn track_movement_geometries(&self) -> Result<Vec<TrackMovementGeometry>, WorldError> {
        let movements = self.alignment_lane_movements();
        let mut result = Vec::new();
        for pair in movements.chunks_exact(2) {
            let movement = pair[0];
            if movement.point.is_none() {
                continue;
            }
            let approach = |port: TrackLanePort| {
                let a = self
                    .data
                    .alignments
                    .iter()
                    .find(|a| a.id == port.endpoint.alignment_id)
                    .expect("validated port");
                let start = port.endpoint.endpoint == AlignmentEndpoint::Start;
                let length = a.length_m();
                // Keep the curve within both physical spans and outside platforms.
                let free = self
                    .data
                    .station_placements
                    .iter()
                    .filter(|p| p.alignment_id == a.id)
                    .map(|p| {
                        (if start { p.position } else { 1.0 - p.position }) * length
                            - p.length_m / 2.0
                    })
                    .fold(length / 2.0, f64::min)
                    .max(0.0);
                let boundaries = a
                    .anchor_indices
                    .clone()
                    .unwrap_or_else(|| vec![0, a.points.len() - 1]);
                let section = if start {
                    &a.points[..=boundaries[1]]
                } else {
                    &a.points[boundaries[boundaries.len() - 2]..]
                };
                let section_length: f64 = section
                    .windows(2)
                    .map(|p| {
                        let (x, y) = vector(p[0], p[1]);
                        x.hypot(y)
                    })
                    .sum();
                let lead = 30.0_f64.min(section_length / 4.0).min(free / 2.0);
                let t = if start {
                    lead / length
                } else {
                    1.0 - lead / length
                };
                let inside = a.point_at(t);
                let epsilon = (lead / length / 10.0).min(1e-5);
                let (x, y) = vector(
                    a.point_at((t - epsilon).max(0.0)),
                    a.point_at((t + epsilon).min(1.0)),
                );
                let norm = x.hypot(y);
                if norm <= 1e-9 || lead <= 1e-9 {
                    return None;
                }
                let count = a
                    .section_infrastructure(if start { 0 } else { a.section_count() - 1 })
                    .expect("validated section")
                    .track_count;
                let lateral = ((f64::from(count) - 1.0) / 2.0 - f64::from(port.lane_index))
                    * TRACK_LANE_SPACING_M;
                let orientation = if start { -1.0 } else { 1.0 };
                let normal = (-y / norm, x / norm);
                let p = offset(inside, normal.0 * lateral, normal.1 * lateral);
                let control = offset(
                    p,
                    x / norm * orientation * lead * 2.0 / 3.0,
                    y / norm * orientation * lead * 2.0 / 3.0,
                );
                Some((p, control, t))
            };
            let (Some((a, b, from_position)), Some((d, c, to_position))) =
                (approach(movement.from), approach(movement.to))
            else {
                return Err(WorldError::InvalidAlignmentConnection);
            };
            let points: Vec<_> = (0..=24)
                .map(|i| {
                    let t = f64::from(i) / 24.0;
                    let u = 1.0 - t;
                    let interpolate = |a: f64, b: f64, c: f64, d: f64| {
                        u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
                    };
                    GeoPoint {
                        latitude: interpolate(a.latitude, b.latitude, c.latitude, d.latitude),
                        longitude: interpolate(a.longitude, b.longitude, c.longitude, d.longitude),
                    }
                })
                .collect();
            result.push(TrackMovementGeometry {
                movement,
                from_position,
                to_position,
                points: points.clone(),
            });
            result.push(TrackMovementGeometry {
                movement: pair[1],
                from_position: to_position,
                to_position: from_position,
                points: points.into_iter().rev().collect(),
            });
        }
        Ok(result)
    }
}
