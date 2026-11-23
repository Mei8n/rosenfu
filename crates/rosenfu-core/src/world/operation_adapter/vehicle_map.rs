//! Verified moving vehicle positions at the committed simulation time.
//! Unsupported geometry is omitted, not guessed.
use super::*;
use crate::operation::{TrainKey, TrainState};
use serde::Serialize;

/// A position for matching the operation snapshot, not a public numeric label
/// or a determination of where the whole consist lies.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LiveRailMarker {
    pub key: TrainKey,
    /// Whole painted alignment chord, not endpoints at the stations.
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
        let Some(op) = self
            .operation()
            .filter(|o| !o.invalidated && !o.bus_configs.is_empty())
        else {
            return Vec::new();
        };
        let roads = &self.data.road_transport;
        let now = op.runtime.now_ms();
        op.runtime
            .trains()
            .take(MAX_TRAINS)
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
                let (edge, position) = if let [resource] = section.resources.as_slice() {
                    let (edge, from, to) = roads.gameplay_bus_leg_chord(
                        route_id,
                        train.leg,
                        leg.from.strip_prefix("bus:")?,
                        leg.to.strip_prefix("bus:")?,
                        resource.strip_prefix("road:edge:")?,
                    )?;
                    if !(0.0..=1.0).contains(&from) || !(0.0..=1.0).contains(&to) || from >= to {
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
                Some(LiveBusMarker {
                    key: train.key.clone(),
                    edge_start: *edge_start,
                    edge_end: *edge_end,
                    position,
                })
            })
            .take(MAX_MARKERS)
            .collect()
    }

    /// Bounded render subset of moving trains on a verified direct, ground,
    /// isolated single-track Normal rail section. Waiting, released and unsupported
    /// movements are omitted. No clock or save state is changed.
    pub fn live_straight_rail_markers(&self) -> Vec<LiveRailMarker> {
        let Some(op) = self.operation().filter(|o| !o.invalidated) else {
            return Vec::new();
        };
        let now = op.runtime.now_ms();
        let candidates: Vec<_> = op
            .runtime
            .trains()
            .take(MAX_TRAINS)
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
                    || a.points.len() != 2
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
                let expected = (from.position - to.position).abs() * a.length_m();
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
                Some(LiveRailMarker {
                    key: c.key.clone(),
                    alignment_start: a.points[0],
                    alignment_end: a.points[1],
                    position,
                })
            })
            .take(MAX_MARKERS)
            .collect()
    }
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
    fn reverse_run_uses_same_lane_in_reverse_and_unproved_curve_is_absent() {
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
