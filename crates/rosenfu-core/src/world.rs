//! Validated, renderer-independent construction data. Not a stable save format yet.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

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
    #[serde(default)]
    pub road_transport: crate::road_transport::RoadTransport,
    /// Exact immutable OFM path proof for each independently authored ordinary Bus route.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ofm_gameplay_bus_routes: BTreeMap<String, crate::ofm_evidence::BusPathBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub road_path_provenance: Vec<AlignmentRoadPathProvenance>,
    /// Runtime-only immutable raw evidence. Plain WorldDraft JSON is NOT a backup.
    #[serde(skip)]
    pub runtime_road_evidence: crate::ofm_evidence::EvidenceStore,
    /// Lightweight manifest; must exactly match the runtime table after restoration.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub road_evidence_descriptors: Vec<crate::ofm_evidence::EvidenceDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub road_snapshots: Vec<RoadSnapshot>,
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
            road_transport: crate::road_transport::RoadTransport::default(),
            ofm_gameplay_bus_routes: BTreeMap::new(),
            road_path_provenance: vec![],
            runtime_road_evidence: crate::ofm_evidence::EvidenceStore::empty(),
            road_evidence_descriptors: vec![],
            road_snapshots: vec![],
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
    InvalidRoadSnapshot,
    InvalidRoadTransport,
    InvalidRoadEvidence,
    InvalidViewState,
    InvalidAlignment,
    InvalidAlignmentConnection,
    InsufficientTurnoutSpace,
    HeightRedevelopmentSpace,
    HeightRedevelopmentStation,
    InvalidAlignmentDesignSpeed,
    InvalidRoutePath,
    InvalidServicePath,
    TramTrackRequiresStop,
    StationAtMixedTrackConnection,
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
        if matches!(self, Self::InvalidRoadTransport) {
            return write!(f,"道路交通の参照が不整合です。併用軌道の関連付けを外してから線路を延伸・変更・撤去してください。");
        }
        if matches!(self, Self::TramTrackRequiresStop) {
            return write!(f, "普通列車はトラム専用線を通れません。トラムとして運行するには、通過ではなくトラムのホームを停車駅に指定してください。");
        }
        if matches!(self, Self::StationAtMixedTrackConnection) {
            return write!(f, "トラム専用線と鉄道線の接続点には駅を設置できません。接続点から離れた場所に設置してください。");
        }
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
        if matches!(self, Self::InvalidRoadTransport) {
            return write!(f,"道路交通の参照が不整合です。併用軌道の関連付けを外してから線路を延伸・変更・撤去してください。");
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
    data: std::sync::Arc<WorldDraft>,
    operation: Option<WorldOperation>,
    fixed_od_scenario: Option<FixedOdScenario>,
}
impl World {
    pub fn new(data: WorldDraft) -> Result<Self, WorldError> {
        data.validate()?;
        Ok(Self {
            data: data.into(),
            operation: None,
            fixed_od_scenario: None,
        })
    }
    /// Restore a saved snapshot with its separately decoded evidence. Attaching
    /// the store does not rewrite the saved descriptors; validation must match
    /// them to the evidence before the draft can become a running world.
    pub fn from_saved_draft(
        mut data: WorldDraft,
        store: crate::ofm_evidence::EvidenceStore,
    ) -> Result<Self, WorldError> {
        data.runtime_road_evidence = store;
        Self::new(data)
    }

