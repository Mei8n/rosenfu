#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod session;
use session::construction_timing;
mod water_zone_delivery;
use rosenfu_core::world::{AlignmentId, GeoPoint, RouteId, ServiceId, StationId, WorldDraft};
use session::WorldEditor;
use std::{path::PathBuf, sync::Mutex};
use tauri::{Manager, State};

#[derive(serde::Serialize)]
struct EngineSessionInfo {
    #[serde(flatten)]
    engine: rosenfu_core::EngineInfo,
    company_setup_required: bool,
}
#[tauri::command]
fn engine_info(editor: State<'_, Mutex<WorldEditor>>) -> Result<EngineSessionInfo, String> {
    Ok(EngineSessionInfo {
        engine: rosenfu_core::engine_info(),
        company_setup_required: editor
            .lock()
            .map_err(|_| "world state unavailable")?
            .company_setup_required(),
    })
}
#[tauri::command]
async fn preview_track_movement_geometry(
    app: tauri::AppHandle,
    world: WorldDraft,
) -> Result<Vec<rosenfu_core::world::TrackMovementGeometry>, String> {
    run_world_snapshot(app, move |snapshot| {
        let _timing = construction_timing::Phase::new("preview_track_movement_geometry");
        snapshot
            .world_for_ui_preview(world)?
            .track_movement_geometries()
            .map_err(|error| error.to_string())
    })
    .await
}
#[tauri::command]
async fn preview_track_render_geometry(
    app: tauri::AppHandle,
    world: WorldDraft,
) -> Result<rosenfu_core::world::TrackRenderGeometry, String> {
    run_world_snapshot(app, move |snapshot| {
        let _timing = construction_timing::Phase::new("preview_track_render_geometry");
        snapshot
            .world_for_ui_preview(world)?
            .track_render_geometry()
            .map_err(|error| error.to_string())
    })
    .await
}
#[tauri::command]
fn ofm_world_token(editor: State<'_, Mutex<WorldEditor>>) -> Result<String, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .ofm_world_token()
}
#[tauri::command]
fn stage_ofm_construction_plan(
    editor: State<'_, Mutex<WorldEditor>>,
    expected_world_token: String,
    plans: rosenfu_core::world::ConstructionPlans,
    raw_evidence: Vec<session::OfmRawInput>,
    tile_bindings: Vec<session::OfmBindingInput>,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .stage_ofm_construction_plan(&expected_world_token, plans, raw_evidence, tile_bindings)
}
#[tauri::command]
fn water_mask_requests(
    bounds: Option<[f64; 4]>,
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<Vec<serde_json::Value>, String> {
    let editor = editor.lock().map_err(|_| "world state unavailable")?;
    if editor.national_water_enabled() {
        return match bounds {
            Some(bounds) => Ok(editor
                .missing_water_zones(bounds)?
                .into_iter()
                .map(|id| serde_json::json!({"id":id,"native":true}))
                .collect()),
            None => Ok(vec![]),
        };
    }
    editor
        .missing_water_masks()
        .into_iter()
        .map(|r| serde_json::to_value(r).map_err(|e| e.to_string()))
        .collect()
}
#[tauri::command]
async fn acquire_water_zone(
    app: tauri::AppHandle,
    delivery: State<'_, std::sync::Arc<water_zone_delivery::WaterZoneDelivery>>,
    id: String,
) -> Result<(), String> {
    if id.len() > 32 {
        return Err("未登録の水域データです。".into());
    }
    let delivery = delivery.inner().clone();
    let zone = tauri::async_runtime::spawn_blocking(move || {
        delivery.acquire(&id, &water_zone_delivery::Cancellation::default())
    })
    .await
    .map_err(|_| "水域取得処理に失敗しました。")?
    .map_err(|e| format!("水域データを取得できませんでした: {e}"))?;
    let editor = app.state::<Mutex<WorldEditor>>();
    let result = editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .install_water_zone(zone);
    result
}

#[tauri::command]
fn install_water_mask(
    editor: State<'_, Mutex<WorldEditor>>,
    id: String,
    bytes_base64: String,
) -> Result<(), String> {
    use base64::Engine;
    if id.len() > 32 || bytes_base64.len() > 5500 {
        return Err("水域データが上限を超えています。".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(bytes_base64)
        .map_err(|_| "水域データを復号できません。")?;
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .install_water_mask(&id, &bytes)
}
#[tauri::command]
fn world_snapshot(editor: State<'_, Mutex<WorldEditor>>) -> Result<WorldDraft, String> {
    Ok(editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .snapshot())
}
#[tauri::command]
fn add_station(
    editor: State<'_, Mutex<WorldEditor>>,
    name: String,
    latitude: f64,
    longitude: f64,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .add_station(name, latitude, longitude)
}
#[tauri::command]
async fn build_alignment_from_reference(
    app: tauri::AppHandle,
    name: String,
    points: Vec<GeoPoint>,
    source_record: rosenfu_core::world::SourceRecord,
    provenance: rosenfu_core::world::AlignmentProvenanceInput,
    track_type: rosenfu_core::world::TrackType,
    design_speed_kmh: Option<f64>,
    anchor_indices: Option<Vec<usize>>,
    infrastructure: Option<rosenfu_core::world::SectionInfrastructure>,
    start_level: Option<rosenfu_core::world::InfrastructureLevel>,
    anchor_levels: Option<Vec<rosenfu_core::world::InfrastructureLevel>>,
    junction_source: Option<rosenfu_core::world::ConstructionJunctionSource>,
) -> Result<WorldDraft, String> {
    run_world_edit(app, move |editor| {
        editor.build_plan(vec![session::PlannedAlignment {
            ofm_path: None,
            anchor_levels,
            junction_source,
            start_level,
            infrastructure: infrastructure.unwrap_or_default(),
            anchor_indices,
            name,
            points,
            track_type,
            design_speed_kmh,
            stations: vec![],
            reference: Some(session::PlannedAlignmentReference {
                source_record,
                provenance,
            }),
            extension: None,
            destination: None,
        }])
    })
    .await
}
#[tauri::command]
async fn build_alignment(
    app: tauri::AppHandle,
    name: String,
    points: Vec<GeoPoint>,
    track_type: rosenfu_core::world::TrackType,
    design_speed_kmh: Option<f64>,
    anchor_indices: Option<Vec<usize>>,
    infrastructure: Option<rosenfu_core::world::SectionInfrastructure>,
    start_level: Option<rosenfu_core::world::InfrastructureLevel>,
    anchor_levels: Option<Vec<rosenfu_core::world::InfrastructureLevel>>,
    junction_source: Option<rosenfu_core::world::ConstructionJunctionSource>,
) -> Result<WorldDraft, String> {
    run_world_edit(app, move |editor| {
        editor.build_plan(vec![session::PlannedAlignment {
            ofm_path: None,
            anchor_levels,
            junction_source,
            start_level,
            infrastructure: infrastructure.unwrap_or_default(),
            anchor_indices,
            name,
            points,
            track_type,
            design_speed_kmh,
            stations: vec![],
            reference: None,
            extension: None,
            destination: None,
        }])
    })
    .await
}
#[tauri::command]
async fn build_alignment_with_stations(
    app: tauri::AppHandle,
    name: String,
    points: Vec<GeoPoint>,
    track_type: rosenfu_core::world::TrackType,
    stations: Vec<session::PlannedStation>,
    reference: Option<session::PlannedAlignmentReference>,
    design_speed_kmh: Option<f64>,
    anchor_indices: Option<Vec<usize>>,
    infrastructure: Option<rosenfu_core::world::SectionInfrastructure>,
    start_level: Option<rosenfu_core::world::InfrastructureLevel>,
    anchor_levels: Option<Vec<rosenfu_core::world::InfrastructureLevel>>,
    junction_source: Option<rosenfu_core::world::ConstructionJunctionSource>,
) -> Result<WorldDraft, String> {
    run_world_edit(app, move |editor| {
        editor.build_plan(vec![session::PlannedAlignment {
            ofm_path: None,
            anchor_levels,
            junction_source,
            start_level,
            infrastructure: infrastructure.unwrap_or_default(),
            anchor_indices,
            name,
            points,
            track_type,
            design_speed_kmh,
            stations,
            reference,
            extension: None,
            destination: None,
        }])
    })
    .await
}
#[tauri::command]
async fn preview_construction_plan(
    editor: State<'_, Mutex<WorldEditor>>,
    alignments: Vec<session::PlannedAlignment>,
    stations: Option<Vec<session::PlannedBuiltStation>>,
    roads: Option<Vec<rosenfu_core::world::RoadSnapshot>>,
    validation_only: Option<bool>,
) -> Result<session::ConstructionPreview, String> {
    let snapshot = editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .clone();
    // Validation can be expensive: never run it on the UI event-loop thread.
    tauri::async_runtime::spawn_blocking(move || {
        if validation_only.unwrap_or(false) {
            return snapshot.validate_construction_placement(
                alignments,
                stations.unwrap_or_default(),
                roads.unwrap_or_default(),
            );
        }
        snapshot.preview_construction_summary(
            alignments,
            stations.unwrap_or_default(),
            roads.unwrap_or_default(),
        )
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn build_plan(
    app: tauri::AppHandle,
    alignments: Vec<session::PlannedAlignment>,
    stations: Option<Vec<session::PlannedBuiltStation>>,
) -> Result<WorldDraft, String> {
    run_world_edit(app, move |editor| {
        editor.build_construction_plan(alignments, stations.unwrap_or_default())
    })
    .await
}
#[tauri::command]
async fn build_plan_with_roads(
    app: tauri::AppHandle,
    alignments: Vec<session::PlannedAlignment>,
    stations: Option<Vec<session::PlannedBuiltStation>>,
    roads: Vec<rosenfu_core::world::RoadSnapshot>,
) -> Result<WorldDraft, String> {
    run_world_edit(app, move |editor| {
        editor.build_construction_plan_with_roads(alignments, stations.unwrap_or_default(), roads)
    })
    .await
}
#[tauri::command]
fn preview_plan_with_roads(
    editor: State<'_, Mutex<WorldEditor>>,
    alignments: Vec<session::PlannedAlignment>,
    stations: Option<Vec<session::PlannedBuiltStation>>,
    roads: Vec<rosenfu_core::world::RoadSnapshot>,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .clone()
        .build_construction_plan_with_roads(alignments, stations.unwrap_or_default(), roads)
}
#[tauri::command]
async fn road_assessments(
    app: tauri::AppHandle,
) -> Result<Vec<(AlignmentId, rosenfu_core::world::RoadAssessment)>, String> {
    run_world_snapshot(app, move |snapshot| {
        let _compute = construction_timing::Phase::new("road_assessments.compute");
        Ok(snapshot.road_assessments())
    })
    .await
}
#[tauri::command]
fn place_station_on_alignment(
    editor: State<'_, Mutex<WorldEditor>>,
    name: String,
    alignment_id: AlignmentId,
    position: f64,
    length_m: f64,
    width_m: f64,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .place_station_on_alignment(name, alignment_id, position, length_m, width_m)
}
#[tauri::command]
fn set_service_appearance(
    editor: State<'_, Mutex<WorldEditor>>,
    service_id: ServiceId,
    color: String,
    shape: rosenfu_core::world::ServiceSymbolShape,
    symbol: String,
    expected_appearance: Option<rosenfu_core::world::ServiceAppearance>,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .set_service_appearance(service_id, color, shape, symbol, expected_appearance)
}
#[tauri::command]
fn set_service_color(
    editor: State<'_, Mutex<WorldEditor>>,
    service_id: ServiceId,
    color: String,
    expected_color: Option<String>,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .set_service_color(service_id, color, expected_color)
}
#[tauri::command]
fn delete_service(
    editor: State<'_, Mutex<WorldEditor>>,
    service_id: ServiceId,
    expected_path: rosenfu_core::world::ServicePath,
    expected_name: String,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .delete_service(service_id, expected_path, expected_name)
}
#[tauri::command]
fn duplicate_service(
    editor: State<'_, Mutex<WorldEditor>>,
    service_id: ServiceId,
    expected_path: rosenfu_core::world::ServicePath,
    expected_name: String,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .duplicate_service(service_id, expected_path, expected_name)
}
#[tauri::command]
fn world_edit_history(
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<session::WorldEditHistory, String> {
    Ok(editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .world_edit_history())
}
#[tauri::command]
async fn undo_world_edit(
    app: tauri::AppHandle,
    expected_world: WorldDraft,
) -> Result<WorldDraft, String> {
    run_world_edit(app, move |editor| editor.undo_world_edit(expected_world)).await
}
#[tauri::command]
async fn redo_world_edit(
    app: tauri::AppHandle,
    expected_world: WorldDraft,
) -> Result<WorldDraft, String> {
    run_world_edit(app, move |editor| editor.redo_world_edit(expected_world)).await
}
#[tauri::command]
fn rename_station(
    editor: State<'_, Mutex<WorldEditor>>,
    station_id: StationId,
    name: String,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .rename_station(station_id, name)
}
// Read-only work uses an owned snapshot; CPU scans never hold the live mutex.
async fn run_world_snapshot<T: Send + 'static>(
    app: tauri::AppHandle,
    operation: impl FnOnce(WorldEditor) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let queue = construction_timing::Phase::new("world_snapshot.queue");
    tauri::async_runtime::spawn_blocking(move || {
        drop(queue);
        let waiting = construction_timing::Phase::new("world_snapshot.lock_clone");
        let snapshot = app
            .state::<Mutex<WorldEditor>>()
            .lock()
            .map_err(|_| "world state unavailable")?
            .clone();
        drop(waiting);
        operation(snapshot)
    })
    .await
    .map_err(|error| format!("描画データの計算を完了できませんでした: {error}"))?
}
// Keep the atomic editor transaction and its mutex on a blocking worker.
// Construction and demolition validate/remap references and snapshots; none of that
// work, including waiting for another editor operation, belongs on the UI thread.
async fn run_world_edit(
    app: tauri::AppHandle,
    operation: impl FnOnce(&mut WorldEditor) -> Result<WorldDraft, String> + Send + 'static,
) -> Result<WorldDraft, String> {
    let queue = construction_timing::Phase::new("world_edit.queue");
    tauri::async_runtime::spawn_blocking(move || {
        drop(queue);
        let waiting = construction_timing::Phase::new("world_edit.lock");
        let state = app.state::<Mutex<WorldEditor>>();
        let mut editor = state.lock().map_err(|_| "world state unavailable")?;
        drop(waiting);
        let _operation = construction_timing::Phase::new("world_edit.transaction");
        operation(&mut editor)
    })
    .await
    .map_err(|error| format!("ワールド更新処理を完了できませんでした: {error}"))?
}
#[tauri::command]
async fn delete_station(
    app: tauri::AppHandle,
    station_id: StationId,
    construction_plans: Option<rosenfu_core::world::ConstructionPlans>,
) -> Result<WorldDraft, String> {
    run_world_edit(app, move |editor| match construction_plans {
        Some(plans) => editor.delete_station_with_plans(station_id, plans),
        None => editor.delete_station(station_id),
    })
    .await
}
#[tauri::command]
async fn delete_alignment(
    app: tauri::AppHandle,
    alignment_id: AlignmentId,
    construction_plans: rosenfu_core::world::ConstructionPlans,
) -> Result<WorldDraft, String> {
    run_world_edit(app, move |editor| {
        editor.delete_alignment_with_plans(alignment_id, construction_plans)
    })
    .await
}
#[tauri::command]
async fn delete_alignment_section(
    app: tauri::AppHandle,
    alignment_id: AlignmentId,
    section_index: usize,
    construction_plans: rosenfu_core::world::ConstructionPlans,
) -> Result<WorldDraft, String> {
    run_world_edit(app, move |editor| {
        editor.delete_alignment_section(alignment_id, section_index, construction_plans)
    })
    .await
}
#[tauri::command]
async fn delete_alignment_sections(
    app: tauri::AppHandle,
    selections: Vec<session::AlignmentSectionSelection>,
    expected_alignments: Vec<rosenfu_core::world::Alignment>,
    construction_plans: rosenfu_core::world::ConstructionPlans,
) -> Result<WorldDraft, String> {
    run_world_edit(app, move |editor| {
        editor.delete_alignment_sections(selections, expected_alignments, construction_plans)
    })
    .await
}
#[tauri::command]
fn update_alignment_anchor_height(
    editor: State<'_, Mutex<WorldEditor>>,
    alignment_id: AlignmentId,
    anchor_index: usize,
    level: rosenfu_core::world::InfrastructureLevel,
    expected_alignment: rosenfu_core::world::Alignment,
    construction_plans: rosenfu_core::world::ConstructionPlans,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .update_alignment_anchor_height(
            alignment_id,
            anchor_index,
            level,
            expected_alignment,
            construction_plans,
        )
}
#[tauri::command]
fn update_alignment_section_infrastructure(
    editor: State<'_, Mutex<WorldEditor>>,
    selections: Vec<session::AlignmentSectionSelection>,
    expected_alignments: Vec<rosenfu_core::world::Alignment>,
    track_count: Option<u8>,
    level: Option<rosenfu_core::world::InfrastructureLevel>,
    construction_plans: rosenfu_core::world::ConstructionPlans,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .update_alignment_section_infrastructure(
            selections,
            expected_alignments,
            track_count,
            level,
            construction_plans,
        )
}
#[tauri::command]
async fn update_alignment_section_track_type(
    app: tauri::AppHandle,
    selections: Vec<session::AlignmentSectionSelection>,
    expected_alignments: Vec<rosenfu_core::world::Alignment>,
    track_type: rosenfu_core::world::TrackType,
    construction_plans: rosenfu_core::world::ConstructionPlans,
) -> Result<WorldDraft, String> {
    run_world_edit(app, move |editor| {
        editor.update_alignment_section_track_type(
            selections,
            expected_alignments,
            track_type,
            construction_plans,
        )
    })
    .await
}
#[tauri::command]
async fn undo_alignment_deletion(app: tauri::AppHandle) -> Result<WorldDraft, String> {
    run_world_edit(app, move |editor| editor.undo_alignment_deletion()).await
}

#[tauri::command]
fn rename_company(
    editor: State<'_, Mutex<WorldEditor>>,
    name: String,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .rename_company(name)
}
#[tauri::command]
fn rename_route(
    editor: State<'_, Mutex<WorldEditor>>,
    route_id: RouteId,
    name: String,
    expected_name: String,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .rename_route(route_id, name, expected_name)
}
#[tauri::command]
fn save_planned_route(
    editor: State<'_, Mutex<WorldEditor>>,
    route_id: Option<RouteId>,
    name: String,
    start: StationId,
    segments: Vec<rosenfu_core::world::RouteSegment>,
    excluded_stations: Vec<StationId>,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .save_planned_route(route_id, name, start, segments, excluded_stations)
}

#[tauri::command]
fn set_service_route_binding(
    editor: State<'_, Mutex<WorldEditor>>,
    service_id: ServiceId,
    binding: rosenfu_core::world::ServiceRouteBindingKind,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .set_service_route_binding(service_id, binding)
}

#[tauri::command]
fn update_station_placement(
    editor: State<'_, Mutex<WorldEditor>>,
    station_id: StationId,
    length_m: f64,
    width_m: f64,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .update_station_placement(station_id, length_m, width_m)
}
#[tauri::command]
fn preview_service_path(
    editor: State<'_, Mutex<WorldEditor>>,
    start: StationId,
    segments: Vec<rosenfu_core::world::RouteSegment>,
) -> Result<rosenfu_core::world::ServicePathPreview, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .preview_service_path(start, segments)
}
#[tauri::command]
fn create_service_path(
    editor: State<'_, Mutex<WorldEditor>>,
    name: String,
    start: StationId,
    segments: Vec<rosenfu_core::world::RouteSegment>,
    stops: Vec<rosenfu_core::world::StationOccurrenceRef>,
    source_route_id: Option<RouteId>,
    source_route_ids: Option<Vec<RouteId>>,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .create_service_path_with_source_routes(
            name,
            start,
            segments,
            stops,
            source_route_id,
            source_route_ids,
        )
}
#[tauri::command]
fn update_service_path(
    editor: State<'_, Mutex<WorldEditor>>,
    service_id: ServiceId,
    name: String,
    stops: Vec<rosenfu_core::world::StationOccurrenceRef>,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .update_service_path(service_id, name, stops)
}
#[tauri::command]
fn preview_service_path_replacement(
    editor: State<'_, Mutex<WorldEditor>>,
    service_id: ServiceId,
    start: StationId,
    segments: Vec<rosenfu_core::world::RouteSegment>,
) -> Result<rosenfu_core::world::ServicePathPreview, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .preview_service_path_replacement(service_id, start, segments)
}
#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn replace_service_path(
    editor: State<'_, Mutex<WorldEditor>>,
    service_id: ServiceId,
    expected_path: rosenfu_core::world::ServicePath,
    name: String,
    start: StationId,
    segments: Vec<rosenfu_core::world::RouteSegment>,
    stops: Vec<rosenfu_core::world::StationOccurrenceRef>,
    source_route_id: Option<RouteId>,
    source_route_ids: Option<Vec<RouteId>>,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .replace_service_path_with_source_routes(
            service_id,
            expected_path,
            name,
            start,
            segments,
            stops,
            source_route_id,
            source_route_ids,
        )
}

#[tauri::command]
fn preview_route_candidates(
    editor: State<'_, Mutex<WorldEditor>>,
    start: StationId,
    end: StationId,
    via_station_ids: Option<Vec<StationId>>,
) -> Result<rosenfu_core::world::RouteCandidates, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .preview_route_candidates_via(start, end, &via_station_ids.unwrap_or_default())
}
#[tauri::command]
fn preview_route_membership(
    editor: State<'_, Mutex<WorldEditor>>,
    start: StationId,
    segments: Vec<rosenfu_core::world::RouteSegment>,
    excluded_stations: Vec<StationId>,
) -> Result<rosenfu_core::world::RouteMembershipPreview, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .preview_route_membership(start, segments, excluded_stations)
}
fn save_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("prototype"))
}
#[derive(serde::Serialize)]
struct AutosaveStatus {
    available: bool,
}
#[derive(serde::Serialize)]
struct SaveResult {
    saved: bool,
}
#[tauri::command]
fn list_save_slots(
    app: tauri::AppHandle,
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<session::SaveSlotList, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .list_save_slots(&save_root(&app)?)
}
#[tauri::command]
fn move_save_slot(
    app: tauri::AppHandle,
    slot_id: String,
    restore: bool,
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<(), String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .move_save_slot(&save_root(&app)?, &slot_id, restore)
}
#[tauri::command]
fn create_new_world(
    company_name: String,
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .create_new_world(company_name)
}
#[tauri::command]
fn load_save_slot(
    app: tauri::AppHandle,
    slot_id: String,
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .load_save_slot(&save_root(&app)?, slot_id)
}
#[tauri::command]
fn save_world_as(
    app: tauri::AppHandle,
    name: String,
    construction_plans: Option<rosenfu_core::world::ConstructionPlans>,
    view_state: Option<rosenfu_core::world::ViewState>,
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<session::ManualSaveResult, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .save_world_as_slot(&save_root(&app)?, name, construction_plans, view_state)
}
#[tauri::command]
fn autosave_status(
    app: tauri::AppHandle,
    slot_id: String,
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<AutosaveStatus, String> {
    let mut editor = editor.lock().map_err(|_| "world state unavailable")?;
    editor.require_active_slot(&slot_id)?;
    let available = editor.slot_autosave_status(&save_root(&app)?)?;
    Ok(AutosaveStatus { available })
}
#[tauri::command]
fn allow_autosave_replacement(
    app: tauri::AppHandle,
    slot_id: String,
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<(), String> {
    let mut editor = editor.lock().map_err(|_| "world state unavailable")?;
    editor.require_active_slot(&slot_id)?;
    let path = editor.active_autosave_path(&save_root(&app)?)?;
    editor.allow_autosave_replacement(&path)
}
#[tauri::command]
fn autosave_world(
    app: tauri::AppHandle,
    slot_id: String,
    construction_plans: Option<rosenfu_core::world::ConstructionPlans>,
    view_state: Option<rosenfu_core::world::ViewState>,
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<SaveResult, String> {
    let mut editor = editor.lock().map_err(|_| "world state unavailable")?;
    editor.autosave_active_slot(&save_root(&app)?, &slot_id, construction_plans, view_state)?;
    Ok(SaveResult { saved: true })
}
#[tauri::command]
fn load_autosave(
    app: tauri::AppHandle,
    slot_id: String,
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<WorldDraft, String> {
    let mut editor = editor.lock().map_err(|_| "world state unavailable")?;
    editor.require_active_slot(&slot_id)?;
    editor.load_slot_autosave(&save_root(&app)?)
}
#[tauri::command]
fn save_world(
    app: tauri::AppHandle,
    construction_plans: Option<rosenfu_core::world::ConstructionPlans>,
    view_state: Option<rosenfu_core::world::ViewState>,
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<session::ManualSaveResult, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .save_active_slot(&save_root(&app)?, construction_plans, view_state)
}
fn main() {
    let mut editor = WorldEditor::new();
    editor
        .enable_national_water()
        .expect("invalid compiled water registry");
    let delivery = water_zone_delivery::WaterZoneDelivery::new(
        rosenfu_core::water_zone::registry::pins()
            .expect("compiled water registry")
            .to_vec(),
        water_zone_delivery::HttpTransport::new().expect("water HTTPS client"),
    )
    .expect("water delivery registry");
    tauri::Builder::default()
        .manage(std::sync::Arc::new(delivery))
        .manage(Mutex::new(editor))
        .invoke_handler(tauri::generate_handler![
            engine_info,
            preview_service_path,
            create_service_path,
            update_service_path,
            preview_service_path_replacement,
            replace_service_path,
            preview_route_candidates,
            preview_route_membership,
            world_snapshot,
            ofm_world_token,
            stage_ofm_construction_plan,
            water_mask_requests,
            acquire_water_zone,
            install_water_mask,
            preview_track_movement_geometry,
            preview_track_render_geometry,
            add_station,
            build_alignment_from_reference,
            build_alignment,
            build_alignment_with_stations,
            build_plan,
            preview_construction_plan,
            build_plan_with_roads,
            preview_plan_with_roads,
            road_assessments,
            update_station_placement,
            place_station_on_alignment,
            world_edit_history,
            undo_world_edit,
            redo_world_edit,
            rename_station,
            set_service_color,
            set_service_appearance,
            duplicate_service,
            delete_service,
            delete_station,
            delete_alignment,
            delete_alignment_section,
            delete_alignment_sections,
            update_alignment_section_infrastructure,
            update_alignment_section_track_type,
            update_alignment_anchor_height,
            undo_alignment_deletion,
            rename_company,
            save_planned_route,
            rename_route,
            set_service_route_binding,
            list_save_slots,
            move_save_slot,
            create_new_world,
            load_save_slot,
            save_world_as,
            save_world,
            autosave_world,
            autosave_status,
            allow_autosave_replacement,
            load_autosave
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Rosenfu");
}
