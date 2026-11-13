//! Desktop application operations. The core owns IDs and referential integrity.
use rosenfu_core::world::{
    AlignmentId, CompanyId, GeoPoint, RouteId, ServiceId, StationId, TrackId, World, WorldDraft,
};
use std::path::{Path, PathBuf};

// Prototype edit extent, not a national boundary or map-coverage guarantee.
fn in_edit_extent(latitude: f64, longitude: f64) -> bool {
    (20.0..=46.0).contains(&latitude) && (122.0..=154.0).contains(&longitude)
}

/// A station attached to the alignment being constructed, not an existing ID.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedStation {
    pub name: String,
    pub position: f64,
    pub length_m: f64,
    pub width_m: f64,
}

/// Optional attribution for a plan copied from real-world railway or road data.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedAlignmentReference {
    pub source_record: rosenfu_core::world::SourceRecord,
    pub provenance: rosenfu_core::world::AlignmentProvenanceInput,
}

/// Existing infrastructure endpoint targeted by a path, not a new alignment.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlannedAlignmentExtension {
    pub alignment_id: AlignmentId,
    pub endpoint: rosenfu_core::world::AlignmentEndpoint,
}

/// One independent path and its stations within an explicitly selected plan.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedAlignment {
    pub name: String,
    pub points: Vec<GeoPoint>,
    pub track_type: rosenfu_core::world::TrackType,
    pub stations: Vec<PlannedStation>,
    pub reference: Option<PlannedAlignmentReference>,
    pub extension: Option<PlannedAlignmentExtension>,
}

#[derive(Clone)]
pub struct WorldEditor {
    world: World,
    company: CompanyId,
    loaded_path: Option<PathBuf>,
    company_setup_required: bool,
    manual_version: Option<rosenfu_storage::FileVersion>,
    manual_repair_allowed: bool,
    autosave_version: Option<(PathBuf, rosenfu_storage::FileVersion)>,
    observed_autosave: Option<(PathBuf, rosenfu_storage::FileVersion)>,
    autosave_blocked: bool,
    autosave_replace_allowed: bool,
}
impl WorldEditor {
    pub fn preview_service_path(
        &self,
        start: StationId,
        segments: Vec<rosenfu_core::world::RouteSegment>,
    ) -> Result<rosenfu_core::world::ServicePathPreview, String> {
        self.world
            .preview_service_path(start, segments)
            .map_err(|e| e.to_string())
    }
    pub fn create_service_path(
        &mut self,
        name: String,
        start: StationId,
        segments: Vec<rosenfu_core::world::RouteSegment>,
        stops: Vec<rosenfu_core::world::StationOccurrenceRef>,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate.create_service_path_unchecked(name, start, segments, stops)?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        *self = candidate;
        Ok(self.snapshot())
    }
    fn create_service_path_unchecked(
        &mut self,
        name: String,
        start: StationId,
        segments: Vec<rosenfu_core::world::RouteSegment>,
        stops: Vec<rosenfu_core::world::StationOccurrenceRef>,
    ) -> Result<WorldDraft, String> {
        if name.trim().is_empty() || name.chars().count() > 80 {
            return Err("系統名は1〜80文字で入力してください。".into());
        }
        self.world
            .add_service_path(self.company, name.trim().into(), start, segments, stops)
            .map_err(|e| e.to_string())?;
        Ok(self.snapshot())
    }
    pub fn preview_service_from_route(
        &self,
        route_id: RouteId,
        from_occurrence: u32,
        to_occurrence: u32,
    ) -> Result<rosenfu_core::world::ServiceRoutePreview, String> {
        self.world
            .preview_service_from_route(route_id, from_occurrence, to_occurrence)
            .map_err(|e| e.to_string())
    }
    pub fn preview_route_candidates(
        &self,
        start: StationId,
        end: StationId,
    ) -> Result<rosenfu_core::world::RouteCandidates, String> {
        self.world
            .route_candidates(start, end)
            .map_err(|e| e.to_string())
    }
    pub fn preview_route_membership(
        &self,
        start: StationId,
        segments: Vec<rosenfu_core::world::RouteSegment>,
        excluded_stations: Vec<StationId>,
    ) -> Result<rosenfu_core::world::RouteMembershipPreview, String> {
        self.world
            .preview_route_membership(start, segments, excluded_stations)
            .map_err(|e| e.to_string())
    }
    pub fn new() -> Self {
        let mut world = World::default();
        let player = world
            .add_player("プレイヤー".into())
            .expect("valid initial player");
        let company = world
            .add_company(player, "新しい鉄道会社".into())
            .expect("valid initial company");
        Self {
            world,
            company,
            loaded_path: None,
            company_setup_required: true,
            manual_version: None,
            manual_repair_allowed: false,
            autosave_version: None,
            observed_autosave: None,
            autosave_blocked: false,
            autosave_replace_allowed: false,
        }
    }
    pub fn company_setup_required(&self) -> bool {
        self.company_setup_required
    }
    pub fn snapshot(&self) -> WorldDraft {
        self.world.snapshot().clone()
    }
    pub fn add_station(
        &mut self,
        name: String,
        latitude: f64,
        longitude: f64,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate.add_station_unchecked(name, latitude, longitude)?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        *self = candidate;
        Ok(self.snapshot())
    }
    fn add_station_unchecked(
        &mut self,
        name: String,
        latitude: f64,
        longitude: f64,
    ) -> Result<WorldDraft, String> {
        if !in_edit_extent(latitude, longitude) {
            return Err(
                "この試作の編集範囲（北緯20〜46度・東経122〜154度）内に駅を配置してください。"
                    .into(),
            );
        }
        if name.trim().is_empty() || name.chars().count() > 80 {
            return Err("駅名は1〜80文字で入力してください。".into());
        }
        self.world
            .add_station(self.company, name.trim().to_owned(), latitude, longitude)
            .map_err(|e| e.to_string())?;
        Ok(self.snapshot())
    }
    pub fn build_alignment_from_reference(
        &mut self,
        name: String,
        points: Vec<GeoPoint>,
        source_record: rosenfu_core::world::SourceRecord,
        provenance: rosenfu_core::world::AlignmentProvenanceInput,
    ) -> Result<WorldDraft, String> {
        self.build_alignment_from_reference_typed(
            name,
            points,
            source_record,
            provenance,
            rosenfu_core::world::TrackType::Normal,
        )
    }
    pub fn build_alignment_from_reference_typed(
        &mut self,
        name: String,
        points: Vec<GeoPoint>,
        source_record: rosenfu_core::world::SourceRecord,
        provenance: rosenfu_core::world::AlignmentProvenanceInput,
        track_type: rosenfu_core::world::TrackType,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate.build_alignment_from_reference_typed_unchecked(
            name,
            points,
            source_record,
            provenance,
            track_type,
        )?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        *self = candidate;
        Ok(self.snapshot())
    }
    fn build_alignment_from_reference_typed_unchecked(
        &mut self,
        name: String,
        points: Vec<GeoPoint>,
        source_record: rosenfu_core::world::SourceRecord,
        provenance: rosenfu_core::world::AlignmentProvenanceInput,
        track_type: rosenfu_core::world::TrackType,
    ) -> Result<AlignmentId, String> {
        if name.trim().is_empty() || name.chars().count() > 80 {
            return Err("路線名は1〜80文字で入力してください。".into());
        }
        if points
            .iter()
            .any(|p| !in_edit_extent(p.latitude, p.longitude))
        {
            return Err("路線は編集範囲内に建設してください。".into());
        }
        let mut candidate = self.world.clone();
        let alignment_id = candidate
            .build_alignment_from_reference_typed(
                self.company,
                name.trim().into(),
                points,
                source_record,
                provenance,
                track_type,
            )
            .map_err(|e| e.to_string())?;
        // Ensure this attributed edit remains serializable within the save limit
        // before publishing either its geometry, metadata, or allocated ID.
        rosenfu_storage::encode(&candidate).map_err(|e| e.to_string())?;
        self.world = candidate;
        Ok(alignment_id)
    }
    /// Extend only after validating the complete candidate and its save representation.
    pub fn extend_alignment(
        &mut self,
        alignment_id: AlignmentId,
        endpoint: rosenfu_core::world::AlignmentEndpoint,
        points: Vec<GeoPoint>,
    ) -> Result<WorldDraft, String> {
        if points
            .iter()
            .any(|p| !in_edit_extent(p.latitude, p.longitude))
        {
            return Err("線路は編集範囲内に建設してください。".into());
        }
        let mut candidate = self.clone();
        candidate
            .world
            .extend_alignment(candidate.company, alignment_id, endpoint, points)
            .map_err(|e| match e {
                rosenfu_core::world::WorldError::InvalidAlignment => {
                    "延伸の接合方向または線形が不正です。既存線路の接線方向へ続けてください。"
                        .to_owned()
                }
                rosenfu_core::world::WorldError::ProvenanceEditUnsupported => {
                    "実在データの出典付き線路の延伸はまだ対応していません。".to_owned()
                }
                rosenfu_core::world::WorldError::ServicePathEditUnsupported => {
                    "延伸によって既存の系統経路が変わるため実行できません。".to_owned()
                }
                _ => e.to_string(),
            })?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        *self = candidate;
        Ok(self.snapshot())
    }

    pub fn build_alignment(
        &mut self,
        name: String,
        points: Vec<GeoPoint>,
    ) -> Result<WorldDraft, String> {
        self.build_alignment_typed(name, points, rosenfu_core::world::TrackType::Normal)
    }
    pub fn build_alignment_typed(
        &mut self,
        name: String,
        points: Vec<GeoPoint>,
        track_type: rosenfu_core::world::TrackType,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate.build_alignment_typed_unchecked(name, points, track_type)?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        *self = candidate;
        Ok(self.snapshot())
    }
    /// Publish the alignment and all planned stations together, or change nothing.
    pub fn build_alignment_with_stations(
        &mut self,
        name: String,
        points: Vec<GeoPoint>,
        track_type: rosenfu_core::world::TrackType,
        stations: Vec<PlannedStation>,
        reference: Option<PlannedAlignmentReference>,
    ) -> Result<WorldDraft, String> {
        self.build_plan(vec![PlannedAlignment {
            name,
            points,
            track_type,
            stations,
            reference,
            extension: None,
        }])
    }

