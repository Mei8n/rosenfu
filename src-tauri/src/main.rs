#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod session;
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
fn preview_track_movement_geometry(
    world: WorldDraft,
) -> Result<Vec<rosenfu_core::world::TrackMovementGeometry>, String> {
    rosenfu_core::world::World::new(world)
        .map_err(|e| e.to_string())?
        .track_movement_geometries()
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn preview_track_render_geometry(
    world: WorldDraft,
) -> Result<rosenfu_core::world::TrackRenderGeometry, String> {
    rosenfu_core::world::World::new(world)
        .map_err(|e| e.to_string())?
        .track_render_geometry()
        .map_err(|e| e.to_string())
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
fn build_alignment_from_reference(
    editor: State<'_, Mutex<WorldEditor>>,
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
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .build_plan(vec![session::PlannedAlignment {
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
}
#[tauri::command]
fn build_alignment(
    editor: State<'_, Mutex<WorldEditor>>,
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
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .build_plan(vec![session::PlannedAlignment {
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
}
#[tauri::command]
fn build_alignment_with_stations(
    editor: State<'_, Mutex<WorldEditor>>,
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
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .build_plan(vec![session::PlannedAlignment {
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
}
#[tauri::command]
fn preview_construction_plan(
    editor: State<'_, Mutex<WorldEditor>>,
    alignments: Vec<session::PlannedAlignment>,
    stations: Option<Vec<session::PlannedBuiltStation>>,
) -> Result<WorldDraft, String> {
    let snapshot = editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .clone();
    snapshot.preview_construction_plan(alignments, stations.unwrap_or_default())
}
#[tauri::command]
fn preview_environment(
    editor: State<'_, Mutex<WorldEditor>>,
    alignments: Vec<session::PlannedAlignment>,
    stations: Vec<session::PlannedBuiltStation>,
    environment: rosenfu_core::world::EnvironmentSnapshot,
) -> Result<session::ConstructionEnvironmentPreview, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .preview_environment(alignments, stations, environment)
}
#[tauri::command]
fn build_plan_with_environment(
    editor: State<'_, Mutex<WorldEditor>>,
    alignments: Vec<session::PlannedAlignment>,
    stations: Vec<session::PlannedBuiltStation>,
    environment: rosenfu_core::world::EnvironmentSnapshot,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .build_plan_with_environment(alignments, stations, environment)
}
#[tauri::command]
fn automatic_crossings(
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<Vec<rosenfu_core::world::AutomaticCrossing>, String> {
    Ok(editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .automatic_crossings())
}
#[tauri::command]
fn build_plan(
    editor: State<'_, Mutex<WorldEditor>>,
    alignments: Vec<session::PlannedAlignment>,
    stations: Option<Vec<session::PlannedBuiltStation>>,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .build_construction_plan(alignments, stations.unwrap_or_default())
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
#[tauri::command]
fn delete_station(
    editor: State<'_, Mutex<WorldEditor>>,
    station_id: StationId,
    construction_plans: Option<rosenfu_core::world::ConstructionPlans>,
) -> Result<WorldDraft, String> {
    let mut editor = editor.lock().map_err(|_| "world state unavailable")?;
    match construction_plans {
        Some(plans) => editor.delete_station_with_plans(station_id, plans),
        None => editor.delete_station(station_id),
    }
}
#[tauri::command]
fn delete_alignment(
    editor: State<'_, Mutex<WorldEditor>>,
    alignment_id: AlignmentId,
    construction_plans: rosenfu_core::world::ConstructionPlans,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .delete_alignment_with_plans(alignment_id, construction_plans)
}
#[tauri::command]
fn delete_alignment_section(
    editor: State<'_, Mutex<WorldEditor>>,
    alignment_id: AlignmentId,
    section_index: usize,
    construction_plans: rosenfu_core::world::ConstructionPlans,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .delete_alignment_section(alignment_id, section_index, construction_plans)
}
#[tauri::command]
fn delete_alignment_sections(
    editor: State<'_, Mutex<WorldEditor>>,
    selections: Vec<session::AlignmentSectionSelection>,
    expected_alignments: Vec<rosenfu_core::world::Alignment>,
    construction_plans: rosenfu_core::world::ConstructionPlans,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .delete_alignment_sections(selections, expected_alignments, construction_plans)
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
fn undo_alignment_deletion(editor: State<'_, Mutex<WorldEditor>>) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .undo_alignment_deletion()
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
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .create_service_path_with_source_route(name, start, segments, stops, source_route_id)
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
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .replace_service_path_with_source_route(
            service_id,
            expected_path,
            name,
            start,
            segments,
            stops,
            source_route_id,
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
    tauri::Builder::default()
        .manage(Mutex::new(WorldEditor::new()))
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
            preview_track_movement_geometry,
            preview_track_render_geometry,
            add_station,
            build_alignment_from_reference,
            build_alignment,
            build_alignment_with_stations,
            build_plan,
            preview_construction_plan,
            preview_environment,
            build_plan_with_environment,
            automatic_crossings,
            update_station_placement,
            place_station_on_alignment,
            rename_station,
            delete_station,
            delete_alignment,
            delete_alignment_section,
            delete_alignment_sections,
            update_alignment_section_infrastructure,
            update_alignment_anchor_height,
            undo_alignment_deletion,
            rename_company,
            save_planned_route,
            set_service_route_binding,
            list_save_slots,
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
