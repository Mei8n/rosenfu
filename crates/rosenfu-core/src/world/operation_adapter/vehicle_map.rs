//! Verified moving vehicle positions at the committed simulation time.
//! Unsupported geometry is omitted, not guessed.
use super::*;
use crate::operation::{TrainKey, TrainState};
use serde::{Deserialize, Serialize};

/// A position for matching the operation snapshot, not a public numeric label
/// or a determination of where the whole consist lies.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LiveRailMarker {
    pub key: TrainKey,
    /// Exact painted chord containing the native position, not station coordinates.
    pub alignment_start: GeoPoint,
    pub alignment_end: GeoPoint,
    /// Authoritative distance fraction on that chord at the committed clock.
    pub position: f64,
}

/// A committed native position on one verified directed roadway chord.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LiveBusMarker {
    pub key: TrainKey,
    pub edge_start: GeoPoint,
    pub edge_end: GeoPoint,
    pub position: f64,
}

/// Camera rectangle in normalized Web Mercator world coordinates. The renderer
/// projects the chord endpoints first, so filtering in latitude space would
/// incorrectly drop markers on diagonal paths.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct VehicleViewport {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
}

impl VehicleViewport {
    pub fn is_valid(self) -> bool {
        [self.left, self.right, self.top, self.bottom]
            .into_iter()
            .all(|v| v.is_finite() && (-1.0..=2.0).contains(&v))
            && self.left < self.right
            && self.top < self.bottom
    }

    fn contains_chord(self, start: GeoPoint, end: GeoPoint, fraction: f64) -> bool {
        fn project(point: GeoPoint) -> (f64, f64) {
            (
                (point.longitude + 180.0) / 360.0,
                (1.0 - point.latitude.to_radians().tan().asinh() / std::f64::consts::PI) / 2.0,
            )
        }
        let (ax, ay) = project(start);
        let (bx, by) = project(end);
        let x = ax + (bx - ax) * fraction;
        let y = ay + (by - ay) * fraction;
        (self.left..=self.right).contains(&x) && (self.top..=self.bottom).contains(&y)
    }
}

fn in_view(view: Option<VehicleViewport>, start: GeoPoint, end: GeoPoint, fraction: f64) -> bool {
    view.is_none_or(|view| view.contains_chord(start, end, fraction))
}

// View-scoped sampling may inspect more candidates, but not unbounded native
// work. Keep server response SVG/IPC budgets unchanged.
const MAX_VIEW_CANDIDATES: usize = 2048;
fn candidate_limit(view: Option<VehicleViewport>) -> usize {
    if view.is_some() {
        MAX_VIEW_CANDIDATES
    } else {
        MAX_TRAINS
    }
}

const MAX_TRAINS: usize = 128;
const MAX_MARKERS: usize = 32;
const MAX_UNIFORM_PIECES: usize = 16;
const MAX_ROWS_PER_TABLE: usize = 8_192;

struct Candidate<'a> {
    key: &'a TrainKey,
    line: AlignmentId,
    from: StationId,
    to: StationId,
    length_m: f64,
    piece_count: usize,
    fraction: f64,
}

impl World {
    /// A small visual subset of ordinary authored, moving bus legs. Look up
    /// the exact committed stop occurrences and directed edge, not the
    /// nearest visible road. Rendering never loads data or reroutes a bus.
    pub fn live_straight_bus_markers(&self) -> Vec<LiveBusMarker> {
        self.live_bus_markers_in(None)
    }