    /// Build only the supplied plan's paths, stations and attribution atomically.
    /// A failed path never publishes earlier paths or consumes their IDs.
    pub fn build_plan(&mut self, alignments: Vec<PlannedAlignment>) -> Result<WorldDraft, String> {
        if alignments.is_empty() {
            return Err("着工する線路がありません。".into());
        }
        let mut candidate = self.clone();
        for alignment in alignments {
            if let Some(extension) = alignment.extension {
                if alignment.reference.is_some() {
                    return Err("実在データ付き計画による延伸はまだ対応していません。".into());
                }
                let old = candidate
                    .world
                    .snapshot()
                    .alignments
                    .iter()
                    .find(|a| a.id == extension.alignment_id)
                    .ok_or("延伸対象の線路がありません。")?;
                if old.track_type != alignment.track_type {
                    return Err("延伸対象と線路種別が一致しません。".into());
                }
                let old_length = old.length_m();
                candidate.extend_alignment(
                    extension.alignment_id,
                    extension.endpoint,
                    alignment.points,
                )?;
                let new_length = candidate
                    .world
                    .snapshot()
                    .alignments
                    .iter()
                    .find(|a| a.id == extension.alignment_id)
                    .ok_or("延伸対象の線路がありません。")?
                    .length_m();
                let added_length = new_length - old_length;
                for station in alignment.stations {
                    if !station.position.is_finite() || !(0.0..=1.0).contains(&station.position) {
                        return Err("駅の計画位置が不正です。".into());
                    }
                    // Plan positions run from the old endpoint outward even at the start.
                    let distance = match extension.endpoint {
                        rosenfu_core::world::AlignmentEndpoint::Start => {
                            added_length * (1.0 - station.position)
                        }
                        rosenfu_core::world::AlignmentEndpoint::End => {
                            old_length + added_length * station.position
                        }
                    };
                    candidate.place_station_on_alignment_unchecked(
                        station.name,
                        extension.alignment_id,
                        distance / new_length,
                        station.length_m,
                        station.width_m,
                    )?;
                }
                continue;
            }
            let alignment_id = match alignment.reference {
                Some(reference) => candidate.build_alignment_from_reference_typed_unchecked(
                    alignment.name,
                    alignment.points,
                    reference.source_record,
                    reference.provenance,
                    alignment.track_type,
                )?,
                None => candidate.build_alignment_typed_unchecked(
                    alignment.name,
                    alignment.points,
                    alignment.track_type,
                )?,
            };
            for station in alignment.stations {
                candidate.place_station_on_alignment_unchecked(
                    station.name,
                    alignment_id,
                    station.position,
                    station.length_m,
                    station.width_m,
                )?;
            }
        }
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        *self = candidate;
        Ok(self.snapshot())
    }
    fn build_alignment_typed_unchecked(
        &mut self,
        name: String,
        points: Vec<GeoPoint>,
        track_type: rosenfu_core::world::TrackType,
    ) -> Result<AlignmentId, String> {
        if name.trim().is_empty() || name.chars().count() > 80 {
            return Err("路線名は1〜80文字で入力してください。".into());
        }
        if points
            .iter()
            .any(|p| !in_edit_extent(p.latitude, p.longitude))
        {
            return Err("路線は編集範囲内に建設してください。".into());
        }
        self.world
            .build_alignment_typed(self.company, name.trim().to_owned(), points, track_type)
            .map_err(|e| e.to_string())
    }
    pub fn place_station_on_alignment(
        &mut self,
        name: String,
        alignment_id: AlignmentId,
        position: f64,
        length_m: f64,
        width_m: f64,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate.place_station_on_alignment_unchecked(
            name,
            alignment_id,
            position,
            length_m,
            width_m,
        )?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        *self = candidate;
        Ok(self.snapshot())
    }
    fn place_station_on_alignment_unchecked(
        &mut self,
        name: String,
        alignment_id: AlignmentId,
        position: f64,
        length_m: f64,
        width_m: f64,
    ) -> Result<StationId, String> {
        if name.trim().is_empty() || name.chars().count() > 80 {
            return Err("駅名は1〜80文字で入力してください。".into());
        }
        self.world
            .place_station_on_alignment(
                self.company,
                name.trim().to_owned(),
                alignment_id,
                position,
                length_m,
                width_m,
            )
            .map_err(|e| e.to_string())
    }
    pub fn update_station_placement(
        &mut self,
        station_id: StationId,
        length_m: f64,
        width_m: f64,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate.update_station_placement_unchecked(station_id, length_m, width_m)?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        *self = candidate;
        Ok(self.snapshot())
    }
    fn update_station_placement_unchecked(
        &mut self,
        station_id: StationId,
        length_m: f64,
        width_m: f64,
    ) -> Result<WorldDraft, String> {
        if !self
            .snapshot()
            .stations
            .iter()
            .any(|s| s.id == station_id && s.owner == self.company)
        {
            return Err("編集する駅が見つかりません。".into());
        }
        self.world
            .update_station_placement(station_id, length_m, width_m)
            .map_err(|e| e.to_string())?;
        Ok(self.snapshot())
    }
    pub fn rename_station(
        &mut self,
        station_id: StationId,
        name: String,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate.rename_station_unchecked(station_id, name)?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        *self = candidate;
        Ok(self.snapshot())
    }
    fn rename_station_unchecked(
        &mut self,
        station_id: StationId,
        name: String,
    ) -> Result<WorldDraft, String> {
        if name.trim().is_empty() || name.chars().count() > 80 {
            return Err("駅名は1〜80文字で入力してください。".into());
        }
        let mut candidate = self.snapshot();
        let station = candidate
            .stations
            .iter_mut()
            .find(|station| station.id == station_id && station.owner == self.company)
            .ok_or("編集する駅が見つかりません。")?;
        station.name = name.trim().to_owned();
        self.world
            .replace(candidate)
            .map_err(|error| error.to_string())?;
        Ok(self.snapshot())
    }
    pub fn delete_station(&mut self, station_id: StationId) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate.delete_station_unchecked(station_id)?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        *self = candidate;
        Ok(self.snapshot())
    }
    fn delete_station_unchecked(&mut self, station_id: StationId) -> Result<WorldDraft, String> {
        let mut candidate = self.snapshot();
        if !candidate
            .stations
            .iter()
            .any(|station| station.id == station_id && station.owner == self.company)
        {
            return Err("削除する駅が見つかりません。".into());
        }
        if candidate.service_paths.iter().any(|p|self.world.service_path_occurrences(p.service_id).is_ok_and(|occurrences|occurrences.iter().any(|o|o.station_id==station_id)))
            || candidate.service_paths.iter().any(|p|p.stops.iter().any(|stop|matches!(stop,rosenfu_core::world::StationOccurrenceRef::OnLeg{station_id:id,..} if *id==station_id)) || p.legs.iter().any(|l|matches!(l.segment,rosenfu_core::world::RouteSegment::Alignment{from,to,..} if from==station_id || to==station_id)))
            || candidate.route_paths.iter().any(|p| p.member_stations.contains(&station_id) || p.segments.iter().any(|segment| matches!(segment, rosenfu_core::world::RouteSegment::Alignment { from, to, .. } if *from == station_id || *to == station_id)))
            || candidate
            .tracks
            .iter()
            .any(|track| track.from == station_id || track.to == station_id)
            || candidate
                .routes
                .iter()
                .any(|route| route.start == station_id)
            || candidate
                .services
                .iter()
                .any(|service| service.start == station_id)
        {
            return Err(
                "線路・路線・運行系統から参照されている駅は削除できません。連鎖削除は行いません。"
                    .into(),
            );
        }
        candidate
            .stations
            .retain(|station| station.id != station_id);
        candidate
            .station_placements
            .retain(|p| p.station_id != station_id);
        self.world
            .replace(candidate)
            .map_err(|error| error.to_string())?;
        Ok(self.snapshot())
    }
    pub fn delete_track(&mut self, track_id: TrackId) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate.delete_track_unchecked(track_id)?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        *self = candidate;
        Ok(self.snapshot())
    }
    fn delete_track_unchecked(&mut self, track_id: TrackId) -> Result<WorldDraft, String> {
        let mut candidate = self.snapshot();
        if !candidate
            .tracks
            .iter()
            .any(|track| track.id == track_id && track.owner == self.company)
        {
            return Err("削除する線路が見つかりません。".into());
        }
        if candidate.service_paths.iter().any(|p|p.legs.iter().any(|l|matches!(l.segment,rosenfu_core::world::RouteSegment::Track{track_id:id} if id==track_id)))
            || candidate.route_paths.iter().any(|p| p.segments.iter().any(|segment| matches!(segment, rosenfu_core::world::RouteSegment::Track { track_id: id } if *id == track_id)))
            || candidate
            .routes
            .iter()
            .any(|route| route.tracks.contains(&track_id))
            || candidate
                .services
                .iter()
                .any(|service| service.tracks.contains(&track_id))
        {
            return Err(
                "路線・運行系統から参照されている線路は削除できません。連鎖削除は行いません。"
                    .into(),
            );
        }
        candidate.tracks.retain(|track| track.id != track_id);
        self.world
            .replace(candidate)
            .map_err(|error| error.to_string())?;
        Ok(self.snapshot())
    }
    pub fn add_track(&mut self, from: StationId, to: StationId) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate.add_track_unchecked(from, to)?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        *self = candidate;
        Ok(self.snapshot())
    }
    fn add_track_unchecked(
        &mut self,
        from: StationId,
        to: StationId,
    ) -> Result<WorldDraft, String> {
        if from == to {
            return Err("接続元と接続先には別の駅を選んでください。".into());
        }
        let data = self.world.snapshot();
        if !data.stations.iter().any(|s| s.id == from) || !data.stations.iter().any(|s| s.id == to)
        {
            return Err("接続する駅が見つかりません。ワールドを確認してください。".into());
        }
        if data.tracks.iter().any(|track| {
            (track.from == from && track.to == to) || (track.from == to && track.to == from)
        }) {
            return Err("この2駅は既に線路で接続されています。".into());
        }
        self.world
            .add_track(self.company, from, to)
            .map_err(|e| e.to_string())?;
        Ok(self.snapshot())
    }
    pub fn rename_company(&mut self, name: String) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate.rename_company_unchecked(name)?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        *self = candidate;
        Ok(self.snapshot())
    }
    fn rename_company_unchecked(&mut self, name: String) -> Result<WorldDraft, String> {
        if name.trim().is_empty() || name.chars().count() > 80 {
            return Err("会社名は1〜80文字で入力してください。".into());
        }
        let mut candidate = self.snapshot();
        let company = candidate
            .companies
            .iter_mut()
            .find(|c| c.id == self.company)
            .ok_or("編集中の会社が見つかりません。")?;
        company.name = name.trim().to_owned();
        self.world.replace(candidate).map_err(|e| e.to_string())?;
        self.company_setup_required = false;
        Ok(self.snapshot())
    }
    pub fn save_planned_route(
        &mut self,
        route_id: Option<RouteId>,
        name: String,
        start: StationId,
        segments: Vec<rosenfu_core::world::RouteSegment>,
        excluded_stations: Vec<StationId>,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate.save_planned_route_unchecked(
            route_id,
            name,
            start,
            segments,
            excluded_stations,
        )?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        *self = candidate;
        Ok(self.snapshot())
    }
    fn save_planned_route_unchecked(
        &mut self,
        route_id: Option<RouteId>,
        name: String,
        start: StationId,
        segments: Vec<rosenfu_core::world::RouteSegment>,
        excluded_stations: Vec<StationId>,
    ) -> Result<WorldDraft, String> {
        if name.trim().is_empty() || name.chars().count() > 80 {
            return Err("路線名は1〜80文字で入力してください。".into());
        }
        let preview = self
            .world
            .preview_route_membership(start, segments, excluded_stations)
            .map_err(|e| e.to_string())?;
        if let Some(id) = route_id {
            let mut candidate = self.snapshot();
            let route = candidate
                .routes
                .iter_mut()
                .find(|r| r.id == id && r.owner == self.company)
                .ok_or("編集する路線が見つかりません。")?;
            route.name = name.trim().to_owned();
            route.start = start;
            route.tracks.clear();
            candidate.route_paths.retain(|p| p.route_id != id);
            candidate.route_paths.push(rosenfu_core::world::RoutePath {
                route_id: id,
                segments: preview.path.segments,
                member_stations: preview.member_stations,
            });
            self.world.replace(candidate).map_err(|e| e.to_string())?;
        } else {
            self.world
                .add_planned_route(
                    self.company,
                    name.trim().to_owned(),
                    start,
                    preview.path.segments,
                    preview.member_stations,
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(self.snapshot())
    }
    pub fn save_route(
        &mut self,
        route_id: Option<RouteId>,
        name: String,
        start: StationId,
        tracks: Vec<TrackId>,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate.save_route_unchecked(route_id, name, start, tracks)?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        *self = candidate;
        Ok(self.snapshot())
    }
    fn save_route_unchecked(
        &mut self,
        route_id: Option<RouteId>,
        name: String,
        start: StationId,
        tracks: Vec<TrackId>,
    ) -> Result<WorldDraft, String> {
        if name.trim().is_empty() || name.chars().count() > 80 {
            return Err("路線名は1〜80文字で入力してください。".into());
        }
        if tracks.is_empty() {
            return Err("路線には1本以上の線路を指定してください。".into());
        }
        if let Some(id) = route_id {
            let mut candidate = self.snapshot();
            if candidate.route_paths.iter().any(|p| p.route_id == id) {
                return Err(
                    "この路線は所属駅付き経路です。旧式の線路編集では上書きできません。".into(),
                );
            }
            let route = candidate
                .routes
                .iter_mut()
                .find(|r| r.id == id && r.owner == self.company)
                .ok_or("編集する路線が見つかりません。")?;
            route.name = name.trim().to_owned();
            route.start = start;
            route.tracks = tracks;
            self.world.replace(candidate).map_err(|e| e.to_string())?;
        } else {
            self.world
                .add_route(self.company, name.trim().to_owned(), start, tracks)
                .map_err(|e| e.to_string())?;
        }
        Ok(self.snapshot())
    }
    pub fn save_service(
        &mut self,
        service_id: Option<ServiceId>,
        name: String,
        start: StationId,
        tracks: Vec<TrackId>,
        stops: Vec<u32>,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate.save_service_unchecked(service_id, name, start, tracks, stops)?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        *self = candidate;
        Ok(self.snapshot())
    }
    fn save_service_unchecked(
        &mut self,
        service_id: Option<ServiceId>,
        name: String,
        start: StationId,
        tracks: Vec<TrackId>,
        stops: Vec<u32>,
    ) -> Result<WorldDraft, String> {
        if name.trim().is_empty() || name.chars().count() > 80 {
            return Err("系統名は1〜80文字で入力してください。".into());
        }
        if let Some(id) = service_id {
            if self
                .world
                .snapshot()
                .service_paths
                .iter()
                .any(|p| p.service_id == id)
            {
                return Err("建設線形対応の系統は現在、新規作成と参照のみ対応しています。旧式編集では上書きできません。".into());
            }
            let mut candidate = self.snapshot();
            let service = candidate
                .services
                .iter_mut()
                .find(|service| service.id == id && service.owner == self.company)
                .ok_or("編集する運行系統が見つかりません。")?;
            service.name = name.trim().to_owned();
            service.start = start;
            service.tracks = tracks;
            service.stops = stops;
            self.world
                .replace(candidate)
                .map_err(|error| error.to_string())?;
        } else {
            self.world
                .add_service(self.company, name.trim().to_owned(), start, tracks, stops)
                .map_err(|error| error.to_string())?;
        }
        Ok(self.snapshot())
    }
    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        let expected = if self.loaded_path.as_deref() == Some(path) {
            self.manual_version
                .as_ref()
                .ok_or("保存元の確認情報がありません。再読み込みしてください。")?
        } else {
            &rosenfu_storage::FileVersion::Missing
        };
        let result = if self.loaded_path.as_deref() == Some(path) && self.manual_repair_allowed {
            rosenfu_storage::replace_world_if_version(path, &self.world, expected)
        } else {
            rosenfu_storage::save_world_if_version(path, &self.world, expected)
        };
        let (_, version) = result.map_err(|e| e.to_string())?;
        self.loaded_path = Some(path.to_owned());
        self.manual_version = Some(version);
        self.manual_repair_allowed = false;
        Ok(())
    }
    /// Record exactly the version shown to the user, without granting replacement.
    pub fn autosave_status(&mut self, path: &Path) -> Result<bool, String> {
        self.observed_autosave = None;
        let version = match rosenfu_storage::file_version(path) {
            Ok(version) => version,
            Err(error) => {
                self.autosave_version = None;
                self.autosave_blocked = true;
                return Err(error.to_string());
            }
        };
        let available = version != rosenfu_storage::FileVersion::Missing;
        // Rechecking a missing slot authorizes only creation at that observed
        // Missing version. A racing external create is still rejected by CAS.
        let already_authorized = !self.autosave_blocked
            && self.autosave_version.as_ref().is_some_and(
                |(authorized_path, authorized_version)| {
                    authorized_path == path && authorized_version == &version
                },
            );
        if !already_authorized {
            self.autosave_version = if available {
                None
            } else {
                Some((path.to_owned(), version.clone()))
            };
            self.autosave_blocked = available;
            self.autosave_replace_allowed = false;
        }
        self.observed_autosave = Some((path.to_owned(), version));
        Ok(available)
    }
    pub fn allow_autosave_replacement(&mut self, path: &Path) -> Result<(), String> {
        self.autosave_version = None;
        self.autosave_blocked = true;
        self.autosave_replace_allowed = false;
        let (observed_path, observed) = self
            .observed_autosave
            .as_ref()
            .ok_or("オートセーブの状態を再確認してください。")?;
        let current = rosenfu_storage::file_version(path).map_err(|e| e.to_string())?;
        if observed_path != path || &current != observed {
            self.autosave_version = None;
            self.observed_autosave = None;
            self.autosave_blocked = true;
            return Err("確認後にオートセーブが変更されました。再確認してください。".into());
        }
        self.autosave_version = Some((path.to_owned(), current));
        self.autosave_replace_allowed = true;
        self.autosave_blocked = false;
        Ok(())
    }
    pub fn autosave(&mut self, path: &Path) -> Result<(), String> {
        let expected = match &self.autosave_version {
            Some((authorized_path, version)) if authorized_path == path => version.clone(),
            _ if !self.autosave_blocked => rosenfu_storage::FileVersion::Missing,
            _ => return Err("オートセーブの置換を再確認してください。".into()),
        };
        let result = if self.autosave_replace_allowed {
            rosenfu_storage::replace_world_if_version(path, &self.world, &expected)
        } else {
            rosenfu_storage::save_world_if_version(path, &self.world, &expected)
        };
        match result {
            Ok((_, version)) => {
                self.autosave_version = Some((path.to_owned(), version));
                self.autosave_replace_allowed = false;
                Ok(())
            }
            Err(error) => {
                self.autosave_version = None;
                self.observed_autosave = None;
                self.autosave_blocked = true;
                Err(error.to_string())
            }
        }
    }
    /// Restore atomically, retaining normal backup behavior for a healthy manual save.
    pub fn load_autosave(
        &mut self,
        autosave_path: &Path,
        manual_path: &Path,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate.load(autosave_path)?;
        let auto_version = candidate
            .manual_version
            .clone()
            .ok_or("復元元が確認できません。")?;
        let manual_version =
            rosenfu_storage::file_version(manual_path).map_err(|e| e.to_string())?;
        let repair_allowed = if manual_version == rosenfu_storage::FileVersion::Missing {
            false
        } else {
            match rosenfu_storage::load_world_with_version(manual_path) {
                Ok((_, loaded_version)) if loaded_version == manual_version => false,
                Ok(_) => return Err(rosenfu_storage::StorageError::FileChanged.to_string()),
                Err(
                    rosenfu_storage::StorageError::Json(_)
                    | rosenfu_storage::StorageError::InvalidHeader
                    | rosenfu_storage::StorageError::InvalidFormat
                    | rosenfu_storage::StorageError::InvalidWorld(_)
                    | rosenfu_storage::StorageError::TooLarge,
                ) => {
                    // The decoding failure must describe the same bytes whose
                    // replacement is authorized, not a concurrent newer file.
                    let current =
                        rosenfu_storage::file_version(manual_path).map_err(|e| e.to_string())?;
                    if current != manual_version {
                        return Err(rosenfu_storage::StorageError::FileChanged.to_string());
                    }
                    true
                }
                // Permission/I/O failures and newer schemas are not corruption.
                Err(error) => return Err(error.to_string()),
            }
        };
        candidate.loaded_path = Some(manual_path.to_owned());
        candidate.manual_version = Some(manual_version);
        candidate.manual_repair_allowed = repair_allowed;
        candidate.autosave_version = Some((autosave_path.to_owned(), auto_version));
        candidate.autosave_blocked = false;
        candidate.autosave_replace_allowed = false;
        *self = candidate;
        Ok(self.snapshot())
    }
    pub fn load(&mut self, path: &Path) -> Result<WorldDraft, String> {
        let (candidate, version) =
            rosenfu_storage::load_world_with_version(path).map_err(|e| e.to_string())?;
        let data = candidate.snapshot();
        if data.players.len() != 1 || data.companies.len() != 1 {
            return Err(
                "この試作画面ではプレイヤー1人・会社1社のワールドのみ読み込めます。".into(),
            );
        }
        if data
            .stations
            .iter()
            .any(|s| !in_edit_extent(s.latitude, s.longitude))
        {
            return Err("編集範囲外の駅を含むため、この試作画面では読み込めません。".into());
        }
        if data
            .alignments
            .iter()
            .flat_map(|a| &a.points)
            .any(|p| !in_edit_extent(p.latitude, p.longitude))
        {
            return Err("編集範囲外の路線を含むため読み込めません。".into());
        }
        let company = data.companies[0].id;
        self.world = candidate;
        self.company = company;
        self.company_setup_required = false;
        self.loaded_path = Some(path.to_owned());
        self.manual_version = Some(version);
        self.manual_repair_allowed = false;
        self.autosave_version = None;
        self.observed_autosave = None;
        self.autosave_blocked = false;
        self.autosave_replace_allowed = false;
        Ok(self.snapshot())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn same_version_status_preserves_restored_and_saved_autosave_permission() {
        let dir = tempfile::tempdir().unwrap();
        let auto = dir.path().join("auto.json");
        let manual = dir.path().join("manual.json");
        WorldEditor::new().autosave(&auto).unwrap();
        let mut restored = WorldEditor::new();
        restored.load_autosave(&auto, &manual).unwrap();
        assert!(restored.autosave_status(&auto).unwrap());
        restored.rename_company("After restore".into()).unwrap();
        restored.autosave(&auto).unwrap();
        assert!(restored.autosave_status(&auto).unwrap());
        restored.rename_company("After save".into()).unwrap();
        restored.autosave(&auto).unwrap();
        std::fs::write(&auto, b"external different version").unwrap();
        assert!(restored.autosave_status(&auto).unwrap());
        assert!(restored.autosave(&auto).is_err());
        assert_eq!(std::fs::read(&auto).unwrap(), b"external different version");
    }
    #[test]
    fn restored_autosave_preserves_healthy_manual_backup_and_creates_missing_manual() {
        let dir = tempfile::tempdir().unwrap();
        let manual = dir.path().join("manual.json");
        let auto = dir.path().join("auto.json");
        let mut original = WorldEditor::new();
        original.save(&manual).unwrap();
        let old = std::fs::read(&manual).unwrap();
        original.rename_company("Recovered".into()).unwrap();
        original.autosave(&auto).unwrap();
        let mut restored = WorldEditor::new();
        restored.load_autosave(&auto, &manual).unwrap();
        assert!(!restored.manual_repair_allowed);
        restored.save(&manual).unwrap();
        assert_eq!(
            std::fs::read(rosenfu_storage::backup_path(&manual)).unwrap(),
            old
        );
        let missing = dir.path().join("missing.json");
        restored.load_autosave(&auto, &missing).unwrap();
        assert!(!restored.manual_repair_allowed);
        restored.save(&missing).unwrap();
        assert!(!rosenfu_storage::backup_path(&missing).exists());
    }
    #[test]
    fn restored_autosave_does_not_authorize_replacing_newer_schema() {
        let dir = tempfile::tempdir().unwrap();
        let manual = dir.path().join("manual.json");
        let auto = dir.path().join("auto.json");
        WorldEditor::new().autosave(&auto).unwrap();
        let bytes = std::fs::read_to_string(&auto).unwrap();
        let newer = bytes.replace("\"schema_version\": 7", "\"schema_version\": 999");
        assert_ne!(bytes, newer);
        std::fs::write(&manual, &newer).unwrap();
        let mut editor = WorldEditor::new();
        let before = editor.snapshot();
        assert!(editor.load_autosave(&auto, &manual).is_err());
        assert_eq!(editor.snapshot(), before);
        assert!(editor.loaded_path.is_none());
        assert_eq!(std::fs::read_to_string(&manual).unwrap(), newer);
    }
    #[test]
    fn failed_load_keeps_manual_baseline_and_failed_status_blocks_autosave() {
        let dir = tempfile::tempdir().unwrap();
        let manual = dir.path().join("manual.json");
        let auto = dir.path().join("auto.json");
        let mut editor = WorldEditor::new();
        editor.save(&manual).unwrap();
        let before = editor.snapshot();
        assert!(editor.load(&auto).is_err());
        assert_eq!(editor.snapshot(), before);
        editor.save(&manual).unwrap();
        std::fs::create_dir(&auto).unwrap();
        assert!(editor.autosave_status(&auto).is_err());
        std::fs::remove_dir(&auto).unwrap();
        assert!(editor.autosave(&auto).is_err());
        assert!(!editor.autosave_status(&auto).unwrap());
        editor.autosave(&auto).unwrap();
    }
    #[test]
    fn replacement_confirmation_cannot_overwrite_a_later_change() {
        let dir = tempfile::tempdir().unwrap();
        let auto = dir.path().join("auto.json");
        std::fs::write(&auto, b"broken old save").unwrap();
        let mut editor = WorldEditor::new();
        assert!(editor.autosave_status(&auto).unwrap());
        editor.allow_autosave_replacement(&auto).unwrap();
        std::fs::write(&auto, b"later save").unwrap();
        assert!(editor.autosave(&auto).is_err());
        assert_eq!(std::fs::read(&auto).unwrap(), b"later save");
        assert!(editor.autosave_status(&auto).unwrap());
        editor.allow_autosave_replacement(&auto).unwrap();
        editor.autosave(&auto).unwrap();
        assert_eq!(
            rosenfu_storage::load_world(&auto).unwrap().snapshot(),
            &editor.snapshot()
        );
    }
    #[test]
    fn save_conflicts_and_external_deletion_do_not_replace_session_or_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manual.json");
        let mut first = WorldEditor::new();
        first.save(&path).unwrap();
        let mut second = WorldEditor::new();
        second.load(&path).unwrap();
        second.rename_company("Other writer".into()).unwrap();
        second.save(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let before = first.snapshot();
        assert!(first.save(&path).is_err());
        assert_eq!(first.snapshot(), before);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_file(&path).unwrap();
        assert!(second.save(&path).is_err());
        assert!(!path.exists());
    }
    #[test]
    fn autosave_replacement_requires_fresh_observation_and_survives_no_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auto.json");
        let mut first = WorldEditor::new();
        first.autosave(&path).unwrap();
        let mut second = WorldEditor::new();
        assert!(second.autosave(&path).is_err());
        assert!(second.allow_autosave_replacement(&path).is_err());
        assert!(second.autosave_status(&path).unwrap());
        first.rename_company("Changed".into()).unwrap();
        first.autosave(&path).unwrap();
        assert!(second.allow_autosave_replacement(&path).is_err());
        assert!(second.autosave_status(&path).unwrap());
        second.allow_autosave_replacement(&path).unwrap();
        second.autosave(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(second.autosave(&path).is_err());
        assert!(second.autosave(&path).is_err());
        assert!(!second.autosave_status(&path).unwrap());
        second.autosave(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(!second.autosave_status(&path).unwrap());
        std::fs::write(&path, b"external create").unwrap();
        assert!(second.autosave(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"external create");
    }
    #[test]
    fn restore_can_repair_invalid_and_oversized_manual_file_without_early_write() {
        let dir = tempfile::tempdir().unwrap();
        let manual = dir.path().join("manual.json");
        let auto = dir.path().join("auto.json");
        WorldEditor::new().autosave(&auto).unwrap();
        for bytes in [
            b"broken JSON".to_vec(),
            vec![b'x'; rosenfu_storage::MAX_SAVE_BYTES + 1],
        ] {
            std::fs::write(&manual, &bytes).unwrap();
            let mut restored = WorldEditor::new();
            restored.load_autosave(&auto, &manual).unwrap();
            assert_eq!(std::fs::read(&manual).unwrap(), bytes);
            restored.save(&manual).unwrap();
            assert_eq!(
                rosenfu_storage::load_world(&manual).unwrap().snapshot(),
                &restored.snapshot()
            );
        }
    }
    #[test]
    fn oversized_edits_preserve_world_and_setup_state() {
        let mut editor = WorldEditor::new();
        let mut huge = editor.snapshot();
        huge.players[0].name = "x".repeat(rosenfu_storage::MAX_SAVE_BYTES);
        editor.world.replace(huge).unwrap();
        let before = editor.snapshot();
        assert!(editor.rename_company("Company".into()).is_err());
        assert!(editor.company_setup_required());
        assert!(editor.add_station("Station".into(), 35.0, 139.0).is_err());
        assert!(editor
            .build_alignment(
                "Track".into(),
                vec![
                    GeoPoint {
                        latitude: 35.0,
                        longitude: 139.0
                    },
                    GeoPoint {
                        latitude: 35.1,
                        longitude: 139.1
                    }
                ]
            )
            .is_err());
        assert_eq!(editor.snapshot(), before);
    }
    #[test]
    fn autosave_is_separate_and_does_not_claim_the_manual_save() {
        let dir = tempfile::tempdir().unwrap();
        let manual = dir.path().join("world.json");
        let auto = dir.path().join("world.autosave.json");
        let mut original = WorldEditor::new();
        original.rename_company("手動保存の会社".into()).unwrap();
        original.save(&manual).unwrap();
        let manual_bytes = std::fs::read(&manual).unwrap();
        let mut fresh = WorldEditor::new();
        fresh.rename_company("新しい会社".into()).unwrap();
        fresh.autosave(&auto).unwrap();
        assert!(fresh.loaded_path.is_none());
        assert!(fresh.save(&manual).is_err());
        assert_eq!(std::fs::read(&manual).unwrap(), manual_bytes);
        let first = fresh.snapshot();
        fresh.add_station("追加駅".into(), 35.67, 139.75).unwrap();
        fresh.autosave(&auto).unwrap();
        assert_eq!(
            rosenfu_storage::load_world(rosenfu_storage::backup_path(&auto))
                .unwrap()
                .snapshot(),
            &first
        );
        assert_eq!(
            rosenfu_storage::load_world(&auto).unwrap().snapshot(),
            &fresh.snapshot()
        );
        assert_eq!(std::fs::read(&manual).unwrap(), manual_bytes);
    }
    #[test]
    fn restoring_autosave_is_atomic_and_explicit_save_targets_manual() {
        let dir = tempfile::tempdir().unwrap();
        let manual = dir.path().join("world.json");
        let auto = dir.path().join("world.autosave.json");
        let mut editor = WorldEditor::new();
        editor.rename_company("保存済み".into()).unwrap();
        editor.save(&manual).unwrap();
        let manual_bytes = std::fs::read(&manual).unwrap();
        editor
            .add_station("自動保存の駅".into(), 35.67, 139.75)
            .unwrap();
        editor.autosave(&auto).unwrap();
        let expected = editor.snapshot();
        let auto_bytes = std::fs::read(&auto).unwrap();
        let mut restored = WorldEditor::new();
        assert_eq!(restored.load_autosave(&auto, &manual).unwrap(), expected);
        assert!(!restored.company_setup_required());
        assert_eq!(restored.loaded_path.as_deref(), Some(manual.as_path()));
        assert_eq!(std::fs::read(&manual).unwrap(), manual_bytes);
        restored
            .add_station("復元後の駅".into(), 35.68, 139.76)
            .unwrap();
        restored.save(&manual).unwrap();
        assert_eq!(std::fs::read(&auto).unwrap(), auto_bytes);
        assert_eq!(
            rosenfu_storage::load_world(&manual).unwrap().snapshot(),
            &restored.snapshot()
        );
        let before = restored.snapshot();
        let before_path = restored.loaded_path.clone();
        std::fs::write(&auto, b"corrupt").unwrap();
        assert!(restored.load_autosave(&auto, &manual).is_err());
        assert_eq!(restored.snapshot(), before);
        assert_eq!(restored.loaded_path, before_path);
        assert!(!restored.company_setup_required());
        assert!(restored.autosave(&auto).is_err());
        assert_eq!(std::fs::read(&auto).unwrap(), b"corrupt");
        assert_eq!(restored.snapshot(), before);
    }
    #[test]
    fn company_setup_tracks_successful_rename_or_load_only() {
        let mut editor = WorldEditor::new();
        assert!(editor.company_setup_required());
        assert!(editor.rename_company(" ".into()).is_err());
        assert!(editor.company_setup_required());
        editor.rename_company("自社鉄道".into()).unwrap();
        assert!(!editor.company_setup_required());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("setup.json");
        editor.save(&path).unwrap();
        let mut fresh = WorldEditor::new();
        assert!(fresh.load(&dir.path().join("missing.json")).is_err());
        assert!(fresh.company_setup_required());
        fresh.load(&path).unwrap();
        assert!(!fresh.company_setup_required());
    }

    #[test]
    fn company_and_route_editing_preserve_ids_and_roundtrip() {
        let mut editor = WorldEditor::new();
        let company_id = editor.snapshot().companies[0].id;
        editor.rename_company("  路線譜鉄道  ".into()).unwrap();
        let a = editor
            .add_station("A".into(), 35.67, 139.71)
            .unwrap()
            .stations[0]
            .id;
        let b = editor
            .add_station("B".into(), 35.68, 139.75)
            .unwrap()
            .stations[1]
            .id;
        let c = editor
            .add_station("C".into(), 35.69, 139.8)
            .unwrap()
            .stations[2]
            .id;
        let ab = editor.add_track(a, b).unwrap().tracks[0].id;
        let bc = editor.add_track(c, b).unwrap().tracks[1].id;
        let created = editor
            .save_route(None, "本線".into(), a, vec![ab, bc])
            .unwrap();
        let id = created.routes[0].id;
        let next_id = created.next_id;
        let edited = editor
            .save_route(Some(id), "新本線".into(), c, vec![bc, ab])
            .unwrap();
        assert_eq!(edited.routes[0].id, id);
        assert_eq!(edited.routes[0].tracks, vec![bc, ab]);
        assert_eq!(edited.next_id, next_id);
        assert_eq!(edited.tracks, created.tracks);
        assert_eq!(edited.stations, created.stations);
        assert_eq!(edited.companies[0].id, company_id);
        assert_eq!(edited.companies[0].name, "路線譜鉄道");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        editor.save(&path).unwrap();
        assert_eq!(WorldEditor::new().load(&path).unwrap(), edited);
    }
    #[test]
    fn invalid_company_or_route_edit_does_not_change_world() {
        let mut editor = WorldEditor::new();
        let a = editor
            .add_station("A".into(), 35.67, 139.71)
            .unwrap()
            .stations[0]
            .id;
        let b = editor
            .add_station("B".into(), 35.68, 139.75)
            .unwrap()
            .stations[1]
            .id;
        let c = editor
            .add_station("C".into(), 35.69, 139.8)
            .unwrap()
            .stations[2]
            .id;
        let ab = editor.add_track(a, b).unwrap().tracks[0].id;
        let before = editor.snapshot();
        assert!(editor.rename_company(" ".into()).is_err());
        assert!(editor.rename_company("A".repeat(81)).is_err());
        assert!(editor.save_route(None, "空".into(), a, vec![]).is_err());
        assert!(editor
            .save_route(None, "不連続".into(), c, vec![ab])
            .is_err());
        assert!(editor
            .save_route(None, "不明".into(), a, vec![TrackId(999)])
            .is_err());
        assert!(editor
            .save_route(Some(RouteId(999)), "編集".into(), a, vec![ab])
            .is_err());
        assert_eq!(editor.snapshot(), before);
        let created = editor.save_route(None, "本線".into(), a, vec![ab]).unwrap();
        let id = created.routes[0].id;
        assert!(editor
            .save_route(Some(id), "不連続".into(), c, vec![ab])
            .is_err());
        assert_eq!(editor.snapshot(), created);
    }
    #[test]
    fn connect_save_and_reload_preserve_tracks() {
        let mut editor = WorldEditor::new();
        let a = editor
            .add_station("A".into(), 35.67, 139.72)
            .unwrap()
            .stations[0]
            .id;
        let b = editor
            .add_station("B".into(), 35.68, 139.78)
            .unwrap()
            .stations[1]
            .id;
        let connected = editor.add_track(a, b).unwrap();
        assert_eq!(connected.tracks.len(), 1);
        assert_eq!(connected.tracks[0].from, a);
        assert_eq!(connected.tracks[0].to, b);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        editor.save(&path).unwrap();
        let mut restored = WorldEditor::new();
        assert_eq!(restored.load(&path).unwrap(), connected);
        assert_eq!(restored.snapshot().next_id, connected.next_id);
    }
    #[test]
    fn self_loop_duplicate_and_missing_station_do_not_mutate_or_allocate() {
        let mut editor = WorldEditor::new();
        let a = editor
            .add_station("A".into(), 35.67, 139.72)
            .unwrap()
            .stations[0]
            .id;
        let b = editor
            .add_station("B".into(), 35.68, 139.78)
            .unwrap()
            .stations[1]
            .id;
        let before = editor.snapshot();
        assert!(editor.add_track(a, a).is_err());
        assert!(editor.add_track(a, StationId(999)).is_err());
        assert_eq!(editor.snapshot(), before);
        let connected = editor.add_track(a, b).unwrap();
        assert!(editor.add_track(a, b).is_err());
        assert!(editor.add_track(b, a).is_err());
        assert_eq!(editor.snapshot(), connected);
    }
    #[test]
    fn place_save_and_reload_preserve_stations_and_next_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        let mut editor = WorldEditor::new();
        editor.add_station("中央".into(), 35.675, 139.75).unwrap();
        editor.save(&path).unwrap();
        let expected = editor.snapshot();
        let mut restored = WorldEditor::new();
        assert_eq!(restored.load(&path).unwrap(), expected);
        restored.add_station("東".into(), 35.675, 139.8).unwrap();
        restored.save(&path).unwrap();
        assert_eq!(restored.snapshot().stations.len(), 2);
        assert_eq!(
            rosenfu_storage::load_world(rosenfu_storage::backup_path(&path))
                .unwrap()
                .snapshot(),
            &expected
        );
    }
    #[test]
    fn invalid_placement_and_load_leave_current_world_unchanged() {
        let mut editor = WorldEditor::new();
        let before = editor.snapshot();
        assert!(editor.add_station(" ".into(), 35.67, 139.75).is_err());
        assert!(editor.add_station("外".into(), 0.0, 0.0).is_err());
        assert!(editor.add_station("外".into(), f64::NAN, 139.75).is_err());
        assert_eq!(editor.snapshot(), before);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        std::fs::write(&path, b"corrupt").unwrap();
        assert!(editor.load(&path).is_err());
        assert_eq!(editor.snapshot(), before);
        rosenfu_storage::create_world(dir.path().join("empty.json"), &World::default()).unwrap();
        assert!(editor.load(&dir.path().join("empty.json")).is_err());
        assert_eq!(editor.snapshot(), before);
    }
    #[test]
    fn outside_edit_extent_cannot_replace_current_session() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("outside.json");
        let mut outside = World::default();
        let p = outside.add_player("A".into()).unwrap();
        let c = outside.add_company(p, "Company".into()).unwrap();
        outside.add_station(c, "Outside".into(), 0.0, 0.0).unwrap();
        rosenfu_storage::create_world(&path, &outside).unwrap();
        let mut editor = WorldEditor::new();
        let before = editor.snapshot();
        assert!(editor.load(&path).is_err());
        assert_eq!(editor.snapshot(), before);
    }
    #[test]
    fn distant_regions_roundtrip_without_changing_world_coordinates_or_schema() {
        let mut editor = WorldEditor::new();
        for (name, lat, lon) in [
            ("札幌", 43.0687, 141.3508),
            ("大阪", 34.7025, 135.4959),
            ("那覇", 26.2124, 127.6809),
            ("南西端", 20.0, 122.0),
            ("北東端", 46.0, 154.0),
        ] {
            editor.add_station(name.into(), lat, lon).unwrap();
        }
        let expected = editor.snapshot();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("regions.json");
        editor.save(&path).unwrap();
        assert_eq!(WorldEditor::new().load(&path).unwrap(), expected);
        for (lat, lon) in [
            (19.99, 130.0),
            (46.01, 130.0),
            (35.0, 121.99),
            (35.0, 154.01),
            (f64::INFINITY, 139.0),
        ] {
            assert!(editor.add_station("外".into(), lat, lon).is_err());
        }
        assert_eq!(editor.snapshot(), expected);
    }
    #[test]
    fn service_data_survives_editor_load_route_and_company_edits_and_save() {
        let mut source = World::default();
        let player = source.add_player("P".into()).unwrap();
        let company = source.add_company(player, "C".into()).unwrap();
        let a = source
            .add_station(company, "A".into(), 35.67, 139.71)
            .unwrap();
        let b = source
            .add_station(company, "B".into(), 35.68, 139.75)
            .unwrap();
        let track = source.add_track(company, a, b).unwrap();
        let route = source
            .add_route(company, "路線".into(), a, vec![track])
            .unwrap();
        source
            .add_service(company, "系統".into(), a, vec![track], vec![0, 1])
            .unwrap();
        let services = source.snapshot().services.clone();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("services.json");
        rosenfu_storage::create_world(&path, &source).unwrap();
        let mut editor = WorldEditor::new();
        editor.load(&path).unwrap();
        editor.rename_company("新会社名".into()).unwrap();
        editor
            .save_route(Some(route), "逆方向の路線".into(), b, vec![track])
            .unwrap();
        editor
            .add_station("新駅".into(), 34.7025, 135.4959)
            .unwrap();
        editor.save(&path).unwrap();
        assert_eq!(editor.snapshot().services, services);
        assert_eq!(WorldEditor::new().load(&path).unwrap().services, services);
    }
    #[test]
    fn service_editing_preserves_identity_and_repeated_stop_occurrences() {
        let mut editor = WorldEditor::new();
        let a = editor
            .add_station("A".into(), 35.67, 139.71)
            .unwrap()
            .stations[0]
            .id;
        let b = editor
            .add_station("B".into(), 35.68, 139.75)
            .unwrap()
            .stations[1]
            .id;
        let track = editor.add_track(a, b).unwrap().tracks[0].id;
        let created = editor
            .save_service(
                None,
                "  循環  ".into(),
                a,
                vec![track, track],
                vec![0, 1, 2],
            )
            .unwrap();
        let id = created.services[0].id;
        assert_eq!(created.services[0].name, "循環");
        let edited = editor
            .save_service(Some(id), "B通過".into(), a, vec![track, track], vec![0, 2])
            .unwrap();
        assert_eq!(edited.services[0].id, id);
        assert_eq!(edited.services[0].stops, vec![0, 2]);
        assert_eq!(edited.next_id, created.next_id);
        assert_eq!(edited.stations, created.stations);
        assert_eq!(edited.tracks, created.tracks);
        assert_eq!(edited.routes, created.routes);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("service-edit.json");
        editor.save(&path).unwrap();
        assert_eq!(WorldEditor::new().load(&path).unwrap(), edited);
    }
    #[test]
    fn invalid_service_edits_leave_world_and_allocator_unchanged() {
        let mut editor = WorldEditor::new();
        let a = editor
            .add_station("A".into(), 35.67, 139.71)
            .unwrap()
            .stations[0]
            .id;
        let b = editor
            .add_station("B".into(), 35.68, 139.75)
            .unwrap()
            .stations[1]
            .id;
        let track = editor.add_track(a, b).unwrap().tracks[0].id;
        let before = editor.snapshot();
        for (name, tracks, stops) in [
            (" ".to_owned(), vec![track], vec![0, 1]),
            ("A".repeat(81), vec![track], vec![0, 1]),
            ("empty".into(), vec![], vec![0]),
            ("bad-track".into(), vec![TrackId(999)], vec![0, 1]),
            ("missing-end".into(), vec![track], vec![0]),
            ("unsorted".into(), vec![track], vec![1, 0]),
        ] {
            assert!(editor.save_service(None, name, a, tracks, stops).is_err());
            assert_eq!(editor.snapshot(), before);
        }
        assert!(editor
            .save_service(
                Some(ServiceId(999)),
                "unknown".into(),
                a,
                vec![track],
                vec![0, 1]
            )
            .is_err());
        assert_eq!(editor.snapshot(), before);
        let created = editor
            .save_service(None, "valid".into(), a, vec![track], vec![0, 1])
            .unwrap();
        assert!(editor
            .save_service(
                Some(created.services[0].id),
                "bad".into(),
                a,
                vec![track],
                vec![0, 2]
            )
            .is_err());
        assert_eq!(editor.snapshot(), created);
    }
    #[test]
    fn station_rename_and_unreferenced_deletion_preserve_ids_and_roundtrip() {
        let mut editor = WorldEditor::new();
        let a = editor
            .add_station("A".into(), 35.67, 139.71)
            .unwrap()
            .stations[0]
            .id;
        let b = editor
            .add_station("B".into(), 35.68, 139.75)
            .unwrap()
            .stations[1]
            .id;
        let track = editor.add_track(a, b).unwrap().tracks[0].id;
        let before = editor.snapshot();
        let renamed = editor.rename_station(a, "  中央  ".into()).unwrap();
        assert_eq!(renamed.stations[0].id, a);
        assert_eq!(renamed.stations[0].name, "中央");
        assert_eq!(renamed.stations[0].latitude, before.stations[0].latitude);
        assert_eq!(renamed.next_id, before.next_id);
        assert_eq!(renamed.tracks, before.tracks);
        editor.delete_track(track).unwrap();
        editor.delete_station(b).unwrap();
        let deleted = editor.snapshot();
        assert_eq!(deleted.next_id, before.next_id);
        assert_eq!(deleted.stations.len(), 1);
        assert!(deleted.tracks.is_empty());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("edited.json");
        editor.save(&path).unwrap();
        let mut restored = WorldEditor::new();
        assert_eq!(restored.load(&path).unwrap(), deleted);
        let created = restored.add_station("new".into(), 35.68, 139.78).unwrap();
        assert!(created.stations[1].id.0 > track.0);
    }
    #[test]
    fn referenced_deletions_and_invalid_station_edits_are_atomic() {
        let mut editor = WorldEditor::new();
        let a = editor
            .add_station("A".into(), 35.67, 139.71)
            .unwrap()
            .stations[0]
            .id;
        let b = editor
            .add_station("B".into(), 35.68, 139.75)
            .unwrap()
            .stations[1]
            .id;
        let track = editor.add_track(a, b).unwrap().tracks[0].id;
        let before = editor.snapshot();
        for name in [" ".to_owned(), "A".repeat(81)] {
            assert!(editor.rename_station(a, name).is_err());
        }
        assert!(editor
            .rename_station(StationId(999), "Missing".into())
            .is_err());
        assert!(editor.delete_station(a).is_err());
        assert!(editor.delete_station(StationId(999)).is_err());
        assert!(editor.delete_track(TrackId(999)).is_err());
        assert_eq!(editor.snapshot(), before);
        editor
            .save_route(None, "Route".into(), a, vec![track])
            .unwrap();
        let with_route = editor.snapshot();
        assert!(editor.delete_track(track).is_err());
        assert_eq!(editor.snapshot(), with_route);
        // Independently verify a service reference also protects a track.
        let mut candidate = editor.snapshot();
        candidate.routes.clear();
        editor.world.replace(candidate).unwrap();
        editor
            .save_service(None, "Service".into(), a, vec![track, track], vec![0, 2])
            .unwrap();
        let with_service = editor.snapshot();
        assert!(editor.delete_track(track).is_err());
        assert!(editor.delete_station(b).is_err());
        assert_eq!(editor.snapshot(), with_service);
        let renamed = editor.rename_station(a, "Renamed".into()).unwrap();
        assert_eq!(renamed.services, with_service.services);
    }
    #[test]
    fn fresh_session_cannot_overwrite_existing_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        let mut first = WorldEditor::new();
        first.save(&path).unwrap();
        let original = std::fs::read(&path).unwrap();
        let mut other = WorldEditor::new();
        assert!(other.save(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(!rosenfu_storage::backup_path(&path).exists());
    }
    #[test]
    fn planned_routes_persist_membership_and_reject_legacy_overwrite_and_deletion() {
        use rosenfu_core::world::RouteSegment;
        let mut editor = WorldEditor::new();
        let a = editor
            .world
            .build_alignment(
                editor.company,
                "A".into(),
                vec![
                    GeoPoint {
                        latitude: 35.0,
                        longitude: 139.0,
                    },
                    GeoPoint {
                        latitude: 35.0,
                        longitude: 139.1,
                    },
                ],
            )
            .unwrap();
        let stations = [0.1, 0.5, 0.9].map(|position| {
            editor
                .world
                .place_station_on_alignment(editor.company, "S".into(), a, position, 100.0, 10.0)
                .unwrap()
        });
        let segments = vec![RouteSegment::Alignment {
            alignment_id: a,
            from: stations[0],
            to: stations[2],
        }];
        let created = editor
            .save_planned_route(
                None,
                " R ".into(),
                stations[0],
                segments.clone(),
                vec![stations[1]],
            )
            .unwrap();
        let id = created.routes[0].id;
        assert_eq!(
            created.route_paths[0].member_stations,
            vec![stations[0], stations[2]]
        );
        assert_eq!(created.routes[0].name, "R");
        assert!(editor
            .save_route(Some(id), "Bad".into(), stations[0], vec![])
            .is_err());
        assert!(editor
            .save_planned_route(
                Some(id),
                "Bad".into(),
                stations[0],
                segments.clone(),
                vec![stations[0]]
            )
            .is_err());
        assert!(editor.delete_station(stations[2]).is_err());
        assert_eq!(editor.snapshot(), created);
        let edited = editor
            .save_planned_route(Some(id), "Edited".into(), stations[0], segments, vec![])
            .unwrap();
        assert_eq!(edited.route_paths[0].member_stations, stations);
        assert!(editor.delete_station(stations[1]).is_err());
        assert_eq!(editor.snapshot(), edited);
        let added = editor
            .world
            .place_station_on_alignment(editor.company, "New".into(), a, 0.3, 100.0, 10.0)
            .unwrap();
        let before = editor.snapshot();
        assert!(!before.route_paths[0].member_stations.contains(&added));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("planned.json");
        editor.save(&path).unwrap();
        let mut restored = WorldEditor::new();
        assert_eq!(restored.load(&path).unwrap(), before);
    }
    #[test]
    fn planned_track_routes_guard_references_without_changing_services() {
        use rosenfu_core::world::RouteSegment;
        let mut editor = WorldEditor::new();
        let a = editor
            .add_station("A".into(), 35.0, 139.0)
            .unwrap()
            .stations[0]
            .id;
        let b = editor
            .add_station("B".into(), 35.0, 139.1)
            .unwrap()
            .stations[1]
            .id;
        let t = editor.add_track(a, b).unwrap().tracks[0].id;
        editor
            .save_service(None, "S".into(), a, vec![t], vec![0, 1])
            .unwrap();
        let services = editor.snapshot().services;
        let saved = editor
            .save_planned_route(
                None,
                "R".into(),
                a,
                vec![RouteSegment::Track { track_id: t }],
                vec![],
            )
            .unwrap();
        assert_eq!(saved.services, services);
        // Remove service so the sidecar is the only Track reference.
        let mut only_route = saved;
        only_route.services.clear();
        editor.world.replace(only_route).unwrap();
        let before = editor.snapshot();
        assert!(editor.delete_track(t).is_err());
        assert_eq!(editor.snapshot(), before);
    }
    #[test]
    fn route_to_service_preview_does_not_save_or_choose_stops() {
        let mut editor = WorldEditor::new();
        let a = editor
            .add_station("A".into(), 35.0, 139.0)
            .unwrap()
            .stations[0]
            .id;
        let b = editor
            .add_station("B".into(), 35.0, 139.1)
            .unwrap()
            .stations[1]
            .id;
        let track = editor.add_track(a, b).unwrap().tracks[0].id;
        let world = editor
            .save_route(None, "Route".into(), a, vec![track])
            .unwrap();
        let preview = editor
            .preview_service_from_route(world.routes[0].id, 1, 0)
            .unwrap();
        assert_eq!(preview.start, b);
        assert_eq!(preview.tracks, vec![track]);
        assert!(preview.requires_stop_selection);
        assert!(editor
            .preview_service_from_route(world.routes[0].id, 0, 99)
            .is_err());
        assert_eq!(editor.snapshot(), world);
    }
    #[test]
    fn new_service_paths_reject_old_edit_and_deletion_and_roundtrip() {
        use rosenfu_core::world::{RouteSegment, StationOccurrenceRef};
        let mut editor = WorldEditor::new();
        let a = editor
            .world
            .build_alignment(
                editor.company,
                "A".into(),
                vec![
                    GeoPoint {
                        latitude: 35.0,
                        longitude: 139.0,
                    },
                    GeoPoint {
                        latitude: 35.0,
                        longitude: 139.1,
                    },
                ],
            )
            .unwrap();
        let stations = [0.1, 0.5, 0.9].map(|position| {
            editor
                .world
                .place_station_on_alignment(editor.company, "S".into(), a, position, 100.0, 10.0)
                .unwrap()
        });
        let segments = vec![RouteSegment::Alignment {
            alignment_id: a,
            from: stations[0],
            to: stations[2],
        }];
        let preview = editor
            .preview_service_path(stations[0], segments.clone())
            .unwrap();
        let before = editor.snapshot();
        assert!(editor
            .create_service_path(
                "Bad".into(),
                stations[0],
                segments.clone(),
                vec![StationOccurrenceRef::Start]
            )
            .is_err());
        assert_eq!(editor.snapshot(), before);
        let stops = preview
            .occurrences
            .into_iter()
            .map(|o| o.reference)
            .collect::<Vec<_>>();
        let created = editor
            .create_service_path("New".into(), stations[0], segments, stops.clone())
            .unwrap();
        assert!(editor
            .save_service(
                Some(created.services[0].id),
                "Bad".into(),
                stations[0],
                vec![],
                vec![]
            )
            .is_err());
        assert!(editor.delete_station(stations[1]).is_err());
        assert!(editor.delete_station(stations[2]).is_err());
        assert_eq!(editor.snapshot(), created);
        editor
            .world
            .place_station_on_alignment(editor.company, "Added".into(), a, 0.3, 100.0, 10.0)
            .unwrap();
        assert_eq!(editor.snapshot().service_paths[0].stops, stops);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new-path.json");
        editor.save(&path).unwrap();
        assert_eq!(WorldEditor::new().load(&path).unwrap(), editor.snapshot());
        // A physical Track may be referenced solely by a ServicePath.
        let track = editor.add_track(stations[0], stations[2]).unwrap().tracks[0].id;
        let segments = vec![RouteSegment::Track { track_id: track }];
        let stops = editor
            .preview_service_path(stations[0], segments.clone())
            .unwrap()
            .occurrences
            .into_iter()
            .map(|o| o.reference)
            .collect();
        editor
            .create_service_path("Track".into(), stations[0], segments, stops)
            .unwrap();
        let before = editor.snapshot();
        assert!(editor.delete_track(track).is_err());
        assert_eq!(editor.snapshot(), before);
    }
    #[test]
    fn reference_geometry_and_provenance_are_atomic_and_roundtrip() {
        use rosenfu_core::world::{
            AlignmentProvenanceInput, ProvenanceMethod, ReferenceMode, SourceRecord,
        };
        let mut editor = WorldEditor::new();
        let gh = "a".repeat(64);
        let sh = "b".repeat(64);
        let key = format!("{gh}:{sh}");
        let source = SourceRecord {
            key: key.clone(),
            dataset_id: "D".into(),
            revision: "R".into(),
            graph_sha256: gh,
            source_sha256: sh,
            source: "Local extract".into(),
            source_date: None,
            license: "ODbL-1.0".into(),
            attribution: "© OpenStreetMap contributors".into(),
            conversion_format_version: 1,
        };
        let provenance = AlignmentProvenanceInput {
            source_key: key,
            method: ProvenanceMethod::OsmSegmentCopyV1,
            osm_way_id: "1".into(),
            mode: ReferenceMode::Road,
            start_segment_index: 0,
            start_fraction: 0.0,
            end_segment_index: 0,
            end_fraction: 1.0,
        };
        let points = vec![
            GeoPoint {
                latitude: 35.0,
                longitude: 139.0,
            },
            GeoPoint {
                latitude: 35.0,
                longitude: 139.1,
            },
        ];
        // Attribution on an earlier path must roll back if any later path fails.
        let attributed_path = || PlannedAlignment {
            extension: None,
            name: "Attributed".into(),
            points: points.clone(),
            track_type: rosenfu_core::world::TrackType::Tram,
            stations: vec![planned_station(0.5)],
            reference: Some(PlannedAlignmentReference {
                source_record: source.clone(),
                provenance: provenance.clone(),
            }),
        };
        let before = editor.snapshot();
        let mut invalid_path = planned_path(0.2);
        invalid_path.stations.push(planned_station(1.1));
        assert!(editor
            .build_plan(vec![attributed_path(), invalid_path])
            .is_err());
        assert_eq!(editor.snapshot(), before);
        let mut expected = editor.clone();
        assert_eq!(
            editor
                .build_plan(vec![attributed_path(), planned_path(0.2)])
                .unwrap(),
            expected
                .build_plan(vec![attributed_path(), planned_path(0.2)])
                .unwrap()
        );
        // Use a fresh editor for the original single-path compatibility checks.
        let mut editor = WorldEditor::new();
        let before = editor.snapshot();
        let mut bad = source.clone();
        bad.source_date = Some("x".repeat(65));
        assert!(editor
            .build_alignment_from_reference("A".into(), points.clone(), bad, provenance.clone())
            .is_err());
        assert_eq!(editor.snapshot(), before);
        // A late station error must also roll back newly allocated source metadata.
        assert!(editor
            .build_alignment_with_stations(
                "Batch".into(),
                points.clone(),
                rosenfu_core::world::TrackType::Tram,
                vec![planned_station(0.3), planned_station(f64::NAN)],
                Some(PlannedAlignmentReference {
                    source_record: source.clone(),
                    provenance: provenance.clone(),
                }),
            )
            .is_err());
        assert_eq!(editor.snapshot(), before);
        let batch = editor
            .build_alignment_with_stations(
                "Batch".into(),
                points.clone(),
                rosenfu_core::world::TrackType::Tram,
                vec![planned_station(0.3)],
                Some(PlannedAlignmentReference {
                    source_record: source.clone(),
                    provenance: provenance.clone(),
                }),
            )
            .unwrap();
        assert_eq!(batch.stations.len(), 1);
        assert_eq!(
            batch.station_placements[0].alignment_id,
            batch.alignments[0].id
        );
        editor
            .build_alignment_from_reference(
                "A".into(),
                points.clone(),
                source.clone(),
                provenance.clone(),
            )
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("provenance.json");
        editor.save(&path).unwrap();
        assert_eq!(WorldEditor::new().load(&path).unwrap(), editor.snapshot());
        // Oversized serializable world must not gain partial geometry/metadata/IDs.
        let mut huge = editor.snapshot();
        huge.companies[0].name = "x".repeat(rosenfu_storage::MAX_SAVE_BYTES);
        editor.world.replace(huge).unwrap();
        let before = editor.snapshot();
        assert!(editor
            .build_alignment_from_reference("B".into(), points, source, provenance)
            .is_err());
        assert_eq!(editor.snapshot(), before);
    }
    #[test]
    fn selected_track_types_roundtrip_through_editor_save_and_load() {
        use rosenfu_core::world::TrackType;
        let mut editor = WorldEditor::new();
        let points = vec![
            GeoPoint {
                latitude: 35.0,
                longitude: 139.0,
            },
            GeoPoint {
                latitude: 35.0,
                longitude: 139.01,
            },
        ];
        for kind in [TrackType::Normal, TrackType::HighSpeed, TrackType::Tram] {
            let snapshot = editor
                .build_alignment_typed("A".into(), points.clone(), kind)
                .unwrap();
            assert_eq!(snapshot.alignments.last().unwrap().track_type, kind);
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("typed.json");
        editor.save(&path).unwrap();
        assert_eq!(WorldEditor::new().load(&path).unwrap(), editor.snapshot());
        let before = editor.snapshot();
        assert!(editor
            .build_alignment_typed("Bad".into(), vec![], TrackType::Tram)
            .is_err());
        assert_eq!(editor.snapshot(), before);
    }
    fn planned_station(position: f64) -> PlannedStation {
        PlannedStation {
            name: "計画駅".into(),
            position,
            length_m: 120.0,
            width_m: 10.0,
        }
    }

    fn planned_alignment_points() -> Vec<GeoPoint> {
        vec![
            GeoPoint {
                latitude: 35.0,
                longitude: 139.0,
            },
            GeoPoint {
                latitude: 35.0,
                longitude: 139.01,
            },
        ]
    }

    #[test]
    fn planned_alignment_and_stations_publish_together_and_roundtrip() {
        use rosenfu_core::world::TrackType;
        let mut editor = WorldEditor::new();
        let snapshot = editor
            .build_alignment_with_stations(
                "計画線".into(),
                planned_alignment_points(),
                TrackType::Tram,
                vec![planned_station(0.0), planned_station(0.8)],
                None,
            )
            .unwrap();
        assert_eq!(snapshot.alignments.len(), 1);
        assert_eq!(snapshot.alignments[0].track_type, TrackType::Tram);
        assert_eq!(snapshot.stations.len(), 2);
        assert_eq!(snapshot.station_placements.len(), 2);
        for placement in &snapshot.station_placements {
            assert_eq!(placement.alignment_id, snapshot.alignments[0].id);
        }
        assert!(snapshot.station_placements[0].position > 0.0);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("planned-stations.json");
        editor.save(&path).unwrap();
        assert_eq!(WorldEditor::new().load(&path).unwrap(), snapshot);
    }

    #[test]
    fn later_invalid_planned_station_does_not_publish_or_consume_ids() {
        use rosenfu_core::world::TrackType;
        let mut editor = WorldEditor::new();
        let before = editor.snapshot();
        for invalid in [
            PlannedStation {
                name: "".into(),
                ..planned_station(0.5)
            },
            planned_station(f64::NAN),
            planned_station(1.1),
            PlannedStation {
                length_m: 1000.0,
                ..planned_station(0.5)
            },
            PlannedStation {
                width_m: 0.0,
                ..planned_station(0.5)
            },
        ] {
            assert!(editor
                .build_alignment_with_stations(
                    "計画線".into(),
                    planned_alignment_points(),
                    TrackType::Normal,
                    vec![planned_station(0.2), invalid],
                    None,
                )
                .is_err());
            assert_eq!(editor.snapshot(), before);
        }
        let actual = editor
            .build_alignment_with_stations(
                "計画線".into(),
                planned_alignment_points(),
                TrackType::Normal,
                vec![planned_station(0.5)],
                None,
            )
            .unwrap();
        let expected = WorldEditor::new()
            .build_alignment_with_stations(
                "計画線".into(),
                planned_alignment_points(),
                TrackType::Normal,
                vec![planned_station(0.5)],
                None,
            )
            .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn invalid_alignment_and_unsaveable_batch_are_atomic() {
        use rosenfu_core::world::TrackType;
        let mut editor = WorldEditor::new();
        let before = editor.snapshot();
        assert!(editor
            .build_alignment_with_stations(
                "計画線".into(),
                vec![],
                TrackType::HighSpeed,
                vec![planned_station(0.5)],
                None,
            )
            .is_err());
        assert_eq!(editor.snapshot(), before);
        let mut huge = editor.snapshot();
        huge.companies[0].name = "x".repeat(rosenfu_storage::MAX_SAVE_BYTES);
        editor.world.replace(huge).unwrap();
        let before = editor.snapshot();
        assert!(editor
            .build_alignment_with_stations(
                "計画線".into(),
                planned_alignment_points(),
                TrackType::Normal,
                vec![planned_station(0.5)],
                None,
            )
            .is_err());
        assert_eq!(editor.snapshot(), before);
    }

    #[test]
    fn empty_station_batch_matches_existing_alignment_command() {
        use rosenfu_core::world::TrackType;
        let expected = WorldEditor::new()
            .build_alignment_typed(
                "計画線".into(),
                planned_alignment_points(),
                TrackType::Normal,
            )
            .unwrap();
        let actual = WorldEditor::new()
            .build_alignment_with_stations(
                "計画線".into(),
                planned_alignment_points(),
                TrackType::Normal,
                vec![],
                None,
            )
            .unwrap();
        assert_eq!(actual, expected);
    }
    fn planned_path(offset: f64) -> PlannedAlignment {
        PlannedAlignment {
            name: format!("計画線{offset}"),
            points: planned_alignment_points()
                .into_iter()
                .map(|mut p| {
                    p.latitude += offset;
                    p
                })
                .collect(),
            track_type: rosenfu_core::world::TrackType::Normal,
            stations: vec![planned_station(0.5)],
            reference: None,
            extension: None,
        }
    }

    #[test]
    fn independent_paths_build_together_and_preserve_existing_world() {
        let mut editor = WorldEditor::new();
        let before = editor.build_plan(vec![planned_path(0.0)]).unwrap();
        let result = editor
            .build_plan(vec![planned_path(0.1), planned_path(0.2)])
            .unwrap();
        assert_eq!(result.alignments.len(), 3);
        assert_eq!(result.stations.len(), 3);
        assert_eq!(result.alignments[0], before.alignments[0]);
        assert_eq!(result.stations[0], before.stations[0]);
        for (placement, alignment) in result.station_placements.iter().zip(&result.alignments) {
            assert_eq!(placement.alignment_id, alignment.id);
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("multi-path.json");
        editor.save(&path).unwrap();
        assert_eq!(WorldEditor::new().load(&path).unwrap(), result);
    }

    #[test]
    fn later_path_failure_rolls_back_entire_plan_and_ids() {
        let mut editor = WorldEditor::new();
        editor.build_plan(vec![planned_path(0.0)]).unwrap();
        let before = editor.snapshot();
        let mut control = editor.clone();
        let mut invalid = planned_path(0.2);
        invalid.stations.push(planned_station(1.1));
        assert!(editor.build_plan(vec![planned_path(0.1), invalid]).is_err());
        assert_eq!(editor.snapshot(), before);
        let mut invalid_geometry = planned_path(0.2);
        invalid_geometry.points.clear();
        assert!(editor
            .build_plan(vec![planned_path(0.1), invalid_geometry])
            .is_err());
        assert_eq!(editor.snapshot(), before);
        assert!(editor.build_plan(vec![]).is_err());
        assert_eq!(editor.snapshot(), before);
        assert_eq!(
            editor
                .build_plan(vec![planned_path(0.1), planned_path(0.2)])
                .unwrap(),
            control
                .build_plan(vec![planned_path(0.1), planned_path(0.2)])
                .unwrap()
        );
    }

    #[test]
    fn plan_payload_uses_frontend_camel_case_and_optional_reference() {
        let payload = serde_json::json!({
            "name": "計画線", "points": planned_alignment_points(),
            "trackType": "normal",
            "stations": [{"name":"駅", "position":0.5, "lengthM":120.0, "widthM":10.0}]
        });
        let alignment: PlannedAlignment = serde_json::from_value(payload).unwrap();
        assert!(alignment.reference.is_none());
        assert!(WorldEditor::new().build_plan(vec![alignment]).is_ok());
    }
    #[test]
    fn extension_batch_is_atomic_and_preserves_old_station_locations() {
        use rosenfu_core::world::AlignmentEndpoint;
        for endpoint in [AlignmentEndpoint::Start, AlignmentEndpoint::End] {
            let mut editor = WorldEditor::new();
            let before = editor.build_plan(vec![planned_path(0.0)]).unwrap();
            let old = &before.alignments[0];
            let anchor = if endpoint == AlignmentEndpoint::Start {
                old.points[0]
            } else {
                *old.points.last().unwrap()
            };
            let extension = || PlannedAlignment {
                name: "延伸".into(),
                points: vec![
                    anchor,
                    GeoPoint {
                        latitude: anchor.latitude,
                        longitude: anchor.longitude
                            + if endpoint == AlignmentEndpoint::Start {
                                -0.02
                            } else {
                                0.02
                            },
                    },
                ],
                track_type: old.track_type,
                stations: vec![planned_station(0.5)],
                reference: None,
                extension: Some(PlannedAlignmentExtension {
                    alignment_id: old.id,
                    endpoint,
                }),
            };
            let mut invalid = planned_path(0.2);
            invalid.points.clear();
            assert!(editor.build_plan(vec![extension(), invalid]).is_err());
            assert_eq!(editor.snapshot(), before);
            let after = editor
                .build_plan(vec![extension(), planned_path(0.3)])
                .unwrap();
            assert_eq!(after.alignments.len(), 2);
            assert_eq!(after.stations[0], before.stations[0]);
            assert_eq!(after.station_placements[1].alignment_id, old.id);
            let a = &after.alignments[0];
            let p = &after.station_placements[1];
            let station = &after.stations[1];
            let expected = a.point_at(p.position);
            assert!((station.latitude - expected.latitude).abs() < 1e-10);
            assert!(
                (station.longitude
                    - (anchor.longitude
                        + if endpoint == AlignmentEndpoint::Start {
                            -0.01
                        } else {
                            0.01
                        }))
                .abs()
                    < 1e-8
            );
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("extension.rosenfu");
            editor.save(&path).unwrap();
            assert_eq!(WorldEditor::new().load(&path).unwrap(), after);
        }
    }
    #[test]
    fn in_service_extension_roundtrips_and_batch_failure_is_atomic() {
        use rosenfu_core::world::{AlignmentEndpoint, RouteSegment};
        for endpoint in [AlignmentEndpoint::Start, AlignmentEndpoint::End] {
            let mut editor = WorldEditor::new();
            let initial = editor.build_plan(vec![planned_path(0.0)]).unwrap();
            let a = initial.alignments[0].id;
            editor
                .place_station_on_alignment("Second".into(), a, 0.8, 100.0, 12.0)
                .unwrap();
            let snapshot = editor.snapshot();
            let segments = vec![RouteSegment::Alignment {
                alignment_id: a,
                from: snapshot.stations[0].id,
                to: snapshot.stations[1].id,
            }];
            let preview = editor
                .preview_service_path(snapshot.stations[0].id, segments.clone())
                .unwrap();
            editor
                .create_service_path(
                    "Service".into(),
                    snapshot.stations[0].id,
                    segments,
                    preview
                        .occurrences
                        .iter()
                        .map(|o| o.reference.clone())
                        .collect(),
                )
                .unwrap();
            let before = editor.snapshot();
            let anchor = if endpoint == AlignmentEndpoint::Start {
                before.alignments[0].points[0]
            } else {
                *before.alignments[0].points.last().unwrap()
            };
            let extension = || PlannedAlignment {
                name: "Extension".into(),
                points: vec![
                    anchor,
                    GeoPoint {
                        latitude: anchor.latitude,
                        longitude: anchor.longitude
                            + if endpoint == AlignmentEndpoint::Start {
                                -0.02
                            } else {
                                0.02
                            },
                    },
                ],
                track_type: before.alignments[0].track_type,
                stations: vec![planned_station(0.5)],
                reference: None,
                extension: Some(PlannedAlignmentExtension {
                    alignment_id: a,
                    endpoint,
                }),
            };
            let mut invalid = planned_path(0.2);
            invalid.points.clear();
            assert!(editor.build_plan(vec![extension(), invalid]).is_err());
            assert_eq!(editor.snapshot(), before);
            let after = editor.build_plan(vec![extension()]).unwrap();
            assert_eq!(after.service_paths, before.service_paths);
            assert_eq!(after.services, before.services);
            for old in &before.stations {
                assert!(after.stations.contains(old));
            }
            let id = before.service_paths[0].service_id;
            assert_eq!(
                editor.world.service_path_occurrences(id).unwrap(),
                preview.occurrences
            );
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("service-extension.rosenfu");
            editor.save(&path).unwrap();
            let mut loaded = WorldEditor::new();
            let mut restored = loaded.load(&path).unwrap();
            // JSON floating-point parsing may differ by one ULP; geometry and
            // references remain exact, while ratios must preserve metre positions.
            for (actual, expected) in restored
                .station_placements
                .iter_mut()
                .zip(&after.station_placements)
            {
                assert!(
                    (actual.position - expected.position).abs() * after.alignments[0].length_m()
                        < 1e-6
                );
                actual.position = expected.position;
            }
            assert_eq!(restored, after);
            assert_eq!(
                loaded.world.service_path_occurrences(id).unwrap(),
                preview.occurrences
            );
        }
    }
}
