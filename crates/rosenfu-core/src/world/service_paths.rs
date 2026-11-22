//! Stable stop references for explicit, independently owned service paths.
use super::*;
use std::{cell::OnceCell, collections::BTreeMap};
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
/// Ephemeral index for one validation pass. The common no-tram world does
/// not scan station placements or classify individual service paths.
struct TramServiceIndex<'a> {
    data: &'a WorldDraft,
    alignments: BTreeSet<AlignmentId>,
    platforms: OnceCell<BTreeMap<StationId, AlignmentId>>,
}
impl<'a> TramServiceIndex<'a> {
    fn new(data: &'a WorldDraft) -> Self {
        let alignments = data
            .alignments
            .iter()
            .filter(|a| a.track_type == TrackType::Tram)
            .map(|a| a.id)
            .collect::<BTreeSet<_>>();
        Self {
            data,
            alignments,
            platforms: OnceCell::new(),
        }
    }

    /// The caller has already resolved the complete path and verified every
    /// selected reference. Consult the physical platform, not its station group.
    fn classify(&self, path: &ServicePath, start: StationId) -> Result<bool, WorldError> {
        if self.alignments.is_empty() {
            return Ok(false);
        }
        let mut tram_path = BTreeSet::new();
        let mut mixed = false;
        let mut see = |id| {
            if self.alignments.contains(&id) {
                tram_path.insert(id);
            } else {
                mixed = true;
            }
        };
        for leg in &path.legs {
            match &leg.segment {
                RouteSegment::Alignment { alignment_id, .. } => see(*alignment_id),
                RouteSegment::ConnectedAlignments { connections, .. } => {
                    for link in connections {
                        see(link.a.alignment_id);
                        see(link.b.alignment_id);
                    }
                }
            }
        }
        if tram_path.is_empty() {
            return Ok(false);
        }
        let platforms = self.platforms.get_or_init(|| {
            self.data
                .station_placements
                .iter()
                .filter(|p| self.alignments.contains(&p.alignment_id))
                .map(|p| (p.station_id, p.alignment_id))
                .collect()
        });
        if !path.stops.iter().any(|stop| {
            let id = match stop {
                StationOccurrenceRef::Start => start,
                StationOccurrenceRef::OnLeg { station_id, .. } => *station_id,
            };
            platforms
                .get(&id)
                .is_some_and(|alignment| tram_path.contains(alignment))
        }) {
            return Err(WorldError::TramTrackRequiresStop);
        }
        Ok(mixed)
    }
}