    pub fn live_bus_markers_in(&self, view: Option<VehicleViewport>) -> Vec<LiveBusMarker> {
        let Some(op) = self
            .operation()
            .filter(|o| !o.invalidated && !o.bus_configs.is_empty())
        else {
            return Vec::new();
        };
        let roads = &self.data.road_transport;
        let now = op.runtime.now_ms();
        // Rail vehicles sort before bus IDs; do not let them consume the
        // bounded bus candidate search before the first bus is considered.
        op.runtime
            .trains_with_service_prefix("bus:")
            .take(candidate_limit(view))
            .filter_map(|train| {
                let route_id = train.key.service_id.strip_prefix("bus:")?;
                if !self.data.ofm_gameplay_bus_routes.contains_key(route_id) {
                    return None;
                }
                let TrainState::Moving {
                    entered_ms,
                    arrival_ms,
                    ..
                } = train.state
                else {
                    return None;
                };
                if now >= arrival_ms {
                    return None;
                }
                let spec = op.runtime.service_spec(&train.key.service_id)?;
                let leg = spec.legs.get(train.leg)?;
                let [section] = leg.sections.as_slice() else {
                    return None;
                };
                let (duration, _) = op
                    .runtime
                    .unblocked_leg_timing(&train.key.service_id, train.leg)?;
                if entered_ms.checked_add(duration) != Some(arrival_ms) {
                    return None;
                }
                let fraction = uniform_position(
                    section,
                    &spec.vehicle,
                    now.checked_sub(entered_ms)?,
                    duration,
                )?;
                let outward = || {
                    let (edge, position) = if let [resource] = section.resources.as_slice() {
                        let (edge, from, to) = roads.gameplay_bus_leg_chord(
                            route_id,
                            train.leg,
                            leg.from.strip_prefix("bus:")?,
                            leg.to.strip_prefix("bus:")?,
                            resource.strip_prefix("road:edge:")?,
                        )?;
                        if !(0.0..=1.0).contains(&from) || !(0.0..=1.0).contains(&to) || from >= to
                        {
                            return None;
                        }
                        (edge, from + (to - from) * fraction)
                    } else {
                        self.data.gameplay_bus_live_outward_chord(
                            route_id, train.leg, leg, section, fraction,
                        )?
                    };
                    let [edge_start, edge_end] = edge.geometry.as_slice() else {
                        return None;
                    };
                    if edge.level != Some(InfrastructureLevel::Ground) || edge_start == edge_end {
                        return None;
                    }
                    Some((*edge_start, *edge_end, position))
                };
                let (edge_start, edge_end, position) = outward().or_else(|| {
                    self.data.gameplay_bus_live_return_chord(
                        route_id, train.leg, spec, section, fraction,
                    )
                })?;
                if !in_view(view, edge_start, edge_end, position) {
                    return None;
                }
                Some(LiveBusMarker {
                    key: train.key.clone(),
                    edge_start,
                    edge_end,
                    position,
                })
            })
            .take(MAX_MARKERS)
            .collect()
    }

    /// Ordinary native marker response. Preserve the old straight Normal
    /// subset, then add only positively proven street-tram movements.
    pub fn live_ground_rail_markers(&self) -> Vec<LiveRailMarker> {
        self.live_rail_markers_in(None)
    }

    pub fn live_rail_markers_in(&self, view: Option<VehicleViewport>) -> Vec<LiveRailMarker> {
        let mut markers = self.isolated_rail_markers_in(view, true);
        if markers.len() < MAX_MARKERS {
            markers.extend(self.joined_street_tram_markers(MAX_MARKERS - markers.len(), view));
        }
        markers
    }

    fn joined_street_tram_markers(
        &self,
        limit: usize,
        view: Option<VehicleViewport>,
    ) -> Vec<LiveRailMarker> {
        let Some(op) = self.operation().filter(|o| !o.invalidated) else {
            return Vec::new();
        };
        // The existing simple marker path remains unaltered. The extra
        // geometry work runs only in bounded worlds with an authored street
        // range and a matching three-section candidate.
        if self.data.road_path_provenance.is_empty()
            || self.data.alignments.len() > MAX_ROWS_PER_TABLE
            || self.data.station_placements.len() > MAX_ROWS_PER_TABLE
            || self.data.alignment_connections.len() > MAX_ROWS_PER_TABLE
        {
            return Vec::new();
        }
        op.runtime
            .trains()
            .take(candidate_limit(view))
            .filter_map(|train| {
                let TrainState::Moving {
                    entered_ms,
                    arrival_ms,
                    ..
                } = train.state
                else {
                    return None;
                };
                let spec = op.runtime.service_spec(&train.key.service_id)?;
                let leg = spec.legs.get(train.leg)?;
                let (lanes, profile) = joined_tram_profile(leg, &spec.vehicle)?;
                if !self.data.road_path_provenance.iter().any(|proof| {
                    proof.alignment_id == lanes[1] && proof.path.carries_street_tram_proof()
                }) {
                    return None;
                }
                Some((train, spec, leg, lanes, profile, entered_ms, arrival_ms))
            })
            .take(8)
            .filter_map(
                |(train, spec, leg, lanes, profile, entered_ms, arrival_ms)| {
                    let (duration, _) = op
                        .runtime
                        .unblocked_leg_timing(&train.key.service_id, train.leg)?;
                    if entered_ms.checked_add(duration)? != arrival_ms {
                        return None;
                    }
                    let fraction = uniform_position(
                        &profile,
                        &spec.vehicle,
                        op.runtime.now_ms().checked_sub(entered_ms)?,
                        duration,
                    )?;
                    let (alignment_start, alignment_end, position) =
                        joined_tram_chord(&self.data, leg, lanes, profile.length_m * fraction)?;
                    if !in_view(view, alignment_start, alignment_end, position) {
                        return None;
                    }
                    Some(LiveRailMarker {
                        key: train.key.clone(),
                        alignment_start,
                        alignment_end,
                        position,
                    })
                },
            )
            .take(limit)
            .collect()
    }

