#[path = "session/road_transport.rs"]
mod road_transport;
pub use road_transport::{GameplayBusPathPreview, GameplayBusPreviewOptions, RoadTramEdit};
// Desktop application operations. The core owns IDs and referential integrity.
#[path = "session/deletion.rs"]
mod deletion;
#[path = "session/history.rs"]
mod history;
#[path = "session/operation.rs"]
mod operation;
pub use deletion::AlignmentSectionSelection;
pub use history::WorldEditHistory;
pub use operation::{CrossingPlanningView, OperationTickView, OperationView};
pub use operation::{FixedOdRoadTransfer, FixedOdScenarioView, ObservedFixedOdInput};
#[path = "session/ofm.rs"]
mod ofm;
pub use ofm::{OfmBindingInput, OfmRawInput};
#[path = "session/save_slots.rs"]
mod save_slots;
use rosenfu_core::world::{
    AlignmentId, CompanyId, GeoPoint, RouteId, ServiceId, StationId, TrackType, World, WorldDraft,
};
pub use save_slots::SaveSlotList;
use std::path::{Path, PathBuf};

// Prototype edit extent, not a national boundary or map-coverage guarantee.
fn in_edit_extent(latitude: f64, longitude: f64) -> bool {
    (20.0..=46.0).contains(&latitude) && (122.0..=154.0).contains(&longitude)
}

/// Match validated snapshots by stable ID, then compare complete persistent values.
fn changed_summary_alignment_ids(
    before: &[rosenfu_core::world::Alignment],
    after: &[rosenfu_core::world::Alignment],
) -> std::collections::BTreeSet<AlignmentId> {
    let previous: std::collections::BTreeMap<_, _> =
        before.iter().map(|line| (line.id, line)).collect();
    after
        .iter()
        .filter(|line| previous.get(&line.id).is_none_or(|old| *old != *line))
        .map(|line| line.id)
        .collect()
}

// Match the previous exact f64 equality: signed zeros are equal; NaN never
// matches even itself. Do not round nearby physical crossings to one key.
fn crossing_key(road_id: &str, point: GeoPoint) -> Option<(&str, u64, u64)> {
    fn coordinate(value: f64) -> Option<u64> {
        (!value.is_nan()).then(|| if value == 0.0 { 0 } else { value.to_bits() })
    }

    Some((
        road_id,
        coordinate(point.latitude)?,
        coordinate(point.longitude)?,
    ))
}

fn count_new_crossings<'a>(
    prior: impl IntoIterator<Item = (&'a str, GeoPoint)>,
    after: impl IntoIterator<Item = (&'a str, GeoPoint)>,
) -> usize {
    let previous: std::collections::HashSet<_> = prior
        .into_iter()
        .filter_map(|(road, point)| crossing_key(road, point))
        .collect();
    after
        .into_iter()
        .filter(|&(road, point)| {
            crossing_key(road, point).is_none_or(|key| !previous.contains(&key))
        })
        .count()
}

#[cfg(test)]
#[test]
fn crossing_summary_index_preserves_exact_equality_and_multiplicity() {
    let p = |latitude, longitude| GeoPoint {
        latitude,
        longitude,
    };
    let before = [
        ("r", p(-0.0, 0.0)),
        ("r", p(1.0, 2.0)),
        ("r", p(1.0, 2.0)),
        ("nan", p(f64::NAN, 2.0)),
        ("nan2", p(1.0, f64::NAN)),
    ];
    let after = [
        ("r", p(0.0, -0.0)),
        ("r", p(1.0, 2.0)),
        ("other", p(1.0, 2.0)),
        ("r", p(1.0 + f64::EPSILON, 2.0)),
        ("r", p(1.0, 2.0 + 2.0 * f64::EPSILON)),
        ("r", p(1.0, 2.0 + 2.0 * f64::EPSILON)),
        ("nan", p(f64::NAN, 2.0)),
        ("nan2", p(1.0, f64::NAN)),
    ];

    let reference = after
        .iter()
        .filter(|(road, point)| {
            !before
                .iter()
                .any(|(old_road, old_point)| old_road == road && old_point == point)
        })
        .count();
    assert_eq!(reference, 6);
    assert_eq!(count_new_crossings(before, after), reference);
    assert_eq!(count_new_crossings([], after), after.len());
    assert_eq!(count_new_crossings(before, []), 0);
}

/// Compact advisory result; no preview world is serialized across IPC.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionPreview {
    pub new_crossing_count: Option<usize>,
    pub roads_unverified: Option<bool>,
    pub validation_error: Option<String>,
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
    pub ofm_path: Option<rosenfu_core::ofm_evidence::PathDescriptor>,
    #[serde(default)]
    pub anchor_levels: Option<Vec<rosenfu_core::world::InfrastructureLevel>>,
    #[serde(default)]
    pub junction_source: Option<rosenfu_core::world::ConstructionJunctionSource>,
    #[serde(default)]
    pub start_level: Option<rosenfu_core::world::InfrastructureLevel>,
    #[serde(default)]
    pub infrastructure: rosenfu_core::world::SectionInfrastructure,
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

// Legacy single-file replacement helpers are retained only for regression tests.
// Production saves use the slot-aware methods below.
#[cfg(test)]
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
#[cfg(test)]
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
    water_model: rosenfu_core::world::ConstructionWaterModel,
    world: World,
    company: CompanyId,
    loaded_path: Option<PathBuf>,
    active_slot_id: Option<String>,
    company_setup_required: bool,
    manual_version: Option<rosenfu_storage::FileVersion>,
    manual_repair_allowed: bool,
    autosave_version: Option<(PathBuf, rosenfu_storage::FileVersion)>,
    observed_autosave: Option<(PathBuf, rosenfu_storage::FileVersion)>,
    autosave_blocked: bool,
    autosave_replace_allowed: bool,
    deleted_alignment: Option<(WorldDraft, WorldDraft)>,
    edit_history: history::EditHistory,
    operation_tick: Option<std::time::Instant>,
    // Invalidates pending schedule edits after another operation intent, including ABA.
    operation_edit_revision: u64,
    // Per-mounted-world and scenario-revision correlation, not a credential.
    fixed_od_token: u64,
    #[cfg(test)]
    pending_manual_replacement: Option<PendingManualReplacement>,
}
impl Default for WorldEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldEditor {
    pub fn set_service_appearance(
        &mut self,
        service_id: ServiceId,
        color: String,
        shape: rosenfu_core::world::ServiceSymbolShape,
        symbol: String,
        expected_appearance: Option<rosenfu_core::world::ServiceAppearance>,
    ) -> Result<WorldDraft, String> {
        let current = self
            .snapshot()
            .service_appearances
            .into_iter()
            .find(|row| row.service_id == service_id);
        if current != expected_appearance {
            return Err("系統の表示設定が変更されています。最新の設定を確認してください。".into());
        }
        let mut candidate = self.clone();
        candidate
            .world
            .set_service_appearance(candidate.company, service_id, color, shape, symbol)
            .map_err(|e| e.to_string())?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        self.commit_edit(candidate, false)?;
        Ok(self.snapshot())
    }
    pub fn set_service_color(
        &mut self,
        service_id: ServiceId,
        color: String,
        expected_color: Option<String>,
    ) -> Result<WorldDraft, String> {
        let current = self
            .snapshot()
            .service_appearances
            .into_iter()
            .find(|row| row.service_id == service_id)
            .map(|row| row.color);
        if current != expected_color {
            return Err("系統色が変更されています。最新の色を確認してください。".into());
        }
        let mut candidate = self.clone();
        candidate
            .world
            .set_service_color(candidate.company, service_id, color)
            .map_err(|e| e.to_string())?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        self.commit_edit(candidate, false)?;
        Ok(self.snapshot())
    }
    /// Delete only the selected service and its own metadata, atomically.
    pub fn delete_service(
        &mut self,
        service_id: ServiceId,
        expected_path: rosenfu_core::world::ServicePath,
        expected_name: String,
    ) -> Result<WorldDraft, String> {
        let before = self.snapshot();
        let service = before
            .services
            .iter()
            .find(|s| s.id == service_id && s.owner == self.company)
            .ok_or("削除する系統が見つかりません。")?;
        if service.name != expected_name
            || before
                .service_paths
                .iter()
                .find(|p| p.service_id == service_id)
                != Some(&expected_path)
        {
            return Err(
                "系統が変更されています。最新の内容を確認してから削除してください。".into(),
            );
        }
        let mut draft = before.clone();
        draft.services.retain(|s| s.id != service_id);
        draft.service_paths.retain(|p| p.service_id != service_id);
        draft
            .service_route_bindings
            .retain(|b| b.service_id != service_id);
        draft
            .service_appearances
            .retain(|a| a.service_id != service_id);
        let mut candidate = self.clone();
        candidate.world.replace(draft).map_err(|e| e.to_string())?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        let after = candidate.snapshot();
        candidate.deleted_alignment = Some((before, after.clone()));
        self.commit_edit(candidate, false)?;
        Ok(after)
    }

