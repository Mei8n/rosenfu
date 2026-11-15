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
/// Named route backed by an explicit alignment RoutePath.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub id: RouteId,
    pub owner: CompanyId,
    pub name: String,
    pub start: StationId,
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
}
/// Single-player local presentation metadata. This is not simulation state and
/// must not be used as shared camera authority by future multiplayer protocols.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewState {
    pub latitude: f64,
    pub longitude: f64,
    pub zoom: f64,
}
impl ViewState {
    pub fn validate(&self) -> Result<(), WorldError> {
        if self.latitude.is_finite()
            && self.longitude.is_finite()
            && self.zoom.is_finite()
            && (20.0..=46.0).contains(&self.latitude)
            && (122.0..=154.0).contains(&self.longitude)
            && (4.0..=20.0).contains(&self.zoom)
        {
            Ok(())
        } else {
            Err(WorldError::InvalidViewState)
        }
    }
}
/// An editable candidate. Validate before applying or loading it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldDraft {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub station_groups: Vec<StationGroup>,
    /// Explicit infrastructure endpoint links, including supported three-port turnouts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alignment_connections: Vec<AlignmentConnection>,
    /// Optional local presentation metadata, independent of physical world geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_state: Option<ViewState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub construction_plans: Option<ConstructionPlans>,
    #[serde(default)]
    pub source_records: Vec<SourceRecord>,
    #[serde(default)]
    pub alignment_provenance: Vec<AlignmentProvenance>,
    #[serde(default)]
    pub service_paths: Vec<ServicePath>,
    #[serde(default)]
    pub service_route_bindings: Vec<ServiceRouteBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub service_appearances: Vec<ServiceAppearance>,
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
            station_groups: vec![],
            alignment_connections: vec![],
            view_state: None,
            construction_plans: None,
            route_paths: vec![],
            service_paths: vec![],
            service_route_bindings: vec![],
            service_appearances: vec![],
            source_records: vec![],
            alignment_provenance: vec![],
            next_id: 1,
            alignments: vec![],
            station_placements: vec![],
            players: vec![],
            companies: vec![],
            stations: vec![],
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
    InvalidConstructionPlans,
    InvalidViewState,
    InvalidAlignment,
    InvalidAlignmentConnection,
    InsufficientTurnoutSpace,
    HeightRedevelopmentSpace,
    HeightRedevelopmentStation,
    InvalidAlignmentDesignSpeed,
    InvalidRoutePath,
    InvalidServicePath,
    InvalidServiceAppearance,
    InvalidServiceRouteBinding,
    InvalidProvenance,
    ProvenanceEditUnsupported,
    ServicePathEditUnsupported,
    InvalidStationPlacement,
    StationOnGradient,
    StationFootprintOverlap,
    InvalidStationGroup,
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
        if matches!(self, Self::StationOnGradient) {
            return write!(f,"ホーム全長が平坦な場所に駅を設置してください。勾配やその境界に重なるホームは設置できません。");
        }
        if matches!(self, Self::HeightRedevelopmentSpace) {
            return write!(f,"選択区間内に入口・出口の勾配と平坦部を収められません。区間を長く選択してください。選択外の高さは変更していません。");
        }
        if matches!(self, Self::HeightRedevelopmentStation) {
            return write!(f,"選択区間にホームがあります。駅の高さを勝手に変更しないため、この高さ変更はできません。");
        }
        if matches!(self, Self::InsufficientTurnoutSpace) {
            return write!(f, "分岐・単複線の合流に必要な空間がありません。接続位置をホームから離すか、前後の区間を長くしてください。");
        }
        if matches!(self, Self::InvalidServiceRouteBinding) {
            return write!(f, "既存系統の経路が割り当てた路線に含まれなくなるため変更できません。路線と系統は変更していません。");
        }
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
        if let Some(view) = self.view_state {
            view.validate()?;
        }
        if let Some(plans) = &self.construction_plans {
            plans.validate()?;
        }
        ids(self.players.iter().map(|x| x.id.0), "player")?;
        ids(self.companies.iter().map(|x| x.id.0), "company")?;
        ids(self.stations.iter().map(|x| x.id.0), "station")?;
        ids(self.routes.iter().map(|x| x.id.0), "route")?;
        ids(self.services.iter().map(|x| x.id.0), "service")?;
        ids(self.alignments.iter().map(|x| x.id.0), "alignment")?;
        self.validate_alignments()?;
        self.validate_station_groups()?;
        self.validate_alignment_connections()?;
        let maximum_id = self
            .players
            .iter()
            .map(|x| x.id.0)
            .chain(self.companies.iter().map(|x| x.id.0))
            .chain(self.stations.iter().map(|x| x.id.0))
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
            if !self.route_paths.iter().any(|path| path.route_id == r.id) {
                return Err(WorldError::InvalidRoutePath);
            }
        }
        // All references and platform footprints are valid by this point. Check
        // derived turnout geometry before any candidate snapshot is committed.
        let geometry_world = World { data: self.clone() };
        geometry_world.validate_turnout_geometry()?;
        self.validate_service_paths()?;
        self.validate_service_route_bindings()?;
        self.validate_service_appearances()?;
        for service in &self.services {
            name(&service.name, "service")?;
            if !owner_exists(service.owner) {
                return Err(WorldError::MissingReference("service.owner"));
            }
            if !station_exists(service.start) {
                return Err(WorldError::MissingReference("service.start"));
            }
            if !self
                .service_paths
                .iter()
                .any(|path| path.service_id == service.id)
            {
                return Err(WorldError::InvalidServicePath);
            }
        }
        Ok(())
    }
}

