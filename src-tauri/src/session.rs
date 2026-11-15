//! Desktop application operations. The core owns IDs and referential integrity.
#[path = "session/deletion.rs"]
mod deletion;
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

/// A station planned on already built infrastructure, placed at commit time.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlannedBuiltStation {
    pub alignment_id: AlignmentId,
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
    #[serde(default)]
    pub anchor_indices: Option<Vec<usize>>,
    pub name: String,
    pub points: Vec<GeoPoint>,
    pub track_type: rosenfu_core::world::TrackType,
    pub design_speed_kmh: Option<f64>,
    pub stations: Vec<PlannedStation>,
    pub reference: Option<PlannedAlignmentReference>,
    pub extension: Option<PlannedAlignmentExtension>,
    pub destination: Option<PlannedAlignmentExtension>,
}

#[derive(Clone)]
struct PendingManualReplacement {
    path: PathBuf,
    version: rosenfu_storage::FileVersion,
    token: String,
}
#[derive(serde::Serialize)]
pub struct ManualReplacementRequired {
    pub token: String,
    pub company_name: String,
}
#[derive(serde::Serialize)]
pub struct ManualSaveResult {
    pub saved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_required: Option<ManualReplacementRequired>,
}
fn next_manual_token() -> Result<String, String> {
    static NEXT: std::sync::Mutex<u64> = std::sync::Mutex::new(0);
    let mut sequence = NEXT.lock().map_err(|_| "保存確認番号を作成できません。")?;
    *sequence = sequence
        .checked_add(1)
        .ok_or("保存確認番号を作成できません。")?;
    // Correlation nonce, not an authentication credential. Unique across editor
    // instances in this process; clients must never infer an approved baseline.
    Ok(format!("{:x}-{:x}", std::process::id(), *sequence))
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
    deleted_alignment: Option<(WorldDraft, WorldDraft)>,
    pending_manual_replacement: Option<PendingManualReplacement>,
}
impl WorldEditor {
    pub fn set_service_route_binding(
        &mut self,
        service_id: ServiceId,
        binding: rosenfu_core::world::ServiceRouteBindingKind,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate
            .world
            .set_service_route_binding(candidate.company, service_id, binding)
            .map_err(|_| {
                "この系統の経路は指定した路線に含まれていません。変更は保存していません。"
                    .to_owned()
            })?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        *self = candidate;
        Ok(self.snapshot())
    }
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
        self.create_service_path_with_source_route(name, start, segments, stops, None)
    }
    pub fn create_service_path_with_source_route(
        &mut self,
        name: String,
        start: StationId,
        segments: Vec<rosenfu_core::world::RouteSegment>,
        stops: Vec<rosenfu_core::world::StationOccurrenceRef>,
        source_route_id: Option<RouteId>,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate.create_service_path_unchecked(name, start, segments, stops)?;
        if let Some(route_id) = source_route_id {
            let id = candidate
                .world
                .snapshot()
                .services
                .last()
                .expect("created service")
                .id;
            candidate
                .world
                .set_service_route_binding(
                    candidate.company,
                    id,
                    rosenfu_core::world::ServiceRouteBindingKind::Route { route_id },
                )
                .map_err(|e| e.to_string())?;
        }
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        *self = candidate;
        Ok(self.snapshot())
    }
    pub fn update_service_path(
        &mut self,
        service_id: ServiceId,
        name: String,
        stops: Vec<rosenfu_core::world::StationOccurrenceRef>,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate
            .world
            .update_service_path(candidate.company, service_id, name, stops)
            .map_err(|e| e.to_string())?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        *self = candidate;
        Ok(self.snapshot())
    }
    pub fn preview_service_path_replacement(
        &self,
        service_id: ServiceId,
        start: StationId,
        segments: Vec<rosenfu_core::world::RouteSegment>,
    ) -> Result<rosenfu_core::world::ServicePathPreview, String> {
        self.world
            .preview_service_path_replacement(self.company, service_id, start, segments)
            .map_err(|e| e.to_string())
    }
    pub fn replace_service_path(
        &mut self,
        service_id: ServiceId,
        expected_path: rosenfu_core::world::ServicePath,
        name: String,
        start: StationId,
        segments: Vec<rosenfu_core::world::RouteSegment>,
        stops: Vec<rosenfu_core::world::StationOccurrenceRef>,
    ) -> Result<WorldDraft, String> {
        self.replace_service_path_with_source_route(
            service_id,
            expected_path,
            name,
            start,
            segments,
            stops,
            None,
        )
    }
    pub fn replace_service_path_with_source_route(
        &mut self,
        service_id: ServiceId,
        expected_path: rosenfu_core::world::ServicePath,
        name: String,
        start: StationId,
        segments: Vec<rosenfu_core::world::RouteSegment>,
        stops: Vec<rosenfu_core::world::StationOccurrenceRef>,
        source_route_id: Option<RouteId>,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        // Rebinding and path replacement are one outer transaction. The temporary
        // independent row exists only in this discarded-on-failure clone.
        if source_route_id.is_some() {
            if let Some(id) = Some(service_id) {
                candidate
                    .world
                    .set_service_route_binding(
                        candidate.company,
                        id,
                        rosenfu_core::world::ServiceRouteBindingKind::Independent,
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
        candidate
            .world
            .replace_service_path(
                candidate.company,
                service_id,
                expected_path,
                name,
                start,
                segments,
                stops,
            )
            .map_err(|e| e.to_string())?;
        if let Some(route_id) = source_route_id {
            let id = service_id;
            candidate
                .world
                .set_service_route_binding(
                    candidate.company,
                    id,
                    rosenfu_core::world::ServiceRouteBindingKind::Route { route_id },
                )
                .map_err(|e| e.to_string())?;
        }
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
        self.preview_route_candidates_via(start, end, &[])
    }
    pub fn preview_route_candidates_via(
        &self,
        start: StationId,
        end: StationId,
        via_station_ids: &[StationId],
    ) -> Result<rosenfu_core::world::RouteCandidates, String> {
        self.world
            .route_candidates_via(start, end, via_station_ids)
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
            deleted_alignment: None,
            pending_manual_replacement: None,
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
            anchor_indices: None,
            name,
            points,
            track_type,
            design_speed_kmh: None,
            stations,
            reference,
            extension: None,
            destination: None,
        }])
    }

    /// Build only the supplied plan's paths, stations and attribution atomically.
    /// A failed path never publishes earlier paths or consumes their IDs.
    pub fn build_plan(&mut self, alignments: Vec<PlannedAlignment>) -> Result<WorldDraft, String> {
        self.build_construction_plan(alignments, vec![])
    }
    pub fn build_construction_plan(
        &mut self,
        alignments: Vec<PlannedAlignment>,
        stations: Vec<PlannedBuiltStation>,
    ) -> Result<WorldDraft, String> {
        if alignments.is_empty() && stations.is_empty() {
            return Err("着工する線路・駅がありません。".into());
        }
        let mut candidate = self.clone();
        // Place these before any extensions. Extending an alignment remaps its
        // existing placements, preserving the planned station's physical location.
        for station in stations {
            candidate.place_station_on_alignment_unchecked(
                station.name,
                station.alignment_id,
                station.position,
                station.length_m,
                station.width_m,
            )?;
        }
        for alignment in alignments {
            let anchor_indices = alignment.anchor_indices.clone();
            if anchor_indices.as_ref().is_some_and(|indices| {
                indices.len() < 2
                    || indices.first() != Some(&0)
                    || indices.last().copied() != alignment.points.len().checked_sub(1)
                    || indices.windows(2).any(|p| p[0] >= p[1])
            }) {
                return Err("線路の区間境界が不正です。".into());
            }
            if alignment.destination.is_some() && alignment.extension.is_none() {
                return Err("接続先を指定するには既存線路から延伸してください。".into());
            }
            if let Some(extension) = alignment.extension {
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
                if alignment
                    .design_speed_kmh
                    .is_some_and(|speed| speed != old.effective_design_speed_kmh())
                {
                    return Err("延伸では元の線路の設計最高速度を引き継ぎます。".into());
                }
                let old_length = old.length_m();
                let old_count = old.points.len();
                let old_indices = old
                    .anchor_indices
                    .clone()
                    .unwrap_or_else(|| vec![0, old_count - 1]);
                let added_count = alignment
                    .points
                    .len()
                    .checked_sub(1)
                    .ok_or("延伸する線路がありません。")?;
                if let Some(reference) = alignment.reference {
                    if alignment
                        .points
                        .iter()
                        .any(|p| !in_edit_extent(p.latitude, p.longitude))
                    {
                        return Err("路線は編集範囲内に建設してください。".into());
                    }
                    candidate
                        .world
                        .extend_alignment_from_reference(
                            candidate.company,
                            extension.alignment_id,
                            extension.endpoint,
                            alignment.points,
                            reference.source_record,
                            reference.provenance,
                        )
                        .map_err(|e| e.to_string())?;
                } else {
                    candidate.extend_alignment(
                        extension.alignment_id,
                        extension.endpoint,
                        alignment.points,
                    )?;
                }
                if let Some(indices) = anchor_indices {
                    let merged = match extension.endpoint {
                        rosenfu_core::world::AlignmentEndpoint::Start => indices
                            .into_iter()
                            .rev()
                            .map(|i| added_count - i)
                            .chain(old_indices.into_iter().skip(1).map(|i| i + added_count))
                            .collect(),
                        rosenfu_core::world::AlignmentEndpoint::End => old_indices
                            .into_iter()
                            .chain(indices.into_iter().skip(1).map(|i| i + old_count - 1))
                            .collect(),
                    };
                    candidate.set_anchor_indices(extension.alignment_id, merged)?;
                }
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
                if let Some(destination) = alignment.destination {
                    candidate
                        .world
                        .connect_alignment_endpoints(
                            candidate.company,
                            rosenfu_core::world::AlignmentEndpointRef {
                                alignment_id: extension.alignment_id,
                                endpoint: extension.endpoint,
                            },
                            rosenfu_core::world::AlignmentEndpointRef {
                                alignment_id: destination.alignment_id,
                                endpoint: destination.endpoint,
                            },
                        )
                        .map_err(|e| e.to_string())?;
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
            if let Some(indices) = anchor_indices {
                candidate.set_anchor_indices(alignment_id, indices)?;
            }
            if let Some(speed) = alignment.design_speed_kmh {
                candidate
                    .world
                    .set_alignment_design_speed(alignment_id, speed)
                    .map_err(|e| e.to_string())?;
            }
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
        if !candidate
            .stations
            .iter()
            .any(|station| station.id == station_id && station.owner == self.company)
        {
            return Err("編集する駅が見つかりません。".into());
        }
        candidate
            .rename_station_group(station_id, name.trim().to_owned())
            .map_err(|e| e.to_string())?;
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
            || candidate.service_paths.iter().any(|p|p.stops.iter().any(|stop|matches!(stop,rosenfu_core::world::StationOccurrenceRef::OnLeg{station_id:id,..} if *id==station_id)) || p.legs.iter().any(|l|matches!(l.segment,rosenfu_core::world::RouteSegment::Alignment{from,to,..} | rosenfu_core::world::RouteSegment::ConnectedAlignments{from,to,..} if from==station_id || to==station_id)))
            || candidate.route_paths.iter().any(|p| p.member_stations.contains(&station_id) || p.segments.iter().any(|segment| matches!(segment, rosenfu_core::world::RouteSegment::Alignment { from, to, .. } | rosenfu_core::world::RouteSegment::ConnectedAlignments { from,to,.. } if *from == station_id || *to == station_id)))
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
        candidate.remove_station_group_member(station_id);
        self.world
            .replace(candidate)
            .map_err(|error| error.to_string())?;
        Ok(self.snapshot())
    }
    pub fn delete_alignment_with_plans(
        &mut self,
        id: AlignmentId,
        plans: rosenfu_core::world::ConstructionPlans,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        let mut draft = candidate.snapshot();
        draft.construction_plans = Some(plans);
        candidate.world.replace(draft).map_err(|e| e.to_string())?;
        let result = candidate.delete_alignment(id)?;
        *self = candidate;
        Ok(result)
    }
    /// Delete one whole alignment, never its dependent equipment or services.
    pub fn delete_alignment(&mut self, id: AlignmentId) -> Result<WorldDraft, String> {
        let before = self.snapshot();
        let mut draft = before.clone();
        if !draft
            .alignments
            .iter()
            .any(|a| a.id == id && a.owner == self.company)
        {
            return Err("削除する自社線路が見つかりません。".into());
        }
        if draft
            .alignment_connections
            .iter()
            .any(|c| c.a.alignment_id == id || c.b.alignment_id == id)
        {
            return Err("接続に使われている線路は削除できません。".into());
        }
        if draft.station_placements.iter().any(|s| s.alignment_id == id)
            || draft.route_paths.iter().any(|p| p.segments.iter().any(|s| matches!(s, rosenfu_core::world::RouteSegment::Alignment { alignment_id, .. } if *alignment_id == id)))
            || draft.service_paths.iter().any(|p| p.legs.iter().any(|l| matches!(l.segment, rosenfu_core::world::RouteSegment::Alignment { alignment_id, .. } if alignment_id == id)))
        {
            return Err("駅・路線・系統が使用中の線路は削除できません。".into());
        }
        if draft.construction_plans.as_ref().is_some_and(|p| {
            p.draft_redo_destination
                .as_ref()
                .is_some_and(|e| e.alignment_id == id)
                || p.parked_paths.iter().any(|path| {
                    path.redo_destination
                        .as_ref()
                        .is_some_and(|e| e.alignment_id == id)
                })
                || p.draft_destination
                    .as_ref()
                    .is_some_and(|e| e.alignment_id == id)
                || p.parked_paths
                    .iter()
                    .any(|p| p.destination.as_ref().is_some_and(|e| e.alignment_id == id))
                || p.draft_extension
                    .as_ref()
                    .is_some_and(|e| e.alignment_id == id)
                || p.parked_paths
                    .iter()
                    .any(|p| p.extension.as_ref().is_some_and(|e| e.alignment_id == id))
                || p.planned_stations
                    .iter()
                    .any(|s| s.alignment_id == Some(id))
        }) {
            return Err("未着工計画が使用中の線路は削除できません。".into());
        }
        draft.alignments.retain(|a| a.id != id);
        draft.alignment_provenance.retain(|p| p.alignment_id != id);
        let mut candidate = self.clone();
        candidate.world.replace(draft).map_err(|e| e.to_string())?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        let after = candidate.snapshot();
        candidate.deleted_alignment = Some((before, after.clone()));
        *self = candidate;
        Ok(after)
    }

    /// Restores only the most recent deletion, without overwriting later edits.
    pub fn undo_alignment_deletion(&mut self) -> Result<WorldDraft, String> {
        let (before, after) = self
            .deleted_alignment
            .as_ref()
            .ok_or("元に戻せる線路削除がありません。")?;
        if &self.snapshot() != after {
            return Err("削除後に変更・保存したため、この削除は元に戻せません。".into());
        }
        let mut candidate = self.clone();
        // Exact post-delete snapshot equality above permits restoration of retired IDs.
        candidate.world = World::new(before.clone()).map_err(|e| e.to_string())?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        candidate.deleted_alignment = None;
        *self = candidate;
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
            let previous = candidate
                .routes
                .iter()
                .find(|r| r.id == id && r.owner == self.company)
                .ok_or("編集する路線が見つかりません。")?;
            let previous_segments = candidate
                .route_paths
                .iter()
                .find(|p| p.route_id == id)
                .map(|p| p.segments.clone())
                .unwrap_or_else(|| {
                    previous
                        .tracks
                        .iter()
                        .map(|track_id| rosenfu_core::world::RouteSegment::Track {
                            track_id: *track_id,
                        })
                        .collect()
                });
            // Services currently own independent physical paths, not route IDs.
            // Do not guess which service depends on a route: until explicit
            // associations exist, reject path changes conservatively.
            if (previous.start != start || previous_segments != preview.path.segments)
                && candidate.services.iter().any(|service| {
                    service.owner == self.company
                        && !candidate
                            .service_route_bindings
                            .iter()
                            .any(|row| row.service_id == service.id)
                })
            {
                return Err("路線との関連が未確認の系統があります。先に系統の路線を割り当てるか、独立系統として指定してください。系統は変更していません。".into());
            }
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
            let previous = candidate
                .routes
                .iter()
                .find(|r| r.id == id && r.owner == self.company)
                .ok_or("編集する路線が見つかりません。")?;
            if (previous.start != start || previous.tracks != tracks)
                && candidate.services.iter().any(|service| {
                    service.owner == self.company
                        && !candidate
                            .service_route_bindings
                            .iter()
                            .any(|row| row.service_id == service.id)
                })
            {
                return Err("路線との関連が未確認の系統があります。先に系統の路線を割り当てるか、独立系統として指定してください。系統は変更していません。".into());
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
        self.save_service_with_source_route(service_id, name, start, tracks, stops, None)
    }
    pub fn save_service_with_source_route(
        &mut self,
        service_id: Option<ServiceId>,
        name: String,
        start: StationId,
        tracks: Vec<TrackId>,
        stops: Vec<u32>,
        source_route_id: Option<RouteId>,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        // Rebinding and path replacement are one outer transaction. The temporary
        // independent row exists only in this discarded-on-failure clone.
        if source_route_id.is_some() {
            if let Some(id) = service_id {
                candidate
                    .world
                    .set_service_route_binding(
                        candidate.company,
                        id,
                        rosenfu_core::world::ServiceRouteBindingKind::Independent,
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
        candidate.save_service_unchecked(service_id, name, start, tracks, stops)?;
        if let Some(route_id) = source_route_id {
            let id = service_id.unwrap_or_else(|| {
                candidate
                    .world
                    .snapshot()
                    .services
                    .last()
                    .expect("created service")
                    .id
            });
            candidate
                .world
                .set_service_route_binding(
                    candidate.company,
                    id,
                    rosenfu_core::world::ServiceRouteBindingKind::Route { route_id },
                )
                .map_err(|e| e.to_string())?;
        }
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
    /// A fresh world may replace an existing healthy save only after approval of
    /// this exact observed version. Loaded-session conflicts stay ordinary errors.
    pub fn request_manual_save(
        &mut self,
        path: &Path,
        plans: Option<rosenfu_core::world::ConstructionPlans>,
        view: Option<rosenfu_core::world::ViewState>,
    ) -> Result<ManualSaveResult, String> {
        self.pending_manual_replacement = None;
        if self.company_setup_required {
            return Err("会社名を設定してから保存してください。".into());
        }
        if self.loaded_path.is_none()
            && rosenfu_storage::file_version(path).map_err(|e| e.to_string())?
                != rosenfu_storage::FileVersion::Missing
        {
            let (previous, version) =
                rosenfu_storage::load_world_with_version(path).map_err(|e| e.to_string())?;
            let token = next_manual_token()?;
            let company_name = previous
                .snapshot()
                .companies
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>()
                .join(" / ");
            self.pending_manual_replacement = Some(PendingManualReplacement {
                path: path.to_owned(),
                version,
                token: token.clone(),
            });
            return Ok(ManualSaveResult {
                saved: false,
                replacement_required: Some(ManualReplacementRequired {
                    token,
                    company_name,
                }),
            });
        }
        self.save_with_view(path, plans, view, false)?;
        Ok(ManualSaveResult {
            saved: true,
            replacement_required: None,
        })
    }
    pub fn cancel_manual_save(&mut self, token: &str) {
        if self
            .pending_manual_replacement
            .as_ref()
            .is_some_and(|pending| pending.token == token)
        {
            self.pending_manual_replacement = None;
        }
    }
    pub fn confirm_manual_save(
        &mut self,
        path: &Path,
        token: &str,
        plans: Option<rosenfu_core::world::ConstructionPlans>,
        view: Option<rosenfu_core::world::ViewState>,
    ) -> Result<ManualSaveResult, String> {
        let pending = self
            .pending_manual_replacement
            .as_ref()
            .filter(|pending| pending.token == token)
            .ok_or("保存確認が無効です。もう一度保存してください。")?;
        if pending.path != path {
            return Err("保存確認の対象が一致しません。".into());
        }
        let pending = self
            .pending_manual_replacement
            .take()
            .ok_or("保存確認がありません。")?;
        if self.loaded_path.is_some() || self.company_setup_required {
            return Err("保存対象のセッションが変更されています。".into());
        }
        let mut candidate = self.clone();
        let mut draft = candidate.snapshot();
        if let Some(plans) = plans {
            draft.construction_plans = Some(plans);
        }
        if let Some(view) = view {
            draft.view_state = Some(view);
        }
        candidate.world = World::new(draft).map_err(|e| e.to_string())?;
        let (_, version) =
            rosenfu_storage::save_world_if_version(path, &candidate.world, &pending.version)
                .map_err(|e| e.to_string())?;
        candidate.loaded_path = Some(path.to_owned());
        candidate.manual_version = Some(version);
        candidate.manual_repair_allowed = false;
        *self = candidate;
        Ok(ManualSaveResult {
            saved: true,
            replacement_required: None,
        })
    }
    /// Save a candidate atomically: a failed validation/write leaves editor state unchanged.
    pub fn save_with_plans(
        &mut self,
        path: &Path,
        plans: Option<rosenfu_core::world::ConstructionPlans>,
        automatic: bool,
    ) -> Result<(), String> {
        self.save_with_view(path, plans, None, automatic)
    }
    /// Persist local single-player view and plans atomically with the world.
    /// Omitted metadata preserves the previous value for older callers.
    pub fn save_with_view(
        &mut self,
        path: &Path,
        plans: Option<rosenfu_core::world::ConstructionPlans>,
        view: Option<rosenfu_core::world::ViewState>,
        automatic: bool,
    ) -> Result<(), String> {
        let mut candidate = self.clone();
        let mut draft = candidate.snapshot();
        if let Some(plans) = plans {
            draft.construction_plans = Some(plans);
        }
        if let Some(view) = view {
            draft.view_state = Some(view);
        }
        candidate.world = World::new(draft).map_err(|e| e.to_string())?;
        if automatic {
            candidate.autosave(path)?;
        } else {
            candidate.save(path)?;
        }
        *self = candidate;
        Ok(())
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
        self.pending_manual_replacement = None;
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
        candidate.deleted_alignment = None;
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
        self.deleted_alignment = None;
        self.world = candidate;
        self.company = company;
        self.company_setup_required = false;
        self.pending_manual_replacement = None;
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
    fn fresh_company(name: &str) -> WorldEditor {
        let mut editor = WorldEditor::new();
        editor.rename_company(name.into()).unwrap();
        editor
    }
    fn pending_token(editor: &mut WorldEditor, path: &Path) -> String {
        let result = editor.request_manual_save(path, None, None).unwrap();
        assert!(!result.saved);
        result.replacement_required.unwrap().token
    }
    #[test]
    fn fresh_manual_replacement_requires_confirmation_and_preserves_backup_and_auto() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        let auto = dir.path().join("auto.json");
        let mut old = fresh_company("Old company");
        old.save(&path).unwrap();
        old.autosave(&auto).unwrap();
        let old_bytes = std::fs::read(&path).unwrap();
        let auto_bytes = std::fs::read(&auto).unwrap();
        let mut new = fresh_company("New company");
        let before = new.snapshot();
        let result = new.request_manual_save(&path, None, None).unwrap();
        let required = result.replacement_required.unwrap();
        assert!(!result.saved);
        assert_eq!(required.company_name, "Old company");
        assert_eq!(new.snapshot(), before);
        assert_eq!(std::fs::read(&path).unwrap(), old_bytes);
        assert!(
            new.confirm_manual_save(&path, &required.token, None, None)
                .unwrap()
                .saved
        );
        assert_eq!(
            rosenfu_storage::load_world(&path)
                .unwrap()
                .snapshot()
                .companies[0]
                .name,
            "New company"
        );
        assert_eq!(
            std::fs::read(rosenfu_storage::backup_path(&path)).unwrap(),
            old_bytes
        );
        assert_eq!(std::fs::read(&auto).unwrap(), auto_bytes);
        assert!(new
            .confirm_manual_save(&path, &required.token, None, None)
            .is_err());
    }
    #[test]
    fn pending_manual_nonce_cancel_new_request_and_invalid_confirmation_are_safe() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        fresh_company("Old").save(&path).unwrap();
        let disk = std::fs::read(&path).unwrap();
        let mut editor = fresh_company("New");
        let before = editor.snapshot();
        let first = pending_token(&mut editor, &path);
        let second = pending_token(&mut editor, &path);
        assert_ne!(first, second);
        editor.cancel_manual_save(&first);
        assert!(editor
            .confirm_manual_save(&path, &first, None, None)
            .is_err());
        editor.cancel_manual_save(&second);
        assert!(editor
            .confirm_manual_save(&path, &second, None, None)
            .is_err());
        let third = pending_token(&mut editor, &path);
        let invalid = rosenfu_core::world::ViewState {
            latitude: 0.0,
            longitude: 0.0,
            zoom: 0.0,
        };
        assert!(editor
            .confirm_manual_save(&path, &third, None, Some(invalid))
            .is_err());
        assert!(editor
            .confirm_manual_save(&path, &third, None, None)
            .is_err());
        assert_eq!(editor.snapshot(), before);
        assert_eq!(std::fs::read(&path).unwrap(), disk);
    }
    #[test]
    fn replacement_conflicts_disappearance_and_loaded_sessions_never_force_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        fresh_company("Old").save(&path).unwrap();
        let mut one = fresh_company("One");
        let mut two = fresh_company("Two");
        let t1 = pending_token(&mut one, &path);
        let t2 = pending_token(&mut two, &path);
        let before = two.snapshot();
        one.confirm_manual_save(&path, &t1, None, None).unwrap();
        let disk = std::fs::read(&path).unwrap();
        assert!(two.confirm_manual_save(&path, &t2, None, None).is_err());
        assert_eq!(two.snapshot(), before);
        assert_eq!(std::fs::read(&path).unwrap(), disk);
        // A loaded session never gets a replacement offer when another writer updates it.
        let mut loaded = WorldEditor::new();
        loaded.load(&path).unwrap();
        one.rename_company("changed".into()).unwrap();
        one.save(&path).unwrap();
        assert!(loaded.request_manual_save(&path, None, None).is_err());
        let token = pending_token(&mut two, &path);
        two.load(&path).unwrap();
        assert!(two.confirm_manual_save(&path, &token, None, None).is_err());
        let mut three = fresh_company("Three");
        let token = pending_token(&mut three, &path);
        std::fs::remove_file(&path).unwrap();
        assert!(three
            .confirm_manual_save(&path, &token, None, None)
            .is_err());
        assert!(!path.exists());
    }
    #[test]
    fn new_manual_save_rejects_setup_corrupt_and_future_schema_without_touching_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        let mut editor = WorldEditor::new();
        assert!(editor.request_manual_save(&path, None, None).is_err());
        assert!(!path.exists());
        editor.rename_company("New".into()).unwrap();
        for bytes in [
            b"corrupt".to_vec(),
            br#"{"format":"rosenfu-world","schema_version":999,"world":{}}"#.to_vec(),
        ] {
            std::fs::write(&path, &bytes).unwrap();
            assert!(editor.request_manual_save(&path, None, None).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
        std::fs::remove_file(&path).unwrap();
        assert!(editor.request_manual_save(&path, None, None).unwrap().saved);
    }
    #[test]
    fn view_manual_autosave_and_invalid_save_preserve_data() {
        use rosenfu_core::world::ViewState;
        let dir = tempfile::tempdir().unwrap();
        let manual = dir.path().join("manual.json");
        let auto = dir.path().join("auto.json");
        let view = ViewState {
            latitude: 35.7,
            longitude: 139.8,
            zoom: 17.25,
        };
        let mut editor = WorldEditor::new();
        editor
            .save_with_view(&manual, None, Some(view), false)
            .unwrap();
        let mut restored = WorldEditor::new();
        assert_eq!(restored.load(&manual).unwrap().view_state, Some(view));
        let bytes = std::fs::read(&manual).unwrap();
        let before = editor.snapshot();
        assert!(editor
            .save_with_view(&manual, None, Some(ViewState { zoom: 21.0, ..view }), false)
            .is_err());
        assert_eq!(editor.snapshot(), before);
        assert_eq!(std::fs::read(&manual).unwrap(), bytes);
        assert!(editor
            .save_with_view(
                &auto,
                None,
                Some(ViewState {
                    latitude: f64::NAN,
                    ..view
                }),
                true
            )
            .is_err());
        assert!(!auto.exists());
        assert_eq!(editor.snapshot(), before);
        let next = ViewState { zoom: 18.5, ..view };
        editor
            .save_with_view(&auto, None, Some(next), true)
            .unwrap();
        assert_eq!(
            restored.load_autosave(&auto, &manual).unwrap().view_state,
            Some(next)
        );
        assert_eq!(std::fs::read(&manual).unwrap(), bytes);
        editor.save_with_plans(&manual, None, false).unwrap();
        assert_eq!(
            rosenfu_storage::load_world(&manual)
                .unwrap()
                .snapshot()
                .view_state,
            Some(next)
        );
    }
    fn draft_plans() -> rosenfu_core::world::ConstructionPlans {
        serde_json::from_value(serde_json::json!({
            "version":1,"plans":[{"id":1,"name":"計画"}],"parkedPaths":[],
            "activePathId":1,"activePlanId":1,"draftPoints":[{"latitude":35.0,"longitude":139.0}],"redoPoints":[],"draftExtension":null,
            "trackType":"normal","designSpeedKmh":120,"roadFollowing":false,"plannedStations":[],"draftFinished":false
        })).unwrap()
    }
    #[test]
    fn plan_save_conflict_does_not_apply_new_draft_or_replace_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        let mut editor = WorldEditor::new();
        editor
            .save_with_plans(&path, Some(draft_plans()), false)
            .unwrap();
        let before = editor.snapshot();
        let mut other = WorldEditor::new();
        other.load(&path).unwrap();
        other.rename_company("External writer".into()).unwrap();
        other.save(&path).unwrap();
        let disk = std::fs::read(&path).unwrap();
        let mut next = draft_plans();
        next.plans[0].name = "Not saved".into();
        assert!(editor.save_with_plans(&path, Some(next), false).is_err());
        assert_eq!(editor.snapshot(), before);
        assert_eq!(std::fs::read(&path).unwrap(), disk);
    }
    #[test]
    fn manual_and_autosave_restore_drafts_and_failed_save_is_atomic() {
        let dir = tempfile::tempdir().unwrap();
        let manual = dir.path().join("world.json");
        let auto = dir.path().join("auto.json");
        let mut editor = WorldEditor::new();
        editor
            .save_with_plans(&manual, Some(draft_plans()), false)
            .unwrap();
        let mut restored = WorldEditor::new();
        assert_eq!(
            restored.load(&manual).unwrap().construction_plans,
            Some(draft_plans())
        );
        let before = editor.snapshot();
        let bytes = std::fs::read(&manual).unwrap();
        let mut invalid = draft_plans();
        invalid.active_plan_id = 999;
        assert!(editor
            .save_with_plans(&manual, Some(invalid), false)
            .is_err());
        assert_eq!(editor.snapshot(), before);
        assert_eq!(std::fs::read(&manual).unwrap(), bytes);
        let mut next = draft_plans();
        next.plans[0].name = "自動保存".into();
        editor
            .save_with_plans(&auto, Some(next.clone()), true)
            .unwrap();
        assert_eq!(
            restored
                .load_autosave(&auto, &manual)
                .unwrap()
                .construction_plans,
            Some(next)
        );
        assert_eq!(std::fs::read(&manual).unwrap(), bytes);
        // Missing payload from older callers preserves the stored plans.
        editor.save_with_plans(&manual, None, false).unwrap();
        assert!(rosenfu_storage::load_world(&manual)
            .unwrap()
            .snapshot()
            .construction_plans
            .is_some());
    }
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
        let newer = bytes.replace(
            &format!("\"schema_version\": {}", rosenfu_storage::SCHEMA_VERSION),
            "\"schema_version\": 999",
        );
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
    fn alignment_deletion_is_atomic_reversible_and_saved() {
        let mut editor = WorldEditor::new();
        editor
            .build_alignment(
                "A".into(),
                vec![
                    GeoPoint {
                        latitude: 35.0,
                        longitude: 139.0,
                    },
                    GeoPoint {
                        latitude: 35.1,
                        longitude: 139.1,
                    },
                ],
            )
            .unwrap();
        let before = editor.snapshot();
        let id = before.alignments[0].id;
        assert!(editor.delete_alignment(AlignmentId(999)).is_err());
        assert_eq!(editor.snapshot(), before);
        let after = editor.delete_alignment(id).unwrap();
        assert!(after.alignments.is_empty());
        assert_eq!(after.next_id, before.next_id);
        assert_eq!(editor.undo_alignment_deletion().unwrap(), before);
        editor.delete_alignment(id).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deleted.json");
        editor.save(&path).unwrap();
        let mut restored = WorldEditor::new();
        assert!(restored.load(&path).unwrap().alignments.is_empty());
        editor.add_station("S".into(), 35.0, 139.0).unwrap();
        let edited = editor.snapshot();
        assert!(editor.undo_alignment_deletion().is_err());
        assert_eq!(editor.snapshot(), edited);
    }
    #[test]
    fn alignment_deletion_refuses_stations_and_plan_references() {
        let mut editor = WorldEditor::new();
        editor
            .build_alignment(
                "A".into(),
                vec![
                    GeoPoint {
                        latitude: 35.0,
                        longitude: 139.0,
                    },
                    GeoPoint {
                        latitude: 35.1,
                        longitude: 139.1,
                    },
                ],
            )
            .unwrap();
        let id = editor.snapshot().alignments[0].id;
        let mut plans = draft_plans();
        plans.draft_extension = Some(
            serde_json::from_value(serde_json::json!({"alignmentId":id,"endpoint":"end"})).unwrap(),
        );
        let before = editor.snapshot();
        assert!(editor.delete_alignment_with_plans(id, plans).is_err());
        assert_eq!(editor.snapshot(), before);
        editor
            .place_station_on_alignment("S".into(), id, 0.5, 100.0, 12.0)
            .unwrap();
        let before = editor.snapshot();
        assert!(editor.delete_alignment(id).is_err());
        assert_eq!(editor.snapshot(), before);
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
        let before = editor.snapshot();
        assert!(editor
            .save_route(Some(route), "逆方向の路線".into(), b, vec![track])
            .is_err());
        assert_eq!(editor.snapshot(), before);
        editor
            .save_route(Some(route), "新しい路線名".into(), a, vec![track])
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
    fn explicit_bindings_allow_unrelated_routes_and_protect_assigned_routes() {
        use rosenfu_core::world::ServiceRouteBindingKind;
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
        let c = editor
            .add_station("C".into(), 35.0, 139.2)
            .unwrap()
            .stations[2]
            .id;
        let ab = editor.add_track(a, b).unwrap().tracks[0].id;
        let bc = editor.add_track(b, c).unwrap().tracks[1].id;
        let route = editor
            .save_route(None, "R".into(), a, vec![ab])
            .unwrap()
            .routes[0]
            .id;
        let other = editor
            .save_route(None, "Other".into(), a, vec![ab])
            .unwrap()
            .routes[1]
            .id;
        let saved = editor
            .save_service_with_source_route(None, "S".into(), a, vec![ab], vec![0, 1], Some(route))
            .unwrap();
        let id = saved.services[0].id;
        editor
            .save_route(Some(other), "Other".into(), b, vec![bc])
            .unwrap();
        let before = editor.snapshot();
        assert!(editor
            .save_route(Some(route), "Changed".into(), b, vec![bc])
            .is_err());
        assert_eq!(editor.snapshot(), before);
        assert!(editor
            .save_service_with_source_route(
                None,
                "Bad".into(),
                b,
                vec![bc],
                vec![0, 1],
                Some(route)
            )
            .is_err());
        assert_eq!(editor.snapshot(), before);
        editor
            .set_service_route_binding(id, ServiceRouteBindingKind::Independent)
            .unwrap();
        editor
            .save_route(Some(route), "Changed".into(), b, vec![bc])
            .unwrap();
        assert_eq!(editor.snapshot().services, before.services);
    }
    #[test]
    fn route_path_edits_conservatively_protect_existing_services_atomically() {
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
        let c = editor
            .add_station("C".into(), 35.0, 139.2)
            .unwrap()
            .stations[2]
            .id;
        let ab = editor.add_track(a, b).unwrap().tracks[0].id;
        let bc = editor.add_track(b, c).unwrap().tracks[1].id;
        let initial = editor.save_route(None, "R".into(), a, vec![ab]).unwrap();
        let route = initial.routes[0].id;
        // No services: editing the route remains supported.
        editor
            .save_route(Some(route), "R".into(), a, vec![ab, bc])
            .unwrap();
        editor
            .save_service(None, "S".into(), a, vec![ab], vec![0, 1])
            .unwrap();
        let before = editor.snapshot();
        assert!(editor
            .save_route(Some(route), "Changed".into(), a, vec![ab])
            .unwrap_err()
            .contains("未確認"));
        assert_eq!(editor.snapshot(), before);
        assert!(editor
            .save_planned_route(
                Some(route),
                "Changed".into(),
                a,
                vec![RouteSegment::Track { track_id: ab }],
                vec![]
            )
            .unwrap_err()
            .contains("未確認"));
        assert_eq!(editor.snapshot(), before);
        // Equivalent legacy -> explicit representation and rename are safe.
        let renamed = editor
            .save_planned_route(
                Some(route),
                "Renamed".into(),
                a,
                vec![
                    RouteSegment::Track { track_id: ab },
                    RouteSegment::Track { track_id: bc },
                ],
                vec![],
            )
            .unwrap();
        assert_eq!(renamed.services, before.services);
        assert_eq!(renamed.service_paths, before.service_paths);
        assert_eq!(renamed.routes[0].name, "Renamed");
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
    fn saved_path_editor_preserves_ids_and_roundtrips_stop_changes() {
        use rosenfu_core::world::RouteSegment;
        let mut editor = WorldEditor::new();
        let a = editor
            .world
            .add_station(editor.company, "A".into(), 35.0, 139.0)
            .unwrap();
        let b = editor
            .world
            .add_station(editor.company, "B".into(), 35.0, 139.1)
            .unwrap();
        let c = editor
            .world
            .add_station(editor.company, "C".into(), 35.0, 139.2)
            .unwrap();
        let ab = editor.world.add_track(editor.company, a, b).unwrap();
        let bc = editor.world.add_track(editor.company, b, c).unwrap();
        let segments = vec![
            RouteSegment::Track { track_id: ab },
            RouteSegment::Track { track_id: bc },
        ];
        let stops: Vec<_> = editor
            .preview_service_path(a, segments.clone())
            .unwrap()
            .occurrences
            .into_iter()
            .map(|o| o.reference)
            .collect();
        let created = editor
            .create_service_path("Local".into(), a, segments, stops.clone())
            .unwrap();
        let id = created.services[0].id;
        let updated = editor
            .update_service_path(
                id,
                "Express".into(),
                vec![stops[0].clone(), stops[2].clone()],
            )
            .unwrap();
        assert_eq!(updated.services[0].id, id);
        assert_eq!(updated.services[0].start, a);
        assert_eq!(updated.service_paths[0].legs, created.service_paths[0].legs);
        assert_eq!(
            updated.service_paths[0].next_leg_key,
            created.service_paths[0].next_leg_key
        );
        assert_eq!(updated.next_id, created.next_id);
        assert!(editor
            .update_service_path(id, "Invalid".into(), vec![])
            .is_err());
        assert_eq!(editor.snapshot(), updated);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("edited-service.json");
        editor.save(&path).unwrap();
        assert_eq!(WorldEditor::new().load(&path).unwrap(), updated);
        editor
            .update_service_path(id, "Local".into(), stops)
            .unwrap();
        assert_eq!(editor.snapshot(), created);
        let reverse = vec![
            RouteSegment::Track { track_id: bc },
            RouteSegment::Track { track_id: ab },
        ];
        let expected = created.service_paths[0].clone();
        let preview = editor
            .preview_service_path_replacement(id, c, reverse.clone())
            .unwrap();
        let stops = preview
            .occurrences
            .into_iter()
            .map(|o| o.reference)
            .collect::<Vec<_>>();
        let replaced = editor
            .replace_service_path(
                id,
                expected.clone(),
                "Reverse".into(),
                c,
                reverse.clone(),
                stops.clone(),
            )
            .unwrap();
        assert_eq!(replaced.services[0].id, id);
        assert_eq!(replaced.services[0].start, c);
        assert_eq!(replaced.next_id, created.next_id);
        assert!(editor
            .replace_service_path(id, expected, "Stale".into(), c, reverse, stops)
            .is_err());
        assert_eq!(editor.snapshot(), replaced);
        editor.save(&path).unwrap();
        assert_eq!(WorldEditor::new().load(&path).unwrap(), replaced);
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
            anchor_indices: None,
            extension: None,
            destination: None,
            name: "Attributed".into(),
            points: points.clone(),
            track_type: rosenfu_core::world::TrackType::Tram,
            design_speed_kmh: None,
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
        // Reference-followed extension shares the original alignment and is atomic
        // with later paths, planned stations, provenance, and allocation state.
        let id = editor.snapshot().alignments[0].id;
        let before = editor.snapshot();
        let reference_extension = || PlannedAlignment {
            anchor_indices: None,
            name: "Reference extension".into(),
            points: vec![
                points[1],
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.2,
                },
            ],
            track_type: rosenfu_core::world::TrackType::Tram,
            design_speed_kmh: None,
            stations: vec![planned_station(0.5)],
            reference: Some(PlannedAlignmentReference {
                source_record: source.clone(),
                provenance: provenance.clone(),
            }),
            destination: None,
            extension: Some(PlannedAlignmentExtension {
                alignment_id: id,
                endpoint: rosenfu_core::world::AlignmentEndpoint::End,
            }),
        };
        let mut invalid = planned_path(0.2);
        invalid.stations.push(planned_station(f64::NAN));
        assert!(editor
            .build_plan(vec![reference_extension(), invalid])
            .is_err());
        assert_eq!(editor.snapshot(), before);
        editor.build_plan(vec![reference_extension()]).unwrap();
        assert_eq!(editor.snapshot().alignments.len(), before.alignments.len());
        assert_eq!(
            editor.snapshot().alignment_provenance.len(),
            before.alignment_provenance.len() + 1
        );
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
    fn built_station(alignment_id: AlignmentId, position: f64) -> PlannedBuiltStation {
        PlannedBuiltStation {
            alignment_id,
            name: "既存線の計画駅".into(),
            position,
            length_m: 100.0,
            width_m: 12.0,
        }
    }
    #[test]
    fn crossing_station_group_rename_delete_save_and_undo_preserve_platform_ids() {
        let mut editor = WorldEditor::new();
        for (lat, lon) in [(0.0, 0.01), (0.01, 0.0), (0.01, 0.01)] {
            editor
                .build_plan(vec![PlannedAlignment {
                    anchor_indices: Some(vec![0, 1]),
                    name: "線路".into(),
                    points: vec![
                        GeoPoint {
                            latitude: 35.0 - lat,
                            longitude: 139.0 - lon,
                        },
                        GeoPoint {
                            latitude: 35.0 + lat,
                            longitude: 139.0 + lon,
                        },
                    ],
                    track_type: rosenfu_core::world::TrackType::Normal,
                    design_speed_kmh: Some(120.),
                    stations: vec![PlannedStation {
                        name: "共通駅".into(),
                        position: 0.5,
                        length_m: 100.,
                        width_m: 12.,
                    }],
                    reference: None,
                    extension: None,
                    destination: None,
                }])
                .unwrap();
        }
        let before = editor.snapshot();
        let ids: Vec<_> = before.stations.iter().map(|s| s.id).collect();
        assert_eq!(before.station_groups[0].members, ids);
        editor.rename_station(ids[2], "中央".into()).unwrap();
        let renamed = editor.snapshot();
        assert!(renamed.stations.iter().all(|s| s.name == "中央"));
        assert_eq!(renamed.station_placements, before.station_placements);
        let plans = serde_json::from_value(serde_json::json!({"version":1,"plans":[{"id":1,"name":"計画"}],"parkedPaths":[],"activePathId":1,"activePlanId":1,"draftPoints":[],"redoPoints":[],"draftExtension":null,"trackType":"normal","designSpeedKmh":120,"roadFollowing":false,"plannedStations":[],"draftFinished":false})).unwrap();
        let removed = editor.delete_station_with_plans(ids[0], plans).unwrap();
        assert_eq!(removed.station_groups[0].station_id, ids[1]);
        assert_eq!(removed.station_groups[0].members, ids[1..]);
        let restored = editor.undo_alignment_deletion().unwrap();
        assert_eq!(restored.stations, renamed.stations);
        assert_eq!(restored.station_groups, renamed.station_groups);
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("groups.json");
        editor.save(&path).unwrap();
        let mut loaded = WorldEditor::new();
        assert_eq!(
            loaded.load(&path).unwrap().station_groups,
            restored.station_groups
        );
        editor.delete_station(ids[0]).unwrap();
        editor.delete_station(ids[1]).unwrap();
        assert!(editor.snapshot().station_groups.is_empty());
        assert_eq!(editor.snapshot().stations[0].id, ids[2]);
    }
    #[test]
    fn overlapping_planned_station_commit_is_atomic() {
        let mut editor = WorldEditor::new();
        let mut base = planned_path(0.0);
        base.stations.clear();
        let before = editor.build_plan(vec![base]).unwrap();
        let id = before.alignments[0].id;
        let result = editor
            .build_construction_plan(vec![], vec![built_station(id, 0.5), built_station(id, 0.5)]);
        assert!(result.unwrap_err().contains("StationFootprintOverlap"));
        assert_eq!(editor.snapshot(), before);
        let mut new_path = planned_path(0.1);
        new_path.stations = vec![
            PlannedStation {
                name: "A".into(),
                position: 0.5,
                length_m: 100.,
                width_m: 12.,
            },
            PlannedStation {
                name: "B".into(),
                position: 0.5,
                length_m: 100.,
                width_m: 12.,
            },
        ];
        assert!(editor
            .build_plan(vec![new_path])
            .unwrap_err()
            .contains("StationFootprintOverlap"));
        assert_eq!(editor.snapshot(), before);
    }
    #[test]
    fn station_only_plan_builds_multiple_stations_and_mixed_plan_is_atomic() {
        let mut editor = WorldEditor::new();
        let mut base = planned_path(0.0);
        base.stations.clear();
        let before = editor.build_plan(vec![base]).unwrap();
        let id = before.alignments[0].id;
        let result = editor
            .build_construction_plan(
                vec![],
                vec![built_station(id, 0.25), built_station(id, 0.75)],
            )
            .unwrap();
        assert_eq!(result.alignments.len(), before.alignments.len());
        assert_eq!(result.stations.len(), before.stations.len() + 2);
        let before = editor.snapshot();
        assert!(editor
            .build_construction_plan(
                vec![planned_path(0.1)],
                vec![built_station(id, 0.4), built_station(id, 1.1)]
            )
            .is_err());
        assert_eq!(editor.snapshot(), before);
        let mut invalid = planned_path(0.1);
        invalid.stations.push(planned_station(1.1));
        assert!(editor
            .build_construction_plan(vec![invalid], vec![built_station(id, 0.4)])
            .is_err());
        assert_eq!(editor.snapshot(), before);
        let result = editor
            .build_construction_plan(vec![planned_path(0.1)], vec![built_station(id, 0.4)])
            .unwrap();
        assert_eq!(result.alignments.len(), before.alignments.len() + 1);
        assert_eq!(result.stations.len(), before.stations.len() + 2);
    }
    #[test]
    fn station_planned_on_existing_line_keeps_location_when_same_plan_extends_start() {
        let mut editor = WorldEditor::new();
        let mut base = planned_path(0.0);
        base.stations.clear();
        let before = editor.build_plan(vec![base]).unwrap();
        let a = &before.alignments[0];
        let expected = a.point_at(0.4);
        let start = a.points[0];
        let extension = PlannedAlignment {
            anchor_indices: None,
            name: "延伸".into(),
            points: vec![
                start,
                GeoPoint {
                    latitude: start.latitude,
                    longitude: start.longitude - 0.01,
                },
            ],
            track_type: a.track_type,
            design_speed_kmh: None,
            stations: vec![],
            reference: None,
            destination: None,
            extension: Some(PlannedAlignmentExtension {
                alignment_id: a.id,
                endpoint: rosenfu_core::world::AlignmentEndpoint::Start,
            }),
        };
        let result = editor
            .build_construction_plan(vec![extension], vec![built_station(a.id, 0.4)])
            .unwrap();
        let station = result.stations.last().unwrap();
        assert!((station.latitude - expected.latitude).abs() < 1e-8);
        assert!((station.longitude - expected.longitude).abs() < 1e-8);
        let placement = result.station_placements.last().unwrap();
        let actual = result.alignments[0].point_at(placement.position);
        assert!((actual.latitude - expected.latitude).abs() < 1e-8);
        assert!((actual.longitude - expected.longitude).abs() < 1e-8);
    }

    fn planned_path(offset: f64) -> PlannedAlignment {
        PlannedAlignment {
            anchor_indices: None,
            name: format!("計画線{offset}"),
            points: planned_alignment_points()
                .into_iter()
                .map(|mut p| {
                    p.latitude += offset;
                    p
                })
                .collect(),
            track_type: rosenfu_core::world::TrackType::Normal,
            design_speed_kmh: None,
            stations: vec![planned_station(0.5)],
            reference: None,
            extension: None,
            destination: None,
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
                anchor_indices: None,
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
                design_speed_kmh: None,
                stations: vec![planned_station(0.5)],
                reference: None,
                destination: None,
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
                anchor_indices: None,
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
                design_speed_kmh: None,
                stations: vec![planned_station(0.5)],
                reference: None,
                destination: None,
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
    #[test]
    fn planned_design_speed_validates_atomically_and_extension_inherits() {
        let mut editor = WorldEditor::new();
        let mut path = planned_path(0.0);
        path.design_speed_kmh = Some(95.0);
        let built = editor.build_plan(vec![path]).unwrap();
        let old = &built.alignments[0];
        assert_eq!(old.design_speed_kmh, Some(95.0));
        for speed in [0.0, 601.0, f64::NAN, f64::INFINITY] {
            let before = editor.snapshot();
            let mut bad = planned_path(0.1);
            bad.design_speed_kmh = Some(speed);
            assert!(editor.build_plan(vec![planned_path(0.2), bad]).is_err());
            assert_eq!(editor.snapshot(), before);
        }
        let endpoint = *old.points.last().unwrap();
        let previous = old.points[old.points.len() - 2];
        let extension = || PlannedAlignment {
            anchor_indices: None,
            name: "Extension".into(),
            points: vec![
                endpoint,
                GeoPoint {
                    latitude: endpoint.latitude + (endpoint.latitude - previous.latitude),
                    longitude: endpoint.longitude + (endpoint.longitude - previous.longitude),
                },
            ],
            track_type: old.track_type,
            design_speed_kmh: None,
            stations: vec![],
            reference: None,
            destination: None,
            extension: Some(PlannedAlignmentExtension {
                alignment_id: old.id,
                endpoint: rosenfu_core::world::AlignmentEndpoint::End,
            }),
        };
        let before = editor.snapshot();
        let mut changed_speed = extension();
        changed_speed.design_speed_kmh = Some(120.0);
        assert!(editor.build_plan(vec![changed_speed]).is_err());
        assert_eq!(editor.snapshot(), before);
        let after = editor.build_plan(vec![extension()]).unwrap();
        assert_eq!(after.alignments[0].design_speed_kmh, Some(95.0));
    }
}

#[cfg(test)]
mod endpoint_join_tests {
    use super::*;
    use rosenfu_core::world::{AlignmentEndpoint, TrackType};
    fn p(longitude: f64) -> GeoPoint {
        GeoPoint {
            latitude: 35.0,
            longitude,
        }
    }
    #[test]
    fn endpoint_join_is_atomic_in_all_orientations_and_survives_save() {
        for source_end in [AlignmentEndpoint::Start, AlignmentEndpoint::End] {
            for target_end in [AlignmentEndpoint::Start, AlignmentEndpoint::End] {
                let mut editor = WorldEditor::new();
                let mut a_points = vec![p(139.0), p(139.01)];
                let mut b_points = vec![p(139.02), p(139.03)];
                if source_end == AlignmentEndpoint::Start {
                    a_points.reverse();
                }
                if target_end == AlignmentEndpoint::End {
                    b_points.reverse();
                }
                let a = editor
                    .world
                    .build_alignment(editor.company, "A".into(), a_points)
                    .unwrap();
                let b = editor
                    .world
                    .build_alignment(editor.company, "B".into(), b_points)
                    .unwrap();
                editor.world.set_alignment_design_speed(b, 80.).unwrap();
                let sa = editor
                    .world
                    .place_station_on_alignment(editor.company, "A駅".into(), a, 0.5, 100., 12.)
                    .unwrap();
                let sb = editor
                    .world
                    .place_station_on_alignment(editor.company, "B駅".into(), b, 0.5, 100., 12.)
                    .unwrap();
                let sa2 = editor
                    .world
                    .place_station_on_alignment(editor.company, "A駅2".into(), a, 0.8, 100., 12.)
                    .unwrap();
                let segments = vec![rosenfu_core::world::RouteSegment::Alignment {
                    alignment_id: a,
                    from: sa,
                    to: sa2,
                }];
                editor
                    .world
                    .add_planned_route(
                        editor.company,
                        "既存路線".into(),
                        sa,
                        segments.clone(),
                        vec![sa, sa2],
                    )
                    .unwrap();
                let preview = editor.preview_service_path(sa, segments.clone()).unwrap();
                editor
                    .create_service_path(
                        "既存系統".into(),
                        sa,
                        segments,
                        preview
                            .occurrences
                            .into_iter()
                            .map(|o| o.reference)
                            .collect(),
                    )
                    .unwrap();
                let before = editor.snapshot();
                let request = || PlannedAlignment {
                    anchor_indices: None,
                    name: "延伸".into(),
                    points: vec![p(139.01), p(139.02)],
                    track_type: TrackType::Normal,
                    design_speed_kmh: Some(120.),
                    stations: vec![],
                    reference: None,
                    extension: Some(PlannedAlignmentExtension {
                        alignment_id: a,
                        endpoint: source_end,
                    }),
                    destination: Some(PlannedAlignmentExtension {
                        alignment_id: b,
                        endpoint: target_end,
                    }),
                };
                let mut invalid = request();
                invalid.points.clear();
                assert!(editor.build_plan(vec![request(), invalid]).is_err());
                assert_eq!(editor.snapshot(), before);
                let after = editor.build_plan(vec![request()]).unwrap();
                assert_eq!(after.alignments.len(), 2);
                assert_eq!(after.alignments[1], before.alignments[1]);
                assert_eq!(after.stations, before.stations);
                assert_eq!(after.next_id, before.next_id);
                assert_eq!(after.alignment_connections.len(), 1);
                assert_eq!(after.routes, before.routes);
                assert_eq!(after.route_paths, before.route_paths);
                assert_eq!(after.services, before.services);
                assert_eq!(after.service_paths, before.service_paths);
                assert_eq!(
                    editor.world.route_candidates(sa, sb).unwrap().status,
                    rosenfu_core::world::RouteSearchStatus::Unique
                );
                assert!(editor.delete_alignment(a).unwrap_err().contains("接続"));
                assert!(editor.delete_alignment(b).unwrap_err().contains("接続"));
                assert_eq!(editor.snapshot(), after);
                let dir = tempfile::tempdir().unwrap();
                let file = dir.path().join("joined.rosenfu");
                editor.save(&file).unwrap();
                assert_eq!(WorldEditor::new().load(&file).unwrap(), after);
            }
        }
    }
    #[test]
    fn moved_or_missing_target_cannot_publish_the_extension() {
        let mut editor = WorldEditor::new();
        let a = editor
            .world
            .build_alignment(editor.company, "A".into(), vec![p(139.), p(139.01)])
            .unwrap();
        let b = editor
            .world
            .build_alignment(editor.company, "B".into(), vec![p(139.021), p(139.03)])
            .unwrap();
        let before = editor.snapshot();
        for target in [b, AlignmentId(999), a] {
            let request = PlannedAlignment {
                anchor_indices: None,
                name: "延伸".into(),
                points: vec![p(139.01), p(139.02)],
                track_type: TrackType::Normal,
                design_speed_kmh: Some(120.),
                stations: vec![],
                reference: None,
                extension: Some(PlannedAlignmentExtension {
                    alignment_id: a,
                    endpoint: AlignmentEndpoint::End,
                }),
                destination: Some(PlannedAlignmentExtension {
                    alignment_id: target,
                    endpoint: AlignmentEndpoint::Start,
                }),
            };
            assert!(editor.build_plan(vec![request]).is_err());
            assert_eq!(editor.snapshot(), before);
        }
    }
    #[test]
    fn close_same_alignment_gap_atomically_and_restore_saved_ring() {
        let mut editor = WorldEditor::new();
        let point = |degrees: f64| GeoPoint {
            latitude: 35. + degrees.to_radians().sin() * 0.01,
            longitude: 139. + degrees.to_radians().cos() * 0.01,
        };
        let original: Vec<_> = (5..=175).map(|i| point(i as f64 * 2.)).collect();
        let a = editor
            .world
            .build_alignment(editor.company, "Ring".into(), original.clone())
            .unwrap();
        let station = editor
            .world
            .place_station_on_alignment(editor.company, "S".into(), a, 0.5, 100., 10.)
            .unwrap();
        let before = editor.snapshot();
        let mut bridge: Vec<_> = (175..=185).map(|i| point(i as f64 * 2.)).collect();
        *bridge.last_mut().unwrap() = original[0];
        let request = || PlannedAlignment {
            name: "Close ring".into(),
            points: bridge.clone(),
            anchor_indices: Some(vec![0, bridge.len() - 1]),
            track_type: TrackType::Normal,
            design_speed_kmh: Some(120.),
            stations: vec![],
            reference: None,
            extension: Some(PlannedAlignmentExtension {
                alignment_id: a,
                endpoint: AlignmentEndpoint::End,
            }),
            destination: Some(PlannedAlignmentExtension {
                alignment_id: a,
                endpoint: AlignmentEndpoint::Start,
            }),
        };
        let mut wrong = request();
        wrong.destination.as_mut().unwrap().endpoint = AlignmentEndpoint::End;
        assert!(editor.build_plan(vec![wrong]).is_err());
        assert_eq!(editor.snapshot(), before);
        let after = editor.build_plan(vec![request()]).unwrap();
        assert_eq!(after.alignments.len(), 1);
        assert_eq!(after.alignments[0].id, a);
        assert_eq!(after.stations[0].id, station);
        assert_eq!(after.alignment_connections.len(), 1);
        assert_eq!(
            after.alignments[0].anchor_indices,
            Some(vec![
                0,
                original.len() - 1,
                original.len() + bridge.len() - 2
            ])
        );
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ring.rosenfu");
        editor.save(&file).unwrap();
        assert_eq!(WorldEditor::new().load(&file).unwrap(), after);
    }
}