    /// Bounded render subset of moving trains on a verified direct, ground,
    /// isolated single-track Normal rail section. Waiting, released and unsupported
    /// movements are omitted. No clock or save state is changed.
    pub fn live_straight_rail_markers(&self) -> Vec<LiveRailMarker> {
        self.isolated_rail_markers_in(None, false)
    }

    fn isolated_rail_markers_in(
        &self,
        view: Option<VehicleViewport>,
        allow_polyline: bool,
    ) -> Vec<LiveRailMarker> {
        let Some(op) = self.operation().filter(|o| !o.invalidated) else {
            return Vec::new();
        };
        let now = op.runtime.now_ms();
        let candidates: Vec<_> = op
            .runtime
            .trains()
            .take(candidate_limit(view))
            .filter_map(|train| {
                let TrainState::Moving {
                    entered_ms,
                    arrival_ms,
                    ..
                } = train.state
                else {
                    return None;
                };
                if now > arrival_ms {
                    return None;
                }
                let spec = op.runtime.service_spec(&train.key.service_id)?;
                let leg = spec.legs.get(train.leg)?;
                let (line, uniform) = same_lane_uniform_section(&leg.sections)?;
                let (duration, _) = op
                    .runtime
                    .unblocked_leg_timing(&train.key.service_id, train.leg)?;
                if entered_ms.checked_add(duration) != Some(arrival_ms) {
                    return None;
                }
                Some(Candidate {
                    key: &train.key,
                    line: AlignmentId(line),
                    from: StationId(leg.from.parse().ok()?),
                    to: StationId(leg.to.parse().ok()?),
                    length_m: uniform.length_m,
                    piece_count: leg.sections.len(),
                    fraction: uniform_position(
                        &uniform,
                        &spec.vehicle,
                        now.checked_sub(entered_ms)?,
                        duration,
                    )?,
                })
            })
            .collect();
        if candidates.is_empty() {
            return Vec::new();
        }

        // One bounded pass per table, and only requested entries retained.
        // Out-of-budget entities are omitted from this view, not prevented
        // from operating.
        let stations: BTreeSet<_> = candidates.iter().flat_map(|c| [c.from, c.to]).collect();
        let lines: BTreeSet<_> = candidates.iter().map(|c| c.line).collect();
        // The physical renderer may trim a connected lane near a turnout,
        // even if this run leg stops before reaching the endpoint. Without
        // that whole paint path we must not assert the original chord.
        if self.data.alignment_connections.len() > MAX_ROWS_PER_TABLE {
            return Vec::new();
        }
        let joined: BTreeSet<_> = self
            .data
            .alignment_connections
            .iter()
            .flat_map(|link| [link.a.alignment_id, link.b.alignment_id])
            .filter(|line| lines.contains(line))
            .collect();
        let by_station: BTreeMap<_, _> = self
            .data
            .station_placements
            .iter()
            .take(MAX_ROWS_PER_TABLE)
            .filter(|p| stations.contains(&p.station_id))
            .map(|p| (p.station_id, p))
            .collect();
        let by_line: BTreeMap<_, _> = self
            .data
            .alignments
            .iter()
            .take(MAX_ROWS_PER_TABLE)
            .filter(|a| lines.contains(&a.id))
            .map(|a| (a.id, a))
            .collect();
        candidates
            .into_iter()
            .filter_map(|c| {
                let a = by_line.get(&c.line)?;
                let from = by_station.get(&c.from)?;
                let to = by_station.get(&c.to)?;
                let infrastructure = a.section_infrastructure(0)?;
                if joined.contains(&a.id)
                    || from.alignment_id != a.id
                    || to.alignment_id != a.id
                    // An isolated single-track section is painted on its
                    // centerline. Joined or lane-changing paths can be
                    // trimmed or offset and require their own proof.
                    || !(2..=64).contains(&a.points.len())
                    || (!allow_polyline && a.points.len() != 2)
                    || a.section_count() != 1
                    || a.track_type != TrackType::Normal
                    || infrastructure.track_count != 1
                    || infrastructure.level != InfrastructureLevel::Ground
                    || a.vertical_profile
                        .iter()
                        .chain(&a.section_profiles)
                        .any(|p| {
                            p.start_height_m != 0.0
                                || p.end_height_m != 0.0
                                || p.start_tangent_m != 0.0
                                || p.end_tangent_m != 0.0
                        })
                    || !(0.0..=1.0).contains(&from.position)
                    || !(0.0..=1.0).contains(&to.position)
                    || from.position == to.position
                {
                    return None;
                }
                // The exact distance and rail lane from append_sections are the
                // proof; no nearest-geometry or screen coordinate inference.
                let length = a.length_m();
                let expected = (from.position - to.position).abs() * length;
                // Passing-platform boundaries split an otherwise identical
                // lane. Recombining their lengths can differ by a few floating
                // point ulps; any meaningful extra distance implies a detour.
                let roundoff = if c.piece_count == 1 {
                    0.0
                } else {
                    8.0 * c.piece_count as f64 * f64::EPSILON * expected
                };
                if !expected.is_finite() || (expected - c.length_m).abs() > roundoff {
                    return None;
                }
                let position = from.position + (to.position - from.position) * c.fraction;
                let (alignment_start, alignment_end, position) = if a.points.len() == 2 {
                    (a.points[0], a.points[1], position)
                } else {
                    polyline_chord(&a.points, length * position)?
                };
                if !in_view(view, alignment_start, alignment_end, position) {
                    return None;
                }
                Some(LiveRailMarker {
                    key: c.key.clone(),
                    alignment_start,
                    alignment_end,
                    position,
                })
            })
            .take(MAX_MARKERS)
            .collect()
    }
}

