//! Validated, renderer-independent construction data. Not a stable save format yet.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

macro_rules! id_type {
    ($($name:ident),+) => { $(
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(pub u64);
        impl TryFrom<String> for $name {
            type Error = String;
            fn try_from(value: String) -> Result<Self, Self::Error> {
                let id = value.parse::<u64>().map_err(|_| "invalid decimal ID".to_owned())?;
                if id == 0 || value != id.to_string() { return Err("ID must be a canonical positive decimal string".into()); }
                Ok(Self(id))
            }
        }
        impl From<$name> for String {
            fn from(id: $name) -> Self { id.0.to_string() }
        }
    )+ };
}
id_type!(
    PlayerId,
    CompanyId,
    StationId,
    TrackId,
    RouteId,
    ServiceId,
    AlignmentId
);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Company {
    pub id: CompanyId,
    pub owner: PlayerId,
    pub name: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Station {
    pub id: StationId,
    pub owner: CompanyId,
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Track {
    pub id: TrackId,
    pub owner: CompanyId,
    pub from: StationId,
    pub to: StationId,
}
/// Ordered track traversal. Track infrastructure is independent of this route.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub id: RouteId,
    pub owner: CompanyId,
    pub name: String,
    pub start: StationId,
    pub tracks: Vec<TrackId>,
}
/// An operating path independent of named routes, sharing track infrastructure.
/// Stops are occurrence indices into the traversed stations: 0 is the start,
/// and `tracks.len()` is the final station. Repeated stations remain distinct.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatingService {
    pub id: ServiceId,
    pub owner: CompanyId,
    pub name: String,
    pub start: StationId,
    pub tracks: Vec<TrackId>,
    pub stops: Vec<u32>,
}
/// An editable candidate. Validate before applying or loading it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldDraft {
    #[serde(default)]
    pub source_records: Vec<SourceRecord>,
    #[serde(default)]
    pub alignment_provenance: Vec<AlignmentProvenance>,
    #[serde(default)]
    pub service_paths: Vec<ServicePath>,
    #[serde(default)]
    pub route_paths: Vec<RoutePath>,
    #[serde(default)]
    pub alignments: Vec<Alignment>,
    #[serde(default)]
    pub station_placements: Vec<StationPlacement>,
    #[serde(with = "decimal_counter")]
    pub next_id: u64,
    pub players: Vec<Player>,
    pub companies: Vec<Company>,
    pub stations: Vec<Station>,
    pub tracks: Vec<Track>,
    pub routes: Vec<Route>,
    pub services: Vec<OperatingService>,
}

