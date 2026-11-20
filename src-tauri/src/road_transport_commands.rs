use crate::session::WorldEditor;
use rosenfu_core::{road_transport::RoadTransport, world::WorldDraft};
use std::sync::Mutex;
use tauri::State;
#[tauri::command]
pub fn replace_road_transport(
    editor: State<'_, Mutex<WorldEditor>>,
    expected_world_token: String,
    expected: RoadTransport,
    replacement: RoadTransport,
) -> Result<WorldDraft, String> {
    let mut editor = editor.lock().map_err(|_| "world state unavailable")?;
    if editor.ofm_world_token()? != expected_world_token {
        return Err("ワールドが変更されています。".into());
    }
    editor.replace_road_transport(expected, replacement)
}
#[tauri::command]
pub fn enable_road_qa_fixture(
    editor: State<'_, Mutex<WorldEditor>>,
    expected_world_token: String,
    confirmed_synthetic: bool,
) -> Result<WorldDraft, String> {
    let mut editor = editor.lock().map_err(|_| "world state unavailable")?;
    if editor.ofm_world_token()? != expected_world_token {
        return Err("ワールドが変更されています。".into());
    }
    editor.enable_road_qa_fixture(confirmed_synthetic)
}
#[tauri::command]
pub fn find_road_path(
    editor: State<'_, Mutex<WorldEditor>>,
    from: String,
    to: String,
    tram: bool,
) -> Result<Vec<String>, String> {
    let world = editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .snapshot();
    world
        .road_transport
        .shortest_path(&world, &from, &to, tram)
        .map_err(|e| format!("道路経路を確定できません: {e:?}"))
}
#[tauri::command]
pub fn edit_road_tram(
    editor: State<'_, Mutex<WorldEditor>>,
    expected_world_token: String,
    edit: crate::session::RoadTramEdit,
) -> Result<WorldDraft, String> {
    let mut editor = editor.lock().map_err(|_| "world state unavailable")?;
    if editor.ofm_world_token()? != expected_world_token {
        return Err("ワールドが変更されています。".into());
    }
    editor.edit_road_tram(edit)
}
