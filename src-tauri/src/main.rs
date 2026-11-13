#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod session;
use rosenfu_core::world::{
    AlignmentId, GeoPoint, RouteId, ServiceId, StationId, TrackId, WorldDraft,
};
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
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .build_alignment_from_reference_typed(name, points, source_record, provenance, track_type)
}
#[tauri::command]
fn build_alignment(
    editor: State<'_, Mutex<WorldEditor>>,
    name: String,
    points: Vec<GeoPoint>,
    track_type: rosenfu_core::world::TrackType,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .build_alignment_typed(name, points, track_type)
}
#[tauri::command]
fn build_alignment_with_stations(
    editor: State<'_, Mutex<WorldEditor>>,
    name: String,
    points: Vec<GeoPoint>,
    track_type: rosenfu_core::world::TrackType,
    stations: Vec<session::PlannedStation>,
    reference: Option<session::PlannedAlignmentReference>,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .build_alignment_with_stations(name, points, track_type, stations, reference)
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
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .delete_station(station_id)
}
#[tauri::command]
fn delete_track(
    editor: State<'_, Mutex<WorldEditor>>,
    track_id: TrackId,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .delete_track(track_id)
}
#[tauri::command]
fn add_track(
    editor: State<'_, Mutex<WorldEditor>>,
    from: StationId,
    to: StationId,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .add_track(from, to)
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
fn save_route(
    editor: State<'_, Mutex<WorldEditor>>,
    route_id: Option<RouteId>,
    name: String,
    start: StationId,
    tracks: Vec<TrackId>,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .save_route(route_id, name, start, tracks)
}
#[tauri::command]
fn save_service(
    editor: State<'_, Mutex<WorldEditor>>,
    service_id: Option<ServiceId>,
    name: String,
    start: StationId,
    tracks: Vec<TrackId>,
    stops: Vec<u32>,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .save_service(service_id, name, start, tracks, stops)
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
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .create_service_path(name, start, segments, stops)
}
#[tauri::command]
fn preview_service_from_route(
    editor: State<'_, Mutex<WorldEditor>>,
    route_id: RouteId,
    from_occurrence: u32,
    to_occurrence: u32,
) -> Result<rosenfu_core::world::ServiceRoutePreview, String> {
    editor
        .lock()
        .map_err(|_| "編集状態を取得できません。".to_string())?
        .preview_service_from_route(route_id, from_occurrence, to_occurrence)
}
#[tauri::command]
fn preview_route_candidates(
    editor: State<'_, Mutex<WorldEditor>>,
    start: StationId,
    end: StationId,
) -> Result<rosenfu_core::world::RouteCandidates, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .preview_route_candidates(start, end)
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
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<SaveResult, String> {
    let path = autosave_path(&app)?;
    std::fs::create_dir_all(path.parent().ok_or("invalid autosave path")?)
        .map_err(|e| e.to_string())?;
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .autosave(&path)?;
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
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<SaveResult, String> {
    let path = save_path(&app)?;
    std::fs::create_dir_all(path.parent().ok_or("invalid save path")?)
        .map_err(|e| e.to_string())?;
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .save(&path)?;
    Ok(SaveResult { saved: true })
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
            preview_service_from_route,
            preview_route_candidates,
            preview_route_membership,
            world_snapshot,
            add_station,
            build_alignment_from_reference,
            build_alignment,
            build_alignment_with_stations,
            update_station_placement,
            place_station_on_alignment,
            add_track,
            rename_station,
            delete_station,
            delete_track,
            rename_company,
            save_route,
            save_planned_route,
            save_service,
            save_world,
            autosave_world,
            autosave_status,
            allow_autosave_replacement,
            load_autosave,
            load_world
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Rosenfu");
}
