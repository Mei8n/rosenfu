//! Authoritative physical service path adapter. Never uses screen coordinates.
use super::*;
use crate::operation::{OperationError, RunLeg, Section, ServiceRunSpec, Timetable, Vehicle};

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
    /// Centre-line distances with conservative turnout caps. One directional
    /// block per anchor section (quad track currently uses one lane/direction).
    pub fn compile_operation(
        &self,
        config: &OperationServiceConfig,
    ) -> Result<ServiceRunSpec, OperationError> {
        self.compile_operation_mode(config, true)
    }

    fn compile_operation_mode(
        &self,
        config: &OperationServiceConfig,
        turnback: bool,
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
        )?;
        if let Some(return_legs) = turnback
            .then(|| {
                self.compile_return_direction(path, &occurrences, config.dwell_ms, &runs)
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
                // All paths sharing either turnout endpoint conflict. Conservative
                // at junctions; independent disconnected crossings never conflict.
                let resource = |e: AlignmentEndpointRef| {
                    format!("rail:join:{}:{:?}", e.alignment_id.0, e.endpoint)
                };
                let extra = vec![resource(exit), resource(entry)];
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
        self.compile_rail_direction(&path.legs, &path.stops, occurrences, dwell_ms, true)?;
        let returns = self.compile_rail_direction(
            &reverse.legs,
            &stops,
            &reverse.occurrences,
            dwell_ms,
            true,
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
                .compile_operation_mode(config, false)
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

fn append_sections(
    a: &Alignment,
    from: f64,
    to: f64,
    cap: f64,
    extra: &[String],
    internal: &[InternalTrackMovementGeometry],
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldOperationSave {
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
        let mut specs = configs
            .iter()
            .map(|c| self.compile_operation(c))
            .collect::<Result<Vec<_>, _>>()?;
        for config in &bus_configs {
            specs.push(self.compile_bus_operation(config)?);
        }
        let runtime = if bus_configs.is_empty() && self.terminal_hold_eligible(&configs, &specs) {
            crate::operation::OperationRuntime::new_with_terminal_hold(specs)?
        } else {
            crate::operation::OperationRuntime::new(specs)?
        };
        self.operation = Some(WorldOperation {
            configs,
            bus_configs,
            runtime,
            invalidated: false,
            pending_target: None,
        });
        Ok(())
    }
    pub fn operation_save(&self) -> Option<WorldOperationSave> {
        self.operation.as_ref().map(|o| WorldOperationSave {
            configs: o.configs.clone(),
            bus_configs: o.bus_configs.clone(),
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
        let progressive = saved.runtime.format == 3;
        let holding = saved.runtime.format == 4;
        if saved.runtime.format != 2
            && !holding
            && !(progressive
                && allow_qualified_qa
                && !saved.invalidated
                && saved.bus_configs.is_empty()
                && self.is_straight_rail_occupancy_qa(&saved.configs))
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
                let matches = if progressive || holding {
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
        self.straight_rail_geometry(configs, false)
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

    /// Narrow acceptance gate, not a general level-crossing detector.
    /// Every alignment in the world must be a local, monotone east-west flat
    /// ground single-track corridor; an optional second corridor must be well
    /// separated and neither corridor can be connected to anything else.
    fn is_straight_rail_occupancy_qa(&self, configs: &[OperationServiceConfig]) -> bool {
        self.straight_rail_geometry(configs, true)
    }

    fn straight_rail_geometry(
        &self,
        configs: &[OperationServiceConfig],
        progressive_test: bool,
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
                || a.infrastructure
                    .iter()
                    .any(|s| *s != SectionInfrastructure::default())
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
        self.operation = previous.operation.clone();
        let invalid = self.operation.as_ref().is_some_and(|o| {
            if o.invalidated {
                return false;
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
            w.compile_operation_mode(&c, false).unwrap()
        );
    }

    #[test]
    fn exact_legacy_outbound_is_neither_rewritten_nor_invalidated_by_unrelated_edits() {
        let (mut w, c) = fixture();
        let legacy = w.compile_operation_mode(&c, false).unwrap();
        let mut legacy_runtime =
            crate::operation::OperationRuntime::new(vec![legacy.clone()]).unwrap();
        legacy_runtime.advance_to(1_000, 100).unwrap();
        let saved = WorldOperationSave {
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
            w.compile_operation_mode(&c, false).unwrap()
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