    pub fn duplicate_service(
        &mut self,
        service_id: ServiceId,
        expected_path: rosenfu_core::world::ServicePath,
        expected_name: String,
    ) -> Result<WorldDraft, String> {
        let snapshot = self.snapshot();
        let source = snapshot
            .services
            .iter()
            .find(|s| s.id == service_id && s.owner == self.company)
            .ok_or("複製する系統が見つかりません。")?;
        if source.name != expected_name
            || snapshot
                .service_paths
                .iter()
                .find(|p| p.service_id == service_id)
                != Some(&expected_path)
        {
            return Err(
                "系統が変更されています。最新の内容を確認してから複製してください。".into(),
            );
        }
        let name = format!(
            "{} のコピー",
            source.name.chars().take(74).collect::<String>()
        );
        let mut candidate = self.clone();
        candidate
            .world
            .duplicate_service(candidate.company, service_id, name)
            .map_err(|e| e.to_string())?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        self.commit_edit(candidate, false)?;
        Ok(self.snapshot())
    }
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
        self.commit_edit(candidate, false)?;
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
    #[cfg(test)]
    pub fn create_service_path(
        &mut self,
        name: String,
        start: StationId,
        segments: Vec<rosenfu_core::world::RouteSegment>,
        stops: Vec<rosenfu_core::world::StationOccurrenceRef>,
    ) -> Result<WorldDraft, String> {
        self.create_service_path_with_source_route(name, start, segments, stops, None)
    }
    #[cfg(test)]
    pub fn create_service_path_with_source_route(
        &mut self,
        name: String,
        start: StationId,
        segments: Vec<rosenfu_core::world::RouteSegment>,
        stops: Vec<rosenfu_core::world::StationOccurrenceRef>,
        source_route_id: Option<RouteId>,
    ) -> Result<WorldDraft, String> {
        self.create_service_path_with_source_routes(
            name,
            start,
            segments,
            stops,
            source_route_id,
            None,
        )
    }
    pub fn create_service_path_with_source_routes(
        &mut self,
        name: String,
        start: StationId,
        segments: Vec<rosenfu_core::world::RouteSegment>,
        stops: Vec<rosenfu_core::world::StationOccurrenceRef>,
        source_route_id: Option<RouteId>,
        source_route_ids: Option<Vec<RouteId>>,
    ) -> Result<WorldDraft, String> {
        let binding = source_route_binding(source_route_id, source_route_ids)?;
        let mut candidate = self.clone();
        candidate.create_service_path_unchecked(name, start, segments, stops)?;
        if let Some(binding) = binding {
            let id = candidate
                .world
                .snapshot()
                .services
                .last()
                .expect("created service")
                .id;
            candidate
                .world
                .set_service_route_binding(candidate.company, id, binding)
                .map_err(|e| e.to_string())?;
        }
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        self.commit_edit(candidate, false)?;
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
        self.apply_reconciled_service_path_edit(candidate, service_id)
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
    #[cfg(test)]
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
    // Keep the established session/IPC call shape; all arguments form one atomic edit.
    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
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
        self.replace_service_path_with_source_routes(
            service_id,
            expected_path,
            name,
            start,
            segments,
            stops,
            source_route_id,
            None,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn replace_service_path_with_source_routes(
        &mut self,
        service_id: ServiceId,
        expected_path: rosenfu_core::world::ServicePath,
        name: String,
        start: StationId,
        segments: Vec<rosenfu_core::world::RouteSegment>,
        stops: Vec<rosenfu_core::world::StationOccurrenceRef>,
        source_route_id: Option<RouteId>,
        source_route_ids: Option<Vec<RouteId>>,
    ) -> Result<WorldDraft, String> {
        let binding = source_route_binding(source_route_id, source_route_ids)?;
        let mut candidate = self.clone();
        // Rebinding and path replacement are one outer transaction. The temporary
        // independent row exists only in this discarded-on-failure clone.
        if binding.is_some() {
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
        if let Some(binding) = binding {
            let id = service_id;
            candidate
                .world
                .set_service_route_binding(candidate.company, id, binding)
                .map_err(|e| e.to_string())?;
        }
        self.apply_reconciled_service_path_edit(candidate, service_id)
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
            water_model: Default::default(),
            world,
            company,
            loaded_path: None,
            active_slot_id: None,
            company_setup_required: true,
            manual_version: None,
            manual_repair_allowed: false,
            autosave_version: None,
            observed_autosave: None,
            autosave_blocked: false,
            autosave_replace_allowed: false,
            deleted_alignment: None,
            edit_history: Default::default(),
            operation_tick: None,
            operation_edit_revision: operation::next_session_token(),
            fixed_od_token: operation::next_session_token(),
            #[cfg(test)]
            pending_manual_replacement: None,
        }
    }
    pub fn enable_national_water(&mut self) -> Result<(), String> {
        self.water_model.enable_national()
    }
    pub fn national_water_enabled(&self) -> bool {
        self.water_model.national_enabled()
    }
    pub fn missing_water_zones(&self, bounds: [f64; 4]) -> Result<Vec<String>, String> {
        // Preserve connected-track coverage, but query the immutable region
        // registry once instead of allocating a result for every track segment.
        let segments = self.world.snapshot().alignments.iter().flat_map(|line| {
            line.points.windows(2).map(|pair| {
                let a = pair[0];
                let b = pair[1];
                [
                    a.longitude.min(b.longitude),
                    a.latitude.min(b.latitude),
                    a.longitude.max(b.longitude),
                    a.latitude.max(b.latitude),
                ]
            })
        });
        let mut ids = self
            .water_model
            .missing_zones_many(std::iter::once(bounds).chain(segments))?;
        ids.sort();
        Ok(ids)
    }

    pub fn install_water_zone(
        &mut self,
        zone: std::sync::Arc<rosenfu_core::water_zone::WaterZone>,
    ) -> Result<(), String> {
        self.water_model.install_zone(zone)
    }
    pub fn missing_water_masks(&self) -> Vec<rosenfu_core::world::WaterMaskRequest> {
        self.water_model.missing_requests()
    }
    pub fn install_water_mask(&mut self, id: &str, bytes: &[u8]) -> Result<(), String> {
        self.water_model.install(id, bytes)
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
        self.water_model
            .validate(self.world.snapshot(), candidate.world.snapshot())?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        self.commit_edit(candidate, true)?;
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
    #[cfg(test)]
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
    #[cfg(test)]
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
        self.water_model
            .validate(self.world.snapshot(), candidate.world.snapshot())?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        self.commit_edit(candidate, true)?;
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
        self.water_model
            .validate(self.world.snapshot(), candidate.world.snapshot())?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        self.commit_edit(candidate, true)?;
        Ok(self.snapshot())
    }

    #[cfg(test)]
    pub fn build_alignment(
        &mut self,
        name: String,
        points: Vec<GeoPoint>,
    ) -> Result<WorldDraft, String> {
        self.build_alignment_typed(name, points, rosenfu_core::world::TrackType::Normal)
    }
    #[cfg(test)]
    pub fn build_alignment_typed(
        &mut self,
        name: String,
        points: Vec<GeoPoint>,
        track_type: rosenfu_core::world::TrackType,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate.build_alignment_typed_unchecked(name, points, track_type)?;
        self.water_model
            .validate(self.world.snapshot(), candidate.world.snapshot())?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        self.commit_edit(candidate, true)?;
        Ok(self.snapshot())
    }
    /// Publish the alignment and all planned stations together, or change nothing.
    #[cfg(test)]
    pub fn build_alignment_with_stations(
        &mut self,
        name: String,
        points: Vec<GeoPoint>,
        track_type: rosenfu_core::world::TrackType,
        stations: Vec<PlannedStation>,
        reference: Option<PlannedAlignmentReference>,
    ) -> Result<WorldDraft, String> {
        self.build_plan(vec![PlannedAlignment {
            ofm_path: None,
            anchor_levels: None,
            junction_source: None,
            start_level: None,
            infrastructure: Default::default(),
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
    /// Read-only preview runs the exact construction pipeline on an isolated
    /// editor clone; neither IDs, undo state nor saved data are changed.
    #[cfg(test)]
    pub fn preview_construction_plan(
        &self,
        alignments: Vec<PlannedAlignment>,
        stations: Vec<PlannedBuiltStation>,
    ) -> Result<WorldDraft, String> {
        self.clone().build_construction_plan(alignments, stations)
    }
    /// Clone only the construction inputs while holding the live editor lock.
    /// The worker never applies an edit, so undo, saves and tick bookkeeping
    /// are deliberately not copied. Keep the World itself (including its
    /// operation state): the size guard must see what the build would check.
    pub fn construction_validation_snapshot(&self) -> Self {
        Self {
            water_model: self.water_model.clone(),
            world: self.world.clone(),
            company: self.company,
            loaded_path: None,
            active_slot_id: None,
            company_setup_required: self.company_setup_required,
            fixed_od_token: self.fixed_od_token,
            manual_version: None,
            manual_repair_allowed: false,
            autosave_version: None,
            observed_autosave: None,
            autosave_blocked: false,
            autosave_replace_allowed: false,
            deleted_alignment: None,
            edit_history: history::EditHistory::default(),
            operation_tick: None,
            operation_edit_revision: self.operation_edit_revision,
            #[cfg(test)]
            pending_manual_replacement: None,
        }
    }
    /// Click placement needs only the exact build verdict, not world-wide crossing
    /// totals. Keep all build/water/saveability checks, without computing those totals.
    pub fn validate_construction_placement(
        mut self,
        alignments: Vec<PlannedAlignment>,
        stations: Vec<PlannedBuiltStation>,
        roads: Vec<rosenfu_core::world::RoadSnapshot>,
    ) -> Result<ConstructionPreview, String> {
        for snapshot in roads {
            self.world
                .add_road_snapshot(snapshot)
                .map_err(|e| e.to_string())?;
        }
        // Water compares against the snapshot after road staging, but before
        // any candidate track/station changes. This copies just the draft, not
        // a second editor or its active operation state.
        let before = self.world.snapshot().clone();
        let validation_error = self
            .prepare_construction_candidate(&before, alignments, stations)
            .err();
        Ok(ConstructionPreview {
            // Not computed, rather than falsely reporting zero/verified.
            new_crossing_count: None,
            roads_unverified: None,
            validation_error,
        })
    }
    /// Use the same road snapshots and construction pipeline as commit, without
    /// changing the live editor. Existing crossings are not charged again.
    pub fn preview_construction_summary(
        mut self,
        alignments: Vec<PlannedAlignment>,
        stations: Vec<PlannedBuiltStation>,
        roads: Vec<rosenfu_core::world::RoadSnapshot>,
    ) -> Result<ConstructionPreview, String> {
        for snapshot in roads {
            self.world
                .add_road_snapshot(snapshot)
                .map_err(|e| e.to_string())?;
        }
        // Refusals include no crossing metrics. Validate first so a rejected
        // hover never scans the current world's road assets. A successful
        // preview keeps the exact before/after topology and separate scans.
        let candidate = match self.prepared_construction_clone(alignments, stations) {
            Ok(candidate) => candidate,
            Err(error) => {
                return Ok(ConstructionPreview {
                    new_crossing_count: None,
                    roads_unverified: None,
                    validation_error: Some(error),
                })
            }
        };
        let before = self.world.snapshot();
        let prior_crossings: Vec<_> = self
            .road_assessments()
            .into_iter()
            .flat_map(|(_, assessment)| assessment.crossings)
            .collect();
        let after = candidate.world.snapshot();
        let changed = changed_summary_alignment_ids(&before.alignments, &after.alignments);
        // This isolated preview does not publish history, operation or OD; none
        // of that metadata is an input to per-world physical road assessment.
        let assessments = candidate.road_assessments();
        let roads_unverified = assessments
            .iter()
            .any(|(id, a)| changed.contains(id) && (!a.roads_ready || a.unknown_road_height));
        // Endpoint splitting may change IDs or ownership, not the physical asset.
        let new_crossing_count = count_new_crossings(
            prior_crossings
                .iter()
                .map(|crossing| (crossing.road_id.as_str(), crossing.point)),
            assessments
                .iter()
                .flat_map(|(_, assessment)| &assessment.crossings)
                .map(|crossing| (crossing.road_id.as_str(), crossing.point)),
        );
        Ok(ConstructionPreview {
            new_crossing_count: Some(new_crossing_count),
            roads_unverified: Some(roads_unverified),
            validation_error: None,
        })
    }
    /// Reference snapshots and construction commit together; failures leave the
    /// current editor (including Undo state) untouched. Road coverage is advisory.
    pub fn build_construction_plan_with_roads(
        &mut self,
        alignments: Vec<PlannedAlignment>,
        stations: Vec<PlannedBuiltStation>,
        roads: Vec<rosenfu_core::world::RoadSnapshot>,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        for snapshot in roads {
            candidate
                .world
                .add_road_snapshot(snapshot)
                .map_err(|error| error.to_string())?;
        }
        // The water model compares physical alignments, stations and connections
        // from the live baseline. Newly staged road snapshots are validated on
        // the candidate, and do not change those inputs.
        candidate.prepare_construction_candidate(self.world.snapshot(), alignments, stations)?;
        candidate.deleted_alignment = None;

        // Preparation already validated the canonical save. Reconciliation
        // only retains these non-draft payloads and may freeze an invalidated
        // runtime; it does not change geometry or synthesize numeric inputs.
        // Recount if reconciliation changed the prepared payload so that the
        // final live state still respects the hard save cap.
        let prepared_operation = candidate.world.operation_save();
        let prepared_od = candidate.world.fixed_od_scenario().cloned();
        candidate.world.retain_operation_from(&self.world);
        candidate.world.retain_fixed_od_scenario_from(&self.world);
        if prepared_operation != candidate.world.operation_save()
            || prepared_od.as_ref() != candidate.world.fixed_od_scenario()
        {
            rosenfu_storage::ensure_world_saveable(&candidate.world)
                .map_err(|error| error.to_string())?;
        }
        self.publish_reconciled_edit(candidate, true)?;
        Ok(self.snapshot())
    }
    pub fn road_assessments(&self) -> Vec<(AlignmentId, rosenfu_core::world::RoadAssessment)> {
        self.world.road_assessments().into_iter().collect()
    }
    pub fn build_construction_plan(
        &mut self,
        alignments: Vec<PlannedAlignment>,
        stations: Vec<PlannedBuiltStation>,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.prepared_construction_clone(alignments, stations)?;
        // Construction is final. Only a successful atomic commit ends prior history.
        candidate.deleted_alignment = None;
        self.commit_edit(candidate, true)?;
        Ok(self.snapshot())
    }

    /// Shared by actual construction and full summary. All geometry, water,
    /// topology, and save-size checks happen before either caller can proceed.
    fn prepared_construction_clone(
        &self,
        alignments: Vec<PlannedAlignment>,
        stations: Vec<PlannedBuiltStation>,
    ) -> Result<Self, String> {
        let mut candidate = self.clone();
        candidate.prepare_construction_candidate(self.world.snapshot(), alignments, stations)?;
        Ok(candidate)
    }

    /// Mutate only the isolated candidate. Both full commit and compact click
    /// validation pass exactly the same topology, water and save-size checks.
    fn prepare_construction_candidate(
        &mut self,
        before: &WorldDraft,
        alignments: Vec<PlannedAlignment>,
        stations: Vec<PlannedBuiltStation>,
    ) -> Result<(), String> {
        if alignments.is_empty() && stations.is_empty() {
            return Err("着工する線路・駅がありません。".into());
        }
        // Keep the candidate explicit throughout the shared mutation body.
        let candidate = self;
        // Track only pieces created by this transaction. This permits several
        // planned splits of one observed source without accepting external edits.
        let mut split_sources: std::collections::BTreeMap<
            AlignmentId,
            (
                rosenfu_core::world::Alignment,
                Vec<rosenfu_core::world::Alignment>,
            ),
        > = std::collections::BTreeMap::new();
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
        for mut alignment in alignments {
            let ofm_path = alignment.ofm_path.clone();
            if let Some(path) = &ofm_path {
                if alignment.reference.is_some() || alignment.junction_source.is_some() {
                    return Err("道路追従と参照元・途中分岐を同時には指定できません。".into());
                }
                let scoped = candidate
                    .world
                    .snapshot()
                    .runtime_road_evidence
                    .scope(&path.required_binding_keys)
                    .map_err(|e| e.to_string())?;
                let rebuilt = rosenfu_core::ofm_evidence::validate_path_descriptor(&scoped, path)
                    .map_err(|e| e.to_string())?;
                alignment.points = rebuilt
                    .points
                    .into_iter()
                    .map(|p| GeoPoint {
                        longitude: p[0],
                        latitude: p[1],
                    })
                    .collect();
                if alignment
                    .points
                    .iter()
                    .any(|p| !in_edit_extent(p.latitude, p.longitude))
                {
                    return Err("路線は編集範囲内に建設してください。".into());
                }
            }

            if !alignment.infrastructure.is_valid() {
                return Err("線路数は1・2・4のいずれかです。".into());
            }
            let anchor_indices = alignment.anchor_indices.clone();
            if anchor_indices.as_ref().is_some_and(|indices| {
                indices.len() < 2
                    || indices.first() != Some(&0)
                    || indices.last().copied() != alignment.points.len().checked_sub(1)
                    || indices.windows(2).any(|p| p[0] >= p[1])
            }) {
                return Err("線路の区間境界が不正です。".into());
            }
            if let Some(source) = alignment.junction_source.take() {
                if alignment.extension.is_some() || !source.is_valid() {
                    return Err("分岐元の指定が不正です。".into());
                }
                let expected = &source.expected_alignment;
                let a = expected.points[source.segment_index];
                let b = expected.points[source.segment_index + 1];
                let point = GeoPoint {
                    latitude: a.latitude + (b.latitude - a.latitude) * source.fraction,
                    longitude: a.longitude + (b.longitude - a.longitude) * source.fraction,
                };
                let (source_id, segment_index, fraction) = if let Some((base, pieces)) =
                    split_sources.get(&source.alignment_id)
                {
                    if base != expected
                        || pieces.iter().any(|piece| {
                            candidate
                                .world
                                .snapshot()
                                .alignments
                                .iter()
                                .find(|a| a.id == piece.id)
                                != Some(piece)
                        })
                    {
                        return Err(
                            "分岐元の線路が変更されています。計画を作り直してください。".into()
                        );
                    }
                    let mut found = None;
                    for piece in pieces {
                        for (index, pair) in piece.points.windows(2).enumerate() {
                            let dx = pair[1].longitude - pair[0].longitude;
                            let dy = pair[1].latitude - pair[0].latitude;
                            let t = ((point.longitude - pair[0].longitude) * dx
                                + (point.latitude - pair[0].latitude) * dy)
                                / (dx * dx + dy * dy);
                            if (0.0..=1.0).contains(&t)
                                && (pair[0].longitude + dx * t - point.longitude).abs() < 1e-10
                                && (pair[0].latitude + dy * t - point.latitude).abs() < 1e-10
                                && !(index == 0 && t < 1e-9)
                                && !(index + 2 == piece.points.len() && t > 1.0 - 1e-9)
                            {
                                if let Some((prior_id, prior_index, prior_t)) = found {
                                    let same_vertex = prior_id == piece.id
                                        && prior_index + 1 == index
                                        && prior_t > 1.0 - 1e-9
                                        && t < 1e-9;
                                    if !same_vertex {
                                        return Err("分岐位置が重複する線形です。分岐を分けて計画してください。".into());
                                    }
                                } else {
                                    found = Some((piece.id, index, t));
                                }
                            }
                        }
                    }
                    found
                        .ok_or("同じ分岐点への重複分岐、または変更された線路には分岐できません。")?
                } else {
                    let original = candidate
                        .world
                        .snapshot()
                        .alignments
                        .iter()
                        .find(|a| a.id == source.alignment_id)
                        .ok_or("分岐元の線路がありません。")?;
                    if original != expected {
                        return Err(
                            "分岐元の線路が変更されています。計画を作り直してください。".into()
                        );
                    }
                    split_sources.insert(
                        source.alignment_id,
                        (expected.clone(), vec![original.clone()]),
                    );
                    (source.alignment_id, source.segment_index, source.fraction)
                };
                let first = alignment
                    .points
                    .first_mut()
                    .ok_or("分岐の線路がありません。")?;
                if (first.latitude - point.latitude).abs() > 1e-7
                    || (first.longitude - point.longitude).abs() > 1e-7
                {
                    return Err("分岐の始点が指定位置と一致しません。".into());
                }
                *first = point;
                let suffix_id = candidate
                    .world
                    .split_alignment_at_position(
                        candidate.company,
                        source_id,
                        segment_index,
                        fraction,
                    )
                    .map_err(|e| format!("この位置では分岐できません: {e}"))?;
                let start_height = candidate
                    .world
                    .snapshot()
                    .alignments
                    .iter()
                    .find(|a| a.id == source_id)
                    .ok_or("分岐元がありません。")?
                    .endpoint_height_m(rosenfu_core::world::AlignmentEndpoint::End);
                if let Some(destination) = &mut alignment.destination {
                    if destination.alignment_id == source_id
                        && destination.endpoint == rosenfu_core::world::AlignmentEndpoint::End
                    {
                        destination.alignment_id = suffix_id;
                    }
                }
                alignment.extension = Some(PlannedAlignmentExtension {
                    alignment_id: if source.reverse { suffix_id } else { source_id },
                    endpoint: if source.reverse {
                        rosenfu_core::world::AlignmentEndpoint::Start
                    } else {
                        rosenfu_core::world::AlignmentEndpoint::End
                    },
                });
                candidate.build_sloped_plan(alignment, start_height)?;
                let (_, pieces) = split_sources
                    .get_mut(&source.alignment_id)
                    .expect("registered source");
                pieces.retain(|p| p.id != source_id);
                for id in [source_id, suffix_id] {
                    pieces.push(
                        candidate
                            .world
                            .snapshot()
                            .alignments
                            .iter()
                            .find(|a| a.id == id)
                            .expect("split piece")
                            .clone(),
                    );
                }
                continue;
            }
            let start_height = if let Some(extension) = &alignment.extension {
                let old = candidate
                    .world
                    .snapshot()
                    .alignments
                    .iter()
                    .find(|a| a.id == extension.alignment_id)
                    .ok_or("延伸対象の線路がありません。")?;
                Some(old.endpoint_height_m(extension.endpoint))
            } else {
                alignment
                    .anchor_levels
                    .as_ref()
                    .and_then(|levels| levels.first())
                    .copied()
                    .or(alignment.start_level)
                    .map(|level| level.height_m())
            };
            // A source-derived path may meet an existing endpoint at its end
            // without extending an existing alignment. Keep it independent and
            // let the shared physical connection validator check exact geometry,
            // height, tangent and grade rather than silently moving its points.
            if ofm_path.is_some()
                && alignment.destination.is_some()
                && alignment.extension.is_none()
            {
                let height =
                    start_height.unwrap_or_else(|| alignment.infrastructure.level.height_m());
                candidate.build_sloped_plan(alignment, height)?;
                continue;
            }
            if let Some(levels) = &alignment.anchor_levels {
                let count = alignment.anchor_indices.as_ref().map_or(2, |a| a.len());
                if levels.len() != count {
                    return Err("アンカーの高さ指定数が一致しません。".into());
                }
                let height = start_height.unwrap_or_else(|| levels[0].height_m());
                candidate.build_sloped_plan(alignment, height)?;
                continue;
            }
            if let Some(start_height) = start_height {
                let finish_height = alignment
                    .destination
                    .as_ref()
                    .and_then(|end| {
                        candidate
                            .world
                            .snapshot()
                            .alignments
                            .iter()
                            .find(|a| a.id == end.alignment_id)
                            .map(|a| a.endpoint_height_m(end.endpoint))
                    })
                    .unwrap_or_else(|| alignment.infrastructure.level.height_m());
                let has_endpoint_grade = alignment.extension.as_ref().is_some_and(|end| {
                    candidate
                        .world
                        .snapshot()
                        .alignments
                        .iter()
                        .find(|a| a.id == end.alignment_id)
                        .is_some_and(|a| a.endpoint_inward_grade(end.endpoint).abs() > 1e-12)
                }) || alignment.destination.as_ref().is_some_and(|end| {
                    candidate
                        .world
                        .snapshot()
                        .alignments
                        .iter()
                        .find(|a| a.id == end.alignment_id)
                        .is_some_and(|a| a.endpoint_inward_grade(end.endpoint).abs() > 1e-12)
                });
                let source_profile = alignment.extension.as_ref().is_some_and(|end| {
                    candidate
                        .world
                        .snapshot()
                        .alignments
                        .iter()
                        .find(|a| a.id == end.alignment_id)
                        .is_some_and(|a| {
                            a.vertical_profile.is_some() || !a.section_profiles.is_empty()
                        })
                });
                if (start_height - finish_height).abs() > 1e-6
                    || has_endpoint_grade
                    || source_profile
                {
                    candidate.build_sloped_plan(alignment, start_height)?;
                    continue;
                }
            }
            if alignment.destination.is_some() && alignment.extension.is_none() {
                return Err("接続先を指定するには既存線路から延伸してください。".into());
            }
            if let Some(extension) = &alignment.extension {
                let old = candidate
                    .world
                    .snapshot()
                    .alignments
                    .iter()
                    .find(|a| a.id == extension.alignment_id)
                    .ok_or("延伸対象の線路がありません。")?;
                if Self::mixed_tram_track(old.track_type, alignment.track_type) {
                    // A change between tram-only and conventional infrastructure
                    // is a junction to a new span, not an extension of the old ID.
                    let start_height = old.endpoint_height_m(extension.endpoint);
                    candidate.build_sloped_plan(alignment, start_height)?;
                    continue;
                }
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
                if let Some(path) = ofm_path {
                    candidate
                        .world
                        .extend_alignment_from_road_path(
                            candidate.company,
                            extension.alignment_id,
                            extension.endpoint,
                            path,
                        )
                        .map_err(|e| e.to_string())?;
                } else if let Some(reference) = alignment.reference {
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
            let alignment_id = if let Some(path) = ofm_path {
                candidate
                    .world
                    .build_alignment_from_road_path(
                        candidate.company,
                        alignment.name.trim().to_owned(),
                        alignment.track_type,
                        path,
                    )
                    .map_err(|e| e.to_string())?
            } else {
                match alignment.reference {
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
                }
            };
            if let Some(indices) = anchor_indices {
                candidate.set_anchor_indices(alignment_id, indices)?;
            }
            {
                let mut draft = candidate.world.snapshot().clone();
                let built = draft
                    .alignments
                    .iter_mut()
                    .find(|item| item.id == alignment_id)
                    .ok_or("線路がありません。")?;
                built.infrastructure = if alignment.infrastructure == Default::default() {
                    Vec::new()
                } else {
                    vec![alignment.infrastructure; built.section_count()]
                };
                candidate.world.replace(draft).map_err(|e| e.to_string())?;
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
        candidate
            .water_model
            .validate(before, candidate.world.snapshot())?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        Ok(())
    }
    fn mixed_tram_track(a: TrackType, b: TrackType) -> bool {
        (a == TrackType::Tram) != (b == TrackType::Tram)
    }
    /// A rising/falling or mixed-track extension is a new explicit span.
    fn build_sloped_plan(
        &mut self,
        alignment: PlannedAlignment,
        start_height: f64,
    ) -> Result<(), String> {
        let inherited_speed = if let Some(extension) = &alignment.extension {
            let source = self
                .world
                .snapshot()
                .alignments
                .iter()
                .find(|a| a.id == extension.alignment_id && a.owner == self.company)
                .ok_or("延伸対象の自社線路がありません。")?;
            if Self::mixed_tram_track(source.track_type, alignment.track_type) {
                alignment.design_speed_kmh
            } else {
                if source.track_type != alignment.track_type {
                    return Err("延伸対象と線路種別が一致しません。".into());
                }
                let speed = source.effective_design_speed_kmh();
                if alignment
                    .design_speed_kmh
                    .is_some_and(|v| (v - speed).abs() > 1e-9)
                {
                    return Err("延伸では元の線路の目標最高速度を引き継ぎます。".into());
                }
                Some(speed)
            }
        } else {
            alignment.design_speed_kmh
        };
        let id = if let Some(path) = alignment.ofm_path {
            self.world
                .build_alignment_from_road_path(
                    self.company,
                    alignment.name.trim().to_owned(),
                    alignment.track_type,
                    path,
                )
                .map_err(|e| e.to_string())?
        } else {
            match alignment.reference {
                Some(reference) => self.build_alignment_from_reference_typed_unchecked(
                    alignment.name,
                    alignment.points,
                    reference.source_record,
                    reference.provenance,
                    alignment.track_type,
                )?,
                None => self.build_alignment_typed_unchecked(
                    alignment.name,
                    alignment.points,
                    alignment.track_type,
                )?,
            }
        };
        let mut draft = self.world.snapshot().clone();
        let built = draft
            .alignments
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or("線路がありません。")?;
        built.anchor_indices = alignment.anchor_indices;
        built.infrastructure = vec![alignment.infrastructure; built.section_count()];
        let start_grade = alignment
            .extension
            .as_ref()
            .and_then(|end| {
                self.world
                    .snapshot()
                    .alignments
                    .iter()
                    .find(|a| a.id == end.alignment_id)
                    .map(|a| -a.endpoint_inward_grade(end.endpoint))
            })
            .unwrap_or(0.0);
        let destination = alignment.destination.as_ref().and_then(|end| {
            self.world
                .snapshot()
                .alignments
                .iter()
                .find(|a| a.id == end.alignment_id)
                .map(|a| {
                    (
                        a.endpoint_height_m(end.endpoint),
                        a.endpoint_inward_grade(end.endpoint),
                    )
                })
        });
        let (finish_height, end_grade) = destination.unwrap_or((
            alignment
                .anchor_levels
                .as_ref()
                .and_then(|levels| levels.last())
                .copied()
                .unwrap_or(alignment.infrastructure.level)
                .height_m(),
            0.0,
        ));
        use rosenfu_core::world::InfrastructureLevel;
        if let Some(actual_level) = [
            InfrastructureLevel::Ground,
            InfrastructureLevel::Elevated,
            InfrastructureLevel::Elevated2,
            InfrastructureLevel::Elevated3,
            InfrastructureLevel::Underground,
            InfrastructureLevel::Underground2,
            InfrastructureLevel::Underground3,
        ]
        .into_iter()
        .find(|level| (level.height_m() - finish_height).abs() < 1e-9)
        {
            for section in &mut built.infrastructure {
                section.level = actual_level;
            }
        }
        let profile = rosenfu_core::world::VerticalProfile {
            start_tangent_m: start_grade * built.length_m(),
            end_tangent_m: end_grade * built.length_m(),
            start_height_m: start_height,
            end_height_m: finish_height,
        };
        // Even a flat inherited height may differ from the selected layer label.
        built.vertical_profile = Some(profile);
        if let Some(levels) = alignment.anchor_levels {
            if levels.len() != built.section_count() + 1 {
                return Err("アンカーの高さ指定数が一致しません。".into());
            }
            let mut heights: Vec<_> = levels.iter().map(|level| level.height_m()).collect();
            heights[0] = start_height;
            *heights.last_mut().unwrap() = finish_height;
            let lengths = built.section_lengths_m();
            built.section_profiles = lengths
                .iter()
                .enumerate()
                .map(|(i, length)| rosenfu_core::world::VerticalProfile {
                    start_height_m: heights[i],
                    end_height_m: heights[i + 1],
                    start_tangent_m: if i == 0 { start_grade * length } else { 0. },
                    end_tangent_m: if i + 1 == lengths.len() {
                        end_grade * length
                    } else {
                        0.
                    },
                })
                .collect();
            for (i, section) in built.infrastructure.iter_mut().enumerate() {
                section.level = levels[i + 1];
            }
            if built
                .section_profiles
                .iter()
                .zip(&lengths)
                .any(|(profile, length)| !profile.is_valid(*length, built.track_type))
            {
                return Err(format!(
                    "アンカー間の勾配が上限 {}‰ を超えます。区間を長くしてください。",
                    built.track_type.maximum_construction_gradient_per_mille()
                ));
            }
            built.vertical_profile = None;
        }
        if let Some(speed) = inherited_speed {
            built.design_speed_kmh = Some(speed);
        }
        if built.section_profiles.is_empty()
            && !profile.is_valid(built.length_m(), built.track_type)
        {
            return Err(format!(
                "勾配が上限 {}‰ を超えます。線路を長くしてください。",
                built.track_type.maximum_construction_gradient_per_mille()
            ));
        }
        self.world.replace(draft).map_err(|e| e.to_string())?;
        if let Some(extension) = alignment.extension {
            self.world
                .connect_alignment_endpoints(
                    self.company,
                    rosenfu_core::world::AlignmentEndpointRef {
                        alignment_id: extension.alignment_id,
                        endpoint: extension.endpoint,
                    },
                    rosenfu_core::world::AlignmentEndpointRef {
                        alignment_id: id,
                        endpoint: rosenfu_core::world::AlignmentEndpoint::Start,
                    },
                )
                .map_err(|e| e.to_string())?;
        }
        if let Some(destination) = alignment.destination {
            self.world
                .connect_alignment_endpoints(
                    self.company,
                    rosenfu_core::world::AlignmentEndpointRef {
                        alignment_id: id,
                        endpoint: rosenfu_core::world::AlignmentEndpoint::End,
                    },
                    rosenfu_core::world::AlignmentEndpointRef {
                        alignment_id: destination.alignment_id,
                        endpoint: destination.endpoint,
                    },
                )
                .map_err(|e| e.to_string())?;
        }
        for station in alignment.stations {
            self.place_station_on_alignment_unchecked(
                station.name,
                id,
                station.position,
                station.length_m,
                station.width_m,
            )?;
        }
        Ok(())
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
        self.water_model
            .validate(self.world.snapshot(), candidate.world.snapshot())?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        self.commit_edit(candidate, true)?;
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
        self.water_model
            .validate(self.world.snapshot(), candidate.world.snapshot())?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        self.commit_edit(candidate, true)?;
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
        self.commit_edit(candidate, false)?;
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
        self.commit_edit(candidate, false)?;
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
        self.commit_edit(candidate, false)?;
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
        self.commit_edit(candidate, false)?;
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
        // Keep the legacy entry point coherent with the common chronology.
        // Its exact equality guard above also permits restoration of old plans.
        let mut expected = self.snapshot();
        expected.construction_plans = before.construction_plans.clone();
        expected.view_state = before.view_state;
        self.undo_world_edit(expected)
    }

    pub fn rename_company(&mut self, name: String) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate.rename_company_unchecked(name)?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        self.commit_edit(candidate, false)?;
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
    pub fn rename_route(
        &mut self,
        route_id: RouteId,
        name: String,
        expected_name: String,
    ) -> Result<WorldDraft, String> {
        if name.trim().is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control)
        {
            return Err("路線名は制御文字を含まない1〜80文字で入力してください。".into());
        }
        let mut candidate = self.clone();
        let mut draft = candidate.snapshot();
        let route = draft
            .routes
            .iter_mut()
            .find(|route| route.id == route_id && route.owner == self.company)
            .ok_or("編集する路線が見つかりません。")?;
        if route.name != expected_name {
            return Err(
                "路線名が変更されています。最新の名前を確認してから変更してください。".into(),
            );
        }
        route.name = name.trim().to_owned();
        candidate.world.replace(draft).map_err(|e| e.to_string())?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        self.commit_edit(candidate, false)?;
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
        self.commit_edit(candidate, false)?;
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
            let previous_segments = &candidate
                .route_paths
                .iter()
                .find(|p| p.route_id == id)
                .ok_or("路線の経路データがありません。")?
                .segments;
            // Services currently own independent physical paths, not route IDs.
            // Do not guess which service depends on a route: until explicit
            // associations exist, reject path changes conservatively.
            if (previous.start != start || previous_segments != &preview.path.segments)
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

    /// A fresh world may replace an existing healthy save only after approval of
    /// this exact observed version. Loaded-session conflicts stay ordinary errors.
    #[cfg(test)]
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
    #[cfg(test)]
    pub fn cancel_manual_save(&mut self, token: &str) {
        if self
            .pending_manual_replacement
            .as_ref()
            .is_some_and(|pending| pending.token == token)
        {
            self.pending_manual_replacement = None;
        }
    }
    #[cfg(test)]
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
        candidate.world.retain_operation_from(&self.world);
        candidate.world.retain_fixed_od_scenario_from(&self.world);
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
    #[cfg(test)]
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
        self.advance_operation()?;
        let mut candidate = self.clone();
        // Save only changes editor metadata. The loaded world and current
        // runtime retain their existing validation; storage still validates
        // the full final archive and checks the exact previous file version.
        candidate
            .world
            .update_save_metadata(plans, view)
            .map_err(|e| e.to_string())?;
        if automatic {
            candidate.autosave(path)?;
        } else {
            candidate.save(path)?;
        }
        *self = candidate;
        Ok(())
    }
    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        self.advance_operation()?;
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
        #[cfg(test)]
        {
            self.pending_manual_replacement = None;
        }
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
        self.advance_operation()?;
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
    #[cfg(test)]
    pub fn load_autosave(
        &mut self,
        autosave_path: &Path,
        manual_path: &Path,
    ) -> Result<WorldDraft, String> {
        self.restore_saved_autosave(autosave_path, manual_path)?;
        Ok(self.snapshot())
    }

    /// Restore atomically, retaining normal backup behavior for a healthy
    /// manual save. Hold only the decoded replacement, not a copy of the
    /// previously mounted world; publish after all fallible reads/checks.
    fn restore_saved_autosave(
        &mut self,
        autosave_path: &Path,
        manual_path: &Path,
    ) -> Result<(), String> {
        let (world, auto_version, company) = Self::read_saved_world(autosave_path)?;
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
        self.install_saved_world(world, company, manual_path, manual_version);
        self.manual_repair_allowed = repair_allowed;
        self.autosave_version = Some((autosave_path.to_owned(), auto_version));
        Ok(())
    }

    #[cfg(test)]
    pub fn load(&mut self, path: &Path) -> Result<WorldDraft, String> {
        let (world, version, company) = Self::read_saved_world(path)?;
        self.install_saved_world(world, company, path, version);
        Ok(self.snapshot())
    }

    fn read_saved_world(
        path: &Path,
    ) -> Result<(World, rosenfu_storage::FileVersion, CompanyId), String> {
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
        Ok((candidate, version, company))
    }

    fn install_saved_world(
        &mut self,
        world: World,
        company: CompanyId,
        path: &Path,
        version: rosenfu_storage::FileVersion,
    ) {
        self.deleted_alignment = None;
        self.clear_edit_history();
        self.world = world;
        self.operation_tick = None;
        self.fixed_od_token = operation::next_session_token();
        self.operation_edit_revision = operation::next_session_token();
        self.company = company;
        self.company_setup_required = false;
        #[cfg(test)]
        {
            self.pending_manual_replacement = None;
        }
        self.loaded_path = Some(path.to_owned());
        self.active_slot_id = None;
        self.manual_version = Some(version);
        self.manual_repair_allowed = false;
        self.autosave_version = None;
        self.observed_autosave = None;
        self.autosave_blocked = false;
        self.autosave_replace_allowed = false;
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
    #[test]
    fn station_water_add_resize_and_plan_refusals_are_atomic() {
        use rosenfu_core::world::{InfrastructureLevel, SectionInfrastructure};
        let mut editor = WorldEditor::new();
        let points = vec![
            GeoPoint {
                longitude: 140.,
                latitude: 33.,
            },
            GeoPoint {
                longitude: 140.02,
                latitude: 33.,
            },
        ];
        // A legacy ground alignment over water may be loaded, but new stations
        // must still respect today's construction model.
        let id = editor
            .world
            .build_alignment(editor.company, "legacy".into(), points)
            .unwrap();
        let before = editor.snapshot();
        assert!(editor
            .place_station_on_alignment("new".into(), id, 0.5, 100., 10.)
            .unwrap_err()
            .contains("地上駅"));
        assert_eq!(editor.snapshot(), before);
        assert!(editor
            .add_station("legacy point".into(), 33., 140.)
            .is_err());
        assert_eq!(editor.snapshot(), before);
        let station = editor
            .world
            .place_station_on_alignment(editor.company, "old".into(), id, 0.5, 100., 10.)
            .unwrap();
        editor.rename_station(station, "renamed".into()).unwrap();
        let before = editor.snapshot();
        assert!(editor.update_station_placement(station, 200., 10.).is_err());
        assert_eq!(editor.snapshot(), before);
        let planned = || PlannedBuiltStation {
            alignment_id: id,
            name: "planned".into(),
            position: 0.8,
            length_m: 100.,
            width_m: 10.,
        };
        assert!(editor
            .preview_construction_plan(vec![], vec![planned()])
            .is_err());
        assert!(editor
            .build_construction_plan(vec![], vec![planned()])
            .is_err());
        assert_eq!(editor.snapshot(), before);
        // An elevated physical platform is allowed; station metadata is unchanged.
        let mut elevated = editor.snapshot();
        elevated.alignments[0].infrastructure = vec![SectionInfrastructure {
            level: InfrastructureLevel::Elevated,
            ..Default::default()
        }];
        editor.world.replace(elevated).unwrap();
        editor
            .place_station_on_alignment("bridge".into(), id, 0.8, 100., 10.)
            .unwrap();
    }
    #[test]
    fn national_water_requests_include_remote_existing_tracks_after_load_and_keep_provider_on_new_world(
    ) {
        use rosenfu_core::water_zone::registry;
        let mut editor = WorldEditor::new();
        let points = vec![
            GeoPoint {
                longitude: 141.350,
                latitude: 43.068,
            },
            GeoPoint {
                longitude: 141.351,
                latitude: 43.069,
            },
        ];
        editor
            .world
            .build_alignment(editor.company, "remote".into(), points.clone())
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        editor.save(&path).unwrap();
        let mut loaded = WorldEditor::new();
        loaded.enable_national_water().unwrap();
        loaded.load(&path).unwrap();
        assert!(loaded.national_water_enabled());
        let local = [139.75, 35.67, 139.76, 35.68];
        let mut expected: std::collections::BTreeSet<String> = registry::intersecting(local)
            .unwrap()
            .iter()
            .map(|p| p.zone_id.clone())
            .collect();
        expected.extend(
            registry::intersecting([141.350, 43.068, 141.351, 43.069])
                .unwrap()
                .iter()
                .map(|p| p.zone_id.clone()),
        );
        assert_eq!(
            loaded.missing_water_zones(local).unwrap(),
            expected.into_iter().collect::<Vec<_>>()
        );
        let before = loaded.snapshot();
        // No implicit empty mask can make a loaded region buildable before acquisition.
        assert!(loaded
            .build_alignment(
                "missing water".into(),
                vec![
                    GeoPoint {
                        longitude: 139.75,
                        latitude: 35.67
                    },
                    GeoPoint {
                        longitude: 139.751,
                        latitude: 35.67
                    }
                ]
            )
            .is_err());
        assert_eq!(loaded.snapshot(), before);
        loaded.create_new_world("new".into()).unwrap();
        assert!(loaded.national_water_enabled());
        assert_eq!(
            loaded.missing_water_zones(local).unwrap(),
            registry::intersecting(local)
                .unwrap()
                .iter()
                .map(|p| p.zone_id.clone())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        );
    }
    #[test]
    #[ignore = "requires local water-only evidence, not redistributed"]
    fn external_national_water_build_save_reload_uses_same_fixed_model() {
        let root = std::path::PathBuf::from(std::env::var("WATER_ZONE_EVIDENCE").unwrap());
        let mut editor = WorldEditor::new();
        editor.enable_national_water().unwrap();
        for pin in rosenfu_core::water_zone::registry::pins().unwrap() {
            let bytes = std::fs::read(root.join(format!("{}.water", pin.zone_id))).unwrap();
            editor
                .install_water_zone(std::sync::Arc::new(
                    rosenfu_core::water_zone::WaterZone::decode(pin.clone(), &bytes).unwrap(),
                ))
                .unwrap();
        }
        editor.rename_company("national test".into()).unwrap();
        editor
            .build_alignment(
                "Tokyo dry model".into(),
                vec![
                    GeoPoint {
                        longitude: 139.7671,
                        latitude: 35.6812,
                    },
                    GeoPoint {
                        longitude: 139.7673,
                        latitude: 35.6814,
                    },
                ],
            )
            .unwrap();
        let expected = editor.snapshot();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        editor.save(&path).unwrap();
        let mut loaded = WorldEditor::new();
        loaded.enable_national_water().unwrap();
        assert_eq!(loaded.load(&path).unwrap(), expected);
        assert!(loaded.national_water_enabled());
        assert!(!loaded
            .missing_water_zones([139.767, 35.681, 139.768, 35.682])
            .unwrap()
            .is_empty());
        // Acquired model data is session state, never silently embedded in schema 25.
        assert!(!std::fs::read_to_string(path)
            .unwrap()
            .contains("2026-11-13"));
    }
    fn pending_token(editor: &mut WorldEditor, path: &Path) -> String {
        let result = editor.request_manual_save(path, None, None).unwrap();
        assert!(!result.saved);
        result.replacement_required.unwrap().token
    }
    pub(super) fn alignment_fixture() -> (WorldEditor, AlignmentId, [StationId; 3]) {
        let mut e = WorldEditor::new();
        let a = e
            .world
            .build_alignment(
                e.company,
                "線形".into(),
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
        let s = [0.1, 0.5, 0.9].map(|position| {
            e.world
                .place_station_on_alignment(e.company, "駅".into(), a, position, 100.0, 10.0)
                .unwrap()
        });
        (e, a, s)
    }
    pub(super) fn span(
        a: AlignmentId,
        from: StationId,
        to: StationId,
    ) -> rosenfu_core::world::RouteSegment {
        rosenfu_core::world::RouteSegment::Alignment {
            alignment_id: a,
            from,
            to,
        }
    }
    fn create_test_service(
        e: &mut WorldEditor,
        start: StationId,
        segments: Vec<rosenfu_core::world::RouteSegment>,
    ) -> WorldDraft {
        let stops = e
            .preview_service_path(start, segments.clone())
            .unwrap()
            .occurrences
            .into_iter()
            .map(|o| o.reference)
            .collect();
        e.create_service_path("系統".into(), start, segments, stops)
            .unwrap()
    }

    #[test]
    fn appearance_compare_and_set_protects_shape_and_symbol_changes() {
        use rosenfu_core::world::ServiceSymbolShape;
        let (mut editor, alignment, stations) = alignment_fixture();
        let created = create_test_service(
            &mut editor,
            stations[0],
            vec![span(alignment, stations[0], stations[2])],
        );
        let service = created.services[0].id;
        let first = editor
            .set_service_appearance(
                service,
                "#112233".into(),
                ServiceSymbolShape::Square,
                "八".into(),
                None,
            )
            .unwrap();
        let stale = first.service_appearances[0].clone();
        let second = editor
            .set_service_appearance(
                service,
                "#112233".into(),
                ServiceSymbolShape::Diamond,
                "快".into(),
                Some(stale.clone()),
            )
            .unwrap();
        assert!(editor
            .set_service_appearance(
                service,
                "#445566".into(),
                ServiceSymbolShape::Circle,
                "普".into(),
                Some(stale)
            )
            .is_err());
        assert_eq!(editor.snapshot(), second);
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("symbol.json");
        editor.save(&path).unwrap();
        assert_eq!(
            WorldEditor::new().load(&path).unwrap().service_appearances,
            second.service_appearances
        );
    }

    #[test]
    fn service_delete_preserves_infrastructure_others_and_roundtrips_undo_redo_save() {
        let (mut e, a, stations) = alignment_fixture();
        let route = e
            .save_planned_route(
                None,
                "R".into(),
                stations[0],
                vec![span(a, stations[0], stations[2])],
                vec![],
            )
            .unwrap()
            .routes[0]
            .id;
        let service =
            create_test_service(&mut e, stations[0], vec![span(a, stations[0], stations[2])])
                .services[0]
                .id;
        e.set_service_route_binding(
            service,
            rosenfu_core::world::ServiceRouteBindingKind::Route { route_id: route },
        )
        .unwrap();
        e.set_service_color(service, "#ab12ef".into(), None)
            .unwrap();
        let original = e.snapshot();
        let path = original.service_paths[0].clone();
        e.duplicate_service(service, path.clone(), "系統".into())
            .unwrap();
        let before = e.snapshot();
        let after = e
            .delete_service(service, path.clone(), "系統".into())
            .unwrap();
        let mut expected = before.clone();
        expected.services.retain(|s| s.id != service);
        expected.service_paths.retain(|p| p.service_id != service);
        expected
            .service_route_bindings
            .retain(|b| b.service_id != service);
        expected
            .service_appearances
            .retain(|a| a.service_id != service);
        assert_eq!(after, expected);
        assert_eq!(after.services.len(), 1);
        for _ in 0..3 {
            assert_eq!(e.undo_alignment_deletion().unwrap(), before);
            assert_eq!(
                e.delete_service(service, path.clone(), "系統".into())
                    .unwrap(),
                after
            );
        }
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("deleted-service.json");
        e.save(&file).unwrap();
        let loaded = WorldEditor::new().load(&file).unwrap();
        assert_eq!(loaded.services, after.services);
        assert_eq!(loaded.service_paths, after.service_paths);
        assert_eq!(loaded.service_route_bindings, after.service_route_bindings);
        assert_eq!(loaded.service_appearances, after.service_appearances);
        assert_eq!(loaded.routes, before.routes);
        assert_eq!(loaded.stations, before.stations);
    }

    #[test]
    fn service_delete_rejects_stale_and_foreign_requests_atomically() {
        let (mut e, a, stations) = alignment_fixture();
        let service =
            create_test_service(&mut e, stations[0], vec![span(a, stations[0], stations[2])])
                .services[0]
                .id;
        let before = e.snapshot();
        let path = before.service_paths[0].clone();
        assert!(e
            .delete_service(service, path.clone(), "古い名前".into())
            .is_err());
        let mut stale = path.clone();
        stale.stops.clear();
        assert!(e.delete_service(service, stale, "系統".into()).is_err());
        assert_eq!(e.snapshot(), before);
        let owner = e.company;
        e.company = rosenfu_core::world::CompanyId(999);
        assert!(e
            .delete_service(service, path.clone(), "系統".into())
            .is_err());
        assert_eq!(e.snapshot(), before);
        e.company = owner;
        e.delete_service(service, path.clone(), "系統".into())
            .unwrap();
        let after = e.snapshot();
        assert!(e.delete_service(service, path, "系統".into()).is_err());
        assert_eq!(e.snapshot(), after);
        e.rename_company("後の編集".into()).unwrap();
        let later = e.snapshot();
        assert!(e.undo_alignment_deletion().is_err());
        assert_eq!(e.snapshot(), later);
    }

    #[test]
    fn service_metadata_duplicate_preserves_path_binding_and_has_independent_edits() {
        let (mut e, a, s) = alignment_fixture();
        let route = e
            .save_planned_route(None, "R".into(), s[0], vec![span(a, s[0], s[2])], vec![])
            .unwrap()
            .routes[0]
            .id;
        let service = create_test_service(&mut e, s[0], vec![span(a, s[0], s[2])]).services[0].id;
        e.set_service_route_binding(
            service,
            rosenfu_core::world::ServiceRouteBindingKind::Route { route_id: route },
        )
        .unwrap();
        e.set_service_color(service, "#ab12ef".into(), None)
            .unwrap();
        let before = e.snapshot();
        let path = before.service_paths[0].clone();
        let after = e
            .duplicate_service(service, path.clone(), "系統".into())
            .unwrap();
        let copy = after.services.iter().find(|s| s.id != service).unwrap();
        assert_eq!(copy.name, "系統 のコピー");
        let mut copied_path = after
            .service_paths
            .iter()
            .find(|p| p.service_id == copy.id)
            .unwrap()
            .clone();
        copied_path.service_id = service;
        assert_eq!(copied_path, path);
        assert_eq!(
            after.service_route_bindings[1].binding,
            before.service_route_bindings[0].binding
        );
        e.set_service_color(copy.id, "#001122".into(), Some("#AB12EF".into()))
            .unwrap();
        let copy_stops = vec![
            path.stops.first().unwrap().clone(),
            path.stops.last().unwrap().clone(),
        ];
        e.update_service_path(copy.id, "独立したコピー".into(), copy_stops.clone())
            .unwrap();
        assert_ne!(copy_stops, path.stops);
        let edited = e.snapshot();
        assert_eq!(
            edited.services.iter().find(|s| s.id == service),
            before.services.first()
        );
        assert_eq!(
            edited
                .service_paths
                .iter()
                .find(|p| p.service_id == service),
            before.service_paths.first()
        );
        assert_eq!(
            edited
                .service_appearances
                .iter()
                .find(|p| p.service_id == service),
            before.service_appearances.first()
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("metadata.json");
        e.save(&path).unwrap();
        assert_eq!(WorldEditor::new().load(&path).unwrap(), edited);
    }
    #[test]
    fn service_metadata_duplicate_does_not_guess_unknown_binding_or_color() {
        let (mut e, a, s) = alignment_fixture();
        let source = create_test_service(&mut e, s[0], vec![span(a, s[0], s[2])]);
        let service = source.services[0].id;
        let result = e
            .duplicate_service(
                service,
                source.service_paths[0].clone(),
                source.services[0].name.clone(),
            )
            .unwrap();
        assert_eq!(result.services.len(), 2);
        assert!(result.service_route_bindings.is_empty());
        assert!(result.service_appearances.is_empty());
    }
    #[test]
    fn service_metadata_rejects_stale_invalid_and_other_owner_without_partial_updates() {
        let (mut e, a, s) = alignment_fixture();
        let service = create_test_service(&mut e, s[0], vec![span(a, s[0], s[2])]).services[0].id;
        let before = e.snapshot();
        let path = before.service_paths[0].clone();
        assert!(e
            .set_service_color(service, "#123456".into(), Some("#abcdef".into()))
            .is_err());
        assert_eq!(e.snapshot(), before);
        assert!(e.set_service_color(service, "bad".into(), None).is_err());
        assert_eq!(e.snapshot(), before);
        assert!(e
            .duplicate_service(service, path.clone(), "古い名前".into())
            .is_err());
        assert_eq!(e.snapshot(), before);
        let mut stale = path.clone();
        stale.stops.pop();
        assert!(e.duplicate_service(service, stale, "系統".into()).is_err());
        assert_eq!(e.snapshot(), before);
        let company = e.company;
        e.company = CompanyId(u64::MAX);
        assert!(e.duplicate_service(service, path, "系統".into()).is_err());
        assert!(e
            .set_service_color(service, "#123456".into(), None)
            .is_err());
        assert_eq!(e.snapshot(), before);
        e.company = company;
    }

    #[test]
    fn route_rename_preserves_everything_except_name_and_roundtrips() {
        let (mut e, a, s) = alignment_fixture();
        let route = e
            .save_planned_route(None, "本線".into(), s[0], vec![span(a, s[0], s[2])], vec![])
            .unwrap()
            .routes[0]
            .id;
        let service = create_test_service(&mut e, s[0], vec![span(a, s[0], s[2])]).services[0].id;
        e.set_service_route_binding(
            service,
            rosenfu_core::world::ServiceRouteBindingKind::Route { route_id: route },
        )
        .unwrap();
        e.set_service_appearance(
            service,
            "#123456".into(),
            rosenfu_core::world::ServiceSymbolShape::Diamond,
            "快".into(),
            None,
        )
        .unwrap();
        let before = e.snapshot();
        let result = e
            .rename_route(route, " 新本線 ".into(), "本線".into())
            .unwrap();
        let mut expected = before;
        expected.routes[0].name = "新本線".into();
        assert_eq!(result, expected);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rename.json");
        e.save(&path).unwrap();
        assert_eq!(WorldEditor::new().load(&path).unwrap(), expected);
    }
    #[test]
    fn route_rename_conflict_invalid_name_and_wrong_owner_are_atomic() {
        let (mut e, a, s) = alignment_fixture();
        let id = e
            .save_planned_route(None, "本線".into(), s[0], vec![span(a, s[0], s[2])], vec![])
            .unwrap()
            .routes[0]
            .id;
        let before = e.snapshot();
        for (name, expected) in [
            ("".to_owned(), "本線"),
            ("x".repeat(81), "本線"),
            ("改\n名".to_owned(), "本線"),
            ("変更".to_owned(), "古い名前"),
        ] {
            assert!(e.rename_route(id, name, expected.into()).is_err());
            assert_eq!(e.snapshot(), before);
        }
        let owner = e.company;
        e.company = CompanyId(u64::MAX);
        assert!(e.rename_route(id, "変更".into(), "本線".into()).is_err());
        assert_eq!(e.snapshot(), before);
        e.company = owner;
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
    fn metadata_only_save_reuses_equal_view_and_preserves_supplied_plan_bits() {
        use rosenfu_core::world::{ConstructionJunctionSource, ViewState};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        let mut editor = WorldEditor::new();
        let point = |longitude| GeoPoint {
            latitude: 35.,
            longitude,
        };
        let draft = editor
            .build_alignment("line".into(), vec![point(139.), point(139.02)])
            .unwrap();
        let alignment = draft.alignments[0].clone();
        let mut plans = draft_plans();
        plans.draft_junction_source = Some(ConstructionJunctionSource {
            reverse: false,
            alignment_id: alignment.id,
            segment_index: 0,
            fraction: 0.,
            expected_alignment: alignment,
        });
        let view = ViewState {
            latitude: 35.,
            longitude: 139.,
            zoom: 14.,
        };
        editor
            .save_with_view(&path, Some(plans.clone()), Some(view), false)
            .unwrap();
        let shared = editor.world.shared_snapshot();
        let original = std::fs::read(&path).unwrap();
        editor
            .save_with_view(&path, None, Some(view), false)
            .unwrap();
        assert!(std::sync::Arc::ptr_eq(
            &shared,
            &editor.world.shared_snapshot()
        ));
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let backup = rosenfu_storage::backup_path(&path);
        assert_eq!(std::fs::read(&backup).unwrap(), original);
        let mut supplied = plans.clone();
        supplied.draft_junction_source.as_mut().unwrap().fraction = -0.;
        assert_eq!(plans, supplied); // PartialEq must not drop the supplied bits.
        editor
            .save_with_view(&path, Some(supplied), None, false)
            .unwrap();
        let written = std::fs::read(&path).unwrap();
        assert_ne!(written, original);
        assert_eq!(std::fs::read(&backup).unwrap(), original);
        let saved = rosenfu_storage::load_world(&path).unwrap();
        let fraction = saved
            .snapshot()
            .construction_plans
            .as_ref()
            .unwrap()
            .draft_junction_source
            .as_ref()
            .unwrap()
            .fraction;
        assert_eq!(fraction.to_bits(), (-0.0_f64).to_bits());
        assert_eq!(saved.snapshot().view_state, Some(view));
        assert_eq!(saved.snapshot(), &editor.snapshot());
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
                        latitude: 35.5,
                        longitude: 139.0,
                    },
                    GeoPoint {
                        latitude: 35.6,
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
                        latitude: 35.5,
                        longitude: 139.0,
                    },
                    GeoPoint {
                        latitude: 35.6,
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
        let (mut e, a, s) = alignment_fixture();
        let created = e
            .save_planned_route(None, "本線".into(), s[0], vec![span(a, s[0], s[2])], vec![])
            .unwrap();
        let id = created.routes[0].id;
        let edited = e
            .save_planned_route(
                Some(id),
                "新本線".into(),
                s[2],
                vec![span(a, s[2], s[0])],
                vec![],
            )
            .unwrap();
        assert_eq!(edited.routes[0].id, id);
        assert_eq!(edited.routes[0].start, s[2]);
        assert_eq!(edited.alignments, created.alignments);
        let renamed = e.rename_company("会社".into()).unwrap();
        assert_eq!(renamed.companies[0].id, created.companies[0].id);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        e.save(&path).unwrap();
        assert_eq!(WorldEditor::new().load(&path).unwrap(), renamed);
    }
    #[test]
    fn invalid_company_or_route_edit_does_not_change_world() {
        let (mut e, a, s) = alignment_fixture();
        let before = e.snapshot();
        assert!(e.rename_company(" ".into()).is_err());
        assert!(e
            .save_planned_route(None, "".into(), s[0], vec![span(a, s[0], s[2])], vec![])
            .is_err());
        assert!(e
            .save_planned_route(None, "R".into(), s[0], vec![], vec![])
            .is_err());
        assert!(e
            .save_planned_route(
                Some(RouteId(999)),
                "R".into(),
                s[0],
                vec![span(a, s[0], s[2])],
                vec![]
            )
            .is_err());
        assert!(e
            .save_planned_route(
                None,
                "R".into(),
                s[0],
                vec![span(a, s[0], StationId(999))],
                vec![]
            )
            .is_err());
        assert_eq!(e.snapshot(), before);
    }
    #[test]
    fn alignment_save_and_reload_preserve_geometry() {
        let (mut e, _, _) = alignment_fixture();
        let before = e.snapshot();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        e.save(&path).unwrap();
        assert_eq!(WorldEditor::new().load(&path).unwrap(), before);
    }
    #[test]
    fn self_loop_duplicate_and_missing_station_do_not_mutate_or_allocate() {
        let (mut e, a, s) = alignment_fixture();
        let before = e.snapshot();
        for segment in [span(a, s[0], s[0]), span(a, s[0], StationId(999))] {
            assert!(e
                .save_planned_route(None, "R".into(), s[0], vec![segment], vec![])
                .is_err());
        }
        assert!(e
            .world
            .place_station_on_alignment(e.company, "重複".into(), a, 0.1, 100.0, 10.0)
            .is_err());
        assert_eq!(e.snapshot(), before);
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
            // This is a persistence/extent fixture, including ocean boundary
            // coordinates, not permission to construct new ground stations there.
            editor
                .world
                .add_station(editor.company, name.into(), lat, lon)
                .unwrap();
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
        let (mut e, a, s) = alignment_fixture();
        let route = e
            .save_planned_route(None, "R".into(), s[0], vec![span(a, s[0], s[2])], vec![])
            .unwrap()
            .routes[0]
            .id;
        let services = create_test_service(&mut e, s[0], vec![span(a, s[0], s[2])]);
        e.rename_company("新会社".into()).unwrap();
        let before = e.snapshot();
        assert!(e
            .save_planned_route(
                Some(route),
                "逆".into(),
                s[2],
                vec![span(a, s[2], s[0])],
                vec![]
            )
            .is_err());
        assert_eq!(e.snapshot(), before);
        e.save_planned_route(
            Some(route),
            "新名".into(),
            s[0],
            vec![span(a, s[0], s[2])],
            vec![],
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("w.json");
        e.save(&path).unwrap();
        let loaded = WorldEditor::new().load(&path).unwrap();
        assert_eq!(loaded.services, services.services);
        assert_eq!(loaded.service_paths, services.service_paths);
    }
    #[test]
    fn service_editing_preserves_identity_and_repeated_stop_occurrences() {
        let (mut e, a, s) = alignment_fixture();
        let created = create_test_service(
            &mut e,
            s[0],
            vec![
                span(a, s[0], s[1]),
                span(a, s[1], s[0]),
                span(a, s[0], s[1]),
            ],
        );
        let id = created.services[0].id;
        let stops = created.service_paths[0].stops.clone();
        let edited = e
            .update_service_path(
                id,
                "改名".into(),
                vec![stops[0].clone(), stops[2].clone(), stops[3].clone()],
            )
            .unwrap();
        assert_eq!(edited.services[0].id, id);
        assert_eq!(edited.service_paths[0].legs, created.service_paths[0].legs);
        assert_eq!(edited.next_id, created.next_id);
        assert_eq!(edited.service_paths[0].stops.len(), 3);
    }
    #[test]
    fn invalid_service_edits_leave_world_and_allocator_unchanged() {
        let (mut e, a, s) = alignment_fixture();
        let created = create_test_service(&mut e, s[0], vec![span(a, s[0], s[2])]);
        let id = created.services[0].id;
        assert!(e
            .update_service_path(id, " ".into(), created.service_paths[0].stops.clone())
            .is_err());
        assert!(e.update_service_path(id, "S".into(), vec![]).is_err());
        assert!(e
            .update_service_path(
                ServiceId(999),
                "S".into(),
                created.service_paths[0].stops.clone()
            )
            .is_err());
        assert_eq!(e.snapshot(), created);
    }
    #[test]
    fn station_rename_and_unreferenced_deletion_preserve_ids_and_roundtrip() {
        let (mut e, _, s) = alignment_fixture();
        let original = e.snapshot();
        let renamed = e.rename_station(s[0], "改名".into()).unwrap();
        assert_eq!(renamed.stations[0].id, s[0]);
        assert_eq!(renamed.next_id, original.next_id);
        let deleted = e.delete_station(s[2]).unwrap();
        assert!(!deleted.stations.iter().any(|station| station.id == s[2]));
        assert_eq!(deleted.alignments, original.alignments);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.json");
        e.save(&path).unwrap();
        assert_eq!(WorldEditor::new().load(&path).unwrap(), deleted);
    }
    #[test]
    fn referenced_deletions_and_invalid_station_edits_are_atomic() {
        let (mut e, a, s) = alignment_fixture();
        let before = e.snapshot();
        assert!(e.rename_station(s[0], " ".into()).is_err());
        assert!(e.delete_station(StationId(999)).is_err());
        assert_eq!(e.snapshot(), before);
        e.save_planned_route(None, "R".into(), s[0], vec![span(a, s[0], s[2])], vec![])
            .unwrap();
        let route = e.snapshot();
        assert!(e.delete_alignment(a).is_err());
        assert!(e.delete_station(s[1]).is_err());
        assert_eq!(e.snapshot(), route);
        let mut no_route = e.snapshot();
        no_route.routes.clear();
        no_route.route_paths.clear();
        e.world.replace(no_route).unwrap();
        let service = create_test_service(&mut e, s[0], vec![span(a, s[0], s[2])]);
        assert!(e.delete_alignment(a).is_err());
        assert!(e.delete_station(s[1]).is_err());
        assert_eq!(e.snapshot(), service);
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
        let (mut e, a, s) = alignment_fixture();
        let r = e
            .save_planned_route(None, "R".into(), s[0], vec![span(a, s[0], s[1])], vec![])
            .unwrap()
            .routes[0]
            .id;
        let other = e
            .save_planned_route(
                None,
                "Other".into(),
                s[0],
                vec![span(a, s[0], s[1])],
                vec![],
            )
            .unwrap()
            .routes[1]
            .id;
        let segments = vec![span(a, s[0], s[1])];
        let stops = e
            .preview_service_path(s[0], segments.clone())
            .unwrap()
            .occurrences
            .into_iter()
            .map(|o| o.reference)
            .collect();
        let saved = e
            .create_service_path_with_source_route("S".into(), s[0], segments, stops, Some(r))
            .unwrap();
        let id = saved.services[0].id;
        e.save_planned_route(
            Some(other),
            "Other".into(),
            s[1],
            vec![span(a, s[1], s[2])],
            vec![],
        )
        .unwrap();
        let before = e.snapshot();
        assert!(e
            .save_planned_route(
                Some(r),
                "Changed".into(),
                s[1],
                vec![span(a, s[1], s[2])],
                vec![]
            )
            .is_err());
        assert_eq!(e.snapshot(), before);
        e.set_service_route_binding(id, ServiceRouteBindingKind::Independent)
            .unwrap();
        e.save_planned_route(
            Some(r),
            "Changed".into(),
            s[1],
            vec![span(a, s[1], s[2])],
            vec![],
        )
        .unwrap();
        assert_eq!(e.snapshot().services, before.services);
        assert_eq!(e.snapshot().service_paths, before.service_paths);
    }
    #[test]
    fn route_path_edits_conservatively_protect_existing_services_atomically() {
        let (mut e, a, s) = alignment_fixture();
        let r = e
            .save_planned_route(None, "R".into(), s[0], vec![span(a, s[0], s[1])], vec![])
            .unwrap()
            .routes[0]
            .id;
        e.save_planned_route(Some(r), "R".into(), s[0], vec![span(a, s[0], s[2])], vec![])
            .unwrap();
        let before = create_test_service(&mut e, s[0], vec![span(a, s[0], s[1])]);
        assert!(e
            .save_planned_route(
                Some(r),
                "Changed".into(),
                s[0],
                vec![span(a, s[0], s[1])],
                vec![]
            )
            .unwrap_err()
            .contains("未確認"));
        assert_eq!(e.snapshot(), before);
        let renamed = e
            .save_planned_route(
                Some(r),
                "Renamed".into(),
                s[0],
                vec![span(a, s[0], s[2])],
                vec![],
            )
            .unwrap();
        assert_eq!(renamed.services, before.services);
        assert_eq!(renamed.service_paths, before.service_paths);
    }
    #[test]
    fn alignment_routes_guard_references_without_changing_services() {
        let (mut e, a, s) = alignment_fixture();
        let services = create_test_service(&mut e, s[0], vec![span(a, s[0], s[2])]);
        let saved = e
            .save_planned_route(None, "R".into(), s[0], vec![span(a, s[0], s[2])], vec![])
            .unwrap();
        assert_eq!(saved.services, services.services);
        assert_eq!(saved.service_paths, services.service_paths);
        let mut only_route = saved;
        only_route.services.clear();
        only_route.service_paths.clear();
        e.world.replace(only_route).unwrap();
        let before = e.snapshot();
        assert!(e.delete_alignment(a).is_err());
        assert_eq!(e.snapshot(), before);
    }
    #[test]
    fn route_to_service_preview_does_not_save_or_choose_stops() {
        let (mut e, a, s) = alignment_fixture();
        e.save_planned_route(
            None,
            "R".into(),
            s[0],
            vec![span(a, s[0], s[2])],
            vec![s[1]],
        )
        .unwrap();
        let before = e.snapshot();
        let preview = e
            .preview_service_path(s[2], vec![span(a, s[2], s[1])])
            .unwrap();
        assert_eq!(
            preview
                .occurrences
                .iter()
                .map(|o| o.station_id)
                .collect::<Vec<_>>(),
            vec![s[2], s[1]]
        );
        assert!(preview.requires_stop_selection);
        assert_eq!(e.snapshot(), before);
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
        let before = editor.snapshot();
        assert!(editor.delete_alignment(a).is_err());
        assert_eq!(editor.snapshot(), before);
    }

    #[test]
    fn saved_path_editor_preserves_ids_and_roundtrips_stop_changes() {
        let (mut editor, alignment, [a, b, c]) = alignment_fixture();
        let segments = vec![span(alignment, a, b), span(alignment, b, c)];
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
        let reverse = vec![span(alignment, c, b), span(alignment, b, a)];
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
        reference_roundtrip(false);
    }
    #[test]
    fn real_rail_network_geometry_and_provenance_are_atomic_and_roundtrip() {
        reference_roundtrip(true);
    }
    fn reference_roundtrip(network: bool) {
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
            conversion_format_version: if network { 2 } else { 1 },
        };
        let provenance = AlignmentProvenanceInput {
            tile_fragment_id: None,
            network_path: if network {
                vec![rosenfu_core::world::RailGraphSpan {
                    region: "jp".into(),
                    chain_id: 7,
                    from_fraction: 0.0,
                    to_fraction: 1.0,
                }]
            } else {
                vec![]
            },
            source_key: key,
            method: if network {
                ProvenanceMethod::RailGraphRouteV1
            } else {
                ProvenanceMethod::OsmSegmentCopyV1
            },
            osm_way_id: if network { String::new() } else { "1".into() },
            mode: if network {
                ReferenceMode::Rail
            } else {
                ReferenceMode::Road
            },
            start_segment_index: 0,
            start_fraction: 0.0,
            end_segment_index: 0,
            end_fraction: if network { 0.0 } else { 1.0 },
        };
        let points = vec![
            GeoPoint {
                latitude: 35.5,
                longitude: 139.0,
            },
            GeoPoint {
                latitude: 35.5,
                longitude: 139.1,
            },
        ];
        // Attribution on an earlier path must roll back if any later path fails.
        let attributed_path = || PlannedAlignment {
            ofm_path: None,
            anchor_levels: None,
            junction_source: None,
            start_level: None,
            infrastructure: Default::default(),
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
            ofm_path: None,
            anchor_levels: None,
            junction_source: None,
            start_level: None,
            infrastructure: Default::default(),
            anchor_indices: None,
            name: "Reference extension".into(),
            points: vec![
                points[1],
                GeoPoint {
                    latitude: 35.5,
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

        let plan = serde_json::from_value(serde_json::json!({
            "name": "着工上限確認",
            "points": planned_alignment_points(),
            "trackType": "normal",
            "stations": [],
        }))
        .unwrap();
        let revision = editor.world_edit_history().revision;
        assert_eq!(
            editor
                .build_construction_plan_with_roads(vec![plan], vec![], vec![])
                .unwrap_err(),
            rosenfu_storage::StorageError::TooLarge.to_string()
        );
        assert_eq!(editor.snapshot(), before);
        assert_eq!(editor.world_edit_history().revision, revision);
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
                    ofm_path: None,
                    anchor_levels: None,
                    junction_source: None,
                    start_level: None,
                    infrastructure: Default::default(),
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
            ofm_path: None,
            anchor_levels: None,
            junction_source: None,
            start_level: None,
            infrastructure: Default::default(),
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

    #[test]
    fn construction_infrastructure_is_physical_and_invalid_batch_is_atomic() {
        use rosenfu_core::world::{InfrastructureLevel, SectionInfrastructure};
        let mut editor = WorldEditor::new();
        let mut path = planned_path(0.);
        path.infrastructure = SectionInfrastructure {
            track_count: 4,
            level: InfrastructureLevel::Elevated3,
        };
        let result = editor.build_plan(vec![path]).unwrap();
        assert_eq!(
            result.alignments[0]
                .section_infrastructure(0)
                .unwrap()
                .track_count,
            4
        );
        assert_eq!(
            result.alignments[0]
                .section_infrastructure(0)
                .unwrap()
                .level,
            InfrastructureLevel::Elevated3
        );
        let before = editor.snapshot();
        let valid = planned_path(0.1);
        let mut invalid = planned_path(0.2);
        invalid.infrastructure.track_count = 3;
        assert!(editor.build_plan(vec![valid, invalid]).is_err());
        assert_eq!(editor.snapshot(), before);
    }

    #[test]
    fn sloped_plan_and_rising_extension_are_atomic_and_preserve_old_track() {
        use rosenfu_core::world::{AlignmentEndpoint, InfrastructureLevel};
        let mut editor = WorldEditor::new();
        let mut flat = planned_path(0.);
        flat.stations.clear();
        let built = editor.build_plan(vec![flat]).unwrap();
        let original = built.alignments[0].clone();
        let mut ramp = planned_path(0.);
        ramp.stations.clear();
        ramp.points = vec![
            GeoPoint {
                latitude: 35.,
                longitude: 139.01,
            },
            GeoPoint {
                latitude: 35.,
                longitude: 139.02,
            },
        ];
        ramp.extension = Some(PlannedAlignmentExtension {
            alignment_id: original.id,
            endpoint: AlignmentEndpoint::End,
        });
        ramp.infrastructure.level = InfrastructureLevel::Elevated;
        let after = editor.build_plan(vec![ramp]).unwrap();
        assert_eq!(after.alignments[0], original);
        assert_eq!(after.alignments.len(), 2);
        assert_eq!(after.alignment_connections.len(), 1);
        assert_eq!(
            after.alignments[1].endpoint_height_m(AlignmentEndpoint::End),
            6.
        );
        assert!(after.alignments[1].vertical_profile.is_some());
        let mut short = planned_path(1.);
        short.stations.clear();
        short.start_level = Some(InfrastructureLevel::Ground);
        short.infrastructure.level = InfrastructureLevel::Elevated3;
        short.points = vec![
            GeoPoint {
                latitude: 36.,
                longitude: 139.,
            },
            GeoPoint {
                latitude: 36.,
                longitude: 139.0001,
            },
        ];
        let before = editor.snapshot();
        assert!(editor.build_plan(vec![short]).is_err());
        assert_eq!(editor.snapshot(), before);
    }

    #[test]
    fn per_anchor_heights_preview_build_flat_platform_and_repeated_split() {
        use rosenfu_core::world::InfrastructureLevel;
        let mut editor = WorldEditor::new();
        let request = || {
            let mut path = planned_path(0.);
            path.stations.clear();
            path.points = (0..4)
                .map(|i| GeoPoint {
                    latitude: 35.,
                    longitude: 139. + i as f64 * 0.01,
                })
                .collect();
            path.anchor_indices = Some(vec![0, 1, 2, 3]);
            path.anchor_levels = Some(vec![
                InfrastructureLevel::Ground,
                InfrastructureLevel::Elevated,
                InfrastructureLevel::Elevated,
                InfrastructureLevel::Ground,
            ]);
            path
        };
        let before = editor.snapshot();
        let preview = editor
            .preview_construction_plan(vec![request()], vec![])
            .unwrap();
        assert_eq!(editor.snapshot(), before);
        let after = editor.build_plan(vec![request()]).unwrap();
        assert_eq!(after, preview);
        for kind in 0..4 {
            let mut invalid = after.clone();
            let profiles = &mut invalid.alignments[0].section_profiles;
            match kind {
                0 => {
                    profiles.pop();
                }
                1 => profiles[1].start_height_m += 1.,
                2 => profiles[1].start_tangent_m = 1.,
                _ => profiles[0].start_tangent_m = 1000.,
            }
            assert!(World::new(invalid).is_err(), "malformed profile {kind}");
        }
        let mut too_short = request();
        for (i, point) in too_short.points.iter_mut().enumerate() {
            point.longitude = 139. + i as f64 * 0.0001;
        }
        assert!(editor
            .preview_construction_plan(vec![too_short], vec![])
            .is_err());
        assert_eq!(editor.snapshot(), after);
        let line = &after.alignments[0];
        let id = line.id;
        assert_eq!(line.section_profiles.len(), 3);
        assert!(line.vertical_profile.is_none());
        assert!((line.height_at_position_m(0.5).unwrap() - 6.).abs() < 1e-9);
        assert!(editor
            .place_station_on_alignment("flat platform".into(), id, 0.5, 100., 10.)
            .is_ok());
        let before = editor.snapshot();
        assert!(editor
            .place_station_on_alignment("slope".into(), id, 0.2, 100., 10.)
            .is_err());
        assert_eq!(editor.snapshot(), before);
        assert!(editor
            .place_station_on_alignment("straddling".into(), id, 1. / 3., 100., 10.)
            .is_err());
        assert_eq!(editor.snapshot(), before);
        let right = editor
            .world
            .split_alignment_at_position(editor.company, id, 0, 0.5)
            .unwrap();
        let left = editor
            .snapshot()
            .alignments
            .iter()
            .find(|a| a.id == id)
            .unwrap()
            .clone();
        assert!((left.height_at_position_m(1.).unwrap() - 3.).abs() < 1e-6);
        let right_line = editor
            .snapshot()
            .alignments
            .iter()
            .find(|a| a.id == right)
            .unwrap()
            .clone();
        assert!(
            (left.endpoint_inward_grade(rosenfu_core::world::AlignmentEndpoint::End)
                + right_line.endpoint_inward_grade(rosenfu_core::world::AlignmentEndpoint::Start))
            .abs()
                < 1e-8
        );
        let saved = serde_json::to_string(&editor.snapshot()).unwrap();
        assert!(World::new(serde_json::from_str(&saved).unwrap()).is_ok());
    }

    #[test]
    fn elevated_flat_junction_keeps_real_height_despite_stale_layer_label() {
        use rosenfu_core::world::{
            AlignmentEndpoint, ConstructionJunctionSource, InfrastructureLevel, TrackType,
        };
        let mut editor = WorldEditor::new();
        let make = |a, b| {
            vec![
                GeoPoint {
                    latitude: 35.,
                    longitude: a,
                },
                GeoPoint {
                    latitude: 35.,
                    longitude: b,
                },
            ]
        };
        let source = editor
            .world
            .build_alignment_ramp(
                editor.company,
                "source".into(),
                make(139., 139.01),
                TrackType::Normal,
                InfrastructureLevel::Elevated,
                InfrastructureLevel::Elevated,
            )
            .unwrap();
        let target = editor
            .world
            .build_alignment_ramp(
                editor.company,
                "target".into(),
                make(139.02, 139.03),
                TrackType::Normal,
                InfrastructureLevel::Elevated,
                InfrastructureLevel::Elevated,
            )
            .unwrap();
        let original = editor
            .snapshot()
            .alignments
            .iter()
            .find(|a| a.id == source)
            .unwrap()
            .clone();
        let mut path = planned_path(0.);
        path.stations.clear();
        path.points = make(139.005, 139.02);
        path.design_speed_kmh = None;
        path.junction_source = Some(ConstructionJunctionSource {
            alignment_id: source,
            expected_alignment: original,
            segment_index: 0,
            fraction: 0.5,
            reverse: false,
        });
        path.destination = Some(PlannedAlignmentExtension {
            alignment_id: target,
            endpoint: AlignmentEndpoint::Start,
        });
        path.infrastructure.level = InfrastructureLevel::Ground;
        let after = editor.build_plan(vec![path]).unwrap();
        assert_eq!(
            after.alignments.last().unwrap().height_at_position_m(0.5),
            Some(6.)
        );
        assert!(after
            .alignments
            .last()
            .unwrap()
            .vertical_profile
            .unwrap()
            .is_flat());
    }

    #[test]
    fn preview_and_build_share_destination_height_grade_and_tangent_validation() {
        use rosenfu_core::world::{AlignmentEndpoint, InfrastructureLevel};
        let mut editor = WorldEditor::new();
        let mut source = planned_path(0.);
        source.stations.clear();
        source.design_speed_kmh = Some(160.);
        let built = editor.build_plan(vec![source]).unwrap();
        let source_id = built.alignments[0].id;
        let target = editor
            .world
            .build_alignment_ramp(
                editor.company,
                "target".into(),
                vec![
                    GeoPoint {
                        latitude: 35.,
                        longitude: 139.03,
                    },
                    GeoPoint {
                        latitude: 35.,
                        longitude: 139.04,
                    },
                ],
                rosenfu_core::world::TrackType::Normal,
                InfrastructureLevel::Elevated,
                InfrastructureLevel::Elevated,
            )
            .unwrap();
        let before = editor.snapshot();
        let request = || {
            let mut path = planned_path(0.);
            path.stations.clear();
            path.design_speed_kmh = None;
            path.points = vec![
                GeoPoint {
                    latitude: 35.,
                    longitude: 139.01,
                },
                GeoPoint {
                    latitude: 35.,
                    longitude: 139.03,
                },
            ];
            path.extension = Some(PlannedAlignmentExtension {
                alignment_id: source_id,
                endpoint: AlignmentEndpoint::End,
            });
            path.destination = Some(PlannedAlignmentExtension {
                alignment_id: target,
                endpoint: AlignmentEndpoint::Start,
            });
            path.infrastructure.level = InfrastructureLevel::Underground3; // Stale UI label must not override target height.
            path
        };
        let preview = editor
            .preview_construction_plan(vec![request()], vec![])
            .unwrap();
        assert_eq!(editor.snapshot(), before);
        let after = editor.build_plan(vec![request()]).unwrap();
        assert_eq!(after, preview);
        let span = after.alignments.last().unwrap();
        assert_eq!(span.endpoint_height_m(AlignmentEndpoint::End), 6.);
        assert_eq!(span.effective_design_speed_kmh(), 160.);
        let mut second = WorldEditor::new();
        second.world = World::new(before.clone()).unwrap();
        second.company = editor.company;
        let invalid = || {
            let mut path = request();
            path.points.insert(
                1,
                GeoPoint {
                    latitude: 35.001,
                    longitude: 139.01,
                },
            );
            path
        };
        let error = second
            .preview_construction_plan(vec![invalid()], vec![])
            .unwrap_err();
        assert_eq!(second.build_plan(vec![invalid()]).unwrap_err(), error);
        assert_eq!(second.snapshot(), before);
    }

    #[test]
    fn imported_junction_transaction_preserves_provenance_and_ends_undo_history() {
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
            tile_fragment_id: None,
            network_path: vec![],
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
                latitude: 35.5,
                longitude: 139.0,
            },
            GeoPoint {
                latitude: 35.5,
                longitude: 139.1,
            },
        ];
        let before = editor
            .build_alignment_from_reference(
                "source".into(),
                points,
                source.clone(),
                provenance.clone(),
            )
            .unwrap();
        let original = before.alignments[0].clone();
        let mut branch = planned_path(0.);
        branch.stations.clear();
        branch.points = vec![
            GeoPoint {
                latitude: 35.5,
                longitude: 139.05,
            },
            GeoPoint {
                latitude: 35.5,
                longitude: 139.051,
            },
            GeoPoint {
                latitude: 35.51,
                longitude: 139.06,
            },
        ];
        branch.junction_source = Some(rosenfu_core::world::ConstructionJunctionSource {
            alignment_id: original.id,
            expected_alignment: original,
            segment_index: 0,
            fraction: 0.5,
            reverse: false,
        });
        let after = editor.build_plan(vec![branch]).unwrap();
        assert_eq!(after.source_records, vec![source]);
        assert_eq!(after.alignment_provenance.len(), 2);
        assert!(after
            .alignment_provenance
            .iter()
            .all(|p| p.source == provenance && p.derived_slice.is_some()));
        let restored =
            World::new(serde_json::from_str(&serde_json::to_string(&after).unwrap()).unwrap())
                .unwrap();
        assert_eq!(restored.snapshot(), &after);
        assert!(editor.undo_alignment_deletion().is_err());
        assert_eq!(editor.snapshot(), after);
    }

    #[test]
    fn ambiguous_repeated_geometry_split_batch_is_atomic() {
        use rosenfu_core::world::ConstructionJunctionSource;
        let mut editor = WorldEditor::new();
        let mut flat = planned_path(0.);
        flat.stations.clear();
        flat.points = [139.0, 139.01, 139.02, 139.01, 139.03]
            .into_iter()
            .map(|longitude| GeoPoint {
                latitude: 35.,
                longitude,
            })
            .collect();
        let built = editor.build_plan(vec![flat]).unwrap();
        let original = built.alignments[0].clone();
        let paths = [(0, 139.005), (1, 139.015)]
            .into_iter()
            .map(|(segment_index, x)| {
                let mut branch = planned_path(0.);
                branch.stations.clear();
                branch.points = vec![
                    GeoPoint {
                        latitude: 35.,
                        longitude: x,
                    },
                    GeoPoint {
                        latitude: 35.,
                        longitude: x + 0.0005,
                    },
                    GeoPoint {
                        latitude: 35.01,
                        longitude: x + 0.001,
                    },
                ];
                branch.junction_source = Some(ConstructionJunctionSource {
                    alignment_id: original.id,
                    expected_alignment: original.clone(),
                    segment_index,
                    fraction: 0.5,
                    reverse: false,
                });
                branch
            })
            .collect();
        assert!(editor
            .build_plan(paths)
            .unwrap_err()
            .contains("重複する線形"));
        assert_eq!(editor.snapshot(), built);
    }

    #[test]
    fn multiple_junctions_from_one_observed_source_commit_and_undo_together() {
        use rosenfu_core::world::ConstructionJunctionSource;
        for fractions in [[0.25, 0.75], [0.75, 0.25]] {
            let mut editor = WorldEditor::new();
            let mut flat = planned_path(0.);
            flat.stations.clear();
            let built = editor.build_plan(vec![flat]).unwrap();
            let original = built.alignments[0].clone();
            let paths = fractions
                .into_iter()
                .map(|fraction| {
                    let mut branch = planned_path(0.);
                    branch.stations.clear();
                    let x = 139.0 + 0.01 * fraction;
                    branch.points = vec![
                        GeoPoint {
                            latitude: 35.,
                            longitude: x,
                        },
                        GeoPoint {
                            latitude: 35.,
                            longitude: x + 0.0005,
                        },
                        GeoPoint {
                            latitude: 35.01,
                            longitude: x + 0.001,
                        },
                    ];
                    branch.junction_source = Some(ConstructionJunctionSource {
                        alignment_id: original.id,
                        expected_alignment: original.clone(),
                        segment_index: 0,
                        fraction,
                        reverse: false,
                    });
                    branch
                })
                .collect();
            let after = editor.build_plan(paths).unwrap();
            assert_eq!(after.alignments.len(), 5);
            assert_eq!(after.alignment_connections.len(), 4);
            let saved = serde_json::to_string(&after).unwrap();
            assert!(World::new(serde_json::from_str(&saved).unwrap()).is_ok());
            assert!(editor.undo_alignment_deletion().is_err());
            assert_eq!(editor.snapshot(), after);
        }
    }

    #[test]
    fn arbitrary_position_junction_commit_is_final_and_stale_source_are_atomic() {
        use rosenfu_core::world::ConstructionJunctionSource;
        let mut editor = WorldEditor::new();
        let mut flat = planned_path(0.);
        flat.stations.clear();
        let built = editor.build_plan(vec![flat]).unwrap();
        let original = built.alignments[0].clone();
        let source = ConstructionJunctionSource {
            reverse: false,
            alignment_id: original.id,
            segment_index: 0,
            fraction: 0.5,
            expected_alignment: original.clone(),
        };
        let mut branch = planned_path(0.);
        branch.stations.clear();
        branch.junction_source = Some(source.clone());
        branch.points = vec![
            GeoPoint {
                latitude: 35.,
                longitude: 139.005,
            },
            GeoPoint {
                latitude: 35.,
                longitude: 139.006,
            },
            GeoPoint {
                latitude: 35.01,
                longitude: 139.015,
            },
        ];
        let after = editor.build_plan(vec![branch]).unwrap();
        assert_eq!(after.alignments.len(), 3);
        assert_eq!(after.alignment_connections.len(), 2);
        assert_eq!(
            after.alignments[0].points.last().unwrap().longitude,
            139.005
        );
        assert!(editor.undo_alignment_deletion().is_err());
        assert_eq!(editor.snapshot(), after);
        let mut stale = planned_path(0.);
        stale.stations.clear();
        stale.junction_source = Some(source);
        stale
            .junction_source
            .as_mut()
            .unwrap()
            .expected_alignment
            .name = "stale".into();
        let before = editor.snapshot();
        assert!(editor.build_plan(vec![stale]).is_err());
        assert_eq!(editor.snapshot(), before);
    }

    #[test]
    fn reverse_interior_turnout_uses_suffix_as_stem() {
        use rosenfu_core::world::{AlignmentEndpoint, ConstructionJunctionSource};
        let mut editor = WorldEditor::new();
        let mut flat = planned_path(0.);
        flat.stations.clear();
        let built = editor.build_plan(vec![flat]).unwrap();
        let original = built.alignments[0].clone();
        let mut branch = planned_path(0.);
        branch.stations.clear();
        branch.junction_source = Some(ConstructionJunctionSource {
            reverse: true,
            alignment_id: original.id,
            segment_index: 0,
            fraction: 0.5,
            expected_alignment: original,
        });
        branch.points = vec![
            GeoPoint {
                latitude: 35.,
                longitude: 139.005,
            },
            GeoPoint {
                latitude: 35.,
                longitude: 139.004,
            },
            GeoPoint {
                latitude: 35.01,
                longitude: 139.,
            },
        ];
        let after = editor.build_plan(vec![branch]).unwrap();
        let suffix = after.alignments[1].id;
        assert_eq!(after.alignment_connections[1].a.alignment_id, suffix);
        assert_eq!(
            after.alignment_connections[1].a.endpoint,
            AlignmentEndpoint::Start
        );
        assert!(editor.undo_alignment_deletion().is_err());
        assert_eq!(editor.snapshot(), after);
    }

    #[test]
    fn ramp_branch_inherits_exact_height_and_grade() {
        use rosenfu_core::world::{
            AlignmentEndpoint, ConstructionJunctionSource, InfrastructureLevel,
        };
        let mut editor = WorldEditor::new();
        let mut ramp = planned_path(0.);
        ramp.stations.clear();
        ramp.start_level = Some(InfrastructureLevel::Ground);
        ramp.infrastructure.level = InfrastructureLevel::Elevated;
        let built = editor.build_plan(vec![ramp]).unwrap();
        let original = built.alignments[0].clone();
        let mut branch = planned_path(0.);
        branch.stations.clear();
        branch.junction_source = Some(ConstructionJunctionSource {
            reverse: false,
            alignment_id: original.id,
            segment_index: 0,
            fraction: 0.5,
            expected_alignment: original.clone(),
        });
        branch.points = vec![
            GeoPoint {
                latitude: 35.,
                longitude: 139.005,
            },
            GeoPoint {
                latitude: 35.,
                longitude: 139.006,
            },
            GeoPoint {
                latitude: 35.01,
                longitude: 139.015,
            },
        ];
        let after = editor.build_plan(vec![branch]).unwrap();
        assert_eq!(after.alignments.len(), 3);
        let stem = &after.alignments[0];
        let branch = &after.alignments[2];
        assert!((branch.endpoint_height_m(AlignmentEndpoint::Start) - 3.).abs() < 1e-8);
        assert!(
            (branch.endpoint_inward_grade(AlignmentEndpoint::Start)
                + stem.endpoint_inward_grade(AlignmentEndpoint::End))
            .abs()
                < 1e-9
        );
        assert!(editor.undo_alignment_deletion().is_err());
        assert_eq!(editor.snapshot(), after);
    }

    fn planned_path(offset: f64) -> PlannedAlignment {
        PlannedAlignment {
            ofm_path: None,
            anchor_levels: None,
            junction_source: None,
            start_level: None,
            infrastructure: Default::default(),
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
                ofm_path: None,
                anchor_levels: None,
                junction_source: None,
                start_level: None,
                infrastructure: Default::default(),
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
                ofm_path: None,
                anchor_levels: None,
                junction_source: None,
                start_level: None,
                infrastructure: Default::default(),
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
            ofm_path: None,
            anchor_levels: None,
            junction_source: None,
            start_level: None,
            infrastructure: Default::default(),
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
                    ofm_path: None,
                    anchor_levels: None,
                    junction_source: None,
                    start_level: None,
                    infrastructure: Default::default(),
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
    fn reopened_mixed_tram_plan_creates_distinct_span_and_preserves_source() {
        use rosenfu_core::world::{ConstructionPlans, SectionInfrastructure};
        for (source_type, built_type) in [
            (TrackType::Normal, TrackType::Tram),
            (TrackType::Tram, TrackType::HighSpeed),
        ] {
            let mut editor = WorldEditor::new();
            editor.company_setup_required = false;
            let source = editor
                .world
                .build_alignment_typed(
                    editor.company,
                    "Existing".into(),
                    vec![p(139.), p(139.01)],
                    source_type,
                )
                .unwrap();
            let desired_speed = built_type.default_design_speed_kmh();
            let points = vec![p(139.01), p(139.02)];
            let plans: ConstructionPlans = serde_json::from_value(serde_json::json!({
                "version":1,"plans":[{"id":1,"name":"Mixed plan"}],"parkedPaths":[],
                "activePathId":1,"activePlanId":1,"draftPoints":points,"redoPoints":[],
                "draftExtension":{"alignmentId":source,"endpoint":"end"},
                "trackType":built_type,"designSpeedKmh":desired_speed,"roadFollowing":false,
                "plannedStations":[],"draftFinished":true
            }))
            .unwrap();
            let d = tempfile::tempdir().unwrap();
            let file = d.path().join("mixed-plan.rosenfu");
            editor
                .save_with_plans(&file, Some(plans.clone()), false)
                .unwrap();
            let reopened = editor.load(&file).unwrap();
            assert_eq!(reopened.construction_plans, Some(plans));
            let expected = reopened.alignments[0].clone();
            let result = editor
                .build_plan(vec![PlannedAlignment {
                    ofm_path: None,
                    anchor_levels: None,
                    junction_source: None,
                    start_level: None,
                    infrastructure: SectionInfrastructure::default(),
                    anchor_indices: None,
                    name: "New type".into(),
                    points,
                    track_type: built_type,
                    design_speed_kmh: Some(desired_speed),
                    stations: vec![],
                    reference: None,
                    extension: Some(PlannedAlignmentExtension {
                        alignment_id: source,
                        endpoint: AlignmentEndpoint::End,
                    }),
                    destination: None,
                }])
                .unwrap();
            assert_eq!(result.alignments.len(), 2);
            assert_eq!(result.alignments[0], expected);
            assert_eq!(result.alignments[1].track_type, built_type);
            assert_eq!(
                result.alignments[1].effective_design_speed_kmh(),
                desired_speed
            );
            assert_eq!(result.alignment_connections.len(), 1);
            assert_eq!(result.alignment_connections[0].a.alignment_id, source);
            assert_eq!(
                result.alignment_connections[0].b.alignment_id,
                result.alignments[1].id
            );
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
                ofm_path: None,
                anchor_levels: None,
                junction_source: None,
                start_level: None,
                infrastructure: Default::default(),
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
            ofm_path: None,
            anchor_levels: None,
            junction_source: None,
            start_level: None,
            infrastructure: Default::default(),
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

#[cfg(test)]
#[path = "session_binding_review_tests.rs"]
mod binding_review_tests;

fn source_route_binding(
    route_id: Option<RouteId>,
    route_ids: Option<Vec<RouteId>>,
) -> Result<Option<rosenfu_core::world::ServiceRouteBindingKind>, String> {
    use rosenfu_core::world::ServiceRouteBindingKind;
    match (route_id, route_ids) {
        (Some(_), Some(_)) => Err("単一路線と直通路線を同時には指定できません。".into()),
        (Some(route_id), None) => Ok(Some(ServiceRouteBindingKind::Route { route_id })),
        (None, Some(route_ids)) => {
            if !(2..=64).contains(&route_ids.len())
                || route_ids
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != route_ids.len()
            {
                return Err("直通系統には重複しない2〜64路線を指定してください。".into());
            }
            Ok(Some(ServiceRouteBindingKind::Routes { route_ids }))
        }
        (None, None) => Ok(None),
    }
}

#[cfg(test)]
mod road_snapshot_transaction_tests {
    use super::*;
    use rosenfu_core::world::{CoverageBounds, RoadSnapshot, RoadWay};
    fn point(x: f64, y: f64) -> GeoPoint {
        GeoPoint {
            longitude: 139.0 + x,
            latitude: 35.0 + y,
        }
    }
    fn roads() -> RoadSnapshot {
        RoadSnapshot {
            coverage_verified: true,
            source: "test-osm".into(),
            source_url: "https://example.org/source.osm.pbf".into(),
            source_sha256: "a".repeat(64),
            attribution: "© OpenStreetMap contributors".into(),
            license: "ODbL-1.0".into(),
            revision: "2026-11-14T00:00:00Z".into(),
            road_coverage: vec![CoverageBounds {
                west: 138.9,
                east: 139.1,
                south: 34.9,
                north: 35.1,
            }],
            roads: vec![RoadWay {
                classification: None,
                id: "way/1".into(),
                coverage: None,
                points: vec![point(0.0, -0.01), point(0.0, 0.01)],
                layer: None,
                level: None,
                bridge: None,
                tunnel: None,
            }],
        }
    }
    fn plan() -> PlannedAlignment {
        PlannedAlignment {
            ofm_path: None,
            anchor_levels: None,
            junction_source: None,
            start_level: None,
            infrastructure: Default::default(),
            anchor_indices: None,
            name: "Crossing".into(),
            points: vec![point(-0.01, 0.0), point(0.01, 0.0)],
            track_type: rosenfu_core::world::TrackType::Normal,
            design_speed_kmh: None,
            stations: vec![],
            reference: None,
            extension: None,
            destination: None,
        }
    }
    // The pre-#198 transaction is a test oracle, not a production fallback.
    fn nested_construction_baseline(
        editor: &mut WorldEditor,
        alignments: Vec<PlannedAlignment>,
        roads: Vec<RoadSnapshot>,
    ) -> Result<WorldDraft, String> {
        let mut outer = editor.clone();
        for snapshot in roads {
            outer
                .world
                .add_road_snapshot(snapshot)
                .map_err(|e| e.to_string())?;
        }
        outer.build_construction_plan(alignments, vec![])?;
        rosenfu_storage::ensure_world_saveable(&outer.world).map_err(|e| e.to_string())?;
        outer.deleted_alignment = None;
        editor.commit_edit(outer, true)?;
        Ok(editor.snapshot())
    }

    #[test]
    fn one_construction_candidate_matches_nested_live_operation_and_history() {
        use rosenfu_core::operation::{Timetable, Vehicle};
        use rosenfu_core::world::{FixedOdScenario, OperationServiceConfig, RouteSegment};

        let mut initial = WorldEditor::new();
        let mut existing = plan();
        existing.points = vec![point(-0.01, 0.0), point(0.01, 0.0)];
        let id = initial
            .build_construction_plan(vec![existing], vec![])
            .unwrap()
            .alignments[0]
            .id;
        let a = initial
            .world
            .place_station_on_alignment(initial.company, "A".into(), id, 0.1, 60., 10.)
            .unwrap();
        let b = initial
            .world
            .place_station_on_alignment(initial.company, "B".into(), id, 0.9, 60., 10.)
            .unwrap();
        let segments = vec![RouteSegment::Alignment {
            alignment_id: id,
            from: a,
            to: b,
        }];
        let occurrences = initial
            .world
            .preview_service_path(a, segments.clone())
            .unwrap()
            .occurrences;
        let service_id = initial
            .world
            .add_service_path(
                initial.company,
                "Shuttle".into(),
                a,
                segments,
                occurrences.into_iter().map(|o| o.reference).collect(),
            )
            .unwrap();
        initial
            .world
            .configure_operation(vec![OperationServiceConfig {
                service_id,
                timetable: Timetable {
                    first_departure_ms: 0,
                    last_departure_ms: 0,
                    headway_ms: 60_000,
                },
                vehicle: Vehicle {
                    max_speed_kmh: 72.,
                    acceleration_mps2: 1.,
                    braking_mps2: 1.,
                    length_m: 40.,
                },
                dwell_ms: 30_000,
            }])
            .unwrap();
        initial
            .world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(1_000, 100)
            .unwrap();
        initial
            .world
            .restore_fixed_od_scenario(Some(FixedOdScenario {
                origin_station_id: a.0.to_string(),
                destination_station_id: b.0.to_string(),
                trips: Some(31.),
                alternative_seconds: None,
            }))
            .unwrap();
        assert_eq!(initial.world.operation_save().unwrap().runtime.format, 4);
        initial.rename_company("Changed".into()).unwrap();

        for (count, valid) in [(1, true), (2, true), (2, false)] {
            let make_plans = || {
                let mut plans: Vec<_> = (1..=count)
                    .map(|i| {
                        let mut p = plan();
                        let y = i as f64 * 0.004;
                        p.points = vec![point(-0.01, y), point(0.01, y)];
                        p
                    })
                    .collect();
                if !valid {
                    plans.last_mut().unwrap().points.truncate(1);
                }
                plans
            };
            let mut expected = initial.clone();
            let mut actual = initial.clone();
            let baseline = nested_construction_baseline(&mut expected, make_plans(), vec![roads()]);
            let result =
                actual.build_construction_plan_with_roads(make_plans(), vec![], vec![roads()]);
            assert_eq!(result, baseline);
            assert_eq!(expected.snapshot(), actual.snapshot());
            assert_eq!(
                expected.world.operation_save(),
                actual.world.operation_save()
            );
            assert_eq!(
                expected.world.fixed_od_scenario(),
                actual.world.fixed_od_scenario()
            );
            if valid {
                assert!(result.is_ok());
                assert_eq!(actual.world.operation().unwrap().invalidated, count == 2);
                let bytes = rosenfu_storage::encode(&actual.world).unwrap();
                assert_eq!(bytes, rosenfu_storage::encode(&expected.world).unwrap());
                assert_eq!(
                    rosenfu_storage::decode(&bytes).unwrap().operation_save(),
                    actual.world.operation_save()
                );
            } else {
                assert!(result.is_err());
                assert_eq!(actual.snapshot(), initial.snapshot());
                assert_eq!(
                    actual.world.operation_save(),
                    initial.world.operation_save()
                );
            }
            let state = |editor: &WorldEditor| {
                let h = editor.world_edit_history();
                (h.revision, h.undo_depth, h.redo_depth)
            };
            assert_eq!(state(&actual), state(&expected));
            if !valid {
                assert_eq!(state(&actual), state(&initial));
            }
        }
    }

    /// Identical release binary for both arms. Selection is optional so a host
    /// can measure per-arm process RSS separately. Times are not CI assertions.
    #[test]
    #[ignore = "manual synthetic release benchmark"]
    fn measure_one_construction_candidate() {
        use std::time::Instant;
        let arm = std::env::var("ROSENFU_CONSTRUCTION_BENCH_ARM").ok();
        assert!(matches!(arm.as_deref(), None | Some("nested" | "single")));
        for n in [100, 1_000, 12_500] {
            let template = WorldEditor::new();
            let mut draft = template.snapshot();
            let mut fixture = template;
            let mut line = plan();
            line.points = vec![point(-0.01, 0.04), point(0.01, 0.04)];
            let prototype = fixture
                .build_construction_plan(vec![line], vec![])
                .unwrap()
                .alignments
                .remove(0);
            for i in 0..n {
                let mut row = prototype.clone();
                row.id = AlignmentId(draft.next_id);
                draft.next_id += 1;
                let y = 0.01 + 0.000003 * i as f64;
                row.points = vec![point(-0.01, y), point(0.01, y)];
                draft.alignments.push(row);
            }
            fixture.world = World::new(draft).unwrap();
            for pair in 0..6 {
                let arms = if pair % 2 == 0 {
                    ["nested", "single"]
                } else {
                    ["single", "nested"]
                };
                let mut prior = None;
                for name in arms {
                    if arm.as_deref().is_some_and(|a| a != name) {
                        continue;
                    }
                    let mut editor = fixture.clone();
                    let request = vec![plan()];
                    let evidence = vec![roads()];
                    let t = Instant::now();
                    let result = if name == "nested" {
                        nested_construction_baseline(&mut editor, request, evidence)
                    } else {
                        editor.build_construction_plan_with_roads(request, vec![], evidence)
                    }
                    .unwrap();
                    let edit_seconds = t.elapsed().as_secs_f64();
                    let t = Instant::now();
                    let save = rosenfu_storage::encode(&editor.world).unwrap();
                    let save_seconds = t.elapsed().as_secs_f64();
                    let ipc = serde_json::to_vec(&result).unwrap();
                    println!("pair={pair} n={n} arm={name} edit_ms={:.3} save_ms={:.3} ipc_bytes={} save_bytes={}",
                        edit_seconds * 1000., save_seconds * 1000., ipc.len(), save.len());
                    if let Some((prior_save, prior_ipc)) = prior.take() {
                        assert_eq!(save, prior_save);
                        assert_eq!(ipc, prior_ipc);
                    } else if arm.is_none() {
                        prior = Some((save, ipc));
                    }
                }
            }
        }
    }

    #[test]
    fn placement_verdict_matches_summary_without_reporting_uncomputed_crossings() {
        let editor = WorldEditor::new();
        let before = editor.snapshot();
        for valid in [true, false] {
            let make_path = || {
                let mut path = plan();
                if !valid {
                    path.points.truncate(1);
                }
                path
            };
            let summary = editor
                .clone()
                .preview_construction_summary(vec![make_path()], vec![], vec![roads()])
                .unwrap();
            let verdict = editor
                .construction_validation_snapshot()
                .validate_construction_placement(vec![make_path()], vec![], vec![roads()])
                .unwrap();
            assert_eq!(verdict.validation_error, summary.validation_error);
            assert_eq!(verdict.validation_error.is_none(), valid);
            assert_eq!(verdict.new_crossing_count, None);
            assert_eq!(verdict.roads_unverified, None);
            assert_eq!(editor.snapshot(), before);
        }
        let mut invalid_roads = roads();
        invalid_roads.source_sha256 = "invalid".into();
        assert!(editor
            .construction_validation_snapshot()
            .validate_construction_placement(vec![plan()], vec![], vec![invalid_roads])
            .is_err());
        assert_eq!(editor.snapshot(), before);
    }
    #[test]
    fn compact_candidate_discard_keeps_live_state_and_full_build_history_boundary() {
        let mut editor = WorldEditor::new();
        editor.rename_company("First".into()).unwrap();
        editor.rename_company("Second".into()).unwrap();
        editor.undo_world_edit(editor.snapshot()).unwrap();
        let before = editor.snapshot();
        let history = editor.world_edit_history();
        assert_eq!((history.undo_depth, history.redo_depth), (1, 1));

        for valid in [true, false] {
            let make_plan = || {
                let mut p = plan();
                if !valid {
                    p.points.truncate(1);
                }
                p
            };
            let mut build_reference = editor.clone();
            let build_result = build_reference.build_construction_plan(vec![make_plan()], vec![]);
            let fast = editor
                .construction_validation_snapshot()
                .validate_construction_placement(vec![make_plan()], vec![], vec![])
                .unwrap();
            assert_eq!(fast.validation_error, build_result.err());
            assert_eq!(fast.validation_error.is_none(), valid);
            let after = editor.world_edit_history();
            assert_eq!(editor.snapshot(), before);
            assert_eq!(after.revision, history.revision);
            assert_eq!(after.undo_depth, history.undo_depth);
            assert_eq!(after.redo_depth, history.redo_depth);
        }

        let built = editor
            .build_construction_plan(vec![plan()], vec![])
            .unwrap();
        assert_eq!(built.alignments[0].id, AlignmentId(before.next_id));
        let after = editor.world_edit_history();
        assert_eq!(after.revision, history.revision + 1);
        assert_eq!((after.undo_depth, after.redo_depth), (0, 0));
    }

    /// To isolate RSS, select one mode and one size and run the test binary in
    /// its own process; the fixture compiles identical inputs for both cases.
    #[test]
    #[ignore = "manual performance measurement"]
    fn measure_compact_construction_preview() {
        use std::time::Instant;
        let n: usize = std::env::var("ROSENFU_PREVIEW_MEASURE_LINES")
            .unwrap_or_else(|_| "100".into())
            .parse()
            .unwrap();
        assert!([100, 1_000, 10_000].contains(&n));
        let mode = std::env::var("ROSENFU_PREVIEW_MEASURE_MODE").unwrap_or_else(|_| "fast".into());
        assert!(matches!(mode.as_str(), "baseline" | "fast"));

        let mut editor = WorldEditor::new();
        let mut draft = editor.snapshot();
        for i in 0..n {
            let x = 0.2 + ((i % 100) as f64) * 0.01;
            let y = 0.2 + ((i / 100) as f64) * 0.01;
            draft.alignments.push(rosenfu_core::world::Alignment {
                id: AlignmentId(draft.next_id),
                owner: editor.company,
                name: format!("Fixture {i}"),
                points: vec![point(x, y), point(x + 0.009, y)],
                anchor_indices: None,
                infrastructure: vec![],
                vertical_profile: None,
                section_profiles: vec![],
                track_type: rosenfu_core::world::TrackType::Normal,
                design_speed_kmh: None,
            });
            draft.next_id += 1;
        }
        editor.world = World::new(draft).unwrap();
        let serialized_save_bytes = rosenfu_storage::encode(&editor.world).unwrap().len();
        let mut save_guard_samples = Vec::new();
        for i in 0..25 {
            let start = Instant::now();
            rosenfu_storage::ensure_world_saveable(&editor.world).unwrap();
            if i >= 5 {
                save_guard_samples.push(start.elapsed());
            }
        }
        let editor = std::sync::Mutex::new(editor);
        let mut samples = Vec::new();
        let mut lock_samples = Vec::new();
        let mut worker_samples = Vec::new();
        let mut result_bytes = 0;
        for i in 0..25 {
            let start = Instant::now();
            let mut snap = {
                let editor = editor.lock().unwrap();
                if mode == "baseline" {
                    editor.clone()
                } else {
                    editor.construction_validation_snapshot()
                }
            };
            let lock_done = Instant::now();
            let result = if mode == "baseline" {
                // The old IPC made a whole-editor clone, staged roads, then
                // performed a full throwaway commit and discarded its draft.
                snap.world.add_road_snapshot(roads()).unwrap();
                ConstructionPreview {
                    new_crossing_count: None,
                    roads_unverified: None,
                    validation_error: snap.build_construction_plan(vec![plan()], vec![]).err(),
                }
            } else {
                snap.validate_construction_placement(vec![plan()], vec![], vec![roads()])
                    .unwrap()
            };
            let end = Instant::now();
            assert!(
                result.validation_error.is_none(),
                "{:?}",
                result.validation_error
            );
            if i >= 5 {
                samples.push(end.duration_since(start));
                lock_samples.push(lock_done.duration_since(start));
                worker_samples.push(end.duration_since(lock_done));
            }
            result_bytes = serde_json::to_vec(&result).unwrap().len();
        }
        for durations in [
            &mut samples,
            &mut lock_samples,
            &mut worker_samples,
            &mut save_guard_samples,
        ] {
            durations.sort_unstable();
        }
        let ms = |durations: &[std::time::Duration], index: usize| {
            durations[index].as_secs_f64() * 1_000.
        };
        eprintln!(
            "{mode} lines={n} p50_ms={:.3} p95_ms={:.3} lock_plus_snapshot_p50_ms={:.3}              worker_p50_ms={:.3} save_guard_p50_ms={:.3} serialized_save_bytes={serialized_save_bytes}              compact_ipc_bytes={result_bytes}",
            ms(&samples, 9),
            ms(&samples, 18),
            ms(&lock_samples, 9),
            ms(&worker_samples, 9),
            ms(&save_guard_samples, 9),
        );
    }

    #[test]
    #[ignore = "manual latency measurement"]
    fn measure_anchor_preview_latency() {
        let mut editor = WorldEditor::new();
        for i in 0..20 {
            let mut path = plan();
            let y = i as f64 * 0.0001;
            path.points = vec![point(-0.01, y), point(0.01, y)];
            editor.build_construction_plan(vec![path], vec![]).unwrap();
        }
        let mut data = roads();
        let base = data.roads[0].clone();
        data.roads = (0..100)
            .map(|i| {
                let mut road = base.clone();
                road.id = format!("way/{i}");
                let x = -0.009 + i as f64 * 0.000018;
                road.points = vec![point(x, -0.01), point(x, 0.02)];
                road
            })
            .collect();
        editor.world.add_road_snapshot(data).unwrap();
        for _ in 0..3 {
            let start = std::time::Instant::now();
            let result = editor
                .clone()
                .preview_construction_summary(vec![plan()], vec![], vec![])
                .unwrap();
            assert!(result.validation_error.is_none());
            eprintln!("summary_ms={:.3}", start.elapsed().as_secs_f64() * 1000.0);
            let start = std::time::Instant::now();
            let fast = editor
                .construction_validation_snapshot()
                .validate_construction_placement(vec![plan()], vec![], vec![])
                .unwrap();
            assert_eq!(fast.validation_error, result.validation_error);
            eprintln!(
                "validation_ms={:.3}",
                start.elapsed().as_secs_f64() * 1000.0
            );
        }
    }
    #[test]
    fn preview_counts_crossings_without_mutating_editor() {
        let mut editor = WorldEditor::new();
        let old = editor.snapshot();
        let preview = editor
            .clone()
            .preview_construction_summary(vec![plan()], vec![], vec![roads()])
            .unwrap();
        assert_eq!(preview.new_crossing_count, Some(1));
        assert!(preview.validation_error.is_none());
        assert_eq!(preview.roads_unverified, Some(false));
        assert_eq!(editor.snapshot(), old);
        editor
            .build_construction_plan_with_roads(vec![plan()], vec![], vec![roads()])
            .unwrap();
        assert_eq!(
            editor.road_assessments()[0].1.crossings.len(),
            preview.new_crossing_count.unwrap()
        );
    }
    #[test]
    fn preview_missing_roads_stays_unknown_and_invalid_plan_keeps_world() {
        let editor = WorldEditor::new();
        let old = editor.snapshot();
        let preview = editor
            .clone()
            .preview_construction_summary(vec![plan()], vec![], vec![])
            .unwrap();
        assert_eq!(preview.new_crossing_count, Some(0));
        assert_eq!(preview.roads_unverified, Some(true));
        let invalid = || {
            let mut plan = plan();
            plan.points.clear();
            plan
        };
        let refused = editor
            .construction_validation_snapshot()
            .preview_construction_summary(vec![invalid()], vec![], vec![roads()])
            .unwrap();
        assert!(refused.validation_error.is_some());
        assert_eq!(refused.new_crossing_count, None);
        assert_eq!(refused.roads_unverified, None);

        // An invalid staged source is still rejected before plan validation.
        let mut invalid_source = roads();
        invalid_source.source_sha256 = "invalid".into();
        let error = editor
            .construction_validation_snapshot()
            .preview_construction_summary(vec![invalid()], vec![], vec![invalid_source.clone()])
            .err()
            .expect("invalid road source must fail");
        let expected = editor
            .construction_validation_snapshot()
            .preview_construction_summary(vec![plan()], vec![], vec![invalid_source])
            .err()
            .expect("invalid road source must fail");
        assert_eq!(error, expected);
        assert_eq!(editor.snapshot(), old);
    }
    #[test]
    fn preview_does_not_count_existing_crossings_again() {
        let mut editor = WorldEditor::new();
        editor
            .build_construction_plan_with_roads(vec![plan()], vec![], vec![roads()])
            .unwrap();
        let mut next = plan();
        next.points = vec![point(-0.01, 0.02), point(0.01, 0.02)];
        let preview = editor
            .preview_construction_summary(vec![next], vec![], vec![])
            .unwrap();
        assert_eq!(preview.new_crossing_count, Some(0));
    }
    #[test]
    fn indexed_summary_diff_matches_full_comparison_for_edits_and_split_ids() {
        let mut editor = WorldEditor::new();
        let before = editor
            .build_construction_plan(vec![plan(), plan(), plan()], vec![])
            .unwrap()
            .alignments;
        let unchanged = before[0].clone();
        let mut same_id_edit = before[1].clone();
        same_id_edit.points[0].longitude -= 0.001;
        let mut split_piece = before[2].clone();
        split_piece.id = AlignmentId(1000);
        let after = vec![split_piece, unchanged, same_id_edit];
        let legacy: std::collections::BTreeSet<_> = after
            .iter()
            .filter(|line| !before.contains(line))
            .map(|line| line.id)
            .collect();
        assert_eq!(legacy, [before[1].id, AlignmentId(1000)].into());
        assert_eq!(changed_summary_alignment_ids(&before, &after), legacy);
    }

    #[test]
    fn preview_shared_anchor_crossing_count_matches_commit_once() {
        let mut editor = WorldEditor::new();
        let split = || {
            let mut split = plan();
            split.points = vec![point(-0.01, 0.0), point(0.0, 0.0), point(0.01, 0.0)];
            split.anchor_indices = Some(vec![0, 1, 2]);
            split
        };
        let preview = editor
            .clone()
            .preview_construction_summary(vec![split()], vec![], vec![roads()])
            .unwrap();
        assert_eq!(preview.validation_error, None);
        assert_eq!(preview.new_crossing_count, Some(1));
        editor
            .build_construction_plan_with_roads(vec![split()], vec![], vec![roads()])
            .unwrap();
        assert_eq!(
            editor
                .road_assessments()
                .iter()
                .map(|(_, a)| a.crossings.len())
                .sum::<usize>(),
            1
        );
    }
    #[test]
    fn preview_extension_keeps_existing_crossing_count_zero() {
        let mut editor = WorldEditor::new();
        editor
            .build_construction_plan_with_roads(vec![plan()], vec![], vec![roads()])
            .unwrap();
        let original = editor.snapshot().alignments[0].clone();
        let extension = || {
            let mut extension = plan();
            extension.points = vec![*original.points.last().unwrap(), point(0.02, 0.0)];
            extension.extension = Some(PlannedAlignmentExtension {
                alignment_id: original.id,
                endpoint: rosenfu_core::world::AlignmentEndpoint::End,
            });
            extension
        };
        let preview = editor
            .clone()
            .preview_construction_summary(vec![extension()], vec![], vec![])
            .unwrap();
        assert_eq!(preview.validation_error, None);
        assert_eq!(preview.new_crossing_count, Some(0));
        editor
            .build_construction_plan_with_roads(vec![extension()], vec![], vec![])
            .unwrap();
        assert_eq!(
            editor
                .road_assessments()
                .iter()
                .map(|(_, a)| a.crossings.len())
                .sum::<usize>(),
            1
        );
    }
    #[test]
    fn road_snapshot_and_alignment_commit_atomically_and_survive_serialization() {
        let mut editor = WorldEditor::new();
        let old = editor.snapshot();
        let mut invalid = plan();
        invalid.points.clear();
        assert!(editor
            .build_construction_plan_with_roads(vec![invalid], vec![], vec![roads()])
            .is_err());
        assert_eq!(editor.snapshot(), old);
        let result = editor
            .build_construction_plan_with_roads(vec![plan()], vec![], vec![roads()])
            .unwrap();
        assert_eq!(result.road_snapshots.len(), 1);
        let decoded: WorldDraft =
            serde_json::from_str(&serde_json::to_string(&result).unwrap()).unwrap();
        assert_eq!(decoded, result);
        let checks = editor.road_assessments();
        assert_eq!(checks.len(), 1);
        assert!(checks[0].1.roads_ready);
        assert_eq!(checks[0].1.crossings.len(), 1);
    }
    #[test]
    fn missing_roads_are_unverified_without_disabling_ordinary_building() {
        let mut editor = WorldEditor::new();
        editor
            .build_construction_plan_with_roads(vec![plan()], vec![], vec![])
            .unwrap();
        let check = &editor.road_assessments()[0].1;
        assert!(!check.roads_ready);
        assert!(check.crossings.is_empty());
    }
}