fn joined_tram_profile(leg: &RunLeg, vehicle: &Vehicle) -> Option<([AlignmentId; 3], Section)> {
    let [a, b, c] = leg.sections.as_slice() else {
        return None;
    };
    let mut lanes = [AlignmentId(0); 3];
    let mut length_m = 0.;
    let cap = a.speed_limit_kmh.min(vehicle.max_speed_kmh);
    for (piece, lane) in [a, b, c].into_iter().zip(&mut lanes) {
        if !(piece.length_m.is_finite() && piece.length_m > 0.)
            || piece.speed_limit_kmh.min(vehicle.max_speed_kmh) != cap
        {
            return None;
        }
        let mut ids = piece
            .resources
            .iter()
            .filter_map(|key| inline_lane_key(key));
        let (id, 0, 0) = ids.next()? else {
            return None;
        };
        if ids.next().is_some() {
            return None;
        }
        *lane = AlignmentId(id);
        length_m += piece.length_m;
    }
    if lanes[0] == lanes[1] || lanes[1] == lanes[2] || lanes[0] == lanes[2] {
        return None;
    }
    Some((
        lanes,
        Section {
            length_m,
            speed_limit_kmh: cap,
            resources: vec![],
        },
    ))
}

fn joined_tram_chord(
    draft: &WorldDraft,
    leg: &RunLeg,
    lanes: [AlignmentId; 3],
    mut remaining: f64,
) -> Option<(GeoPoint, GeoPoint, f64)> {
    let station = |key: &str| {
        let id = StationId(key.parse().ok()?);
        draft.station_placements.iter().find(|p| p.station_id == id)
    };
    let start = station(&leg.from)?;
    let end = station(&leg.to)?;
    if start.alignment_id != lanes[0] || end.alignment_id != lanes[2] {
        return None;
    }
    let line = |id| draft.alignments.iter().find(|a| a.id == id);
    let lines = [line(lanes[0])?, line(lanes[1])?, line(lanes[2])?];
    if !draft.road_path_provenance.iter().any(|proof| {
        proof.alignment_id == lanes[1]
            && proof.start_point_index == 0
            && proof.end_point_index.checked_add(1) == Some(lines[1].points.len())
            && proof.from_position == 0.
            && proof.to_position == 1.
            && proof.path.carries_street_tram_proof()
    }) {
        return None;
    }
    for a in lines {
        if a.track_type != TrackType::Tram
            || !(2..=64).contains(&a.points.len())
            || a.section_count() != 1
            || a.section_infrastructure(0)? != SectionInfrastructure::default()
            || a.vertical_profile
                .iter()
                .chain(&a.section_profiles)
                .any(|p| {
                    p.start_height_m != 0.
                        || p.end_height_m != 0.
                        || p.start_tangent_m != 0.
                        || p.end_tangent_m != 0.
                })
        {
            return None;
        }
    }

    let mut joins = [None, None];
    for link in &draft.alignment_connections {
        let a = lanes.iter().position(|id| *id == link.a.alignment_id);
        let b = lanes.iter().position(|id| *id == link.b.alignment_id);
        let pair = match (a, b) {
            (None, None) => continue,
            (Some(i), Some(j)) if i + 1 == j => (i, [link.a.endpoint, link.b.endpoint]),
            (Some(i), Some(j)) if j + 1 == i => (j, [link.b.endpoint, link.a.endpoint]),
            _ => return None, // other joins can change the rendered lane
        };
        if joins[pair.0].replace(pair.1).is_some() {
            return None;
        }
    }
    let [Some([a_exit, b_entry]), Some([b_exit, c_entry])] = joins else {
        return None;
    };
    if b_entry == b_exit
        || !straight_tram_join(lines[0], a_exit, lines[1], b_entry)
        || !straight_tram_join(lines[1], b_exit, lines[2], c_entry)
    {
        return None;
    }
    let position = |side| match side {
        AlignmentEndpoint::Start => 0.,
        AlignmentEndpoint::End => 1.,
    };
    let spans = [
        [start.position, position(a_exit)],
        [position(b_entry), position(b_exit)],
        [position(c_entry), end.position],
    ];
    for (i, (a, [from, to])) in lines.into_iter().zip(spans).enumerate() {
        let length = a.length_m();
        let piece = leg.sections[i].length_m;
        if (length * (to - from).abs() - piece).abs() > 1e-7 {
            return None;
        }
    }
    for (i, (a, [from, to])) in lines.into_iter().zip(spans).enumerate() {
        let piece = leg.sections[i].length_m;
        if remaining > piece && i < 2 {
            remaining -= piece;
            continue;
        }
        let offset = a.length_m() * (from + (to - from) * (remaining / piece).clamp(0., 1.));
        return polyline_chord(&a.points, offset);
    }
    None
}

