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
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .build_plan(vec![session::PlannedAlignment {
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
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .build_plan(vec![session::PlannedAlignment {
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
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .build_plan(vec![session::PlannedAlignment {
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
fn save_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("prototype")
        .join("world.rosenfu.json"))
}
fn autosave_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(save_path(app)?.with_file_name("world.autosave.rosenfu.json"))
}
#[derive(serde::Serialize)]
struct AutosaveStatus {
    available: bool,
}
#[tauri::command]
fn autosave_status(
    app: tauri::AppHandle,
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<AutosaveStatus, String> {
    let available = editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .autosave_status(&autosave_path(&app)?)?;
    Ok(AutosaveStatus { available })
}
#[tauri::command]
fn allow_autosave_replacement(
    app: tauri::AppHandle,
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<(), String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .allow_autosave_replacement(&autosave_path(&app)?)
}
#[tauri::command]
fn autosave_world(
    app: tauri::AppHandle,
    construction_plans: Option<rosenfu_core::world::ConstructionPlans>,
    view_state: Option<rosenfu_core::world::ViewState>,
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<SaveResult, String> {
    let path = autosave_path(&app)?;
    std::fs::create_dir_all(path.parent().ok_or("invalid autosave path")?)
        .map_err(|e| e.to_string())?;
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .save_with_view(&path, construction_plans, view_state, true)?;
    Ok(SaveResult { saved: true })
}
#[tauri::command]
fn load_autosave(
    app: tauri::AppHandle,
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .load_autosave(&autosave_path(&app)?, &save_path(&app)?)
}
#[derive(serde::Serialize)]
struct SaveResult {
    saved: bool,
}
#[tauri::command]
fn save_world(
    app: tauri::AppHandle,
    construction_plans: Option<rosenfu_core::world::ConstructionPlans>,
    view_state: Option<rosenfu_core::world::ViewState>,
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<session::ManualSaveResult, String> {
    let path = save_path(&app)?;
    std::fs::create_dir_all(path.parent().ok_or("invalid save path")?)
        .map_err(|e| e.to_string())?;
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .request_manual_save(&path, construction_plans, view_state)
}
#[tauri::command]
fn confirm_manual_save(
    app: tauri::AppHandle,
    token: String,
    construction_plans: Option<rosenfu_core::world::ConstructionPlans>,
    view_state: Option<rosenfu_core::world::ViewState>,
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<session::ManualSaveResult, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .confirm_manual_save(&save_path(&app)?, &token, construction_plans, view_state)
}
#[tauri::command]
fn cancel_manual_save(token: String, editor: State<'_, Mutex<WorldEditor>>) -> Result<(), String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .cancel_manual_save(&token);
    Ok(())
}
#[tauri::command]
fn load_world(
    app: tauri::AppHandle,
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .load(&save_path(&app)?)
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
            add_station,
            build_alignment_from_reference,
            build_alignment,
            build_alignment_with_stations,
            build_plan,
            update_station_placement,
            place_station_on_alignment,
            rename_station,
            delete_station,
            delete_alignment,
            delete_alignment_section,
            undo_alignment_deletion,
            rename_company,
            save_planned_route,
            set_service_route_binding,
            save_world,
            confirm_manual_save,
            cancel_manual_save,
            autosave_world,
            autosave_status,
            allow_autosave_replacement,
            load_autosave,
            load_world
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Rosenfu");
}
