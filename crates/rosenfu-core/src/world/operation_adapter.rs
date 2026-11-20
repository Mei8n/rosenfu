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
        let movements = self.track_movement_geometries().map_err(|_| invalid())?;
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
        for leg in &path.legs {
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
                let matching: Vec<_> = movements
                    .iter()
                    .filter(|m| m.movement.from.endpoint == exit && m.movement.to.endpoint == entry)
                    .collect();
                if matching.is_empty() {
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
                    if path.stops.contains(&occurrence.reference) {
                        if pending.is_empty() {
                            return Err(invalid());
                        }
                        runs.push(RunLeg {
                            from: last_stop.0.to_string(),
                            to: p.station_id.0.to_string(),
                            sections: std::mem::take(&mut pending),
                            dwell_ms: config.dwell_ms,
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
        Ok(ServiceRunSpec {
            service_id: config.service_id.0.to_string(),
            vehicle: config.vehicle.clone(),
            timetable: config.timetable.clone(),
            legs: runs,
        })
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
        let lane = if tracks == 1 || to > from {
            0
        } else {
            tracks - 1
        };
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
    pub runtime: crate::operation::OperationSave,
    pub invalidated: bool,
}
#[derive(Debug, Clone, PartialEq)]
pub struct WorldOperation {
    pub configs: Vec<OperationServiceConfig>,
    pub runtime: crate::operation::OperationRuntime,
    pub invalidated: bool,
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
        let specs = configs
            .iter()
            .map(|c| self.compile_operation(c))
            .collect::<Result<Vec<_>, _>>()?;
        let runtime = crate::operation::OperationRuntime::new(specs)?;
        self.operation = Some(WorldOperation {
            configs,
            runtime,
            invalidated: false,
        });
        Ok(())
    }
    pub fn operation_save(&self) -> Option<WorldOperationSave> {
        self.operation.as_ref().map(|o| WorldOperationSave {
            configs: o.configs.clone(),
            runtime: o.runtime.save(),
            invalidated: o.invalidated,
        })
    }
    pub fn restore_operation(
        &mut self,
        saved: Option<WorldOperationSave>,
    ) -> Result<(), OperationError> {
        let Some(saved) = saved else {
            self.operation = None;
            return Ok(());
        };
        if !saved.invalidated {
            let specs = saved
                .configs
                .iter()
                .map(|c| self.compile_operation(c))
                .collect::<Result<Vec<_>, _>>()?;
            let map = |v: Vec<ServiceRunSpec>| {
                v.into_iter()
                    .map(|s| (s.service_id.clone(), s))
                    .collect::<std::collections::BTreeMap<_, _>>()
            };
            if map(specs) != map(saved.runtime.services.clone()) {
                return Err(OperationError::InvalidPath);
            }
        }
        let mut runtime = crate::operation::OperationRuntime::restore(saved.runtime)?;
        if saved.invalidated {
            runtime.set_paused(true);
        }
        self.operation = Some(WorldOperation {
            configs: saved.configs,
            runtime,
            invalidated: saved.invalidated,
        });
        Ok(())
    }
    /// Used only by edit transactions/Undo, never world loading or new world.
    /// Invalidated trains remain frozen and visibly require explicit restart.
    pub fn retain_operation_from(&mut self, previous: &World) {
        self.operation = previous.operation.clone();
        let invalid = self.operation.as_ref().is_some_and(|o| {
            !o.invalidated
                && o.configs.iter().any(|config| {
                    self.compile_operation(config).ok().as_ref()
                        != o.runtime
                            .save()
                            .services
                            .iter()
                            .find(|s| s.service_id == config.service_id.0.to_string())
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
        assert_eq!(s.legs.len(), 1);
        assert_eq!(s.legs[0].sections.len(), 2);
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
}
