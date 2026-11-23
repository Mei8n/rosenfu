use crate::session::{
    FixedOdRoadTransfer, ObservedFixedOdInput, OperationTickView, OperationView, WorldEditor,
};
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
pub fn preview_crossing_planning(
    editor: State<'_, Mutex<WorldEditor>>,
    expected_configuration_token: String,
    start_ms: String,
) -> Result<crate::session::CrossingPlanningView, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .preview_crossing_planning(&expected_configuration_token, &start_ms)
}
#[tauri::command]
pub fn standard_schedule_status(
    editor: State<'_, Mutex<WorldEditor>>,
    service: ServiceId,
    revision: String,
) -> Result<serde_json::Value, String> {
    #[cfg(feature = "standard-periodic-rail")]
    {
        let status = editor
            .lock()
            .map_err(|_| "world state unavailable")?
            .standard_schedule_status(service, &revision)?;
        serde_json::to_value(status).map_err(|_| "時間帯ダイヤを読み取れませんでした。".into())
    }
    #[cfg(not(feature = "standard-periodic-rail"))]
    {
        let _ = (editor, service, revision);
        Err("このビルドでは時間帯ダイヤを利用できません。".into())
    }
}

#[tauri::command]
pub fn apply_standard_schedule(
    editor: State<'_, Mutex<WorldEditor>>,
    service: ServiceId,
    expected: String,
    base_minutes: u16,
    overrides: serde_json::Value,
    boundaries_minutes: [u16; 6],
) -> Result<OperationView, String> {
    #[cfg(feature = "standard-periodic-rail")]
    {
        let overrides = serde_json::from_value(overrides)
            .map_err(|_| "時間帯ごとの間隔を確認してください。")?;
        editor
            .lock()
            .map_err(|_| "world state unavailable")?
            .apply_standard_schedule(
                service,
                &expected,
                base_minutes,
                overrides,
                boundaries_minutes,
            )
    }
    #[cfg(not(feature = "standard-periodic-rail"))]
    {
        let _ = (
            editor,
            service,
            expected,
            base_minutes,
            overrides,
            boundaries_minutes,
        );
        Err("このビルドでは時間帯ダイヤを利用できません。".into())
    }
}

#[tauri::command]
pub fn preview_standard_schedule_od(
    editor: State<'_, Mutex<WorldEditor>>,
    service: ServiceId,
    revision: String,
    query: serde_json::Value,
) -> Result<serde_json::Value, String> {
    #[cfg(feature = "standard-periodic-rail")]
    {
        let query =
            serde_json::from_value(query).map_err(|_| "時間指定の固定ODを確認してください。")?;
        let result = editor
            .lock()
            .map_err(|_| "world state unavailable")?
            .preview_standard_schedule_od(service, &revision, query)?;
        serde_json::to_value(result).map_err(|_| "時間指定の固定ODを読み取れませんでした。".into())
    }
    #[cfg(not(feature = "standard-periodic-rail"))]
    {
        let _ = (editor, service, revision, query);
        Err("このビルドでは時間帯ダイヤを利用できません。".into())
    }
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
pub fn preview_nominal_trainsets(
    editor: State<'_, Mutex<WorldEditor>>,
    service_id: ServiceId,
    configuration_token: String,
    request: rosenfu_core::operation::service_planning::FleetPlanningRequest,
) -> Result<rosenfu_core::operation::service_planning::NominalFleetPlanning, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .preview_nominal_trainsets(service_id, &configuration_token, request)
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
pub fn advance_operation(
    editor: State<'_, Mutex<WorldEditor>>,
    known_configuration_token: Option<String>,
) -> Result<OperationTickView, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .advance_operation_with_configuration(known_configuration_token.as_deref())
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
pub fn preview_observed_fixed_od(
    editor: State<'_, Mutex<WorldEditor>>,
    query: ObservedFixedOdInput,
) -> Result<rosenfu_core::demand::ObservedFixedOd, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .preview_observed_fixed_od(
            query.origin_station_id,
            query.destination_station_id,
            query.trips,
            query.alternative_seconds,
        )
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
#[allow(clippy::too_many_arguments)]
pub fn configure_bus_operation(
    editor: State<'_, Mutex<WorldEditor>>,
    route_id: String,
    timetable: Timetable,
    vehicle: Vehicle,
    assumed_speed_kmh: f64,
    dwell_ms: u64,
    turnaround_ms: Option<u64>,
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
            turnaround_ms,
            &expected_operation_token,
        )
}
