#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[tauri::command]
fn engine_info() -> rosenfu_core::EngineInfo {
    rosenfu_core::engine_info()
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![engine_info])
        .run(tauri::generate_context!())
        .expect("failed to run Rosenfu");
}