/// Resolve distance by the same physical chord lengths used by Alignment.
fn polyline_chord(points: &[GeoPoint], mut offset: f64) -> Option<(GeoPoint, GeoPoint, f64)> {
    for pair in points.windows(2) {
        let length = alignment::distance(pair[0], pair[1]);
        if length > 0. && offset <= length + 1e-7 {
            return Some((pair[0], pair[1], (offset / length).clamp(0., 1.)));
        }
        offset -= length;
    }
    None
}

fn straight_tram_join(
    a: &Alignment,
    exit: AlignmentEndpoint,
    b: &Alignment,
    entry: AlignmentEndpoint,
) -> bool {
    let pair = |p: &[GeoPoint], side| match side {
        AlignmentEndpoint::Start => [p[0], p[1]],
        AlignmentEndpoint::End => [p[p.len() - 1], p[p.len() - 2]],
    };
    let [a, a_outer] = pair(&a.points, exit);
    let [b, b_outer] = pair(&b.points, entry);
    if a != b {
        return false;
    }
    let direction = |p: GeoPoint| {
        let x = (p.longitude - a.longitude) * a.latitude.to_radians().cos();
        let y = p.latitude - a.latitude;
        let norm = x.hypot(y);
        [x / norm, y / norm]
    };
    let [ax, ay] = direction(a_outer);
    let [bx, by] = direction(b_outer);
    ax * bx + ay * by <= -0.999_999
}

/// The compiler can split one physical track lane at passed (non-stop)
/// platforms. Only a same-cap run on exactly that lane admits one envelope.
fn same_lane_uniform_section(sections: &[Section]) -> Option<(u64, Section)> {
    if sections.len() > MAX_UNIFORM_PIECES {
        return None;
    }
    let first = sections.first()?;
    let [resource] = first.resources.as_slice() else {
        return None;
    };
    let (alignment, 0, 0) = inline_lane_key(resource)? else {
        return None;
    };
    let mut length_m = 0.0;
    for section in sections {
        if section.resources.as_slice() != first.resources.as_slice()
            || section.speed_limit_kmh != first.speed_limit_kmh
            || !section.length_m.is_finite()
            || section.length_m <= 0.0
        {
            return None;
        }
        length_m += section.length_m;
    }
    length_m.is_finite().then_some((
        alignment,
        Section {
            length_m,
            speed_limit_kmh: first.speed_limit_kmh,
            resources: Vec::new(),
        },
    ))
}