mod decimal_counter {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        let value = String::deserialize(deserializer)?;
        let counter = value.parse::<u64>().map_err(serde::de::Error::custom)?;
        if counter == 0 || value != counter.to_string() {
            return Err(serde::de::Error::custom("invalid next_id"));
        }
        Ok(counter)
    }
}
impl Default for WorldDraft {
    fn default() -> Self {
        Self {
            route_paths: vec![],
            service_paths: vec![],
            source_records: vec![],
            alignment_provenance: vec![],
            next_id: 1,
            alignments: vec![],
            station_placements: vec![],
            players: vec![],
            companies: vec![],
            stations: vec![],
            tracks: vec![],
            routes: vec![],
            services: vec![],
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldError {
    InvalidId(&'static str),
    EmptyName(&'static str),
    MissingReference(&'static str),
    InvalidCoordinate,
    InvalidAlignment,
    InvalidRoutePath,
    InvalidServicePath,
    InvalidProvenance,
    ProvenanceEditUnsupported,
    ServicePathEditUnsupported,
    InvalidStationPlacement,
    SelfLoop,
    EmptyRoute,
    DisconnectedRoute,
    EmptyService,
    DisconnectedService,
    InvalidServiceStops,
    InvalidCounter,
    IdExhausted,
    UnallocatedId,
}
impl std::fmt::Display for WorldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for WorldError {}

/// Only valid snapshots can enter the running world.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct World {
    data: WorldDraft,
}
impl World {
    pub fn new(data: WorldDraft) -> Result<Self, WorldError> {
        data.validate()?;
        Ok(Self { data })
    }
    fn insert(&mut self, insert: impl FnOnce(&mut WorldDraft, u64)) -> Result<u64, WorldError> {
        let id = self.data.next_id;
        let mut candidate = self.data.clone();
        candidate.next_id = id.checked_add(1).ok_or(WorldError::IdExhausted)?;
        insert(&mut candidate, id);
        candidate.validate()?;
        self.data = candidate;
        Ok(id)
    }
    pub fn add_player(&mut self, name: String) -> Result<PlayerId, WorldError> {
        self.insert(|data, id| {
            data.players.push(Player {
                id: PlayerId(id),
                name,
            })
        })
        .map(PlayerId)
    }
    pub fn add_company(&mut self, owner: PlayerId, name: String) -> Result<CompanyId, WorldError> {
        self.insert(|data, id| {
            data.companies.push(Company {
                id: CompanyId(id),
                owner,
                name,
            })
        })
        .map(CompanyId)
    }
    pub fn add_station(
        &mut self,
        owner: CompanyId,
        name: String,
        latitude: f64,
        longitude: f64,
    ) -> Result<StationId, WorldError> {
        self.insert(|data, id| {
            data.stations.push(Station {
                id: StationId(id),
                owner,
                name,
                latitude,
                longitude,
            })
        })
        .map(StationId)
    }
    pub fn add_track(
        &mut self,
        owner: CompanyId,
        from: StationId,
        to: StationId,
    ) -> Result<TrackId, WorldError> {
        self.insert(|data, id| {
            data.tracks.push(Track {
                id: TrackId(id),
                owner,
                from,
                to,
            })
        })
        .map(TrackId)
    }
    pub fn add_route(
        &mut self,
        owner: CompanyId,
        name: String,
        start: StationId,
        tracks: Vec<TrackId>,
    ) -> Result<RouteId, WorldError> {
        self.insert(|data, id| {
            data.routes.push(Route {
                id: RouteId(id),
                owner,
                name,
                start,
                tracks,
            })
        })
        .map(RouteId)
    }
    pub fn add_service(
        &mut self,
        owner: CompanyId,
        name: String,
        start: StationId,
        tracks: Vec<TrackId>,
        stops: Vec<u32>,
    ) -> Result<ServiceId, WorldError> {
        self.insert(|data, id| {
            data.services.push(OperatingService {
                id: ServiceId(id),
                owner,
                name,
                start,
                tracks,
                stops,
            })
        })
        .map(ServiceId)
    }
    pub fn snapshot(&self) -> &WorldDraft {
        &self.data
    }
    /// Atomic candidate replacement: failures leave the live state unchanged.
    pub fn replace(&mut self, candidate: WorldDraft) -> Result<(), WorldError> {
        candidate.validate()?;
        candidate.preserve_service_paths(&self.data)?;
        candidate.preserve_provenance(&self.data)?;
        if candidate.next_id != self.data.next_id {
            return Err(WorldError::InvalidCounter);
        }
        macro_rules! existing_only {
            ($field:ident) => {
                if candidate
                    .$field
                    .iter()
                    .any(|entry| !self.data.$field.iter().any(|old| old.id == entry.id))
                {
                    return Err(WorldError::UnallocatedId);
                }
            };
        }
        existing_only!(players);
        existing_only!(companies);
        existing_only!(stations);
        existing_only!(tracks);
        existing_only!(routes);
        existing_only!(services);
        existing_only!(alignments);
        self.data = candidate;
        Ok(())
    }
}
fn ids(values: impl Iterator<Item = u64>, entity: &'static str) -> Result<(), WorldError> {
    let mut seen = BTreeSet::new();
    for id in values {
        if id == 0 || !seen.insert(id) {
            return Err(WorldError::InvalidId(entity));
        }
    }
    Ok(())
}
fn name(value: &str, entity: &'static str) -> Result<(), WorldError> {
    if value.trim().is_empty() {
        return Err(WorldError::EmptyName(entity));
    }
    Ok(())
}
impl WorldDraft {
    pub fn validate(&self) -> Result<(), WorldError> {
        ids(self.players.iter().map(|x| x.id.0), "player")?;
        ids(self.companies.iter().map(|x| x.id.0), "company")?;
        ids(self.stations.iter().map(|x| x.id.0), "station")?;
        ids(self.tracks.iter().map(|x| x.id.0), "track")?;
        ids(self.routes.iter().map(|x| x.id.0), "route")?;
        ids(self.services.iter().map(|x| x.id.0), "service")?;
        ids(self.alignments.iter().map(|x| x.id.0), "alignment")?;
        self.validate_alignments()?;
        let maximum_id = self
            .players
            .iter()
            .map(|x| x.id.0)
            .chain(self.companies.iter().map(|x| x.id.0))
            .chain(self.stations.iter().map(|x| x.id.0))
            .chain(self.tracks.iter().map(|x| x.id.0))
            .chain(self.routes.iter().map(|x| x.id.0))
            .chain(self.services.iter().map(|x| x.id.0))
            .chain(self.alignments.iter().map(|x| x.id.0))
            .max()
            .unwrap_or(0);
        if self.next_id == 0 || self.next_id <= maximum_id {
            return Err(WorldError::InvalidCounter);
        }
        for p in &self.players {
            name(&p.name, "player")?;
        }
        for c in &self.companies {
            name(&c.name, "company")?;
            if !self.players.iter().any(|p| p.id == c.owner) {
                return Err(WorldError::MissingReference("company.owner"));
            }
        }
        let owner_exists = |owner| self.companies.iter().any(|c| c.id == owner);
        let station_exists = |id| self.stations.iter().any(|s| s.id == id);
        for s in &self.stations {
            name(&s.name, "station")?;
            if !owner_exists(s.owner) {
                return Err(WorldError::MissingReference("station.owner"));
            }
            if !s.latitude.is_finite()
                || !s.longitude.is_finite()
                || !(-90.0..=90.0).contains(&s.latitude)
                || !(-180.0..=180.0).contains(&s.longitude)
            {
                return Err(WorldError::InvalidCoordinate);
            }
        }
        for t in &self.tracks {
            if !owner_exists(t.owner) {
                return Err(WorldError::MissingReference("track.owner"));
            }
            if !station_exists(t.from) || !station_exists(t.to) {
                return Err(WorldError::MissingReference("track.station"));
            }
            if t.from == t.to {
                return Err(WorldError::SelfLoop);
            }
        }
        self.validate_provenance()?;
        self.validate_route_paths()?;
        for r in &self.routes {
            name(&r.name, "route")?;
            if !owner_exists(r.owner) {
                return Err(WorldError::MissingReference("route.owner"));
            }
            if !station_exists(r.start) {
                return Err(WorldError::MissingReference("route.start"));
            }
            if self.route_paths.iter().any(|path| path.route_id == r.id) {
                continue;
            }
            if r.tracks.is_empty() {
                return Err(WorldError::EmptyRoute);
            }
            let mut current = r.start;
            for id in &r.tracks {
                let t = self
                    .tracks
                    .iter()
                    .find(|t| t.id == *id)
                    .ok_or(WorldError::MissingReference("route.track"))?;
                current = if t.from == current {
                    t.to
                } else if t.to == current {
                    t.from
                } else {
                    return Err(WorldError::DisconnectedRoute);
                };
            }
        }
        self.validate_service_paths()?;
        for service in &self.services {
            name(&service.name, "service")?;
            if !owner_exists(service.owner) {
                return Err(WorldError::MissingReference("service.owner"));
            }
            if !station_exists(service.start) {
                return Err(WorldError::MissingReference("service.start"));
            }
            if self
                .service_paths
                .iter()
                .any(|p| p.service_id == service.id)
            {
                continue;
            }
            if service.tracks.is_empty() {
                return Err(WorldError::EmptyService);
            }
            let mut current = service.start;
            for id in &service.tracks {
                let track = self
                    .tracks
                    .iter()
                    .find(|track| track.id == *id)
                    .ok_or(WorldError::MissingReference("service.track"))?;
                current = if track.from == current {
                    track.to
                } else if track.to == current {
                    track.from
                } else {
                    return Err(WorldError::DisconnectedService);
                };
            }
            let final_index =
                u32::try_from(service.tracks.len()).map_err(|_| WorldError::InvalidServiceStops)?;
            if service.stops.first() != Some(&0)
                || service.stops.last() != Some(&final_index)
                || service.stops.iter().any(|index| *index > final_index)
                || service.stops.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(WorldError::InvalidServiceStops);
            }
        }
        Ok(())
    }
}

mod alignment;
pub use alignment::{Alignment, AlignmentEndpoint, GeoPoint, StationPlacement, TrackType};

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> WorldDraft {
        WorldDraft {
            route_paths: vec![],
            service_paths: vec![],
            source_records: vec![],
            alignment_provenance: vec![],
            next_id: 4,
            alignments: vec![],
            station_placements: vec![],
            services: vec![],
            players: vec![Player {
                id: PlayerId(1),
                name: "Player".into(),
            }],
            companies: vec![Company {
                id: CompanyId(1),
                owner: PlayerId(1),
                name: "Railway".into(),
            }],
            stations: (1..=3)
                .map(|id| Station {
                    id: StationId(id),
                    owner: CompanyId(1),
                    name: format!("Station {id}"),
                    latitude: 35.0,
                    longitude: 139.0,
                })
                .collect(),
            tracks: vec![
                Track {
                    id: TrackId(1),
                    owner: CompanyId(1),
                    from: StationId(1),
                    to: StationId(2),
                },
                Track {
                    id: TrackId(2),
                    owner: CompanyId(1),
                    from: StationId(3),
                    to: StationId(2),
                },
            ],
            routes: vec![Route {
                id: RouteId(1),
                owner: CompanyId(1),
                name: "Main".into(),
                start: StationId(1),
                tracks: vec![TrackId(1), TrackId(2)],
            }],
        }
    }
    #[test]
    fn empty_and_connected_worlds_are_valid() {
        assert!(World::new(WorldDraft::default()).is_ok());
        assert!(sample().validate().is_ok());
    }
    #[test]
    fn ids_are_nonzero_and_unique_per_entity_type() {
        let mut data = sample();
        data.players.push(data.players[0].clone());
        assert_eq!(data.validate(), Err(WorldError::InvalidId("player")));
        data = sample();
        data.players[0].id = PlayerId(0);
        assert_eq!(data.validate(), Err(WorldError::InvalidId("player")));
    }
    #[test]
    fn invalid_deletion_does_not_mutate_world() {
        let mut world = World::new(sample()).unwrap();
        let original = world.clone();
        let mut candidate = world.snapshot().clone();
        candidate.stations.remove(0);
        assert!(world.replace(candidate).is_err());
        assert_eq!(world, original);
    }
    #[test]
    fn coordinated_deletion_is_valid() {
        let mut data = sample();
        data.routes.clear();
        data.tracks.clear();
        data.stations.clear();
        assert!(data.validate().is_ok());
    }
    #[test]
    fn orphan_owner_is_rejected() {
        let mut data = sample();
        data.companies[0].owner = PlayerId(99);
        assert_eq!(
            data.validate(),
            Err(WorldError::MissingReference("company.owner"))
        );
    }
    #[test]
    fn coordinates_must_be_finite_and_in_range() {
        for latitude in [f64::NAN, f64::INFINITY, 90.01, -90.01] {
            let mut data = sample();
            data.stations[0].latitude = latitude;
            assert_eq!(data.validate(), Err(WorldError::InvalidCoordinate));
        }
    }
    #[test]
    fn disconnected_and_missing_route_tracks_are_rejected() {
        let mut data = sample();
        data.routes[0].tracks.reverse();
        assert_eq!(data.validate(), Err(WorldError::DisconnectedRoute));
        data.routes[0].tracks = vec![TrackId(99)];
        assert_eq!(
            data.validate(),
            Err(WorldError::MissingReference("route.track"))
        );
    }
    #[test]
    fn empty_names_routes_and_self_loops_are_rejected() {
        let mut data = sample();
        data.stations[0].name = "  ".into();
        assert_eq!(data.validate(), Err(WorldError::EmptyName("station")));
        data = sample();
        data.routes[0].tracks.clear();
        assert_eq!(data.validate(), Err(WorldError::EmptyRoute));
        data = sample();
        data.tracks[0].to = data.tracks[0].from;
        assert_eq!(data.validate(), Err(WorldError::SelfLoop));
    }
}

mod route_planning;
pub use route_planning::{
    RouteCandidate, RouteCandidates, RouteMembershipPreview, RoutePlanningError, RouteSearchStatus,
    RouteSegment,
};

mod route_paths;
pub use route_paths::RoutePath;

mod service_planning;
pub use service_planning::{ServiceDraftStation, ServicePlanningError, ServiceRoutePreview};

mod service_paths;
pub use service_paths::{
    ServiceLeg, ServicePath, ServicePathOccurrence, ServicePathPreview, StationOccurrenceRef,
};

mod provenance;
pub use provenance::{
    AlignmentProvenance, AlignmentProvenanceInput, ProvenanceMethod, ReferenceMode, SourceRecord,
};
