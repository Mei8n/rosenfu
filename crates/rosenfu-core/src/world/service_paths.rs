//! Stable stop references for explicit, independently owned service paths.
use super::*;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceLeg {
    #[serde(with = "decimal_counter")]
    pub key: u64,
    pub segment: RouteSegment,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StationOccurrenceRef {
    Start,
    OnLeg {
        #[serde(with = "decimal_counter")]
        leg_key: u64,
        station_id: StationId,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServicePath {
    pub service_id: ServiceId,
    #[serde(with = "decimal_counter")]
    pub next_leg_key: u64,
    pub legs: Vec<ServiceLeg>,
    pub stops: Vec<StationOccurrenceRef>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServicePathOccurrence {
    pub station_id: StationId,
    pub reference: StationOccurrenceRef,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServicePathPreview {
    pub legs: Vec<ServiceLeg>,
    pub occurrences: Vec<ServicePathOccurrence>,
    pub requires_stop_selection: bool,
}
fn resolve(
    data: &WorldDraft,
    start: StationId,
    legs: &[ServiceLeg],
) -> Result<Vec<ServicePathOccurrence>, WorldError> {
    if legs.is_empty() || legs.len() > 256 || !data.stations.iter().any(|s| s.id == start) {
        return Err(WorldError::InvalidServicePath);
    }
    let mut keys = BTreeSet::new();
    let mut occurrences = vec![ServicePathOccurrence {
        station_id: start,
        reference: StationOccurrenceRef::Start,
    }];
    let mut current = start;
    for leg in legs {
        if leg.key == 0 || !keys.insert(leg.key) {
            return Err(WorldError::InvalidServicePath);
        }
        let span = route_paths::resolve_path(data, current, std::slice::from_ref(&leg.segment))
            .map_err(|_| WorldError::InvalidServicePath)?;
        for station_id in span.into_iter().skip(1) {
            occurrences.push(ServicePathOccurrence {
                station_id,
                reference: StationOccurrenceRef::OnLeg {
                    leg_key: leg.key,
                    station_id,
                },
            });
            current = station_id;
        }
    }
    Ok(occurrences)
}
impl WorldDraft {
    pub(super) fn validate_service_paths(&self) -> Result<(), WorldError> {
        let mut ids = BTreeSet::new();
        for path in &self.service_paths {
            let service = self
                .services
                .iter()
                .find(|s| s.id == path.service_id)
                .ok_or(WorldError::InvalidServicePath)?;
            if !ids.insert(path.service_id)
                || !service.tracks.is_empty()
                || !service.stops.is_empty()
                || path.next_leg_key == 0
                || path.legs.iter().any(|l| l.key >= path.next_leg_key)
            {
                return Err(WorldError::InvalidServicePath);
            }
            let occurrences = resolve(self, service.start, &path.legs)?;
            if path.stops.first() != Some(&StationOccurrenceRef::Start)
                || path.stops.last() != occurrences.last().map(|o| &o.reference)
                || path.stops.len() < 2
            {
                return Err(WorldError::InvalidServicePath);
            }
            let mut cursor = 0;
            for stop in &path.stops {
                cursor += occurrences[cursor..]
                    .iter()
                    .position(|o| &o.reference == stop)
                    .ok_or(WorldError::InvalidServicePath)?
                    + 1;
            }
        }
        Ok(())
    }
    /// This initial API does not edit saved paths or move referenced infrastructure.
    /// Names and platform dimensions may change; added interior stations may not
    /// retarget existing stops. Deleting an entire service is still representable.
    pub(super) fn preserve_service_paths(&self, old: &Self) -> Result<(), WorldError> {
        for path in &old.service_paths {
            let Some(service) = self.services.iter().find(|s| s.id == path.service_id) else {
                continue;
            };
            let old_service = old
                .services
                .iter()
                .find(|s| s.id == path.service_id)
                .ok_or(WorldError::InvalidServicePath)?;
            if self
                .service_paths
                .iter()
                .find(|p| p.service_id == path.service_id)
                != Some(path)
                || service.start != old_service.start
            {
                return Err(WorldError::ServicePathEditUnsupported);
            }
            let old_occurrences = resolve(old, old_service.start, &path.legs)?;
            for occurrence in old_occurrences {
                let id = occurrence.station_id;
                // Initial version protects every traversed station, including passes.
                if let Some(new_station) = self.stations.iter().find(|s| s.id == id) {
                    let old_station = old
                        .stations
                        .iter()
                        .find(|s| s.id == id)
                        .ok_or(WorldError::InvalidServicePath)?;
                    if new_station.latitude != old_station.latitude
                        || new_station.longitude != old_station.longitude
                    {
                        return Err(WorldError::ServicePathEditUnsupported);
                    }
                    let old_p = old.station_placements.iter().find(|p| p.station_id == id);
                    let new_p = self.station_placements.iter().find(|p| p.station_id == id);
                    if old_p.map(|p| (p.alignment_id, p.position))
                        != new_p.map(|p| (p.alignment_id, p.position))
                    {
                        return Err(WorldError::ServicePathEditUnsupported);
                    }
                } else {
                    return Err(WorldError::ServicePathEditUnsupported);
                }
            }
            for leg in &path.legs {
                match leg.segment {
                    RouteSegment::Alignment { alignment_id, .. } => {
                        if self
                            .alignments
                            .iter()
                            .find(|a| a.id == alignment_id)
                            .map(|a| &a.points)
                            != old
                                .alignments
                                .iter()
                                .find(|a| a.id == alignment_id)
                                .map(|a| &a.points)
                        {
                            return Err(WorldError::ServicePathEditUnsupported);
                        }
                    }
                    RouteSegment::Track { track_id } => {
                        if self
                            .tracks
                            .iter()
                            .find(|t| t.id == track_id)
                            .map(|t| (t.from, t.to))
                            != old
                                .tracks
                                .iter()
                                .find(|t| t.id == track_id)
                                .map(|t| (t.from, t.to))
                        {
                            return Err(WorldError::ServicePathEditUnsupported);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
impl World {
    pub fn preview_service_path(
        &self,
        start: StationId,
        segments: Vec<RouteSegment>,
    ) -> Result<ServicePathPreview, WorldError> {
        let legs = segments
            .into_iter()
            .enumerate()
            .map(|(i, segment)| ServiceLeg {
                key: i as u64 + 1,
                segment,
            })
            .collect::<Vec<_>>();
        let occurrences = resolve(&self.data, start, &legs)?;
        Ok(ServicePathPreview {
            legs,
            occurrences,
            requires_stop_selection: true,
        })
    }
    pub fn add_service_path(
        &mut self,
        owner: CompanyId,
        name: String,
        start: StationId,
        segments: Vec<RouteSegment>,
        stops: Vec<StationOccurrenceRef>,
    ) -> Result<ServiceId, WorldError> {
        let preview = self.preview_service_path(start, segments)?;
        self.insert(|data, id| {
            data.services.push(OperatingService {
                id: ServiceId(id),
                owner,
                name,
                start,
                tracks: vec![],
                stops: vec![],
            });
            data.service_paths.push(ServicePath {
                service_id: ServiceId(id),
                next_leg_key: preview.legs.len() as u64 + 1,
                legs: preview.legs,
                stops,
            });
        })
        .map(ServiceId)
    }
    pub fn service_path_occurrences(
        &self,
        id: ServiceId,
    ) -> Result<Vec<ServicePathOccurrence>, WorldError> {
        let service = self
            .data
            .services
            .iter()
            .find(|s| s.id == id)
            .ok_or(WorldError::InvalidServicePath)?;
        let path = self
            .data
            .service_paths
            .iter()
            .find(|p| p.service_id == id)
            .ok_or(WorldError::InvalidServicePath)?;
        resolve(&self.data, service.start, &path.legs)
    }
}