fn uniform_position(
    section: &Section,
    vehicle: &Vehicle,
    elapsed: u64,
    duration: u64,
) -> Option<f64> {
    // Invert the same single-section running_time_profile with endpoint
    // velocities zero. Use the committed elapsed time, not a UI lerp.
    let length = section.length_m;
    let a = vehicle.acceleration_mps2;
    let b = vehicle.braking_mps2;
    let cap = section.speed_limit_kmh.min(vehicle.max_speed_kmh) / 3.6;
    if ![length, a, b, cap]
        .into_iter()
        .all(|v| v.is_finite() && v > 0.0)
    {
        return None;
    }
    let peak = ((2.0 * length) / (1.0 / a + 1.0 / b)).sqrt().min(cap);
    let accelerate = peak / a;
    let brake = peak / b;
    let accelerate_m = peak * peak / (2.0 * a);
    let brake_m = peak * peak / (2.0 * b);
    let cruise = (length - accelerate_m - brake_m).max(0.0) / peak;
    let total = accelerate + brake + cruise;
    if !total.is_finite() || (total * 1_000.0).ceil() != duration as f64 || elapsed > duration {
        return None;
    }
    if elapsed == duration {
        return Some(1.0);
    }
    let seconds = elapsed as f64 / 1_000.0;
    let distance = if seconds <= accelerate {
        0.5 * a * seconds * seconds
    } else if seconds <= accelerate + cruise {
        accelerate_m + peak * (seconds - accelerate)
    } else {
        let remaining = (total - seconds).max(0.0);
        length - 0.5 * b * remaining * remaining
    };
    distance
        .is_finite()
        .then(|| (distance / length).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joined_tram_marker_never_uses_one_speed_when_caps_differ_or_repeats_a_lane() {
        let mut leg = RunLeg {
            from: "1".into(),
            to: "2".into(),
            sections: (11..14)
                .map(|line| Section {
                    length_m: 30.,
                    speed_limit_kmh: 50.,
                    resources: vec![format!("rail:{line}:section:0:lane:0")],
                })
                .collect(),
            dwell_ms: 0,
        };
        let vehicle = Vehicle {
            max_speed_kmh: 40.,
            acceleration_mps2: 1.,
            braking_mps2: 1.,
            length_m: 10.,
        };
        assert!(joined_tram_profile(&leg, &vehicle).is_some());
        leg.sections[1].speed_limit_kmh = 20.;
        assert!(joined_tram_profile(&leg, &vehicle).is_none());
        leg.sections[1].speed_limit_kmh = 50.;
        leg.sections[2].resources = vec!["rail:11:section:0:lane:0".into()];
        assert!(joined_tram_profile(&leg, &vehicle).is_none());
    }

    fn fixture(curved: bool, second: bool, pass_through: bool) -> World {
        let mut world = World::default();
        let player = world.add_player("test".into()).unwrap();
        let company = world.add_company(player, "rail".into()).unwrap();
        let point = |latitude, longitude| GeoPoint {
            latitude,
            longitude,
        };
        let mut geometry = vec![point(35.0, 139.0)];
        if curved {
            geometry.push(point(35.001, 139.003));
        }
        geometry.push(point(35.0, if pass_through { 139.010 } else { 139.006 }));
        let alignment = world
            .build_alignment(company, "line".into(), geometry)
            .unwrap();
        let from = world
            .place_station_on_alignment(company, "A".into(), alignment, 0.1, 80., 10.)
            .unwrap();
        let to = world
            .place_station_on_alignment(company, "B".into(), alignment, 0.9, 80., 10.)
            .unwrap();
        let passed = pass_through.then(|| {
            world
                .place_station_on_alignment(company, "Pass".into(), alignment, 0.2, 80., 10.)
                .unwrap()
        });
        let segments = vec![RouteSegment::Alignment {
            alignment_id: alignment,
            from,
            to,
        }];
        let stops = world
            .preview_service_path(from, segments.clone())
            .unwrap()
            .occurrences
            .into_iter()
            .filter(|visit| Some(visit.station_id) != passed)
            .map(|visit| visit.reference)
            .collect();
        let service_id = world
            .add_service_path(company, "service".into(), from, segments, stops)
            .unwrap();
        world
            .configure_operation(vec![OperationServiceConfig {
                service_id,
                vehicle: Vehicle {
                    max_speed_kmh: 36.0,
                    acceleration_mps2: 2.0,
                    braking_mps2: 1.0,
                    length_m: 20.0,
                },
                timetable: Timetable {
                    first_departure_ms: 0,
                    last_departure_ms: if second { 1000 } else { 0 },
                    headway_ms: 1000,
                },
                dwell_ms: 5_000,
            }])
            .unwrap();
        world
    }

    #[test]
    fn real_track_positions_use_committed_time_and_hide_waiting_trains() {
        let mut world = fixture(false, true, false);
        let before = world.operation_save();
        assert!(world.live_straight_rail_markers().is_empty());
        assert_eq!(before, world.operation_save());
        world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(5_000, 16)
            .unwrap();
        assert_eq!(world.operation().unwrap().runtime.trains().count(), 2);
        let captured = world.operation_save();
        let markers = world.live_straight_rail_markers();
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].key.scheduled_ms, 0);
        // 10 m/s at 5s from rest; acceleration covers exactly 25m.
        let alignment = &world.snapshot().alignments[0];
        let expected = 0.1 + 25.0 / alignment.length_m();
        assert!((markers[0].position - expected).abs() < 1e-10);
        assert_eq!(markers[0].alignment_start, alignment.points[0]);
        assert_eq!(markers[0].alignment_end, alignment.points[1]);
        assert_eq!(captured, world.operation_save());
        world.operation_mut().unwrap().invalidated = true;
        assert!(world.live_straight_rail_markers().is_empty());
    }

    #[test]
    fn offscreen_trains_do_not_spend_the_visible_marker_budget() {
        let mut world = World::default();
        let player = world.add_player("player".into()).unwrap();
        let company = world.add_company(player, "rail".into()).unwrap();
        let mut configs = Vec::new();
        let mut latitudes = BTreeMap::new();
        for i in 0..=MAX_MARKERS {
            let longitude = 139.;
            let latitude = 35. + i as f64 * 0.001;
            let alignment = world
                .build_alignment(
                    company,
                    format!("Rail {i}"),
                    vec![
                        GeoPoint {
                            latitude,
                            longitude,
                        },
                        GeoPoint {
                            latitude,
                            longitude: longitude + 0.006,
                        },
                    ],
                )
                .unwrap();
            let from = world
                .place_station_on_alignment(company, format!("A {i}"), alignment, 0.1, 80., 10.)
                .unwrap();
            let to = world
                .place_station_on_alignment(company, format!("B {i}"), alignment, 0.9, 80., 10.)
                .unwrap();
            let segments = vec![RouteSegment::Alignment {
                alignment_id: alignment,
                from,
                to,
            }];
            let stops = world
                .preview_service_path(from, segments.clone())
                .unwrap()
                .occurrences
                .into_iter()
                .map(|visit| visit.reference)
                .collect();
            let service_id = world
                .add_service_path(company, format!("Service {i}"), from, segments, stops)
                .unwrap();
            latitudes.insert(service_id.0.to_string(), latitude);
            configs.push(OperationServiceConfig {
                service_id,
                vehicle: Vehicle {
                    max_speed_kmh: 36.,
                    acceleration_mps2: 2.,
                    braking_mps2: 1.,
                    length_m: 20.,
                },
                timetable: Timetable {
                    first_departure_ms: 0,
                    last_departure_ms: 0,
                    headway_ms: 1000,
                },
                dwell_ms: 5000,
            });
        }
        world.configure_operation(configs).unwrap();
        world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(5000, 4096)
            .unwrap();
        let global = world.live_ground_rail_markers();
        assert_eq!(global.len(), MAX_MARKERS);
        let (wanted, latitude) = latitudes
            .iter()
            .find(|(key, _)| !global.iter().any(|marker| &marker.key.service_id == *key))
            .unwrap();
        let mercator =
            |lat: f64| (1.0 - lat.to_radians().tan().asinh() / std::f64::consts::PI) / 2.0;
        let scope = VehicleViewport {
            left: (138.99 + 180.) / 360.,
            right: (139.01 + 180.) / 360.,
            top: mercator(latitude + 0.0004),
            bottom: mercator(latitude - 0.0004),
        };
        assert!(!global.iter().any(|m| scope.contains_chord(
            m.alignment_start,
            m.alignment_end,
            m.position
        )));
        let before = world.operation_save();
        let scoped = world.live_rail_markers_in(Some(scope));
        assert_eq!(scoped.len(), 1);
        assert_eq!(&scoped[0].key.service_id, wanted);
        assert_eq!(before, world.operation_save());
    }

    #[test]
    fn reverse_run_uses_same_lane_in_reverse_and_legacy_view_is_straight_only() {
        let mut world = fixture(false, false, false);
        let op = world.operation_mut().unwrap();
        let spec = op.runtime.save().services.remove(0);
        assert_eq!(spec.legs.len(), 2);
        let (forward, stand) = op
            .runtime
            .unblocked_leg_timing(&spec.service_id, 0)
            .unwrap();
        op.runtime.advance_to(forward + stand + 5_000, 32).unwrap();
        let markers = world.live_straight_rail_markers();
        assert_eq!(markers.len(), 1);
        let alignment = &world.snapshot().alignments[0];
        let expected = 0.9 - 25.0 / alignment.length_m();
        assert!((markers[0].position - expected).abs() < 1e-10);
        assert_eq!(markers[0].alignment_start, alignment.points[0]);
        assert_eq!(markers[0].alignment_end, alignment.points[1]);
        assert_eq!(markers[0].key.scheduled_ms, 0);

        let mut curved = fixture(true, false, false);
        let op = curved.operation_mut().unwrap();
        op.runtime.advance_to(5_000, 16).unwrap();
        assert_eq!(op.runtime.trains().count(), 1);
        assert!(curved.live_straight_rail_markers().is_empty());
    }

    #[test]
    fn verified_isolated_curve_marker_follows_each_chord_in_both_directions() {
        let mut world = fixture(true, false, false);
        let alignment = &world.snapshot().alignments[0];
        let points = alignment.points.clone();
        let length = alignment.length_m();
        let first = alignment::distance(points[0], points[1]);
        let op = world.operation_mut().unwrap();
        let spec = op.runtime.save().services.remove(0);
        let (forward, stand) = op
            .runtime
            .unblocked_leg_timing(&spec.service_id, 0)
            .unwrap();
        op.runtime.advance_to(5000, 16).unwrap();

        let marker = world.live_ground_rail_markers().pop().unwrap();
        assert_eq!(
            [marker.alignment_start, marker.alignment_end],
            [points[0], points[1]]
        );
        assert!((marker.position - (length * 0.1 + 25.) / first).abs() < 1e-10);

        let op = world.operation_mut().unwrap();
        op.runtime.advance_to(forward + stand + 5000, 32).unwrap();
        let marker = world.live_ground_rail_markers().pop().unwrap();
        assert_eq!(
            [marker.alignment_start, marker.alignment_end],
            [points[1], points[2]]
        );
        assert!((marker.position - (length * 0.9 - 25. - first) / (length - first)).abs() < 1e-10);
    }

    #[test]
    fn only_one_lane_is_not_enough_when_renderer_may_trim_joined_endpoints() {
        let mut world = fixture(false, false, false);
        let first = &world.snapshot().alignments[0];
        let first_id = first.id;
        let owner = first.owner;
        let endpoint = first.points[1];
        let next = world
            .build_alignment(
                owner,
                "next".into(),
                vec![
                    endpoint,
                    GeoPoint {
                        longitude: endpoint.longitude + 0.01,
                        ..endpoint
                    },
                ],
            )
            .unwrap();
        world
            .connect_alignment_endpoints(
                owner,
                AlignmentEndpointRef {
                    alignment_id: first_id,
                    endpoint: AlignmentEndpoint::End,
                },
                AlignmentEndpointRef {
                    alignment_id: next,
                    endpoint: AlignmentEndpoint::Start,
                },
            )
            .unwrap();
        let op = world.operation_mut().unwrap();
        let spec = op.runtime.save().services.remove(0);
        assert!(same_lane_uniform_section(&spec.legs[0].sections).is_some());
        op.runtime.advance_to(5_000, 16).unwrap();
        assert!(op.runtime.trains().next().is_some());
        assert!(world.live_straight_rail_markers().is_empty());
    }

    #[test]
    fn same_lane_passed_platform_does_not_hide_moving_train_or_reset_speed() {
        let mut world = fixture(false, false, true);
        let op = world.operation_mut().unwrap();
        let spec = op.runtime.save().services.remove(0);
        let sections = &spec.legs[0].sections;
        assert_eq!(
            sections.len(),
            2,
            "actual compiler splits at passed station"
        );
        assert_eq!(sections[0].resources, sections[1].resources);
        assert_eq!(sections[0].speed_limit_kmh, sections[1].speed_limit_kmh);
        op.runtime.advance_to(12_000, 32).unwrap();
        let markers = world.live_straight_rail_markers();
        assert_eq!(markers.len(), 1);
        let alignment = &world.snapshot().alignments[0];
        // 25 m while accelerating, then 70 m cruising. This has already
        // passed the 0.2 platform on this line without making a stop.
        let expected_position = 0.1 + 95.0 / alignment.length_m();
        assert!(expected_position > 0.2);
        assert!((markers[0].position - expected_position).abs() < 1e-10);

        let mut ambiguous = sections.to_vec();
        ambiguous[1].speed_limit_kmh -= 1.0;
        assert!(same_lane_uniform_section(&ambiguous).is_none());
        ambiguous[1].speed_limit_kmh += 1.0;
        ambiguous[1].resources.push("turnout".into());
        assert!(same_lane_uniform_section(&ambiguous).is_none());
    }

    #[test]
    fn kinematics_acceleration_cruise_braking_and_arrival_not_linear() {
        let vehicle = Vehicle {
            max_speed_kmh: 36.0,
            acceleration_mps2: 2.,
            braking_mps2: 1.,
            length_m: 20.,
        };
        let section = Section {
            length_m: 200.,
            speed_limit_kmh: 72.,
            resources: Vec::new(),
        };
        let duration =
            crate::operation::running_time_ms(std::slice::from_ref(&section), &vehicle).unwrap();
        assert_eq!(duration, 27_500);
        for (elapsed, distance) in [
            (0, 0.),
            (5_000, 25.),
            (10_000, 75.),
            (22_500, 187.5),
            (27_500, 200.),
        ] {
            assert!(
                (uniform_position(&section, &vehicle, elapsed, duration).unwrap() * 200.
                    - distance)
                    .abs()
                    < 1e-9
            );
        }
        assert!(uniform_position(&section, &vehicle, duration + 1, duration).is_none());
        assert!(uniform_position(&section, &vehicle, 5_000, duration + 1).is_none());
    }
}
