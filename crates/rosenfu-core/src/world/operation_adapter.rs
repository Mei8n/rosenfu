//! Authoritative physical service path adapter. Never uses screen coordinates.
use super::*;
use crate::operation::{
    fixed_refuge_resources, inline_lane_key, inline_platform_handoff, OperationError,
    OperationRuntime, RailAdmissionModel, RunLeg, Section, ServiceRunSpec, Timetable, Vehicle,
};
use std::collections::{BTreeMap, BTreeSet};

type RailCrossings = BTreeMap<AlignmentId, Vec<(f64, String)>>;
const MAX_CROSSING_PAIR_CHECKS: usize = 32_768;
const MAX_CROSSING_REFERENCES: usize = 16_384;
const LANE_JOIN_PREFIX: &str = "rail:lane-join:";

#[derive(Clone, Copy)]
struct RailReservations<'a> {
    crossings: &'a RailCrossings,
    parallel_joins: bool,
}

fn has_parallel_join_locks(spec: &ServiceRunSpec) -> bool {
    spec.legs
        .iter()
        .flat_map(|l| &l.sections)
        .flat_map(|s| &s.resources)
        .any(|id| id.starts_with(LANE_JOIN_PREFIX))
}

/// Only exact two-point, equal-height, opposed straight tangents. Merely
/// having equal track counts does not prove independent lanes.
fn straight_parallel_join(
    a: &Alignment,
    exit: AlignmentEndpoint,
    b: &Alignment,
    entry: AlignmentEndpoint,
) -> Option<u8> {
    if a.id == b.id
        || a.track_type != TrackType::Normal
        || b.track_type != TrackType::Normal
        || a.section_count() != 1
        || b.section_count() != 1
        || a.vertical_profile.is_some()
        || b.vertical_profile.is_some()
    {
        return None;
    }
    let aa = a.section_infrastructure(0)?;
    let bb = b.section_infrastructure(0)?;
    if aa.track_count != 2 || aa != bb {
        return None;
    }
    let points = |a: &Alignment, endpoint| {
        let [start, end] = <[GeoPoint; 2]>::try_from(a.points.as_slice()).ok()?;
        Some(if endpoint == AlignmentEndpoint::Start {
            (start, end)
        } else {
            (end, start)
        })
    };
    let (at, ai) = points(a, exit)?;
    let (bt, bi) = points(b, entry)?;
    if at != bt {
        return None;
    }
    let cos = at.latitude.to_radians().cos();
    let u = (
        (ai.longitude - at.longitude) * cos,
        ai.latitude - at.latitude,
    );
    let v = (
        (bi.longitude - bt.longitude) * cos,
        bi.latitude - bt.latitude,
    );
    let norm = u.0.hypot(u.1) * v.0.hypot(v.1);
    let dot = u.0 * v.0 + u.1 * v.1;
    let cross = u.0 * v.1 - u.1 * v.0;
    (norm > 1e-12 && dot < 0.0 && cross.abs() <= norm * 1e-9).then_some(aa.track_count)
}

fn lane_join_resource(
    exit: AlignmentEndpointRef,
    exit_lane: u8,
    entry: AlignmentEndpointRef,
    entry_lane: u8,
) -> String {
    let key = |e: AlignmentEndpointRef, lane| {
        (e.alignment_id.0, e.endpoint == AlignmentEndpoint::End, lane)
    };
    let mut endpoints = [key(exit, exit_lane), key(entry, entry_lane)];
    endpoints.sort_unstable();
    let [(ai, ae, al), (bi, be, bl)] = endpoints;
    format!(
        "{LANE_JOIN_PREFIX}{ai}:{}:{al}:{bi}:{}:{bl}",
        u8::from(ae),
        u8::from(be)
    )
}

fn crossing(a: &Alignment, b: &Alignment) -> Option<(f64, f64)> {
    let [p, q] = [a.points[0], a.points[1]];
    let [r, s] = [b.points[0], b.points[1]];
    let u = (q.longitude - p.longitude, q.latitude - p.latitude);
    let v = (s.longitude - r.longitude, s.latitude - r.latitude);
    let cross = |(x, y): (f64, f64), (a, b): (f64, f64)| x * b - y * a;
    let denominator = cross(u, v);
    if denominator.abs() <= 1e-4 * u.0.hypot(u.1) * v.0.hypot(v.1) {
        return None;
    }
    let delta = (r.longitude - p.longitude, r.latitude - p.latitude);
    let (ta, tb) = (cross(delta, v) / denominator, cross(delta, u) / denominator);
    if !(1e-8..1.0 - 1e-8).contains(&ta) || !(1e-8..1.0 - 1e-8).contains(&tb) {
        return None;
    }
    let aa = a.height_at_position_m(ta)?;
    let bb = b.height_at_position_m(tb)?;
    ((aa - bb).abs() <= 0.01).then_some((ta, tb))
}
type CrossingBounds<'a> = (f64, f64, f64, f64, &'a Alignment);
fn bounded_flat_crossings(
    mut lines: Vec<CrossingBounds<'_>>,
    limit: usize,
) -> Result<(RailCrossings, usize), OperationError> {
    // Choose the sparser bounding-box axis in O(n log n). A large
    // parallel yard does not consume a quadratic pair budget.
    fn overlap_cost(lines: &[CrossingBounds<'_>], y: bool) -> usize {
        let mut ranges: Vec<_> = lines
            .iter()
            .map(|a| if y { (a.2, a.3) } else { (a.0, a.1) })
            .collect();
        ranges.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        let mut ends: Vec<_> = ranges.iter().map(|p| p.1).collect();
        ends.sort_unstable_by(f64::total_cmp);
        let (mut finished, mut count) = (0, 0usize);
        for (i, (start, _)) in ranges.into_iter().enumerate() {
            while finished < ends.len() && ends[finished] < start {
                finished += 1;
            }
            count = count.saturating_add(i - finished);
        }
        count
    }
    if overlap_cost(&lines, true) < overlap_cost(&lines, false) {
        for b in &mut lines {
            *b = (b.2, b.3, b.0, b.1, b.4);
        }
    }
    lines.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.4.id.cmp(&b.4.id)));
    let mut result = RailCrossings::new();
    let mut checks = 0;
    for (i, &(_, x_max, y_min, y_max, a)) in lines.iter().enumerate() {
        for &(x_min, _, other_min, other_max, b) in &lines[i + 1..] {
            if x_min > x_max {
                break;
            }
            checks += 1;
            if checks > limit {
                return Err(OperationError::CrossingBudget);
            }
            if other_min > y_max || other_max < y_min {
                continue;
            }
            let (lo, hi) = if a.id < b.id { (a, b) } else { (b, a) };
            if let Some((ta, tb)) = crossing(lo, hi) {
                let key = format!("rail:crossing:{}:{}", lo.id.0, hi.id.0);
                result.entry(lo.id).or_default().push((ta, key.clone()));
                result.entry(hi.id).or_default().push((tb, key));
            }
        }
    }
    // A different sweep axis or unconnected candidate must not change
    // resource order in an otherwise identical operation plan.
    for row in result.values_mut() {
        row.sort_unstable_by(|a, b| a.1.cmp(&b.1));
    }
    Ok((result, checks))
}
fn crossing_lock_count(spec: &ServiceRunSpec) -> usize {
    spec.legs
        .iter()
        .flat_map(|l| &l.sections)
        .flat_map(|s| &s.resources)
        .filter(|r| r.starts_with("rail:crossing:"))
        .count()
}

