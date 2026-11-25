//! Per-lane geometric paths for explicit endpoint movements.
use super::*;

/// Gameplay centre spacing, in metres. This is not an engineering standard.
pub const TRACK_LANE_SPACING_M: f64 = 4.0;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrackMovementGeometry {
    pub properties: TurnoutMovementProperties,
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
        self.track_movement_geometries_impl(true)
    }
    fn track_movement_geometries_impl(
        &self,
        evaluate_properties: bool,
    ) -> Result<Vec<TrackMovementGeometry>, WorldError> {
        let movements = self.alignment_lane_movements();
        let mut result = Vec::new();
        for pair in movements.as_chunks::<2>().0 {
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
            let track_cap = [
                movement.from.endpoint.alignment_id,
                movement.to.endpoint.alignment_id,
            ]
            .iter()
            .map(|id| {
                self.data
                    .alignments
                    .iter()
                    .find(|line| line.id == *id)
                    .unwrap()
                    .effective_design_speed_kmh()
            })
            .fold(f64::INFINITY, f64::min);
            let properties = if evaluate_properties {
                movement_properties(a, b, c, d, track_cap, None)
            } else {
                TurnoutMovementProperties::validation_only()
            };
            result.push(TrackMovementGeometry {
                properties,
                movement,
                from_position,
                to_position,
                points: points.clone(),
            });
            result.push(TrackMovementGeometry {
                properties,
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
type EndpointLane = (AlignmentId, u8);
type SectionLane = (AlignmentId, usize, u8);

struct LaneJoinIndex<'a> {
    endpoints: BTreeMap<EndpointLane, Vec<&'a TrackMovementGeometry>>,
    internal: BTreeMap<SectionLane, Vec<&'a InternalTrackMovementGeometry>>,
}

impl<'a> LaneJoinIndex<'a> {
    fn new(
        endpoints: &'a [TrackMovementGeometry],
        internal: &'a [InternalTrackMovementGeometry],
    ) -> Self {
        let mut result = Self {
            endpoints: BTreeMap::new(),
            internal: BTreeMap::new(),
        };
        // Keep the movement order within each lane. Equal positions still
        // select the last matching join, exactly as the full scan did.
        for movement in endpoints {
            let from = movement.movement.from;
            result
                .endpoints
                .entry((from.endpoint.alignment_id, from.lane_index))
                .or_default()
                .push(movement);
        }
        for movement in internal {
            let from = movement.movement.from;
            result
                .internal
                .entry((from.alignment_id, from.section_index, from.lane_index))
                .or_default()
                .push(movement);
        }
        result
    }

    fn endpoint(&self, key: EndpointLane) -> &[&'a TrackMovementGeometry] {
        self.endpoints.get(&key).map_or(&[], Vec::as_slice)
    }

    fn internal(&self, key: SectionLane) -> &[&'a InternalTrackMovementGeometry] {
        self.internal.get(&key).map_or(&[], Vec::as_slice)
    }
}

impl World {
    pub fn track_render_geometry(&self) -> Result<TrackRenderGeometry, WorldError> {
        let movements = self.track_movement_geometries()?;
        let internal_movements = self.internal_track_movement_geometries()?;
        let joins = LaneJoinIndex::new(&movements, &internal_movements);
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
                    for movement in joins.endpoint((a.id, lane_index)) {
                        let port = movement.movement.from;
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
                    for movement in joins.internal((a.id, section_index, lane_index)) {
                        let port = movement.movement.from;
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
    pub properties: TurnoutMovementProperties,
    pub movement: InternalTrackMovement,
    pub from_position: f64,
    pub to_position: f64,
    pub points: Vec<GeoPoint>,
}
impl World {
    pub fn internal_track_movement_geometries(
        &self,
    ) -> Result<Vec<InternalTrackMovementGeometry>, WorldError> {
        self.internal_track_movement_geometries_impl(true)
    }
    fn internal_track_movement_geometries_impl(
        &self,
        evaluate_properties: bool,
    ) -> Result<Vec<InternalTrackMovementGeometry>, WorldError> {
        let mut result = Vec::new();
        for pair in self.internal_lane_movements().as_chunks::<2>().0 {
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
            let properties = if evaluate_properties {
                movement_properties(p0, p1, p2, p3, a.effective_design_speed_kmh(), None)
            } else {
                TurnoutMovementProperties::validation_only()
            };
            result.push(InternalTrackMovementGeometry {
                properties,
                movement,
                from_position,
                to_position,
                points: points.clone(),
            });
            result.push(InternalTrackMovementGeometry {
                properties,
                movement: pair[1],
                from_position: to_position,
                to_position: from_position,
                points: points.into_iter().rev().collect(),
            });
        }
        Ok(result)
    }
}

/// Game calibration, not an approved railway engineering or comfort standard.
pub const TURNOUT_GAME_LATERAL_ACCELERATION_MPS2: f64 = 0.65;
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct TurnoutMovementProperties {
    pub length_m: f64,
    /// Curvature extrema from isolated roots of the cubic curvature derivative.
    /// None denotes a straight movement, not an unknown equipment limit.
    pub minimum_radius_m: Option<f64>,
    pub curvature_speed_limit_kmh: Option<f64>,
    pub track_speed_limit_kmh: f64,
    /// Reserved for a known switch design. No turnout number is guessed.
    pub equipment_speed_limit_kmh: Option<f64>,
    pub speed_limit_kmh: f64,
    pub lateral_acceleration_mps2: f64,
}
impl TurnoutMovementProperties {
    // Private sentinel, never returned from a public geometry query.
    fn validation_only() -> Self {
        Self {
            length_m: 0.,
            minimum_radius_m: None,
            curvature_speed_limit_kmh: None,
            track_speed_limit_kmh: 0.,
            equipment_speed_limit_kmh: None,
            speed_limit_kmh: 0.,
            lateral_acceleration_mps2: 0.,
        }
    }
}
impl World {
    pub(super) fn validate_turnout_geometry(&self) -> Result<(), WorldError> {
        self.track_movement_geometries_impl(false)?;
        self.internal_track_movement_geometries_impl(false)?;
        Ok(())
    }
}
fn poly_value(p: &[f64], t: f64) -> f64 {
    p.iter().rev().fold(0.0, |v, c| v * t + c)
}
fn poly_derivative(p: &[f64]) -> Vec<f64> {
    p.iter()
        .enumerate()
        .skip(1)
        .map(|(i, c)| *c * i as f64)
        .collect()
}
fn poly_scale(p: &[f64], scale: f64) -> Vec<f64> {
    p.iter().map(|v| v * scale).collect()
}
fn poly_add(a: &[f64], b: &[f64]) -> Vec<f64> {
    (0..a.len().max(b.len()))
        .map(|i| a.get(i).unwrap_or(&0.0) + b.get(i).unwrap_or(&0.0))
        .collect()
}
fn poly_sub(a: &[f64], b: &[f64]) -> Vec<f64> {
    poly_add(a, &poly_scale(b, -1.0))
}
fn poly_mul(a: &[f64], b: &[f64]) -> Vec<f64> {
    if a.is_empty() || b.is_empty() {
        return vec![];
    }
    let mut p = vec![0.0; a.len() + b.len() - 1];
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            p[i + j] += x * y;
        }
    }
    p
}
/// Recursive derivative isolation partitions [0,1] into monotone intervals;
/// sign-changing roots are bisected, and repeated roots are checked at extrema.
fn poly_roots_unit(coefficients: &[f64]) -> Vec<f64> {
    let mut p = coefficients.to_vec();
    while p.last() == Some(&0.0) {
        p.pop();
    }
    if p.len() < 2 {
        return vec![];
    }
    if p.len() == 2 {
        let root = -p[0] / p[1];
        return if root.is_finite() && (0.0..=1.0).contains(&root) {
            vec![root]
        } else {
            vec![]
        };
    }
    let mut cuts = vec![0.0];
    cuts.extend(poly_roots_unit(&poly_derivative(&p)));
    cuts.push(1.0);
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-14);
    let tolerance = p.iter().map(|c| c.abs()).sum::<f64>() * 1e-12;
    let mut roots: Vec<_> = cuts
        .iter()
        .copied()
        .filter(|t| poly_value(&p, *t).abs() <= tolerance)
        .collect();
    for window in cuts.windows(2) {
        let (mut lo, mut hi) = (window[0], window[1]);
        let mut left = poly_value(&p, lo);
        let right = poly_value(&p, hi);
        if left == 0.0 || right == 0.0 || left.is_sign_positive() == right.is_sign_positive() {
            continue;
        }
        for _ in 0..80 {
            let mid = (lo + hi) / 2.0;
            let value = poly_value(&p, mid);
            if value == 0.0 {
                lo = mid;
                hi = mid;
                break;
            }
            if value.is_sign_positive() == left.is_sign_positive() {
                lo = mid;
                left = value;
            } else {
                hi = mid;
            }
        }
        roots.push((lo + hi) / 2.0);
    }
    roots
}
fn movement_properties(
    a: GeoPoint,
    b: GeoPoint,
    c: GeoPoint,
    d: GeoPoint,
    track_cap: f64,
    equipment_cap: Option<f64>,
) -> TurnoutMovementProperties {
    let (bx, by) = vector(a, b);
    let (cx, cy) = vector(a, c);
    let (dx, dy) = vector(a, d);
    let mut maximum_curvature: f64 = 0.0;
    let mut length = 0.0;
    let mut previous = (0.0, 0.0);
    for i in 0..=1024 {
        let t = f64::from(i) / 1024.0;
        let u = 1.0 - t;
        let px = 3.0 * u * u * t * bx + 3.0 * u * t * t * cx + t * t * t * dx;
        let py = 3.0 * u * u * t * by + 3.0 * u * t * t * cy + t * t * t * dy;
        if i > 0 {
            length += (px - previous.0).hypot(py - previous.1);
        }
        previous = (px, py);
    }
    // v(t)=v0+v1*t+v2*t²; curvature extrema satisfy
    // 2*N'(t)*Q(t)-3*N(t)*Q'(t)=0, N=cross(v,v'), Q=dot(v,v).
    let vx = vec![
        3.0 * bx,
        6.0 * (cx - 2.0 * bx),
        3.0 * (dx - 3.0 * cx + 3.0 * bx),
    ];
    let vy = vec![
        3.0 * by,
        6.0 * (cy - 2.0 * by),
        3.0 * (dy - 3.0 * cy + 3.0 * by),
    ];
    let n = poly_sub(
        &poly_mul(&vx, &poly_derivative(&vy)),
        &poly_mul(&vy, &poly_derivative(&vx)),
    );
    let q = poly_add(&poly_mul(&vx, &vx), &poly_mul(&vy, &vy));
    let stationary = poly_sub(
        &poly_scale(&poly_mul(&poly_derivative(&n), &q), 2.0),
        &poly_scale(&poly_mul(&n, &poly_derivative(&q)), 3.0),
    );
    let mut candidates = vec![0.0, 1.0];
    candidates.extend(poly_roots_unit(&stationary));
    candidates.extend(poly_roots_unit(&poly_derivative(&q)));
    for t in candidates {
        let speed_sq = poly_value(&q, t);
        if speed_sq <= 1e-12 || !speed_sq.is_finite() {
            maximum_curvature = f64::INFINITY;
            break;
        }
        maximum_curvature = maximum_curvature.max(poly_value(&n, t).abs() / speed_sq.powf(1.5));
    }
    let radius = (maximum_curvature > 1e-10).then(|| 1.0 / maximum_curvature);
    let curve = radius.map(|r| 3.6 * (TURNOUT_GAME_LATERAL_ACCELERATION_MPS2 * r).sqrt());
    let cap = track_cap
        .min(equipment_cap.unwrap_or(f64::INFINITY))
        .min(curve.unwrap_or(f64::INFINITY));
    TurnoutMovementProperties {
        length_m: length,
        minimum_radius_m: radius,
        curvature_speed_limit_kmh: curve,
        track_speed_limit_kmh: track_cap,
        equipment_speed_limit_kmh: equipment_cap,
        speed_limit_kmh: cap,
        lateral_acceleration_mps2: TURNOUT_GAME_LATERAL_ACCELERATION_MPS2,
    }
}

#[cfg(test)]
mod speed_tests {
    use super::*;
    #[test]
    fn analytic_curvature_extrema_cover_sharp_s_curves_and_stationary_derivatives() {
        let origin = GeoPoint {
            latitude: 35.0,
            longitude: 139.0,
        };
        let p = |x, y| offset(origin, x, y);
        let sharp = movement_properties(
            p(0., 0.),
            p(0.001, 0.),
            p(10., 4.),
            p(10.001, 4.),
            120.,
            None,
        );
        let regular = movement_properties(p(0., 0.), p(3., 0.), p(7., 4.), p(10., 4.), 120., None);
        assert!(sharp.speed_limit_kmh < regular.speed_limit_kmh);
        let cusp = movement_properties(p(0., 0.), p(10., 0.), p(-10., 0.), p(0., 0.), 120., None);
        assert_eq!(cusp.speed_limit_kmh, 0.);
        let roots = poly_roots_unit(&[0.09, -0.6, 1.0]);
        assert!(roots.iter().any(|r| (*r - 0.3).abs() < 1e-8));
        // A narrow extremum between regular sample positions must still be found.
        let r = 0.000_321_234;
        let roots = poly_roots_unit(&[r * r, -2.0 * r, 1.0]);
        assert!(roots.iter().any(|x| (*x - r).abs() < 1e-10));
    }

    #[test]
    fn lane_join_index_matches_scan_order_without_unrelated_candidates() {
        let position = |id| GeoPoint {
            latitude: f64::from(id),
            longitude: 139.0,
        };
        let endpoint = |id, lane_index, side, point| {
            let from = TrackLanePort {
                endpoint: AlignmentEndpointRef {
                    alignment_id: AlignmentId(id),
                    endpoint: side,
                },
                lane_index,
            };
            TrackMovementGeometry {
                properties: TurnoutMovementProperties::validation_only(),
                movement: TrackMovement {
                    from,
                    to: from,
                    point: None,
                },
                from_position: 0.25,
                to_position: 0.25,
                points: vec![position(point)],
            }
        };
        let internal = |id, section_index, lane_index, side, point| {
            let from = TrackSectionPort {
                alignment_id: AlignmentId(id),
                section_index,
                lane_index,
                side,
            };
            InternalTrackMovementGeometry {
                properties: TurnoutMovementProperties::validation_only(),
                movement: InternalTrackMovement {
                    from,
                    to: from,
                    point: None,
                },
                from_position: 0.5,
                to_position: 0.5,
                points: vec![position(point)],
            }
        };
        let start = AlignmentEndpoint::Start;
        let end = AlignmentEndpoint::End;
        let mut endpoints = vec![endpoint(1, 0, start, 31)];
        let mut internals = vec![internal(1, 0, 0, end, 35)];
        // Finite candidate vectors include many geographically unrelated lanes.
        for id in 2..=1001 {
            endpoints.push(endpoint(id, 0, start, 33));
            internals.push(internal(id, 0, 0, end, 36));
        }
        // Insert a later match at the same position, different lanes and sections.
        endpoints.push(endpoint(1, 0, start, 32));
        endpoints.push(endpoint(1, 1, end, 34));
        internals.push(internal(1, 1, 0, start, 37));
        internals.push(internal(1, 0, 0, end, 38));
        internals.push(internal(1, 0, 1, end, 39));
        let index = LaneJoinIndex::new(&endpoints, &internals);
        for lane in 0..=2 {
            let expected: Vec<_> = endpoints
                .iter()
                .filter(|m| {
                    m.movement.from.endpoint.alignment_id == AlignmentId(1)
                        && m.movement.from.lane_index == lane
                })
                .collect();
            assert_eq!(index.endpoint((AlignmentId(1), lane)), expected.as_slice());
            for section in 0..=2 {
                let expected: Vec<_> = internals
                    .iter()
                    .filter(|m| {
                        let from = m.movement.from;
                        from.alignment_id == AlignmentId(1)
                            && from.lane_index == lane
                            && from.section_index == section
                    })
                    .collect();
                assert_eq!(
                    index.internal((AlignmentId(1), section, lane)),
                    expected.as_slice()
                );
            }
        }
        assert_eq!(index.endpoint((AlignmentId(1), 0)).len(), 2);
        assert_eq!(index.internal((AlignmentId(1), 0, 0)).len(), 2);
        assert!(index.endpoint((AlignmentId(1002), 0)).is_empty());
        assert!(index.internal((AlignmentId(1002), 0, 0)).is_empty());
    }

    #[test]
    fn turnout_speed_uses_local_curvature_not_total_length_or_net_heading() {
        let origin = GeoPoint {
            latitude: 35.0,
            longitude: 139.0,
        };
        let p = |x, y| offset(origin, x, y);
        // Parallel entry/exit headings still contain an S curve.
        let short = movement_properties(p(0., 0.), p(5., 0.), p(10., 4.), p(15., 4.), 120., None);
        let long = movement_properties(p(0., 0.), p(10., 0.), p(20., 4.), p(30., 4.), 120., None);
        assert!(short.minimum_radius_m.is_some());
        assert!(short.speed_limit_kmh < long.speed_limit_kmh);
        let straight = movement_properties(
            p(0., 0.),
            p(10., 0.),
            p(20., 0.),
            p(30., 0.),
            120.,
            Some(45.),
        );
        assert_eq!(straight.speed_limit_kmh, 45.);
        let longer = movement_properties(
            p(0., 0.),
            p(100., 0.),
            p(200., 0.),
            p(300., 0.),
            120.,
            Some(45.),
        );
        assert_eq!(longer.speed_limit_kmh, straight.speed_limit_kmh);
        let low_track =
            movement_properties(p(0., 0.), p(10., 0.), p(20., 4.), p(30., 4.), 5., None);
        assert_eq!(low_track.speed_limit_kmh, 5.);
    }
}