pub(super) fn is_mixed_tram_service(
    data: &WorldDraft,
    path: &ServicePath,
    start: StationId,
) -> Result<bool, WorldError> {
    TramServiceIndex::new(data).classify(path, start)
}
impl WorldDraft {
    /// Only for the endpoint-extension command, which preserves old geometry and
    /// coordinates by construction. No service definitions or occurrences may change.
    pub(super) fn preserve_service_traversals(&self, old: &Self) -> Result<(), WorldError> {
        if self.services != old.services || self.service_paths != old.service_paths {
            return Err(WorldError::ServicePathEditUnsupported);
        }
        for path in &old.service_paths {
            let service = old
                .services
                .iter()
                .find(|s| s.id == path.service_id)
                .ok_or(WorldError::InvalidServicePath)?;
            if resolve(self, service.start, &path.legs)? != resolve(old, service.start, &path.legs)?
            {
                return Err(WorldError::ServicePathEditUnsupported);
            }
        }
        Ok(())
    }
    pub(super) fn validate_service_paths(&self) -> Result<(), WorldError> {
        if self.service_paths.is_empty() {
            return Ok(());
        }
        let tram = TramServiceIndex::new(self);
        let mut ids = BTreeSet::new();
        for path in &self.service_paths {
            let service = self
                .services
                .iter()
                .find(|s| s.id == path.service_id)
                .ok_or(WorldError::InvalidServicePath)?;
            if !ids.insert(path.service_id)
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
            tram.classify(path, service.start)?;
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
                    RouteSegment::ConnectedAlignments {
                        ref connections, ..
                    } => {
                        for id in connections
                            .iter()
                            .flat_map(|link| [link.a.alignment_id, link.b.alignment_id])
                        {
                            if self
                                .alignments
                                .iter()
                                .find(|a| a.id == id)
                                .map(|a| &a.points)
                                != old
                                    .alignments
                                    .iter()
                                    .find(|a| a.id == id)
                                    .map(|a| &a.points)
                            {
                                return Err(WorldError::ServicePathEditUnsupported);
                            }
                        }
                    }
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
    /// Edit only a saved path's label and stop policy. Physical geometry, leg keys,
    /// service identity, and the allocator are never changed by this operation.
    pub fn update_service_path(
        &mut self,
        owner: CompanyId,
        id: ServiceId,
        name: String,
        stops: Vec<StationOccurrenceRef>,
    ) -> Result<(), WorldError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(WorldError::EmptyName("service (1–80 characters)"));
        }
        let mut candidate = self.data.clone();
        let service = candidate
            .services
            .iter_mut()
            .find(|s| s.id == id && s.owner == owner)
            .ok_or(WorldError::MissingReference("owned service"))?;
        let path = candidate
            .service_paths
            .iter_mut()
            .find(|p| p.service_id == id)
            .ok_or(WorldError::InvalidServicePath)?;
        service.name = name.to_owned();
        path.stops = stops;
        candidate.validate()?;
        self.data = candidate;
        Ok(())
    }
    /// Preview a replacement with fresh keys. Retired leg identities are never reused.
    pub fn preview_service_path_replacement(
        &self,
        owner: CompanyId,
        id: ServiceId,
        start: StationId,
        segments: Vec<RouteSegment>,
    ) -> Result<ServicePathPreview, WorldError> {
        if !self
            .data
            .services
            .iter()
            .any(|s| s.id == id && s.owner == owner)
        {
            return Err(WorldError::MissingReference("owned service"));
        }
        let path = self
            .data
            .service_paths
            .iter()
            .find(|p| p.service_id == id)
            .ok_or(WorldError::InvalidServicePath)?;
        if segments.is_empty() || segments.len() > 256 {
            return Err(WorldError::InvalidServicePath);
        }
        // Reserve the next counter as well as all leg keys before constructing anything.
        path.next_leg_key
            .checked_add(segments.len() as u64)
            .ok_or(WorldError::InvalidServicePath)?;
        let legs = segments
            .into_iter()
            .enumerate()
            .map(|(i, segment)| ServiceLeg {
                key: path.next_leg_key + i as u64,
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

    /// Explicit atomic replacement. The caller must reselect stops from a fresh
    /// preview and supply the saved path it edited; concurrent edits are rejected.
    #[allow(clippy::too_many_arguments)]
    pub fn replace_service_path(
        &mut self,
        owner: CompanyId,
        id: ServiceId,
        expected_path: ServicePath,
        name: String,
        start: StationId,
        segments: Vec<RouteSegment>,
        stops: Vec<StationOccurrenceRef>,
    ) -> Result<(), WorldError> {
        if self.data.service_paths.iter().find(|p| p.service_id == id) != Some(&expected_path) {
            return Err(WorldError::ServicePathEditUnsupported);
        }
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(WorldError::EmptyName("service (1–80 characters)"));
        }
        let preview = self.preview_service_path_replacement(owner, id, start, segments)?;
        let next_leg_key = expected_path
            .next_leg_key
            .checked_add(preview.legs.len() as u64)
            .ok_or(WorldError::InvalidServicePath)?;
        let mut candidate = self.data.clone();
        let service = candidate
            .services
            .iter_mut()
            .find(|s| s.id == id && s.owner == owner)
            .ok_or(WorldError::MissingReference("owned service"))?;
        service.name = name.to_owned();
        service.start = start;
        let path = candidate
            .service_paths
            .iter_mut()
            .find(|p| p.service_id == id)
            .ok_or(WorldError::InvalidServicePath)?;
        *path = ServicePath {
            service_id: id,
            next_leg_key,
            legs: preview.legs,
            stops,
        };
        candidate.validate()?;
        self.data = candidate;
        Ok(())
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
