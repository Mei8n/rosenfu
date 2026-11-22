use crate::session::{FixedOdRoadTransfer, OperationView, WorldEditor};
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
    expected_operation_token: String,
) -> Result<OperationView, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .configure_operation_service(
            service_id,
            timetable,
            vehicle,
            dwell_ms,
            &expected_operation_token,
        )
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

#[tauri::command]
pub fn fixed_od_scenario(
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<crate::session::FixedOdScenarioView, String> {
    Ok(editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .fixed_od_scenario())
}

#[tauri::command]
pub fn set_fixed_od_scenario(
    editor: State<'_, Mutex<WorldEditor>>,
    expected_token: String,
    scenario: Option<rosenfu_core::world::FixedOdScenario>,
) -> Result<crate::session::FixedOdScenarioView, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .set_fixed_od_scenario(&expected_token, scenario)
}

#[tauri::command]
pub fn preview_rail_journey_replay(
    editor: State<'_, Mutex<WorldEditor>>,
    origin: String,
    destination: String,
) -> Result<rosenfu_core::operation::ReplayItinerary, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .preview_rail_journey_replay(&origin, &destination)
}

#[tauri::command]
pub fn preview_fixed_od(
    editor: State<'_, Mutex<WorldEditor>>,
    origin_station_id: String,
    destination_station_id: String,
    trips: Option<f64>,
    alternative_seconds: Option<f64>,
    transfer: Option<FixedOdRoadTransfer>,
) -> Result<rosenfu_core::demand::OdAssignment, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .preview_fixed_od_with_transfer(
            origin_station_id,
            destination_station_id,
            trips,
            alternative_seconds,
            transfer,
        )
}

#[tauri::command]
pub fn configure_bus_operation(
    editor: State<'_, Mutex<WorldEditor>>,
    route_id: String,
    timetable: Timetable,
    vehicle: Vehicle,
    assumed_speed_kmh: f64,
    dwell_ms: u64,
    expected_operation_token: String,
) -> Result<OperationView, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .configure_bus_operation(
            route_id,
            timetable,
            vehicle,
            assumed_speed_kmh,
            dwell_ms,
            &expected_operation_token,
        )
}
