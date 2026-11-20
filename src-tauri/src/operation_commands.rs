use crate::session::{OperationView, WorldEditor};
use rosenfu_core::{
    operation::{Timetable, Vehicle},
    world::ServiceId,
};
use std::sync::Mutex;
use tauri::State;
#[tauri::command]
pub fn operation_snapshot(editor: State<'_, Mutex<WorldEditor>>) -> Result<OperationView, String> {
    Ok(editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .operation_snapshot())
}
#[tauri::command]
pub fn configure_operation_service(
    editor: State<'_, Mutex<WorldEditor>>,
    service_id: ServiceId,
    timetable: Timetable,
    vehicle: Vehicle,
    dwell_ms: u64,
) -> Result<OperationView, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .configure_operation_service(service_id, timetable, vehicle, dwell_ms)
}
#[tauri::command]
pub fn set_operation_clock(
    editor: State<'_, Mutex<WorldEditor>>,
    paused: bool,
    speed: u32,
) -> Result<OperationView, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .set_operation_clock(paused, speed)
}
#[tauri::command]
pub fn advance_operation(editor: State<'_, Mutex<WorldEditor>>) -> Result<OperationView, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .advance_operation()
}
#[tauri::command]
pub fn reset_operation(editor: State<'_, Mutex<WorldEditor>>) -> Result<OperationView, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .reset_operation()
}