mod alignment_connections;
pub use alignment_connections::{AlignmentConnection, AlignmentEndpointRef};
mod alignment;
pub use alignment::{
    Alignment, AlignmentEndpoint, GeoPoint, InfrastructureLevel, SectionInfrastructure,
    StationPlacement, TrackType,
};

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> WorldDraft {
        let mut w = World::default();
        let p = w.add_player("Player".into()).unwrap();
        let c = w.add_company(p, "Railway".into()).unwrap();
        let a = w
            .build_alignment(
                c,
                "Line".into(),
                vec![
                    GeoPoint {
                        latitude: 35.,
                        longitude: 139.,
                    },
                    GeoPoint {
                        latitude: 35.,
                        longitude: 139.1,
                    },
                ],
            )
            .unwrap();
        let stations: Vec<_> = [0.1, 0.5, 0.9]
            .into_iter()
            .map(|position| {
                w.place_station_on_alignment(c, "S".into(), a, position, 20., 10.)
                    .unwrap()
            })
            .collect();
        w.add_planned_route(
            c,
            "R".into(),
            stations[0],
            vec![RouteSegment::Alignment {
                alignment_id: a,
                from: stations[0],
                to: stations[2],
            }],
            stations,
        )
        .unwrap();
        w.snapshot().clone()
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
        data.route_paths.clear();
        data.station_placements.clear();
        data.alignments.clear();
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
            data.station_placements.clear();
            data.route_paths.clear();
            data.routes.clear();
            data.stations[0].latitude = latitude;
            assert_eq!(data.validate(), Err(WorldError::InvalidCoordinate));
        }
    }
    #[test]
    fn disconnected_and_missing_route_paths_are_rejected() {
        let mut data = sample();
        let RouteSegment::Alignment { ref mut from, .. } = data.route_paths[0].segments[0] else {
            unreachable!()
        };
        *from = data.stations[1].id;
        assert_eq!(data.validate(), Err(WorldError::InvalidRoutePath));
        let mut data = sample();
        let RouteSegment::Alignment {
            ref mut alignment_id,
            ..
        } = data.route_paths[0].segments[0]
        else {
            unreachable!()
        };
        *alignment_id = AlignmentId(999);
        assert_eq!(data.validate(), Err(WorldError::InvalidRoutePath));
    }
    #[test]
    fn empty_names_paths_and_zero_length_segments_are_rejected() {
        let mut data = sample();
        data.stations[0].name = " ".into();
        assert_eq!(data.validate(), Err(WorldError::EmptyName("station")));
        let mut data = sample();
        data.route_paths[0].segments.clear();
        assert_eq!(data.validate(), Err(WorldError::InvalidRoutePath));
        let mut data = sample();
        let RouteSegment::Alignment {
            from, ref mut to, ..
        } = data.route_paths[0].segments[0]
        else {
            unreachable!()
        };
        *to = from;
        assert_eq!(data.validate(), Err(WorldError::InvalidRoutePath));
    }
}

mod route_planning;
pub use route_planning::{
    RouteCandidate, RouteCandidates, RouteMembershipPreview, RoutePlanningError, RouteSearchStatus,
    RouteSegment,
};

mod route_paths;
pub use route_paths::RoutePath;

mod service_paths;
pub use service_paths::{
    ServiceLeg, ServicePath, ServicePathOccurrence, ServicePathPreview, StationOccurrenceRef,
};

mod provenance;
pub use provenance::{
    AlignmentProvenance, AlignmentProvenanceInput, ProvenanceGeometrySlice, ProvenanceMethod,
    RailGraphSpan, ReferenceMode, SourceRecord,
};

mod construction_plans;
pub use construction_plans::{ConstructionJunctionSource, ConstructionPlans};

mod station_groups;
pub use station_groups::StationGroup;

mod service_route_bindings;
pub use service_route_bindings::{ServiceRouteBinding, ServiceRouteBindingKind};

mod service_appearance;
pub use service_appearance::{ServiceAppearance, ServiceSymbolShape};
mod track_topology;
pub use track_topology::{
    InternalTrackMovement, InternalTrackPoint, TrackLanePort, TrackMovement, TrackPointResource,
    TrackSectionPort,
};

mod vertical_profile;
pub use vertical_profile::{VerticalProfile, LEVEL_SPACING_M};

mod junctions;

mod turnout_geometry;
pub use turnout_geometry::{
    InternalTrackMovementGeometry, TrackLaneGeometry, TrackMovementGeometry, TrackRenderGeometry,
    TurnoutMovementProperties, TRACK_LANE_SPACING_M, TURNOUT_GAME_LATERAL_ACCELERATION_MPS2,
};

mod height_redevelopment;