    /// UI JSON omits runtime evidence. Preserve the CURRENT table and reject
    /// manifest changes; explicit evidence attachment uses a different validated API.
    /// Returns a replacement without modifying the current world on any failure.
    pub fn replace_retaining_road_evidence(
        &self,
        mut candidate: WorldDraft,
    ) -> Result<Self, WorldError> {
        if candidate.road_evidence_descriptors != self.data.road_evidence_descriptors {
            return Err(WorldError::InvalidRoadEvidence);
        }
        candidate.runtime_road_evidence = self.data.runtime_road_evidence.clone();
        let mut replacement = self.clone();
        replacement.replace(candidate)?;
        Ok(replacement)
    }
    fn insert(&mut self, insert: impl FnOnce(&mut WorldDraft, u64)) -> Result<u64, WorldError> {
        let id = self.data.next_id;
        let mut candidate = self.snapshot().clone();
        candidate.next_id = id.checked_add(1).ok_or(WorldError::IdExhausted)?;
        insert(&mut candidate, id);
        candidate.validate()?;
        self.data = candidate.into();
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
    /// Retain exactly this immutable snapshot without deep-copying map assets.
    /// Validated replacements publish a new allocation; scoped mutations are
    /// copy-on-write. Operation clock and scenario state are held separately.
    pub fn shared_snapshot(&self) -> std::sync::Arc<WorldDraft> {
        self.data.clone()
    }
    fn data_mut(&mut self) -> &mut WorldDraft {
        std::sync::Arc::make_mut(&mut self.data)
    }
    /// Atomic candidate replacement: failures leave the live state unchanged.
    pub fn replace(&mut self, mut candidate: WorldDraft) -> Result<(), WorldError> {
        // Explicit alignment removal drops only its derived range records; source
        // evidence is retained for Undo and other references.
        candidate
            .road_path_provenance
            .retain(|p| candidate.alignments.iter().any(|a| a.id == p.alignment_id));
        candidate.validate()?;
        candidate.preserve_service_paths(&self.data)?;
        candidate.preserve_provenance(&self.data)?;
        if candidate.next_id != self.data.next_id {
            return Err(WorldError::InvalidCounter);
        }
        macro_rules! existing_only {
            ($field:ident) => {
                let existing: BTreeSet<_> = self.data.$field.iter().map(|entry| entry.id).collect();
                if candidate
                    .$field
                    .iter()
                    .any(|entry| !existing.contains(&entry.id))
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
        self.data = candidate.into();
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
    /// Explicitly attach a validated immutable table and its matching manifest.
    /// No path-to-alignment provenance or construction authorization is added here.
    pub fn with_road_evidence(
        mut self,
        store: crate::ofm_evidence::EvidenceStore,
    ) -> Result<Self, WorldError> {
        self.road_evidence_descriptors = store.descriptors();
        self.runtime_road_evidence = store;
        self.validate()?;
        Ok(self)
    }
    /// Restore raw evidence without overwriting a claimed serialized manifest.
    /// Failure leaves the original draft unchanged.
    pub fn restore_road_evidence(
        &mut self,
        store: crate::ofm_evidence::EvidenceStore,
    ) -> Result<(), WorldError> {
        let mut candidate = self.clone();
        candidate.runtime_road_evidence = store;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
    pub fn validate(&self) -> Result<(), WorldError> {
        if self.road_evidence_descriptors != self.runtime_road_evidence.descriptors() {
            return Err(WorldError::InvalidRoadEvidence);
        }
        roads::validate_road_snapshots(&self.road_snapshots)?;
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
        self.road_transport
            .validate_world(self)
            .map_err(|_| WorldError::InvalidRoadTransport)?;
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
        self.validate_road_path_provenance()?;
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
        let geometry_world = World {
            data: self.clone().into(),
            operation: None,
            fixed_od_scenario: None,
        };
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
mod fixed_od_scenario;
pub use alignment::{
    Alignment, AlignmentEndpoint, GeoPoint, InfrastructureLevel, SectionInfrastructure,
    StationPlacement, TrackType,
};
pub use fixed_od_scenario::FixedOdScenario;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_keeps_membership_ids_and_rejects_reintroducing_a_missing_id_atomically() {
        let mut world = World::default();
        let player = world.add_player("p".into()).unwrap();
        let owner = world.add_company(player, "c".into()).unwrap();
        for (name, latitude) in [("a", 35.0), ("b", 35.1)] {
            world
                .build_alignment(
                    owner,
                    name.into(),
                    vec![
                        GeoPoint {
                            latitude,
                            longitude: 139.0,
                        },
                        GeoPoint {
                            latitude,
                            longitude: 139.01,
                        },
                    ],
                )
                .unwrap();
        }
        let mut reordered = world.snapshot().clone();
        reordered.alignments.reverse();
        world.replace(reordered.clone()).unwrap();
        let removed = reordered.alignments.remove(0);
        world.replace(reordered.clone()).unwrap();
        let mut stale = reordered.clone();
        stale.alignments.push(removed);
        // The counter and IDs are structurally valid, but replacement may not
        // resurrect an alignment absent from the current world.
        stale.validate().unwrap();
        assert_eq!(world.replace(stale), Err(WorldError::UnallocatedId));
        assert_eq!(world.snapshot(), &reordered);
    }

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

mod roads;
pub use roads::{
    assess_road_crossings, AutomaticCrossing, CoverageBounds, RoadAssessment, RoadClassification,
    RoadSnapshot, RoadWay,
};

mod water_model;
pub use water_model::WATER_MODEL_REVISION;
mod road_path;
pub use road_path::AlignmentRoadPathProvenance;

mod water_fine;
mod water_national;
pub use water_fine::{water_mask_requests, ConstructionWaterModel, WaterMaskRequest};

mod operation_adapter;
pub use operation_adapter::{
    BusOperationConfig, OperationServiceConfig, WorldOperation, WorldOperationSave,
};