fn has_crossing_locks(spec: &ServiceRunSpec) -> bool {
    crossing_lock_count(spec) != 0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationServiceConfig {
    pub service_id: ServiceId,
    pub timetable: Timetable,
    pub vehicle: Vehicle,
    pub dwell_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BusOperationConfig {
    pub route_id: String,
    pub timetable: Timetable,
    pub vehicle: Vehicle,
    /// Explicit game assumption, not an observed legal road speed.
    pub assumed_speed_kmh: f64,
    pub dwell_ms: u64,
}
#[derive(Clone)]
struct Span {
    id: AlignmentId,
    from: f64,
    to: f64,
    cap: f64,
    extra: Vec<String>,
}
impl World {
    /// Narrow first slice: independent, local, two-point flat single-track
    /// alignments used by directly configured rail services. No route join is
    /// inferred. Non-eligible geometry is deliberately not claimed protected.
    fn active_flat_crossings(
        &self,
        configs: &[OperationServiceConfig],
        limit: usize,
    ) -> Result<RailCrossings, OperationError> {
        if configs.is_empty() {
            return Ok(RailCrossings::new());
        }
        let services: BTreeSet<_> = configs.iter().map(|c| c.service_id).collect();
        let active: BTreeSet<_> = self
            .data
            .service_paths
            .iter()
            .filter(|p| services.contains(&p.service_id))
            .flat_map(|p| &p.legs)
            .filter_map(|l| match &l.segment {
                RouteSegment::Alignment { alignment_id, .. } => Some(*alignment_id),
                _ => None,
            })
            .collect();
        if active.len() < 2 {
            return Ok(RailCrossings::new());
        }
        let connected: BTreeSet<_> = self
            .data
            .alignment_connections
            .iter()
            .flat_map(|c| [c.a.alignment_id, c.b.alignment_id])
            .collect();
        let lines: Vec<_> = self
            .data
            .alignments
            .iter()
            .filter(|a| {
                active.contains(&a.id)
                    && !connected.contains(&a.id)
                    && a.track_type == TrackType::Normal
                    && a.points.len() == 2
                    && a.section_count() == 1
                    && a.vertical_profile.is_none()
                    && a.section_profiles.is_empty()
                    && a.section_infrastructure(0)
                        .is_some_and(|i| i.track_count == 1)
                    && (a.points[0].longitude - a.points[1].longitude).abs() <= 0.25
                    && (a.points[0].latitude - a.points[1].latitude).abs() <= 0.25
            })
            .map(|a| {
                let (p, q) = (a.points[0], a.points[1]);
                (
                    p.longitude.min(q.longitude),
                    p.longitude.max(q.longitude),
                    p.latitude.min(q.latitude),
                    p.latitude.max(q.latitude),
                    a,
                )
            })
            .collect();
        let (crossings, _) = bounded_flat_crossings(lines, limit)?;
        if crossings.is_empty() {
            return Ok(crossings);
        }
        // Whole-leg rear clearance assumes a vehicle can clear the crossing
        // before stopping. Refuse a new configuration at a served platform
        // too close to a crossing; do not silently offer false protection.
        let served_starts: BTreeSet<_> = self
            .data
            .services
            .iter()
            .filter(|s| services.contains(&s.id))
            .map(|s| s.start)
            .collect();
        let served_in_legs: BTreeSet<_> = self
            .data
            .service_paths
            .iter()
            .filter(|s| services.contains(&s.service_id))
            .flat_map(|s| s.stops.iter())
            .filter_map(|r| match *r {
                StationOccurrenceRef::OnLeg { station_id, .. } => Some(station_id),
                StationOccurrenceRef::Start => None,
            })
            .collect();
        let vehicle_margin = configs
            .iter()
            .map(|c| c.vehicle.length_m)
            .filter(|length| length.is_finite())
            .fold(0.0, f64::max)
            + 2.0;
        let locations: BTreeMap<_, _> = self
            .data
            .alignments
            .iter()
            .filter_map(|a| {
                let mut positions: Vec<_> = crossings.get(&a.id)?.iter().map(|x| x.0).collect();
                positions.sort_unstable_by(f64::total_cmp);
                Some((a.id, (a.length_m(), positions)))
            })
            .collect();
        for platform in &self.data.station_placements {
            if !served_starts.contains(&platform.station_id)
                && !served_in_legs.contains(&platform.station_id)
            {
                continue;
            }
            let Some((length, positions)) = locations.get(&platform.alignment_id) else {
                continue;
            };
            let margin = (platform.length_m / 2.0 + vehicle_margin) / length;
            let first = positions.partition_point(|&p| p < platform.position - margin);
            if positions
                .get(first)
                .is_some_and(|&p| p <= platform.position + margin)
            {
                return Err(OperationError::CrossingPlatform);
            }
        }
        Ok(crossings)
    }

    /// Centre-line distances with conservative turnout caps. One directional
    /// block per anchor section (quad track currently uses one lane/direction).
    pub fn compile_operation(
        &self,
        config: &OperationServiceConfig,
    ) -> Result<ServiceRunSpec, OperationError> {
        self.compile_operation_mode(config, true, &RailCrossings::new())
    }

    fn compile_operation_mode(
        &self,
        config: &OperationServiceConfig,
        turnback: bool,
        crossings: &RailCrossings,
    ) -> Result<ServiceRunSpec, OperationError> {
        self.compile_operation_reservations(
            config,
            turnback,
            RailReservations {
                crossings,
                parallel_joins: false,
            },
        )
    }

    fn compile_operation_reservations(
        &self,
        config: &OperationServiceConfig,
        turnback: bool,
        rules: RailReservations<'_>,
    ) -> Result<ServiceRunSpec, OperationError> {
        let invalid = || OperationError::InvalidPath;
        let path = self
            .data
            .service_paths
            .iter()
            .find(|p| p.service_id == config.service_id)
            .ok_or_else(invalid)?;
        let occurrences = self
            .service_path_occurrences(config.service_id)
            .map_err(|_| invalid())?;
        let mut runs = self.compile_rail_direction(
            &path.legs,
            &path.stops,
            &occurrences,
            config.dwell_ms,
            false,
            rules,
        )?;
        if let Some(return_legs) = turnback
            .then(|| {
                self.compile_return_direction(path, &occurrences, config.dwell_ms, &runs, rules)
                    .ok()
            })
            .flatten()
        {
            runs.extend(return_legs);
        }
        Ok(ServiceRunSpec {
            service_id: config.service_id.0.to_string(),
            vehicle: config.vehicle.clone(),
            timetable: config.timetable.clone(),
            legs: runs,
        })
    }

    /// Convert authoritative geometry in this direction. Never reverse already
    /// compiled sections: lane selection, caps and turnout incidence are directional.
    fn compile_rail_direction(
        &self,
        legs: &[ServiceLeg],
        stops: &[StationOccurrenceRef],
        occurrences: &[ServicePathOccurrence],
        dwell_ms: u64,
        check_lane_incidence: bool,
        rules: RailReservations<'_>,
    ) -> Result<Vec<RunLeg>, OperationError> {
        let invalid = || OperationError::InvalidPath;
        let movements = self.track_movement_geometries().map_err(|_| invalid())?;
        let incidence = self.alignment_lane_movements();
        let internal = self
            .internal_track_movement_geometries()
            .map_err(|_| invalid())?;
        let placement = |station: StationId| {
            self.data
                .station_placements
                .iter()
                .find(|p| p.station_id == station)
                .ok_or_else(invalid)
        };
        let alignment = |id: AlignmentId| {
            self.data
                .alignments
                .iter()
                .find(|a| a.id == id)
                .ok_or_else(invalid)
        };
        let mut runs = Vec::new();
        let mut pending = Vec::new();
        let mut last_stop = occurrences.first().ok_or_else(invalid)?.station_id;
        for leg in legs {
            let (from, to, connections) = match &leg.segment {
                RouteSegment::Alignment { from, to, .. } => (*from, *to, &[][..]),
                RouteSegment::ConnectedAlignments {
                    from,
                    to,
                    connections,
                } => (*from, *to, connections.as_slice()),
            };
            let start = placement(from)?;
            let end = placement(to)?;
            let mut id = start.alignment_id;
            let mut position = start.position;
            let mut spans = Vec::new();
            let endpoint_position = |e: AlignmentEndpoint| {
                if e == AlignmentEndpoint::Start {
                    0.0
                } else {
                    1.0
                }
            };
            for link in connections {
                let (exit, entry) = if link.a.alignment_id == id {
                    (link.a, link.b)
                } else if link.b.alignment_id == id {
                    (link.b, link.a)
                } else {
                    return Err(invalid());
                };
                let port_lane =
                    |end: AlignmentEndpointRef, forward| -> Result<u8, OperationError> {
                        let a = alignment(end.alignment_id)?;
                        let section = if end.endpoint == AlignmentEndpoint::Start {
                            0
                        } else {
                            a.section_count() - 1
                        };
                        let tracks = a
                            .section_infrastructure(section)
                            .ok_or_else(invalid)?
                            .track_count;
                        Ok(directional_lane(tracks, forward))
                    };
                let exit_lane = port_lane(exit, exit.endpoint == AlignmentEndpoint::End)?;
                let entry_lane = port_lane(entry, entry.endpoint == AlignmentEndpoint::Start)?;
                let connects =
                    |m: &&TrackMovement| m.from.endpoint == exit && m.to.endpoint == entry;
                let connected: Vec<_> = incidence.iter().filter(connects).collect();
                if connected.is_empty()
                    || (check_lane_incidence
                        && !connected.iter().any(|m| {
                            m.from.lane_index == exit_lane && m.to.lane_index == entry_lane
                        }))
                {
                    return Err(invalid());
                }
                let matching: Vec<_> = movements
                    .iter()
                    .filter(|m| m.movement.from.endpoint == exit && m.movement.to.endpoint == entry)
                    .collect();
                // A straight, equal-count join has real lane incidence but no
                // curved turnout geometry. Genuine turnouts still require it.
                if matching.is_empty() && connected.iter().any(|m| m.point.is_some()) {
                    return Err(invalid());
                }
                let cap = matching
                    .iter()
                    .map(|m| m.properties.speed_limit_kmh)
                    .fold(alignment(id)?.effective_design_speed_kmh(), f64::min);
                // Genuine turnouts and unqualified geometry retain the shared
                // endpoint lock. Widen only an exact direct lane correspondence.
                let verified_lanes = rules.parallel_joins
                    && matching.is_empty()
                    && connected.iter().all(|m| m.point.is_none())
                    && straight_parallel_join(
                        alignment(exit.alignment_id)?,
                        exit.endpoint,
                        alignment(entry.alignment_id)?,
                        entry.endpoint,
                    )
                    .is_some_and(|width| {
                        connected.len() == usize::from(width)
                            && (0..width).all(|lane| {
                                connected
                                    .iter()
                                    .filter(|m| m.from.lane_index == lane)
                                    .count()
                                    == 1
                                    && connected.iter().filter(|m| m.to.lane_index == lane).count()
                                        == 1
                            })
                    })
                    && connected
                        .iter()
                        .any(|m| m.from.lane_index == exit_lane && m.to.lane_index == entry_lane);
                let extra = if verified_lanes {
                    vec![lane_join_resource(exit, exit_lane, entry, entry_lane)]
                } else {
                    let resource = |e: AlignmentEndpointRef| {
                        format!("rail:join:{}:{:?}", e.alignment_id.0, e.endpoint)
                    };
                    vec![resource(exit), resource(entry)]
                };
                spans.push(Span {
                    id,
                    from: position,
                    to: endpoint_position(exit.endpoint),
                    cap,
                    extra: extra.clone(),
                });
                id = entry.alignment_id;
                position = endpoint_position(entry.endpoint);
            }
            if id != end.alignment_id {
                return Err(invalid());
            }
            spans.push(Span {
                id,
                from: position,
                to: end.position,
                cap: alignment(id)?.effective_design_speed_kmh(),
                extra: Vec::new(),
            });
            let leg_occurrences: Vec<_> = occurrences.iter().filter(|o| matches!(o.reference, StationOccurrenceRef::OnLeg {leg_key,..} if leg_key == leg.key)).collect();
            let mut next_occurrence = 0;
            for span in spans {
                let a = alignment(span.id)?;
                let forward = span.to > span.from;
                let mut cursor = span.from;
                while next_occurrence < leg_occurrences.len() {
                    let occurrence = leg_occurrences[next_occurrence];
                    let p = placement(occurrence.station_id)?;
                    if p.alignment_id != span.id
                        || p.position < span.from.min(span.to)
                        || p.position > span.from.max(span.to)
                    {
                        break;
                    }
                    append_sections(
                        a,
                        cursor,
                        p.position,
                        span.cap,
                        &span.extra,
                        &internal,
                        rules
                            .crossings
                            .get(&a.id)
                            .map(Vec::as_slice)
                            .unwrap_or_default(),
                        &mut pending,
                    )?;
                    cursor = p.position;
                    if stops.contains(&occurrence.reference) {
                        if pending.is_empty() {
                            return Err(invalid());
                        }
                        runs.push(RunLeg {
                            from: last_stop.0.to_string(),
                            to: p.station_id.0.to_string(),
                            sections: std::mem::take(&mut pending),
                            dwell_ms,
                        });
                        last_stop = p.station_id;
                    }
                    next_occurrence += 1;
                }
                if (forward && cursor < span.to) || (!forward && cursor > span.to) {
                    append_sections(
                        a,
                        cursor,
                        span.to,
                        span.cap,
                        &span.extra,
                        &internal,
                        rules
                            .crossings
                            .get(&a.id)
                            .map(Vec::as_slice)
                            .unwrap_or_default(),
                        &mut pending,
                    )?;
                }
            }
            if next_occurrence != leg_occurrences.len() {
                return Err(invalid());
            }
        }
        if !pending.is_empty() || runs.is_empty() {
            return Err(invalid());
        }
        Ok(runs)
    }

    fn compile_return_direction(
        &self,
        path: &ServicePath,
        occurrences: &[ServicePathOccurrence],
        dwell_ms: u64,
        outbound: &[RunLeg],
        rules: RailReservations<'_>,
    ) -> Result<Vec<RunLeg>, OperationError> {
        let invalid = || OperationError::InvalidPath;
        let start = occurrences.first().ok_or_else(invalid)?.station_id;
        let terminal = occurrences.last().ok_or_else(invalid)?.station_id;
        // An explicit already-closed service has no inferred extra run.
        if start == terminal {
            return Err(invalid());
        }
        let segments = path
            .legs
            .iter()
            .rev()
            .map(|leg| match &leg.segment {
                RouteSegment::Alignment {
                    alignment_id,
                    from,
                    to,
                } => RouteSegment::Alignment {
                    alignment_id: *alignment_id,
                    from: *to,
                    to: *from,
                },
                RouteSegment::ConnectedAlignments {
                    from,
                    to,
                    connections,
                } => RouteSegment::ConnectedAlignments {
                    from: *to,
                    to: *from,
                    connections: connections
                        .iter()
                        .rev()
                        .map(|link| AlignmentConnection {
                            a: link.b,
                            b: link.a,
                        })
                        .collect(),
                },
            })
            .collect();
        let reverse = self
            .preview_service_path(terminal, segments)
            .map_err(|_| invalid())?;
        if reverse.occurrences.len() != occurrences.len() {
            return Err(invalid());
        }

        // Match the complete occurrence order; a repeated station ID by itself
        // cannot tell us whether the train stopped at this particular visit.
        let mut stops = Vec::new();
        for (after, before) in reverse.occurrences.iter().zip(occurrences.iter().rev()) {
            if after.station_id != before.station_id {
                return Err(invalid());
            }
            if path.stops.contains(&before.reference) {
                stops.push(after.reference.clone());
            }
        }
        // The outbound compiler historically tolerated selecting lanes that do
        // not meet at these joins. Don't infer a return for such a route.
        self.compile_rail_direction(&path.legs, &path.stops, occurrences, dwell_ms, true, rules)?;
        let returns = self.compile_rail_direction(
            &reverse.legs,
            &stops,
            &reverse.occurrences,
            dwell_ms,
            true,
            rules,
        )?;
        let incoming = outbound
            .last()
            .and_then(|l| l.sections.last())
            .ok_or_else(invalid)?;
        let departing = returns
            .first()
            .and_then(|l| l.sections.first())
            .ok_or_else(invalid)?;
        // No cross-track switch may be inferred from a shared station marker.
        // Our directional selectors coincide on a physical lane only for a
        // single-track section; crossover routing is not modeled here.
        if lane_resource(incoming).is_none() || lane_resource(incoming) != lane_resource(departing)
        {
            return Err(invalid());
        }
        Ok(returns)
    }

    fn operation_spec_matches(
        &self,
        config: &OperationServiceConfig,
        saved: &ServiceRunSpec,
    ) -> bool {
        self.compile_operation(config)
            .is_ok_and(|current| current == *saved)
            || self
                .compile_operation_mode(config, false, &RailCrossings::new())
                .is_ok_and(|outbound| outbound == *saved)
    }
}

fn lane_resource(section: &Section) -> Option<&str> {
    section
        .resources
        .iter()
        .map(String::as_str)
        .find(|id| id.starts_with("rail:") && id.contains(":section:") && id.contains(":lane:"))
}

fn directional_lane(tracks: u8, forward: bool) -> u8 {
    if forward {
        0
    } else {
        tracks - 1
    }
}

#[allow(clippy::too_many_arguments)]
fn append_sections(
    a: &Alignment,
    from: f64,
    to: f64,
    cap: f64,
    extra: &[String],
    internal: &[InternalTrackMovementGeometry],
    crossings: &[(f64, String)],
    out: &mut Vec<Section>,
) -> Result<(), OperationError> {
    if (from - to).abs() < 1e-12 {
        return Ok(());
    }
    let length = a.length_m();
    let mut distances = vec![0.0];
    for p in a.points.windows(2) {
        distances.push(distances.last().unwrap() + alignment::distance(p[0], p[1]));
    }
    let anchors = a
        .anchor_indices
        .clone()
        .unwrap_or_else(|| vec![0, a.points.len() - 1]);
    let mut pieces = Vec::new();
    for (index, pair) in anchors.windows(2).enumerate() {
        let low = (distances[pair[0]] / length).max(from.min(to));
        let high = (distances[pair[1]] / length).min(from.max(to));
        if high - low <= 1e-12 {
            continue;
        }
        let tracks = a
            .section_infrastructure(index)
            .ok_or(OperationError::InvalidPath)?
            .track_count;
        let lane = directional_lane(tracks, to > from);
        let mut resources = extra.to_vec();
        resources.extend(
            crossings
                .iter()
                .filter(|(position, _)| low <= *position && *position <= high)
                .map(|(_, key)| key.clone()),
        );
        resources.push(format!("rail:{}:section:{index}:lane:{lane}", a.id.0));
        let mut speed = cap.min(a.effective_design_speed_kmh());
        for movement in internal.iter().filter(|m| {
            m.movement.from.alignment_id == a.id
                && m.from_position.min(m.to_position) < high
                && m.from_position.max(m.to_position) > low
        }) {
            speed = speed.min(movement.properties.speed_limit_kmh);
            if let Some(point) = movement.movement.point {
                resources.push(format!("rail:{}:point:{}", a.id.0, point.anchor_index));
            }
        }
        pieces.push(Section {
            length_m: (high - low) * length,
            speed_limit_kmh: speed,
            resources,
        });
    }
    if to < from {
        pieces.reverse();
    }
    out.extend(pieces);
    Ok(())
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldOperationSave {
    #[serde(default, skip_serializing_if = "is_false")]
    pub crossing_reservations: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub parallel_join_lanes: bool,
    pub configs: Vec<OperationServiceConfig>,
    pub bus_configs: Vec<BusOperationConfig>,
    pub runtime: crate::operation::OperationSave,
    pub invalidated: bool,
    pub pending_target: Option<u64>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct WorldOperation {
    pub configs: Vec<OperationServiceConfig>,
    pub bus_configs: Vec<BusOperationConfig>,
    pub runtime: crate::operation::OperationRuntime,
    pub invalidated: bool,
    pub pending_target: Option<u64>,
}
impl World {
    pub fn operation(&self) -> Option<&WorldOperation> {
        self.operation.as_ref()
    }
    pub fn operation_mut(&mut self) -> Option<&mut WorldOperation> {
        self.operation.as_mut()
    }
    /// Explicit reset/configuration only. Editing infrastructure never rewinds time.
    pub fn configure_operation(
        &mut self,
        configs: Vec<OperationServiceConfig>,
    ) -> Result<(), OperationError> {
        let bus_configs = self
            .operation
            .as_ref()
            .map(|o| o.bus_configs.clone())
            .unwrap_or_default();
        self.configure_all_operation(configs, bus_configs)
    }
    pub fn configure_all_operation(
        &mut self,
        configs: Vec<OperationServiceConfig>,
        bus_configs: Vec<BusOperationConfig>,
    ) -> Result<(), OperationError> {
        self.operation = Some(self.compile_all_operation(configs, bus_configs, None)?);
        Ok(())
    }

    /// Update configurations without replacing or replaying any active train.
    /// A caller may not silently discard a pending catch-up target.
    pub fn reconfigure_idle_operation(
        &mut self,
        configs: Vec<OperationServiceConfig>,
        bus_configs: Vec<BusOperationConfig>,
        edited_service: &str,
    ) -> Result<(), OperationError> {
        let old = self.operation.as_ref();
        if old.is_some_and(|op| {
            !op.runtime.paused()
                || op.runtime.trains().next().is_some()
                || op.pending_target.is_some()
        }) {
            return Err(OperationError::InvalidClock);
        }
        let started = old.filter(|old| old.runtime.has_started());
        let previous_admission = started.map(|old| old.runtime.admission_model());
        let mut candidate = self.compile_all_operation(configs, bus_configs, previous_admission)?;
        if let Some(old) = started {
            if !candidate
                .runtime
                .retains_admission(&old.runtime, edited_service, old.invalidated)
            {
                return Err(OperationError::ReservationScopeChanged);
            }
        }
        if let Some(old) = old {
            candidate.runtime.retain_idle_clock(&old.runtime)?;
        }
        self.operation = Some(candidate);
        Ok(())
    }

    fn compile_all_operation(
        &self,
        configs: Vec<OperationServiceConfig>,
        bus_configs: Vec<BusOperationConfig>,
        previous_admission: Option<RailAdmissionModel>,
    ) -> Result<WorldOperation, OperationError> {
        let crossings = self.active_flat_crossings(&configs, MAX_CROSSING_PAIR_CHECKS)?;
        let mut specs = Vec::with_capacity(configs.len() + bus_configs.len());
        let mut crossing_refs = 0usize;
        for config in &configs {
            let spec = self.compile_operation_reservations(
                config,
                true,
                RailReservations {
                    crossings: &crossings,
                    parallel_joins: true,
                },
            )?;
            crossing_refs = crossing_refs.saturating_add(crossing_lock_count(&spec));
            if crossing_refs > MAX_CROSSING_REFERENCES {
                return Err(OperationError::CrossingBudget);
            }
            specs.push(spec);
        }
        for config in &bus_configs {
            specs.push(self.compile_bus_operation(config)?);
        }
        use RailAdmissionModel::*;
        let qualifies = |mode| match mode {
            Fixed => true,
            _ if !bus_configs.is_empty() => false,
            TurnbackHold => self.terminal_hold_eligible(&configs, &specs),
            FixedRefuge => self.fixed_refuge_eligible(&configs, &specs),
            InlinePlatformHandoff => self.inline_handoff_eligible(&configs, &specs),
            InlineTerminalDwell => self.inline_terminal_dwell_eligible(&configs, &specs),
            Progressive => self.following_eligible(&configs, &specs),
        };
        // Fresh setup can choose a current supported model. A local edit of
        // an existing simulation must not choose a newer one for the world.
        let admission = match previous_admission {
            Some(mode) if qualifies(mode) => mode,
            Some(_) => return Err(OperationError::ReservationScopeChanged),
            None => [
                TurnbackHold,
                FixedRefuge,
                InlinePlatformHandoff,
                InlineTerminalDwell,
                Progressive,
            ]
            .into_iter()
            .find(|mode| qualifies(*mode))
            .unwrap_or(Fixed),
        };
        let runtime = match admission {
            Fixed => OperationRuntime::new(specs)?,
            Progressive => OperationRuntime::new_with_progressive_rail(specs)?,
            TurnbackHold => OperationRuntime::new_with_terminal_hold(specs)?,
            FixedRefuge => OperationRuntime::new_with_fixed_refuge(specs)?,
            InlineTerminalDwell => OperationRuntime::new_with_inline_terminal_dwell(specs)?,
            InlinePlatformHandoff => OperationRuntime::new_with_inline_platform_handoff(specs)?,
        };
        Ok(WorldOperation {
            configs,
            bus_configs,
            runtime,
            invalidated: false,
            pending_target: None,
        })
    }
    pub fn operation_save(&self) -> Option<WorldOperationSave> {
        self.operation.as_ref().map(|o| WorldOperationSave {
            configs: o.configs.clone(),
            bus_configs: o.bus_configs.clone(),
            crossing_reservations: o.configs.iter().any(|c| {
                o.runtime
                    .service_spec(&c.service_id.0.to_string())
                    .is_some_and(has_crossing_locks)
            }),
            parallel_join_lanes: o.configs.iter().any(|c| {
                o.runtime
                    .service_spec(&c.service_id.0.to_string())
                    .is_some_and(has_parallel_join_locks)
            }),
            runtime: o.runtime.save(),
            invalidated: o.invalidated,
            pending_target: o.pending_target,
        })
    }
    pub fn restore_operation(
        &mut self,
        saved: Option<WorldOperationSave>,
    ) -> Result<(), OperationError> {
        self.restore_operation_checked(saved, false)
    }

    fn restore_operation_checked(
        &mut self,
        saved: Option<WorldOperationSave>,
        allow_qualified_qa: bool,
    ) -> Result<(), OperationError> {
        let Some(saved) = saved else {
            self.operation = None;
            return Ok(());
        };
        if saved.crossing_reservations != saved.runtime.services.iter().any(has_crossing_locks)
            || saved.parallel_join_lanes
                != saved.runtime.services.iter().any(has_parallel_join_locks)
        {
            return Err(OperationError::InvalidPath);
        }
        let progressive = saved.runtime.format == 3;
        let holding = saved.runtime.format == 4;
        let refuge = saved.runtime.format == 7;
        let refuge_qualified = refuge
            && saved.bus_configs.is_empty()
            && !saved.parallel_join_lanes
            && !saved.crossing_reservations
            && if saved.invalidated {
                saved.runtime.paused
                    && Self::fixed_refuge_specs(&saved.configs, &saved.runtime.services).is_some()
            } else {
                self.fixed_refuge_eligible(&saved.configs, &saved.runtime.services)
            };
        let handoff = saved.runtime.format == 6;
        let handoff_qualified = handoff
            && saved.bus_configs.is_empty()
            && !saved.parallel_join_lanes
            && !saved.crossing_reservations
            && if saved.invalidated {
                saved.runtime.paused
                    && Self::inline_handoff_specs(&saved.configs, &saved.runtime.services)
            } else {
                self.inline_handoff_eligible(&saved.configs, &saved.runtime.services)
            };
        let platform = saved.runtime.format == 5;
        let platform_qualified = platform
            && saved.bus_configs.is_empty()
            && !saved.parallel_join_lanes
            && !saved.crossing_reservations
            && if saved.invalidated {
                saved.runtime.paused
                    && Self::following_specs(&saved.configs, &saved.runtime.services)
            } else {
                self.inline_terminal_dwell_eligible(&saved.configs, &saved.runtime.services)
            };
        let progressive_qualified = progressive
            && saved.bus_configs.is_empty()
            && !saved.parallel_join_lanes
            && !saved.crossing_reservations
            && if saved.invalidated {
                saved.runtime.paused
                    && Self::following_specs(&saved.configs, &saved.runtime.services)
            } else {
                self.following_eligible(&saved.configs, &saved.runtime.services)
                    || (allow_qualified_qa && self.is_straight_rail_occupancy_qa(&saved.configs))
            };
        if saved.runtime.format != 2
            && !holding
            && !progressive_qualified
            && !platform_qualified
            && !handoff_qualified
            && !refuge_qualified
        {
            return Err(OperationError::InvalidClock);
        }
        if holding
            && (!saved.bus_configs.is_empty()
                || !(1..=4).contains(&saved.configs.len())
                || saved
                    .configs
                    .iter()
                    .map(|c| c.service_id.0.to_string())
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != saved.configs.len()
                || saved.configs.len() != saved.runtime.services.len()
                || saved.configs.iter().any(|c| {
                    !saved.runtime.services.iter().any(|s| {
                        s.service_id == c.service_id.0.to_string()
                            && s.vehicle == c.vehicle
                            && s.timetable == c.timetable
                            && s.legs.first().is_some_and(|l| l.dwell_ms == c.dwell_ms)
                    })
                })
                || (!saved.invalidated
                    && !self.terminal_hold_eligible(&saved.configs, &saved.runtime.services)))
        {
            return Err(OperationError::InvalidPath);
        }
        if !saved.invalidated {
            if saved
                .runtime
                .services
                .iter()
                .map(crossing_lock_count)
                .sum::<usize>()
                > MAX_CROSSING_REFERENCES
            {
                return Err(OperationError::CrossingBudget);
            }
            // Legacy serialized rail specs have no crossing resource IDs;
            // restore them unchanged, not by rewriting active reservations.
            let crossings = if saved.runtime.services.iter().any(has_crossing_locks) {
                self.active_flat_crossings(&saved.configs, MAX_CROSSING_PAIR_CHECKS)?
            } else {
                RailCrossings::new()
            };
            let mut specs = saved
                .runtime
                .services
                .iter()
                .map(|spec| (spec.service_id.as_str(), spec))
                .collect::<std::collections::BTreeMap<_, _>>();
            if specs.len() != saved.runtime.services.len()
                || specs.len() != saved.configs.len() + saved.bus_configs.len()
            {
                return Err(OperationError::InvalidPath);
            }
            for config in &saved.configs {
                let id = config.service_id.0.to_string();
                let spec = specs
                    .remove(id.as_str())
                    .ok_or(OperationError::InvalidPath)?;
                let matches = if saved.parallel_join_lanes
                    || !crossings.is_empty()
                    || has_crossing_locks(spec)
                {
                    self.compile_operation_reservations(
                        config,
                        true,
                        RailReservations {
                            crossings: &crossings,
                            parallel_joins: saved.parallel_join_lanes,
                        },
                    )
                    .is_ok_and(|current| current == *spec)
                } else if progressive || holding || platform || handoff || refuge {
                    self.compile_operation(config)
                        .is_ok_and(|current| current == *spec)
                } else {
                    self.operation_spec_matches(config, spec)
                };
                if !matches {
                    return Err(OperationError::InvalidPath);
                }
            }
            for config in &saved.bus_configs {
                let compiled = self.compile_bus_operation(config)?;
                if specs.remove(compiled.service_id.as_str()) != Some(&compiled) {
                    return Err(OperationError::InvalidPath);
                }
            }
        }
        if saved
            .pending_target
            .is_some_and(|t| t < saved.runtime.now_ms)
        {
            return Err(OperationError::InvalidClock);
        }
        let mut runtime = if holding {
            crate::operation::OperationRuntime::restore_with_terminal_hold(saved.runtime)?
        } else if refuge_qualified {
            crate::operation::OperationRuntime::restore_with_fixed_refuge(saved.runtime)?
        } else if handoff_qualified {
            crate::operation::OperationRuntime::restore_with_inline_platform_handoff(saved.runtime)?
        } else if platform_qualified {
            crate::operation::OperationRuntime::restore_with_inline_terminal_dwell(saved.runtime)?
        } else if progressive_qualified {
            crate::operation::OperationRuntime::restore_with_progressive_rail(saved.runtime)?
        } else {
            crate::operation::OperationRuntime::restore(saved.runtime)?
        };
        if saved.invalidated {
            runtime.set_paused(true);
        }
        self.operation = Some(WorldOperation {
            configs: saved.configs,
            bus_configs: saved.bus_configs,
            runtime,
            invalidated: saved.invalidated,
            pending_target: saved.pending_target,
        });
        Ok(())
    }
    fn terminal_hold_eligible(
        &self,
        configs: &[OperationServiceConfig],
        specs: &[ServiceRunSpec],
    ) -> bool {
        self.straight_rail_geometry(configs, false, 1)
            && specs.len() == configs.len()
            && specs.iter().all(|spec| {
                let [outbound, returning] = spec.legs.as_slice() else {
                    return false;
                };
                if outbound.from != returning.to || outbound.to != returning.from {
                    return false;
                }
                // The initial model excludes points, junction locks and multiple lanes.
                let resources = |leg: &RunLeg| {
                    leg.sections
                        .iter()
                        .flat_map(|section| section.resources.iter().cloned())
                        .collect::<std::collections::BTreeSet<_>>()
                };
                let first = resources(outbound);
                if first.is_empty()
                    || first != resources(returning)
                    || first
                        .iter()
                        .any(|r| !r.contains(":section:") || !r.ends_with(":lane:0"))
                {
                    return false;
                }
                [&outbound.from, &outbound.to].iter().all(|&station| {
                    let Ok(id) = station.parse::<u64>() else {
                        return false;
                    };
                    let Some(p) = self
                        .data
                        .station_placements
                        .iter()
                        .find(|p| p.station_id == StationId(id))
                    else {
                        return false;
                    };
                    let Some(line) = self.data.alignments.iter().find(|a| a.id == p.alignment_id)
                    else {
                        return false;
                    };
                    if p.length_m < spec.vehicle.length_m + 4. || p.width_m < 4.1 {
                        return false;
                    }
                    let centre = p.position * line.length_m();
                    let start = centre - p.length_m / 2. - 2.;
                    let end = centre + p.length_m / 2. + 2.;
                    let mut section_start = 0.;
                    line.section_lengths_m()
                        .iter()
                        .enumerate()
                        .any(|(i, length)| {
                            let section_end = section_start + length;
                            let fits = start >= section_start
                                && end <= section_end
                                && first
                                    .contains(&format!("rail:{}:section:{i}:lane:0", line.id.0));
                            section_start = section_end;
                            fits
                        })
                })
            })
    }

    /// Frozen runtimes no longer have the original geometry, but still must
    /// obey the same two-service, shared-stems, one-parked-owner proof.
    fn fixed_refuge_specs<'a>(
        configs: &[OperationServiceConfig],
        specs: &'a [ServiceRunSpec],
    ) -> Option<(&'a ServiceRunSpec, &'a ServiceRunSpec, [u64; 3], u64)> {
        let [a, b] = configs else {
            return None;
        };
        if a.service_id == b.service_id || specs.len() != 2 {
            return None;
        }
        let matched = |c: &OperationServiceConfig| {
            specs.iter().find(|s| {
                s.service_id == c.service_id.0.to_string()
                    && s.timetable == c.timetable
                    && s.vehicle == c.vehicle
                    && s.vehicle.length_m <= 80.
                    && s.legs.iter().all(|l| l.dwell_ms == c.dwell_ms)
            })
        };
        let (sa, sb) = (matched(a)?, matched(b)?);
        let (local, express) = match (sa.legs.len(), sb.legs.len()) {
            (2, 1) => (sa, sb),
            (1, 2) => (sb, sa),
            _ => return None,
        };
        if local.legs[0].from != express.legs[0].from
            || local.legs[1].to != express.legs[0].to
            || local.legs[0].to == local.legs[0].from
            || local.legs[0].to == local.legs[1].to
        {
            return None;
        }
        let local_ids @ [west, _, east] = fixed_refuge_resources(local)?;
        let [w, express_branch, e] = fixed_refuge_resources(express)?;
        (west == w && east == e).then_some((local, express, local_ids, express_branch))
    }

    fn fixed_refuge_eligible(
        &self,
        configs: &[OperationServiceConfig],
        specs: &[ServiceRunSpec],
    ) -> bool {
        use AlignmentEndpoint::{End, Start};
        let d = &self.data;
        let Some((local, express, [w, mid, e], express_branch)) =
            Self::fixed_refuge_specs(configs, specs)
        else {
            return false;
        };
        if d.alignments.len() != 4
            || d.alignment_connections.len() != 4
            || !d.road_transport.bus_routes.is_empty()
            || !d.road_transport.tram_sections.is_empty()
        {
            return false;
        }
        let get = |id| d.alignments.iter().find(|a| a.id.0 == id);
        let (Some(west), Some(middle), Some(east), Some(other)) = (
            get(w),
            get(mid),
            get(e),
            d.alignments.iter().find(|a| ![w, mid, e].contains(&a.id.0)),
        ) else {
            return false;
        };
        if ![mid, other.id.0].contains(&express_branch) {
            return false;
        }
        let links = [(west, middle), (west, other), (middle, east), (other, east)];
        if links.into_iter().any(|(a, b)| {
            let from = AlignmentEndpointRef {
                alignment_id: a.id,
                endpoint: End,
            };
            let to = AlignmentEndpointRef {
                alignment_id: b.id,
                endpoint: Start,
            };
            !d.alignment_connections
                .iter()
                .any(|link| (link.a == from && link.b == to) || (link.a == to && link.b == from))
        }) {
            return false;
        }
        let latitude = west.points[0].latitude;
        if latitude.abs() > 60.0 || west.points[0].longitude.abs() > 179.0 {
            return false;
        }
        for (a, count) in [(west, 2), (east, 2), (middle, 1), (other, 1)] {
            if a.track_type != TrackType::Normal
                || a.vertical_profile.is_some()
                || !a.section_profiles.is_empty()
                || a.section_count() != 1
                || a.points.len() > 16
                || a.section_infrastructure(0)
                    != Some(SectionInfrastructure {
                        track_count: count,
                        level: InfrastructureLevel::Ground,
                    })
                || a.points
                    .windows(2)
                    .any(|p| p[0].longitude >= p[1].longitude)
                || a.points
                    .iter()
                    .any(|p| (p.latitude - latitude).abs() > 0.002)
                || a.points.last().unwrap().longitude - a.points[0].longitude > 0.05
            {
                return false;
            }
        }
        if west
            .points
            .iter()
            .chain(&east.points)
            .any(|p| p.latitude != latitude)
        {
            return false;
        }
        let straight = |a: &Alignment| a.points.iter().all(|p| p.latitude == latitude);
        let curved = match (straight(middle), straight(other)) {
            (true, false) => other,
            (false, true) => middle,
            _ => return false,
        };
        let interior = &curved.points[1..curved.points.len() - 1];
        let side = interior
            .first()
            .map_or(0., |p| (p.latitude - latitude).signum());
        if interior
            .iter()
            .any(|p| (p.latitude - latitude) * side <= 0.)
            || side == 0.
        {
            return false;
        }
        let placement = |id: &str, a: &Alignment| {
            let id = StationId(id.parse().ok()?);
            d.station_placements
                .iter()
                .find(|p| p.station_id == id && p.alignment_id == a.id)
        };
        let (Some(a), Some(b), Some(c)) = (
            placement(&local.legs[0].from, west),
            placement(&local.legs[0].to, middle),
            placement(&local.legs[1].to, east),
        ) else {
            return false;
        };
        if express.legs[0].to != local.legs[1].to {
            return false;
        }
        let train = local.vehicle.length_m.max(express.vehicle.length_m);
        let fits = |p: &StationPlacement, line: &Alignment, train: f64| {
            // Curved turnout geometry leads are capped at 30 m. Keep the
            // entire parked envelope at least 35 m away from either throat.
            let margin = train.max(p.length_m / 2.) + 35.;
            p.length_m >= train + 4.
                && p.width_m >= 4.1
                && p.position * line.length_m() >= margin
                && (1. - p.position) * line.length_m() >= margin
        };
        if !fits(a, west, train)
            || !fits(b, middle, local.vehicle.length_m)
            || !fits(c, east, train)
        {
            return false;
        }
        // Prove the sibling track is laterally separate across the *whole*
        // parked envelope, not merely from the other track's centre at B.
        // The sibling curve is monotone in longitude, so the minimum on a
        // clipped interval occurs at a true polyline vertex or an endpoint.
        let margin = (local.vehicle.length_m.max(b.length_m / 2.) + 4.) / middle.length_m();
        let left = middle.point_at(b.position - margin).longitude;
        let right = middle.point_at(b.position + margin).longitude;
        let minimum_m = b.width_m / 2. + 4.;
        let mut covered = left;
        for p in curved.points.windows(2) {
            let a = p[0].longitude.max(left);
            let b = p[1].longitude.min(right);
            if a > b {
                continue;
            }
            if a > covered {
                return false;
            }
            for x in [a, b] {
                let t = (x - p[0].longitude) / (p[1].longitude - p[0].longitude);
                let delta = (p[0].latitude - latitude) * (1. - t) + (p[1].latitude - latitude) * t;
                if side * delta.to_radians() * 6_371_000. < minimum_m {
                    return false;
                }
            }
            covered = b;
        }
        right > left && covered >= right
    }

    fn inline_handoff_specs(configs: &[OperationServiceConfig], specs: &[ServiceRunSpec]) -> bool {
        (1..=4).contains(&configs.len())
            && configs.len() == specs.len()
            && configs
                .iter()
                .map(|c| c.service_id)
                .collect::<BTreeSet<_>>()
                .len()
                == configs.len()
            && specs.iter().any(|s| s.legs.len() == 2)
            && configs.iter().all(|c| {
                specs
                    .iter()
                    .find(|s| s.service_id == c.service_id.0.to_string())
                    .is_some_and(|s| {
                        s.timetable == c.timetable
                            && s.vehicle == c.vehicle
                            && s.vehicle.length_m <= 80.
                            && s.legs.iter().all(|l| l.dwell_ms == c.dwell_ms)
                            && inline_platform_handoff(s)
                    })
            })
    }

    fn inline_handoff_eligible(
        &self,
        configs: &[OperationServiceConfig],
        specs: &[ServiceRunSpec],
    ) -> bool {
        Self::inline_handoff_specs(configs, specs)
            && self.straight_rail_geometry(configs, true, 2)
            && specs.iter().all(|spec| {
                spec.legs.iter().all(|leg| {
                    [
                        (leg.from.as_str(), leg.sections.first()),
                        (leg.to.as_str(), leg.sections.last()),
                    ]
                    .into_iter()
                    .all(|(id, section)| {
                        let Some((alignment, block, _)) =
                            section.and_then(lane_resource).and_then(inline_lane_key)
                        else {
                            return false;
                        };
                        let Ok(id) = id.parse::<u64>() else {
                            return false;
                        };
                        let Some(p) = self.data.station_placements.iter().find(|p| {
                            p.station_id == StationId(id)
                                && p.alignment_id == AlignmentId(alignment)
                        }) else {
                            return false;
                        };
                        let Some(line) =
                            self.data.alignments.iter().find(|a| a.id == p.alignment_id)
                        else {
                            return false;
                        };
                        if p.length_m < spec.vehicle.length_m + 4. || p.width_m < 4.1 {
                            return false;
                        }
                        let lengths = line.section_lengths_m();
                        let Some(length) = lengths.get(block) else {
                            return false;
                        };
                        let left = lengths[..block].iter().sum::<f64>();
                        let centre = p.position * line.length_m();
                        let margin = spec.vehicle.length_m.max(p.length_m / 2.) + 2.;
                        centre - margin >= left && centre + margin <= left + length
                    })
                })
            })
    }

    // A station group/name is not an occupancy resource. For the existing
    // qualified one-way corridor, the terminal's actual rail placement and
    // the service's last directional lane establish inline block occupancy.
    // No admission across a turnout or across a second platform track is inferred.
    fn inline_terminal_dwell_eligible(
        &self,
        configs: &[OperationServiceConfig],
        specs: &[ServiceRunSpec],
    ) -> bool {
        self.following_eligible(configs, specs)
            && specs.iter().all(|spec| {
                let Ok(id) = spec.legs[0].to.parse::<u64>() else {
                    return false;
                };
                let Some(terminal) = self
                    .data
                    .station_placements
                    .iter()
                    .find(|placement| placement.station_id == StationId(id))
                else {
                    return false;
                };
                // following_eligible already checks the full train and platform
                // do not protrude into a different physical anchor block.
                terminal.length_m >= spec.vehicle.length_m + 4. && terminal.width_m >= 4.1
            })
    }

    // Product following is intentionally narrower than the test-only
    // single-track experiment: only direct, one-way legs on independent
    // double-track corridors, so no train tries to reacquire its own approach
    // in reverse and no other route can enter across a turnout.
    fn following_eligible(
        &self,
        configs: &[OperationServiceConfig],
        specs: &[ServiceRunSpec],
    ) -> bool {
        self.straight_rail_geometry(configs, true, 2)
            && Self::following_specs(configs, specs)
            && specs.iter().all(|spec| {
                let leg = &spec.legs[0];
                [&leg.from, &leg.to].iter().all(|id| {
                    let Ok(id) = id.parse::<u64>() else {
                        return false;
                    };
                    let Some(p) = self
                        .data
                        .station_placements
                        .iter()
                        .find(|p| p.station_id == StationId(id))
                    else {
                        return false;
                    };
                    let Some(line) = self.data.alignments.iter().find(|a| a.id == p.alignment_id)
                    else {
                        return false;
                    };
                    let centre = p.position * line.length_m();
                    // No parked train protrudes into an adjacent anchor
                    // section at either modeled endpoint.
                    let margin = spec.vehicle.length_m.max(p.length_m / 2.) + 2.;
                    let mut left = 0.;
                    line.section_lengths_m().into_iter().any(|length| {
                        let right = left + length;
                        let fits = centre - margin >= left && centre + margin <= right;
                        left = right;
                        fits
                    })
                })
            })
    }

    // The intentionally frozen form cannot be checked against edited
    // geography. Still require a scoped one-way rail spec, no road/point keys,
    // and matching configs. It remains paused until explicit reconfiguration.
    fn following_specs(configs: &[OperationServiceConfig], specs: &[ServiceRunSpec]) -> bool {
        (1..=4).contains(&configs.len())
            && configs.len() == specs.len()
            && configs
                .iter()
                .map(|c| c.service_id)
                .collect::<BTreeSet<_>>()
                .len()
                == configs.len()
            && configs.iter().all(|c| {
                specs
                    .iter()
                    .find(|spec| spec.service_id == c.service_id.0.to_string())
                    .is_some_and(|spec| {
                        let [leg] = spec.legs.as_slice() else {
                            return false;
                        };
                        spec.timetable == c.timetable
                            && spec.vehicle == c.vehicle
                            && spec.vehicle.length_m <= 80.
                            && leg.dwell_ms == c.dwell_ms
                            && !leg.sections.is_empty()
                            && leg.from != leg.to
                            && leg.sections.iter().all(|s| {
                                s.resources.len() == 1
                                    && lane_resource(s).is_some_and(|r| {
                                        r.ends_with(":lane:0") || r.ends_with(":lane:1")
                                    })
                            })
                    })
            })
    }

    /// Narrow acceptance gate, not a general level-crossing detector.
    /// Every alignment in the world must be a local, monotone east-west flat
    /// ground single-track corridor; an optional second corridor must be well
    /// separated and neither corridor can be connected to anything else.
    fn is_straight_rail_occupancy_qa(&self, configs: &[OperationServiceConfig]) -> bool {
        self.straight_rail_geometry(configs, true, 1)
    }

    fn straight_rail_geometry(
        &self,
        configs: &[OperationServiceConfig],
        progressive_test: bool,
        tracks: u8,
    ) -> bool {
        let d = &self.data;
        let min_points = if progressive_test { 3 } else { 2 };
        let min_blocks = if progressive_test { 3 } else { 1 };
        if !(1..=2).contains(&d.alignments.len())
            || !(1..=4).contains(&configs.len())
            || !d.alignment_connections.is_empty()
            || !d.road_transport.bus_routes.is_empty()
            || !d.road_transport.tram_sections.is_empty()
        {
            return false;
        }
        let mut latitudes = Vec::new();
        for a in &d.alignments {
            if a.track_type != TrackType::Normal
                || !(min_points..=64).contains(&a.points.len())
                || !(min_blocks..=8).contains(&a.section_count())
                || a.vertical_profile.is_some()
                || !a.section_profiles.is_empty()
                || (0..a.section_count()).any(|i| {
                    a.section_infrastructure(i)
                        != Some(SectionInfrastructure {
                            track_count: tracks,
                            level: InfrastructureLevel::Ground,
                        })
                })
            {
                return false;
            }
            let first = a.points[0];
            let last = a.points[a.points.len() - 1];
            if first.latitude.abs() > 60.0
                || first.longitude.abs() > 179.0
                || !(0.005..=0.05).contains(&(last.longitude - first.longitude))
                || a.points.iter().any(|p| p.latitude != first.latitude)
                || a.points
                    .windows(2)
                    .any(|p| p[0].longitude >= p[1].longitude)
                || latitudes
                    .iter()
                    .any(|&latitude: &f64| (first.latitude - latitude).abs() < 0.002)
            {
                return false;
            }
            latitudes.push(first.latitude);
        }
        configs.iter().all(|c| {
            c.vehicle.length_m <= 80.0
                && d.service_paths
                    .iter()
                    .find(|p| p.service_id == c.service_id)
                    .is_some_and(|p| {
                        p.legs.len() == 1
                            && matches!(p.legs[0].segment, RouteSegment::Alignment { .. })
                    })
        })
    }

    #[cfg(test)]
    fn configure_straight_rail_occupancy_qa(
        &mut self,
        configs: Vec<OperationServiceConfig>,
    ) -> Result<(), OperationError> {
        if !self.is_straight_rail_occupancy_qa(&configs) {
            return Err(OperationError::InvalidPath);
        }
        let specs = configs
            .iter()
            .map(|c| self.compile_operation(c))
            .collect::<Result<Vec<_>, _>>()?;
        let runtime = crate::operation::OperationRuntime::new_with_progressive_rail(specs)?;
        self.operation = Some(WorldOperation {
            configs,
            bus_configs: Vec::new(),
            runtime,
            invalidated: false,
            pending_target: None,
        });
        Ok(())
    }

    /// Used only by edit transactions/Undo, never world loading or new world.
    /// Invalidated trains remain frozen and visibly require explicit restart.
    pub fn retain_operation_from(&mut self, previous: &World) {
        self.retain_operation_with_crossing_limit(previous, MAX_CROSSING_PAIR_CHECKS);
    }
    fn retain_operation_with_crossing_limit(&mut self, previous: &World, limit: usize) {
        self.operation = previous.operation.clone();
        let invalid = self.operation.as_ref().is_some_and(|o| {
            if o.invalidated {
                return false;
            }
            if o.runtime.progressive_rail() {
                let active = o
                    .configs
                    .iter()
                    .map(|c| self.compile_operation(c))
                    .collect::<Result<Vec<_>, _>>();
                return match active {
                    Ok(specs) => {
                        !(if o.runtime.fixed_refuge() {
                            self.fixed_refuge_eligible(&o.configs, &specs)
                        } else if o.runtime.inline_platform_handoff() {
                            self.inline_handoff_eligible(&o.configs, &specs)
                        } else if o.runtime.inline_terminal_dwell() {
                            self.inline_terminal_dwell_eligible(&o.configs, &specs)
                        } else {
                            self.following_eligible(&o.configs, &specs)
                        }) || specs
                            .iter()
                            .any(|spec| o.runtime.service_spec(&spec.service_id) != Some(spec))
                    }
                    Err(_) => true,
                };
            }
            let old_crossings = o.configs.iter().any(|c| {
                o.runtime
                    .service_spec(&c.service_id.0.to_string())
                    .is_some_and(has_crossing_locks)
            });
            let crossings = match self.active_flat_crossings(&o.configs, limit) {
                Ok(crossings) => crossings,
                // Existing legacy runs have no crossing-lock contract. A budget
                // overrun alone must not reset a still-valid legacy schedule.
                Err(OperationError::CrossingBudget | OperationError::CrossingPlatform)
                    if !old_crossings =>
                {
                    RailCrossings::new()
                }
                Err(_) => return true,
            };
            let new_crossing_relation = !old_crossings
                && !crossings.is_empty()
                && previous
                    .active_flat_crossings(&o.configs, limit)
                    .is_ok_and(|before| before != crossings);
            let old_parallel_joins = o.configs.iter().any(|c| {
                o.runtime
                    .service_spec(&c.service_id.0.to_string())
                    .is_some_and(has_parallel_join_locks)
            });
            if old_parallel_joins || old_crossings || new_crossing_relation {
                return o.configs.iter().any(|c| {
                    self.compile_operation_reservations(
                        c,
                        true,
                        RailReservations {
                            crossings: &crossings,
                            parallel_joins: old_parallel_joins,
                        },
                    )
                    .ok()
                    .as_ref()
                        != o.runtime.service_spec(&c.service_id.0.to_string())
                });
            }
            if o.runtime.terminal_hold() {
                let current = o
                    .configs
                    .iter()
                    .map(|config| self.compile_operation(config))
                    .collect::<Result<Vec<_>, _>>();
                return match current {
                    Ok(specs) => {
                        !self.terminal_hold_eligible(&o.configs, &specs)
                            || specs
                                .iter()
                                .any(|spec| o.runtime.service_spec(&spec.service_id) != Some(spec))
                    }
                    Err(_) => true,
                };
            }
            o.configs.iter().any(|config| {
                o.runtime
                    .service_spec(&config.service_id.0.to_string())
                    .is_none_or(|spec| !self.operation_spec_matches(config, spec))
            })
        });
        let invalid = invalid
            || self.operation.as_ref().is_some_and(|o| {
                !o.invalidated
                    && o.bus_configs.iter().any(|config| {
                        self.compile_bus_operation(config).ok().as_ref()
                            != o.runtime.service_spec(&format!("bus:{}", config.route_id))
                    })
            });
        if invalid {
            if let Some(o) = &mut self.operation {
                o.invalidated = true;
                o.runtime.set_paused(true);
            }
        }
    }
}

impl World {
    pub fn compile_bus_operation(
        &self,
        config: &BusOperationConfig,
    ) -> Result<ServiceRunSpec, OperationError> {
        if !config.assumed_speed_kmh.is_finite()
            || !(1.0..=120.0).contains(&config.assumed_speed_kmh)
        {
            return Err(OperationError::InvalidVehicle);
        }
        let roads = &self.data.road_transport;
        let legs = roads
            .bus_legs(&self.data, &config.route_id)
            .map_err(|_| OperationError::InvalidPath)?;
        let runs = legs
            .into_iter()
            .map(|leg| RunLeg {
                from: format!("bus:{}", leg.from_stop_id),
                to: format!("bus:{}", leg.to_stop_id),
                dwell_ms: config.dwell_ms,
                sections: vec![Section {
                    length_m: leg.length_m,
                    speed_limit_kmh: config.assumed_speed_kmh,
                    resources: leg
                        .traversed_edges
                        .iter()
                        .map(|id| format!("road:edge:{id}"))
                        .collect(),
                }],
            })
            .collect();
        Ok(ServiceRunSpec {
            service_id: format!("bus:{}", config.route_id),
            vehicle: config.vehicle.clone(),
            timetable: config.timetable.clone(),
            legs: runs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (World, OperationServiceConfig) {
        let mut w = World::default();
        let p = w.add_player("p".into()).unwrap();
        let c = w.add_company(p, "c".into()).unwrap();
        let a = w
            .build_alignment(
                c,
                "line".into(),
                vec![
                    GeoPoint {
                        latitude: 35.,
                        longitude: 139.,
                    },
                    GeoPoint {
                        latitude: 35.,
                        longitude: 139.02,
                    },
                ],
            )
            .unwrap();
        let s = w
            .place_station_on_alignment(c, "A".into(), a, 0.1, 100., 10.)
            .unwrap();
        let m = w
            .place_station_on_alignment(c, "B".into(), a, 0.5, 100., 10.)
            .unwrap();
        let e = w
            .place_station_on_alignment(c, "C".into(), a, 0.9, 100., 10.)
            .unwrap();
        let segments = vec![RouteSegment::Alignment {
            alignment_id: a,
            from: s,
            to: e,
        }];
        let preview = w.preview_service_path(s, segments.clone()).unwrap();
        let stops = preview
            .occurrences
            .iter()
            .filter(|o| o.station_id != m)
            .map(|o| o.reference.clone())
            .collect();
        let service_id = w
            .add_service_path(c, "express".into(), s, segments, stops)
            .unwrap();
        let config = OperationServiceConfig {
            service_id,
            timetable: Timetable {
                first_departure_ms: 0,
                last_departure_ms: 60000,
                headway_ms: 60000,
            },
            vehicle: Vehicle {
                max_speed_kmh: 100.,
                acceleration_mps2: 1.,
                braking_mps2: 1.,
                length_m: 80.,
            },
            dwell_ms: 30000,
        };
        (w, config)
    }
    #[test]
    fn crossing_sweep_counts_sparse_and_dense_inputs_and_stops_at_budget() {
        let (w, _) = fixture();
        let base = &w.snapshot().alignments[0];
        let spread: Vec<_> = (0..1000)
            .map(|k| (k as f64, k as f64 + 0.25, 0.0, 1.0, base))
            .collect();
        let (found, checks) = bounded_flat_crossings(spread, MAX_CROSSING_PAIR_CHECKS).unwrap();
        assert!(found.is_empty());
        assert_eq!(checks, 0);
        let dense = |n| vec![(0.0, 1.0, 0.0, 1.0, base); n];
        let (found, checks) = bounded_flat_crossings(dense(256), MAX_CROSSING_PAIR_CHECKS).unwrap();
        assert!(found.is_empty());
        assert_eq!(checks, 32_640);
        assert_eq!(
            bounded_flat_crossings(dense(257), MAX_CROSSING_PAIR_CHECKS),
            Err(OperationError::CrossingBudget)
        );
        let parallel: Vec<_> = (0..1000)
            .map(|k| (0.0, 1.0, k as f64, k as f64, base))
            .collect();
        assert_eq!(
            bounded_flat_crossings(parallel, MAX_CROSSING_PAIR_CHECKS)
                .unwrap()
                .1,
            0
        );
        let mut grid = Vec::new();
        for region in 0..5 {
            for k in 0..20 {
                let (x, y) = (131.0 + region as f64 * 2.0, 35.0 + region as f64);
                let offset = (k + 1) as f64 * 0.001;
                for (u, v) in [
                    (
                        GeoPoint {
                            latitude: y + offset,
                            longitude: x,
                        },
                        GeoPoint {
                            latitude: y + offset,
                            longitude: x + 0.021,
                        },
                    ),
                    (
                        GeoPoint {
                            latitude: y,
                            longitude: x + offset,
                        },
                        GeoPoint {
                            latitude: y + 0.021,
                            longitude: x + offset,
                        },
                    ),
                ] {
                    let mut a = base.clone();
                    a.id = AlignmentId(grid.len() as u64 + 1000);
                    a.points = vec![u, v];
                    grid.push(a);
                }
            }
        }
        let bounds = grid
            .iter()
            .map(|a| {
                (
                    a.points[0].longitude,
                    a.points[1].longitude,
                    a.points[0].latitude,
                    a.points[1].latitude,
                    a,
                )
            })
            .collect();
        let start = std::time::Instant::now();
        let (found, checks) = bounded_flat_crossings(bounds, MAX_CROSSING_PAIR_CHECKS).unwrap();
        assert_eq!(checks, 2950);
        assert_eq!(found.values().map(Vec::len).sum::<usize>(), 4000);
        println!(
            "crossing_grid regions=5 corridors=200 checks={checks} locks=2000 elapsed_ms={:.3}",
            start.elapsed().as_secs_f64() * 1000.0
        );
        println!(
            "crossing_pair_checks sparse_1000=0 dense_256=32640 dense_257=budget_exceeded_at_32769"
        );
    }
    #[test]
    fn crossing_scan_budget_does_not_invalidate_a_live_legacy_run() {
        let (mut world, a) = fixture();
        let owner = world.data.alignments[0].owner;
        let cross = world
            .build_alignment(
                owner,
                "cross".into(),
                vec![
                    GeoPoint {
                        latitude: 34.99,
                        longitude: 139.01,
                    },
                    GeoPoint {
                        latitude: 35.01,
                        longitude: 139.01,
                    },
                ],
            )
            .unwrap();
        let first = world
            .place_station_on_alignment(owner, "D".into(), cross, 0.1, 80., 10.)
            .unwrap();
        let last = world
            .place_station_on_alignment(owner, "E".into(), cross, 0.9, 80., 10.)
            .unwrap();
        let segments = vec![RouteSegment::Alignment {
            alignment_id: cross,
            from: first,
            to: last,
        }];
        let preview = world.preview_service_path(first, segments.clone()).unwrap();
        let service_id = world
            .add_service_path(
                owner,
                "crossing service".into(),
                first,
                segments,
                preview
                    .occurrences
                    .iter()
                    .map(|o| o.reference.clone())
                    .collect(),
            )
            .unwrap();
        let b = OperationServiceConfig {
            service_id,
            ..a.clone()
        };
        let configs = vec![a, b];
        assert_eq!(
            world.active_flat_crossings(&configs, 0),
            Err(OperationError::CrossingBudget)
        );
        let legacy_specs = configs
            .iter()
            .map(|c| world.compile_operation(c).unwrap())
            .collect();
        world.configure_operation(configs.clone()).unwrap();
        let mut legacy = world.clone();
        legacy.operation_mut().unwrap().runtime =
            crate::operation::OperationRuntime::new(legacy_specs).unwrap();
        legacy
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(0, 100)
            .unwrap();
        let mut edited = legacy.clone();
        edited.data.companies[0].name = "renamed".into();
        edited.retain_operation_with_crossing_limit(&legacy, 0);
        assert_eq!(edited.operation_save(), legacy.operation_save());
        assert!(!edited.operation().unwrap().invalidated);
        edited.retain_operation_from(&legacy);
        assert_eq!(edited.operation_save(), legacy.operation_save());

        // A keyed run cannot carry forward unknown/incomplete crossing locks.
        // Fault injection: a real previously successful unchanged case cannot
        // exceed the same fixed budget without a relevant geometry change.
        let mut locked = world.clone();
        locked.data.companies[0].name = "renamed".into();
        locked.retain_operation_with_crossing_limit(&world, 0);
        assert!(locked.operation().unwrap().invalidated);
        assert!(locked.operation().unwrap().runtime.paused());
    }

    #[test]
    fn adapter_uses_physical_distance_and_passes_nonstop_station() {
        let (w, c) = fixture();
        let s = w.compile_operation(&c).unwrap();
        assert_eq!(s.legs.len(), 2);
        assert_eq!(s.legs[0].sections.len(), 2);
        assert_eq!(s.legs[1].sections.len(), 2);
        assert_eq!(s.legs[0].from, s.legs[1].to);
        assert_eq!(s.legs[0].to, s.legs[1].from);
        let d: f64 = s.legs[0].sections.iter().map(|s| s.length_m).sum();
        assert!((d - w.snapshot().alignments[0].length_m() * 0.8).abs() < 0.001);
        assert_eq!(
            s.legs[0].sections[0].resources,
            s.legs[0].sections[1].resources
        );
    }
    #[test]
    fn edit_invalidation_preserves_clock_and_save_restore() {
        let (mut w, c) = fixture();
        w.configure_operation(vec![c]).unwrap();
        w.operation_mut()
            .unwrap()
            .runtime
            .advance_to(1000, 100)
            .unwrap();
        let saved = w.operation_save();
        let mut restored = World::new(w.snapshot().clone()).unwrap();
        restored.restore_operation(saved).unwrap();
        assert_eq!(restored, w);
        let mut changed = World::new(w.snapshot().clone()).unwrap();
        let a = changed.data.alignments[0].id;
        changed.set_alignment_design_speed(a, 50.).unwrap();
        changed.retain_operation_from(&w);
        assert!(changed.operation().unwrap().invalidated);
        assert_eq!(changed.operation().unwrap().runtime.now_ms(), 1000);
        assert!(World::new(w.snapshot().clone())
            .unwrap()
            .operation()
            .is_none());
        let mut new = World::new(changed.snapshot().clone()).unwrap();
        new.restore_operation(changed.operation_save()).unwrap();
        assert!(new.operation().unwrap().invalidated);
    }
    #[test]
    fn turnback_reuses_train_after_dwell_and_clearance_and_restores_in_flight() {
        use crate::operation::TrainState;
        for dwell in [0, 30_000] {
            let (mut w, mut c) = fixture();
            c.timetable.last_departure_ms = 0; // exactly one scheduled train
            c.dwell_ms = dwell;
            w.configure_operation(vec![c.clone()]).unwrap();
            let runtime = &mut w.operation_mut().unwrap().runtime;
            let spec = runtime
                .service_spec(&c.service_id.0.to_string())
                .unwrap()
                .clone();
            runtime.advance_to(0, 100).unwrap();
            let train = runtime.trains().next().unwrap().clone();
            let TrainState::Moving {
                arrival_ms,
                release_ms,
                ..
            } = train.state
            else {
                panic!("expected outbound movement");
            };
            let ready = (arrival_ms + dwell).max(release_ms);
            let reached = runtime.advance_to(ready - 1, 100).unwrap();
            assert_eq!(reached.arrivals.len(), 1, "no arrival at passed station");
            assert_eq!(reached.arrivals[0].station_id, spec.legs[0].to);
            assert_eq!(reached.arrivals[0].train, train.key);
            let now = runtime.trains().next().unwrap();
            if let TrainState::Moving { entered_ms, .. } = now.state {
                assert_eq!(now.leg, 0);
                assert_eq!(entered_ms, 0);
            }
            runtime.advance_to(ready, 100).unwrap();
            let turning = runtime.trains().next().unwrap();
            assert_eq!(turning.key, train.key);
            assert_eq!(turning.leg, 1);
            let TrainState::Moving {
                entered_ms,
                release_ms,
                ..
            } = turning.state
            else {
                panic!("expected return movement");
            };
            assert_eq!(entered_ms, ready);
            let saved = w.operation_save();
            let mut restored = World::new(w.snapshot().clone()).unwrap();
            restored.restore_operation(saved).unwrap();
            let finish = |w: &mut World| {
                w.operation_mut()
                    .unwrap()
                    .runtime
                    .advance_to(release_ms * 2, 100)
                    .unwrap()
            };
            let a = finish(&mut w);
            assert_eq!(finish(&mut restored), a);
            assert_eq!(a.arrivals.len(), 1);
            assert_eq!(a.arrivals[0].station_id, spec.legs[0].from);
            assert_eq!(a.arrivals[0].train, train.key);
            assert_eq!(restored.operation().unwrap().runtime.trains().count(), 0);
        }
    }

    #[test]
    fn dual_track_terminal_is_outbound_only_without_crossover() {
        let (w, c) = fixture();
        let mut draft = w.snapshot().clone();
        draft.alignments[0].infrastructure = vec![SectionInfrastructure {
            track_count: 2,
            ..Default::default()
        }];
        let w = World::new(draft).unwrap();
        assert_eq!(w.compile_operation(&c).unwrap().legs.len(), 1);
        assert_eq!(
            w.compile_operation(&c).unwrap(),
            w.compile_operation_mode(&c, false, &RailCrossings::new())
                .unwrap()
        );
    }

    #[test]
    fn exact_legacy_outbound_is_neither_rewritten_nor_invalidated_by_unrelated_edits() {
        let (mut w, c) = fixture();
        let legacy = w
            .compile_operation_mode(&c, false, &RailCrossings::new())
            .unwrap();
        let mut legacy_runtime =
            crate::operation::OperationRuntime::new(vec![legacy.clone()]).unwrap();
        legacy_runtime.advance_to(1_000, 100).unwrap();
        let saved = WorldOperationSave {
            crossing_reservations: false,
            parallel_join_lanes: false,
            configs: vec![c.clone()],
            bus_configs: vec![],
            runtime: legacy_runtime.save(),
            invalidated: false,
            pending_target: None,
        };
        w.restore_operation(Some(saved.clone())).unwrap();
        assert_eq!(w.operation_save(), Some(saved.clone()));
        let mut edit = World::new(w.snapshot().clone()).unwrap();
        edit.retain_operation_from(&w);
        assert_eq!(edit.operation_save(), Some(saved.clone()));
        let mut tampered = saved.clone();
        tampered.runtime.services[0].legs[0].dwell_ms += 1;
        assert_eq!(
            edit.restore_operation(Some(tampered)),
            Err(OperationError::InvalidPath)
        );
        assert_eq!(edit.operation_save(), Some(saved));
        let a = edit.data.alignments[0].id;
        edit.set_alignment_design_speed(a, 50.).unwrap();
        edit.retain_operation_from(&w);
        assert!(edit.operation().unwrap().invalidated);
    }

    #[test]
    fn already_closed_service_is_not_doubled() {
        let (mut w, c) = fixture();
        let id = c.service_id;
        let path = w
            .data
            .service_paths
            .iter_mut()
            .find(|p| p.service_id == id)
            .unwrap();
        let RouteSegment::Alignment {
            alignment_id,
            from,
            to,
        } = path.legs[0].segment
        else {
            panic!()
        };
        path.legs.push(ServiceLeg {
            key: 2,
            segment: RouteSegment::Alignment {
                alignment_id,
                from: to,
                to: from,
            },
        });
        path.next_leg_key = 3;
        path.stops.push(StationOccurrenceRef::OnLeg {
            leg_key: 2,
            station_id: from,
        });
        let w = World::new(w.snapshot().clone()).unwrap();
        assert_eq!(w.compile_operation(&c).unwrap().legs.len(), 2);
        assert_eq!(
            w.compile_operation(&c).unwrap(),
            w.compile_operation_mode(&c, false, &RailCrossings::new())
                .unwrap()
        );
    }
    #[test]
    fn reversed_connected_corridor_preserves_join_incidence_and_stop_flags() {
        let (mut w, mut config) = fixture();
        let base = w.data.alignments[0].id;
        let owner = w.data.alignments[0].owner;
        let first = w.service_path_occurrences(config.service_id).unwrap()[0].station_id;
        let build = |w: &mut World, a, b, name: &str| {
            w.build_alignment(
                owner,
                name.into(),
                vec![
                    GeoPoint {
                        latitude: 35.,
                        longitude: a,
                    },
                    GeoPoint {
                        latitude: 35.,
                        longitude: b,
                    },
                ],
            )
            .unwrap()
        };
        let second = build(&mut w, 139.02, 139.04, "middle");
        let third = build(&mut w, 139.04, 139.06, "end");
        let platform = w
            .place_station_on_alignment(owner, "D".into(), third, 0.4, 100., 10.)
            .unwrap();
        let boundary = |left, right| AlignmentConnection {
            a: AlignmentEndpointRef {
                alignment_id: left,
                endpoint: AlignmentEndpoint::End,
            },
            b: AlignmentEndpointRef {
                alignment_id: right,
                endpoint: AlignmentEndpoint::Start,
            },
        };
        let joins = vec![boundary(base, second), boundary(second, third)];
        for j in &joins {
            w.connect_alignment_endpoints(owner, j.a, j.b).unwrap();
        }
        let segments = vec![RouteSegment::ConnectedAlignments {
            from: first,
            to: platform,
            connections: joins,
        }];
        let preview = w.preview_service_path(first, segments.clone()).unwrap();
        let stop_ref = preview.occurrences.last().unwrap().reference.clone();
        config.service_id = w
            .add_service_path(
                owner,
                "connected".into(),
                first,
                segments,
                vec![StationOccurrenceRef::Start, stop_ref],
            )
            .unwrap();
        let spec = w.compile_operation(&config).unwrap();
        assert_eq!(spec.legs.len(), 2);
        let a = &spec.legs[0];
        let b = &spec.legs[1];
        assert_eq!(a.to, platform.0.to_string());
        assert_eq!(b.to, first.0.to_string());
        assert!(a
            .sections
            .iter()
            .any(|s| s.resources.iter().any(|id| id.starts_with("rail:join:"))));
        assert!(b
            .sections
            .iter()
            .any(|s| s.resources.iter().any(|id| id.starts_with("rail:join:"))));
        assert_eq!(
            a.sections
                .iter()
                .filter_map(lane_resource)
                .collect::<Vec<_>>(),
            b.sections
                .iter()
                .rev()
                .filter_map(lane_resource)
                .collect::<Vec<_>>()
        );

        // A turnback can use different lanes away from the terminal. At this
        // declared Y transition the out/return lane choices have real incidence.
        let mut dual_lead = w.snapshot().clone();
        dual_lead.alignments[0].infrastructure = vec![SectionInfrastructure {
            track_count: 2,
            ..Default::default()
        }];
        let dual_lead = World::new(dual_lead).unwrap();
        let spec = dual_lead.compile_operation(&config).unwrap();
        assert_eq!(spec.legs.len(), 2);
        let out = &spec.legs[0].sections;
        let back = &spec.legs[1].sections;
        assert!(lane_resource(out.first().unwrap())
            .unwrap()
            .ends_with(":lane:0"));
        assert!(lane_resource(back.last().unwrap())
            .unwrap()
            .ends_with(":lane:1"));
        assert_eq!(
            lane_resource(out.last().unwrap()),
            lane_resource(back.first().unwrap())
        );
    }
    fn straight_occupancy_fixture() -> (World, Vec<OperationServiceConfig>) {
        let mut w = World::default();
        let player = w.add_player("QA".into()).unwrap();
        let company = w.add_company(player, "QA railway".into()).unwrap();
        let make_points = |latitude| {
            (0..4)
                .map(|i| GeoPoint {
                    latitude,
                    longitude: 139.0 + 0.0044 * i as f64,
                })
                .collect::<Vec<_>>()
        };
        let main = w
            .build_alignment(company, "main corridor".into(), make_points(35.0))
            .unwrap();
        let parallel = w
            .build_alignment(company, "separate corridor".into(), make_points(35.003))
            .unwrap();
        let mut d = w.snapshot().clone();
        for a in &mut d.alignments {
            a.anchor_indices = Some(vec![0, 1, 2, 3]);
        }
        w.replace(d).unwrap();

        let mut station = |name: &str, line, position| {
            w.place_station_on_alignment(company, name.into(), line, position, 60., 10.)
                .unwrap()
        };
        let a = station("A", main, 0.10);
        let b = station("B", main, 0.23);
        let _c = station("C", main, 0.50);
        let d = station("D", main, 0.90);
        let e = station("E", parallel, 0.10);
        let f = station("F", parallel, 0.90);
        let mut add_service = |name: &str, line, from, to| {
            let segments = vec![RouteSegment::Alignment {
                alignment_id: line,
                from,
                to,
            }];
            let p = w.preview_service_path(from, segments.clone()).unwrap();
            let stops = vec![
                p.occurrences[0].reference.clone(),
                p.occurrences.last().unwrap().reference.clone(),
            ];
            let service_id = w
                .add_service_path(company, name.into(), from, segments, stops)
                .unwrap();
            OperationServiceConfig {
                service_id,
                timetable: Timetable {
                    first_departure_ms: 0,
                    last_departure_ms: 0,
                    headway_ms: 60000,
                },
                vehicle: Vehicle {
                    max_speed_kmh: 72.,
                    acceleration_mps2: 1.,
                    braking_mps2: 1.,
                    length_m: 40.,
                },
                dwell_ms: 10000,
            }
        };
        let configs = vec![
            add_service("leader", main, a, d),
            add_service("short opposing", main, b, a),
            add_service("separate control", parallel, f, e),
        ];
        (w, configs)
    }

    fn inline_platform_fixture(
        dwell_ms: u64,
        shared_terminal: bool,
    ) -> (World, Vec<OperationServiceConfig>) {
        let (world, mut configs) = straight_occupancy_fixture();
        let mut draft = world.snapshot().clone();
        draft.alignments[0].infrastructure = vec![
            SectionInfrastructure {
                track_count: 2,
                ..Default::default()
            };
            3
        ];
        draft.alignments[1].infrastructure = draft.alignments[0].infrastructure.clone();
        let company = draft.alignments[0].owner;
        let mut world = World::new(draft).unwrap();
        configs.truncate(1);
        let base = configs[0].clone();
        configs[0].dwell_ms = dwell_ms;
        let original = world
            .data
            .service_paths
            .iter()
            .find(|path| path.service_id == base.service_id)
            .unwrap();
        let RouteSegment::Alignment {
            alignment_id,
            from,
            to,
        } = original.legs[0].segment
        else {
            panic!("fixture route")
        };
        let approach = world
            .data
            .station_placements
            .iter()
            .find(|p| p.alignment_id == alignment_id && p.position == 0.50)
            .unwrap()
            .station_id;
        let mut add = |name: &str, from, to, timetabled_at| {
            let segments = vec![RouteSegment::Alignment {
                alignment_id,
                from,
                to,
            }];
            let preview = world.preview_service_path(from, segments.clone()).unwrap();
            let stops = vec![
                preview.occurrences[0].reference.clone(),
                preview.occurrences.last().unwrap().reference.clone(),
            ];
            let service_id = world
                .add_service_path(company, name.into(), from, segments, stops)
                .unwrap();
            OperationServiceConfig {
                service_id,
                timetable: Timetable {
                    first_departure_ms: timetabled_at,
                    last_departure_ms: timetabled_at,
                    ..base.timetable.clone()
                },
                dwell_ms: 0,
                vehicle: base.vehicle.clone(),
            }
        };
        configs.push(add(
            "same direction follower",
            from,
            if shared_terminal { to } else { approach },
            1000,
        ));
        configs.push(add(
            "opposite physical lane from shared station",
            to,
            from,
            1000,
        ));
        (world, configs)
    }

    #[test]
    fn idle_timetable_edit_retains_legacy_progressive_instead_of_selecting_inline_dwell() {
        let (mut world, mut configs) = inline_platform_fixture(30_000, true);
        for config in &mut configs {
            config.timetable.first_departure_ms += 500_000;
            config.timetable.last_departure_ms += 500_000;
        }
        world.configure_operation(configs.clone()).unwrap();
        assert_eq!(world.operation_save().unwrap().runtime.format, 5);
        let specs = configs
            .iter()
            .map(|config| world.compile_operation(config))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let legacy = crate::operation::OperationRuntime::new_with_progressive_rail(specs).unwrap();
        let runtime = &mut world.operation_mut().unwrap().runtime;
        *runtime = legacy;
        runtime.advance_to(200_000, 128).unwrap();
        let mut reopened = World::new(world.snapshot().clone()).unwrap();
        reopened.restore_operation(world.operation_save()).unwrap();
        let unaffected = configs[0].service_id.0.to_string();
        let run_before = reopened
            .operation()
            .unwrap()
            .runtime
            .service_spec(&unaffected)
            .unwrap()
            .clone();
        let target = configs[2].service_id.0.to_string();
        configs[2].timetable.first_departure_ms += 1_000;
        configs[2].timetable.last_departure_ms += 1_000;
        reopened
            .reconfigure_idle_operation(configs.clone(), vec![], &target)
            .unwrap();
        let after = reopened.operation_save().unwrap();
        assert_eq!(after.runtime.format, 3);
        assert_eq!(after.runtime.now_ms, 200_000);
        assert_eq!(after.configs, configs);
        assert_eq!(
            reopened
                .operation()
                .unwrap()
                .runtime
                .service_spec(&unaffected),
            Some(&run_before)
        );
        let mut restored = World::new(reopened.snapshot().clone()).unwrap();
        restored.restore_operation(Some(after)).unwrap();
        assert_eq!(restored.operation_save(), reopened.operation_save());
    }

    #[test]
    fn inline_platform_dwell_blocks_only_the_occupied_physical_lane() {
        use crate::operation::TrainState;

        let dwell = 30_000;
        let (mut world, configs) = inline_platform_fixture(dwell, true);
        world.configure_operation(configs.clone()).unwrap();
        let specs = configs
            .iter()
            .map(|config| world.compile_operation(config))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let mut legacy = world.clone();
        legacy.operation_mut().unwrap().runtime =
            crate::operation::OperationRuntime::new_with_progressive_rail(specs).unwrap();
        let legacy_saved = legacy.operation_save().unwrap();
        let mut legacy_loaded = World::new(legacy.snapshot().clone()).unwrap();
        legacy_loaded
            .restore_operation(Some(legacy_saved.clone()))
            .unwrap();
        assert_eq!(legacy_loaded.operation_save(), Some(legacy_saved));
        assert_eq!(legacy_loaded.operation_save().unwrap().runtime.format, 3);
        let ids: Vec<_> = configs.iter().map(|c| c.service_id.0.to_string()).collect();
        let advance = |world: &mut World, at| {
            world
                .operation_mut()
                .unwrap()
                .runtime
                .advance_to(at, 100)
                .unwrap()
        };
        let state = |world: &World, id: &str| {
            world
                .operation()
                .unwrap()
                .runtime
                .trains()
                .find(|train| train.key.service_id == id)
                .unwrap()
                .state
                .clone()
        };
        advance(&mut world, 1000);
        let TrainState::Moving {
            arrival_ms,
            release_ms,
            ..
        } = state(&world, &ids[0])
        else {
            panic!("leader must depart")
        };
        assert_eq!((arrival_ms, release_ms), (68_094, 98_094));
        advance(&mut legacy_loaded, arrival_ms + dwell - 1);
        assert!(matches!(
            state(&legacy_loaded, &ids[1]),
            TrainState::Moving {
                entered_ms: 70_094,
                ..
            }
        ));
        let replay = |w: &World| {
            w.operation()
                .unwrap()
                .runtime
                .preview_passenger_replay(300_000, 100)
                .unwrap()
        };
        let old_replay = replay(&legacy);
        let new_replay = replay(&world);
        let leg = |r: &crate::operation::PassengerReplay, service: &str| {
            r.completed_legs
                .iter()
                .find(|l| l.train.service_id == service)
                .map(|l| (l.entered_ms, l.arrived_ms, l.cumulative_reservation_wait_ms))
                .unwrap()
        };
        assert_eq!(leg(&new_replay, &ids[1]), (98_094, 166_188, 97_094));
        assert_eq!(leg(&old_replay, &ids[1]), (70_094, 138_188, 69_094));
        assert_eq!(leg(&old_replay, &ids[2]), leg(&new_replay, &ids[2]));
        assert_eq!(leg(&new_replay, &ids[0]), (0, 68_094, 0));
        assert_eq!(world.operation().unwrap().runtime.now_ms(), 1_000);
        assert!(matches!(
            state(&world, &ids[2]),
            TrainState::Moving {
                entered_ms: 1000,
                ..
            }
        ));
        advance(&mut world, arrival_ms + dwell - 1);
        assert!(matches!(state(&world, &ids[1]), TrainState::Waiting { .. }));
        advance(&mut world, arrival_ms + dwell);
        assert!(matches!(
            state(&world, &ids[1]),
            TrainState::Moving { entered_ms, .. } if entered_ms == arrival_ms+dwell
        ));
    }

    #[test]
    fn inline_terminal_dwell_releases_a_proven_clear_approach_at_arrival() {
        let (mut world, configs) = inline_platform_fixture(30_000, false);
        let follower_id = configs[1].service_id.0.to_string();
        world.configure_operation(configs).unwrap();
        let r = &world.operation().unwrap().runtime;
        let replay = r.preview_passenger_replay(300_000, 100).unwrap();
        let follower = replay
            .completed_legs
            .iter()
            .find(|leg| leg.train.service_id == follower_id)
            .unwrap();
        // Follower leaves on the same event the leader reaches its terminal
        // while terminal dwell still keeps the *last* physical block occupied.
        assert_eq!(follower.entered_ms, 68_094);
        assert_eq!(follower.cumulative_reservation_wait_ms, 67_094);
    }

    #[test]
    fn straight_rail_world_conflict_reopens_waiting_and_replays_identically() {
        use crate::operation::TrainState;

        let (mut world, configs) = straight_occupancy_fixture();
        let mut format2 = world.clone();
        format2.configure_operation(configs.clone()).unwrap();
        let legacy_specs = configs
            .iter()
            .map(|c| format2.compile_operation(c))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        format2.operation_mut().unwrap().runtime =
            crate::operation::OperationRuntime::new(legacy_specs).unwrap();
        world
            .configure_straight_rail_occupancy_qa(configs.clone())
            .unwrap();
        let ids = configs
            .iter()
            .map(|c| c.service_id.0.to_string())
            .collect::<Vec<_>>();
        let world_at = |w: &mut World, at| {
            w.operation_mut()
                .unwrap()
                .runtime
                .advance_to(at, 100)
                .unwrap()
        };
        let state = |w: &World, id: &str| {
            w.operation()
                .unwrap()
                .runtime
                .trains()
                .find(|t| t.key.service_id == id)
                .unwrap()
                .state
                .clone()
        };
        world_at(&mut world, 1000);
        world_at(&mut format2, 1000);
        assert!(matches!(
            state(&world, &ids[0]),
            TrainState::Moving { entered_ms: 0, .. }
        ));
        assert!(matches!(
            state(&world, &ids[1]),
            TrainState::Waiting { ready_ms: 0 }
        ));
        assert!(matches!(
            state(&world, &ids[2]),
            TrainState::Moving { entered_ms: 0, .. }
        ));

        // Reopen the two serialized parts of a World operation snapshot using
        // the same validator, with internal QA permission. The public World
        // restore entry (and hence ordinary file loading) does not grant it.
        let bytes = serde_json::to_vec(&(world.snapshot(), world.operation_save())).unwrap();
        let (draft, saved): (WorldDraft, Option<WorldOperationSave>) =
            serde_json::from_slice(&bytes).unwrap();
        let mut reopened = World::new(draft).unwrap();
        assert_eq!(
            reopened.restore_operation(saved.clone()),
            Err(OperationError::InvalidClock)
        );
        assert!(reopened.operation().is_none());
        reopened.restore_operation_checked(saved, true).unwrap();
        assert_eq!(reopened.operation_save(), world.operation_save());

        let mut scan = world.clone();
        let mut departed = None;
        for target in (2000..100000).step_by(1000) {
            world_at(&mut scan, target);
            if let TrainState::Moving { entered_ms, .. } = state(&scan, &ids[1]) {
                departed = Some(entered_ms);
                break;
            }
        }
        let entry = departed.expect("short opposing train never left");
        let runtime = &world.operation().unwrap().runtime;
        let leader = runtime.service_spec(&ids[0]).unwrap();
        let sections = &leader.legs[0].sections;
        // The adapter splits a physical resource at intervening pass-through
        // station samples. Use the last occurrence of the same lane, then
        // the first trustworthy sampled boundary beyond tail clearance.
        let first_lane = lane_resource(&sections[0]).unwrap();
        let mut cumulative_m = 0.0;
        let mut boundaries = Vec::new();
        let mut last_first_lane_m = 0.0;
        for section in sections {
            cumulative_m += section.length_m;
            boundaries.push(cumulative_m);
            if lane_resource(section) == Some(first_lane) {
                last_first_lane_m = cumulative_m;
            }
        }
        let boundary_m = boundaries
            .into_iter()
            .find(|meters| *meters > last_first_lane_m + leader.vehicle.length_m)
            .unwrap();
        let maximum_mps = leader.vehicle.max_speed_kmh / 3.6;
        assert!(entry as f64 >= 1_000.0 * boundary_m / maximum_mps);
        world_at(&mut world, entry - 1);
        assert!(matches!(state(&world, &ids[1]), TrainState::Waiting { .. }));
        world_at(&mut world, entry);
        world_at(&mut format2, entry);
        assert!(
            matches!(state(&world, &ids[0]), TrainState::Moving { arrival_ms, .. } if arrival_ms > entry)
        );
        assert!(
            matches!(state(&world, &ids[1]), TrainState::Moving { entered_ms, .. } if entered_ms == entry)
        );
        assert!(matches!(
            state(&format2, &ids[1]),
            TrainState::Waiting { .. }
        ));
        let expected = world_at(&mut reopened, entry).arrivals;
        assert!(expected.is_empty());
        assert_eq!(reopened.operation_save(), world.operation_save());
        assert_eq!(
            world_at(&mut reopened, 500_000),
            world_at(&mut world, 500_000)
        );
        assert_eq!(reopened.operation_save(), world.operation_save());

        // At legacy mode's saved time the second service still waits. Reopen
        // using the production validator and ensure there is no upgrade.
        let mut old = World::new(format2.snapshot().clone()).unwrap();
        old.restore_operation(format2.operation_save()).unwrap();
        assert_eq!(old.operation_save(), format2.operation_save());
        assert_eq!(old.operation_save().unwrap().runtime.format, 2);
    }

    #[test]
    fn qualified_terminal_hold_is_saved_and_reopens_only_on_safe_geometry() {
        use crate::operation::TrainState;

        let (mut world, mut configs) = straight_occupancy_fixture();
        configs[0].dwell_ms = 30_000;
        world.configure_operation(configs.clone()).unwrap();
        let leader = configs[0].service_id.0.to_string();
        let rival = configs[1].service_id.0.to_string();
        let tick = |w: &mut World, at| {
            w.operation_mut()
                .unwrap()
                .runtime
                .advance_to(at, 100)
                .unwrap()
        };
        let t = |w: &World, id: &str| {
            w.operation()
                .unwrap()
                .runtime
                .trains()
                .find(|t| t.key.service_id == id)
                .unwrap()
                .clone()
        };
        tick(&mut world, 0);
        let outbound = t(&world, &leader);
        let TrainState::Moving {
            arrival_ms,
            release_ms,
            ..
        } = outbound.state
        else {
            panic!("fixture leader must depart")
        };
        assert_eq!(release_ms, arrival_ms + 30_000);
        tick(&mut world, release_ms - 1);
        assert!(matches!(
            t(&world, &rival).state,
            TrainState::Waiting { .. }
        ));
        let bytes = serde_json::to_vec(&(world.snapshot(), world.operation_save())).unwrap();
        let (draft, saved): (WorldDraft, Option<WorldOperationSave>) =
            serde_json::from_slice(&bytes).unwrap();
        assert_eq!(saved.as_ref().unwrap().runtime.format, 4);
        let mut reopened = World::new(draft.clone()).unwrap();
        reopened.restore_operation(saved.clone()).unwrap();
        assert_eq!(world.operation_save(), reopened.operation_save());
        let mut changed = draft;
        changed.station_placements[0].length_m = 32.;
        let mut stale = World::new(changed).unwrap();
        assert_eq!(
            stale.restore_operation(saved),
            Err(OperationError::InvalidPath)
        );
        stale.retain_operation_from(&world);
        assert!(stale.operation().unwrap().invalidated);
        assert!(stale.operation().unwrap().runtime.paused());
        let frozen = stale.operation_save().unwrap();
        let mut reopened_frozen = World::new(stale.snapshot().clone()).unwrap();
        reopened_frozen
            .restore_operation(Some(frozen.clone()))
            .unwrap();
        assert_eq!(reopened_frozen.operation_save(), Some(frozen.clone()));
        let mut bad_frozen = frozen;
        bad_frozen.configs[0].dwell_ms += 1;
        assert_eq!(
            reopened_frozen.restore_operation(Some(bad_frozen)),
            Err(OperationError::InvalidPath)
        );
        tick(&mut world, release_ms);
        let returning = t(&world, &leader);
        assert_eq!(returning.key, outbound.key);
        assert_eq!(returning.leg, 1);
        assert!(matches!(
            t(&world, &rival).state,
            TrainState::Waiting { .. }
        ));
        tick(&mut reopened, release_ms);
        assert_eq!(world.operation_save(), reopened.operation_save());
        assert_eq!(tick(&mut world, 350_000), tick(&mut reopened, 350_000));
        assert_eq!(world.operation_save(), reopened.operation_save());

        // Outside the geometry qualification the unchanged legacy model remains.
        configs[0].vehicle.length_m = 81.;
        world.configure_operation(configs).unwrap();
        assert_eq!(world.operation_save().unwrap().runtime.format, 2);
    }

    #[test]
    fn straight_rail_qa_rejects_topology_changes_and_stale_specs() {
        let (mut world, configs) = straight_occupancy_fixture();
        world
            .configure_straight_rail_occupancy_qa(configs.clone())
            .unwrap();
        let mut saved = world.operation_save().unwrap();
        let mut stale = World::new(world.snapshot().clone()).unwrap();
        saved.runtime.services[0].legs.pop(); // legacy outbound-only cannot be a format3 grandfather
        assert_eq!(
            stale.restore_operation_checked(Some(saved), true),
            Err(OperationError::InvalidPath)
        );
        let mut d = world.snapshot().clone();
        // Reposition the parallel corridor and its stations together. Its
        // separation is below this pilot's proof margin, even with no join.
        d.alignments[1]
            .points
            .iter_mut()
            .for_each(|p| p.latitude = 35.0005);
        let separate = d.alignments[1].id;
        let stations: Vec<_> = d
            .station_placements
            .iter()
            .filter(|p| p.alignment_id == separate)
            .map(|p| p.station_id)
            .collect();
        d.stations
            .iter_mut()
            .filter(|s| stations.contains(&s.id))
            .for_each(|s| s.latitude = 35.0005);
        let mut changed = World::new(d).unwrap();
        assert_eq!(
            changed.configure_straight_rail_occupancy_qa(configs),
            Err(OperationError::InvalidPath)
        );
        assert_eq!(
            changed.restore_operation_checked(world.operation_save(), true),
            Err(OperationError::InvalidClock)
        );
        assert!(changed.operation().is_none());
    }
}
