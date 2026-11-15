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
                if norm <= 1e-9 || lead < TRACK_LANE_SPACING_M {
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
                return Err(WorldError::InsufficientTurnoutSpace);
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

/// Built lane polylines already trimmed to the curved movement joins.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrackLaneGeometry {
    pub alignment_id: AlignmentId,
    pub section_index: usize,
    pub lane_index: u8,
    pub points: Vec<GeoPoint>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrackRenderGeometry {
    pub internal_movements: Vec<InternalTrackMovementGeometry>,
    pub lanes: Vec<TrackLaneGeometry>,
    pub movements: Vec<TrackMovementGeometry>,
}
impl World {
    pub fn track_render_geometry(&self) -> Result<TrackRenderGeometry, WorldError> {
        let movements = self.track_movement_geometries()?;
        let internal_movements = self.internal_track_movement_geometries()?;
        let mut lanes = Vec::new();
        for a in &self.data.alignments {
            let length = a.length_m();
            let mut positions = vec![0.0];
            let mut accumulated = 0.0;
            for pair in a.points.windows(2) {
                accumulated += super::alignment::distance(pair[0], pair[1]);
                positions.push(accumulated / length);
            }
            let boundaries = a
                .anchor_indices
                .clone()
                .unwrap_or_else(|| vec![0, a.points.len() - 1]);
            for (section_index, anchors) in boundaries.windows(2).enumerate() {
                let count = a
                    .section_infrastructure(section_index)
                    .expect("validated section")
                    .track_count;
                for lane_index in 0..count {
                    let mut low = positions[anchors[0]];
                    let mut high = positions[anchors[1]];
                    let mut low_join = None;
                    let mut high_join = None;
                    for movement in &movements {
                        let port = movement.movement.from;
                        if port.endpoint.alignment_id != a.id || port.lane_index != lane_index {
                            continue;
                        }
                        let t = movement.from_position;
                        let join = movement.points[0];
                        match port.endpoint.endpoint {
                            AlignmentEndpoint::Start if section_index == 0 && t >= low => {
                                low = t;
                                low_join = Some(join);
                            }
                            AlignmentEndpoint::End
                                if section_index + 1 == a.section_count() && t <= high =>
                            {
                                high = t;
                                high_join = Some(join);
                            }
                            _ => {}
                        }
                    }
                    for movement in &internal_movements {
                        let port = movement.movement.from;
                        if port.alignment_id != a.id
                            || port.section_index != section_index
                            || port.lane_index != lane_index
                        {
                            continue;
                        }
                        match port.side {
                            AlignmentEndpoint::Start if movement.from_position >= low => {
                                low = movement.from_position;
                                low_join = Some(movement.points[0]);
                            }
                            AlignmentEndpoint::End if movement.from_position <= high => {
                                high = movement.from_position;
                                high_join = Some(movement.points[0]);
                            }
                            _ => {}
                        }
                    }
                    if low >= high {
                        return Err(WorldError::InsufficientTurnoutSpace);
                    }
                    let lateral = ((f64::from(count) - 1.0) / 2.0 - f64::from(lane_index))
                        * TRACK_LANE_SPACING_M;
                    let mut ts = vec![low];
                    ts.extend(positions.iter().copied().filter(|t| *t > low && *t < high));
                    ts.push(high);
                    let mut points: Vec<_> = ts
                        .into_iter()
                        .map(|t| {
                            let (x, y) = vector(
                                a.point_at((t - 1e-5).max(0.0)),
                                a.point_at((t + 1e-5).min(1.0)),
                            );
                            let norm = x.hypot(y);
                            if norm <= 1e-9 {
                                a.point_at(t)
                            } else {
                                offset(a.point_at(t), -y / norm * lateral, x / norm * lateral)
                            }
                        })
                        .collect();
                    if let Some(p) = low_join {
                        points[0] = p;
                    }
                    if let Some(p) = high_join {
                        *points.last_mut().expect("two endpoints") = p;
                    }
                    lanes.push(TrackLaneGeometry {
                        alignment_id: a.id,
                        section_index,
                        lane_index,
                        points,
                    });
                }
            }
        }
        Ok(TrackRenderGeometry {
            lanes,
            movements,
            internal_movements,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InternalTrackMovementGeometry {
    pub movement: InternalTrackMovement,
    pub from_position: f64,
    pub to_position: f64,
    pub points: Vec<GeoPoint>,
}
impl World {
    pub fn internal_track_movement_geometries(
        &self,
    ) -> Result<Vec<InternalTrackMovementGeometry>, WorldError> {
        let mut result = Vec::new();
        for pair in self.internal_lane_movements().chunks_exact(2) {
            let movement = pair[0];
            if movement.point.is_none() {
                continue;
            }
            let a = self
                .data
                .alignments
                .iter()
                .find(|a| a.id == movement.from.alignment_id)
                .expect("validated alignment");
            let length = a.length_m();
            let mut positions = vec![0.0];
            let mut accumulated = 0.0;
            for points in a.points.windows(2) {
                accumulated += super::alignment::distance(points[0], points[1]);
                positions.push(accumulated / length);
            }
            let anchors = a.anchor_indices.as_ref().expect("multiple sections");
            let approach = |port: TrackSectionPort| {
                let start = port.side == AlignmentEndpoint::Start;
                let lo = positions[anchors[port.section_index]];
                let hi = positions[anchors[port.section_index + 1]];
                let boundary = if start { lo } else { hi };
                let free = self
                    .data
                    .station_placements
                    .iter()
                    .filter(|p| p.alignment_id == a.id)
                    .map(|p| (p.position - boundary).abs() * length - p.length_m / 2.0)
                    .fold(length, f64::min)
                    .max(0.0);
                let lead = 30.0_f64.min((hi - lo) * length / 4.0).min(free / 2.0);
                if lead < TRACK_LANE_SPACING_M {
                    return None;
                }
                let t = boundary + if start { lead / length } else { -lead / length };
                let epsilon = (lead / length / 10.0).min(1e-5);
                let (x, y) = vector(a.point_at(t - epsilon), a.point_at(t + epsilon));
                let norm = x.hypot(y);
                if norm <= 1e-9 {
                    return None;
                }
                let count = a
                    .section_infrastructure(port.section_index)
                    .unwrap()
                    .track_count;
                let lateral = ((f64::from(count) - 1.0) / 2.0 - f64::from(port.lane_index))
                    * TRACK_LANE_SPACING_M;
                let p = offset(a.point_at(t), -y / norm * lateral, x / norm * lateral);
                let direction = if start { -1.0 } else { 1.0 };
                let c = offset(
                    p,
                    x / norm * direction * lead * 2.0 / 3.0,
                    y / norm * direction * lead * 2.0 / 3.0,
                );
                Some((p, c, t))
            };
            let (Some((p0, p1, from_position)), Some((p3, p2, to_position))) =
                (approach(movement.from), approach(movement.to))
            else {
                return Err(WorldError::InsufficientTurnoutSpace);
            };
            let points: Vec<_> = (0..=24)
                .map(|i| {
                    let t = f64::from(i) / 24.0;
                    let u = 1.0 - t;
                    let interp = |a: f64, b: f64, c: f64, d: f64| {
                        u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
                    };
                    GeoPoint {
                        latitude: interp(p0.latitude, p1.latitude, p2.latitude, p3.latitude),
                        longitude: interp(p0.longitude, p1.longitude, p2.longitude, p3.longitude),
                    }
                })
                .collect();
            result.push(InternalTrackMovementGeometry {
                movement,
                from_position,
                to_position,
                points: points.clone(),
            });
            result.push(InternalTrackMovementGeometry {
                movement: pair[1],
                from_position: to_position,
                to_position: from_position,
                points: points.into_iter().rev().collect(),
            });
        }
        Ok(result)
    }
}
