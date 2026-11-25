use crate::session::WorldEditor;
use rosenfu_core::{
    road_transport::{PilotRoadMode, RoadTransport},
    world::WorldDraft,
};
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
        .native_shortest_path(
            &world,
            &from,
            &to,
            if tram {
                PilotRoadMode::Tram
            } else {
                PilotRoadMode::Bus
            },
        )
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

/// Select a release-approved asset by stable ID; never accept local paths or bytes.
#[tauri::command]
pub fn install_native_road_pilot(
    editor: State<'_, Mutex<WorldEditor>>,
    expected_world_token: String,
    asset_id: String,
) -> Result<WorldDraft, String> {
    let mut editor = editor.lock().map_err(|_| "world state unavailable")?;
    if editor.ofm_world_token()? != expected_world_token {
        return Err("ワールドが変更されています。".into());
    }
    editor.install_native_road_pilot(&asset_id)
}

#[tauri::command]
pub fn list_native_road_regions(
) -> Result<Vec<rosenfu_core::road_transport::NativeRoadRegion>, String> {
    rosenfu_core::road_transport::native_road_regions()
        .map_err(|error| format!("道路データの確認に失敗しました: {error:?}"))
}

#[tauri::command]
pub fn edit_native_pilot_bus(
    editor: State<'_, Mutex<WorldEditor>>,
    expected_world_token: String,
    edit: rosenfu_core::road_transport::PilotBusEdit,
) -> Result<WorldDraft, String> {
    let mut editor = editor.lock().map_err(|_| "world state unavailable")?;
    if editor.ofm_world_token()? != expected_world_token {
        return Err("ワールドが変更されています。".into());
    }
    editor.edit_native_pilot_bus(edit)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CachedOfmBusTile {
    raw_evidence: crate::session::OfmRawInput,
    tile_binding: crate::session::OfmBindingInput,
}

fn cached_tiles(
    raw_evidence: Option<crate::session::OfmRawInput>,
    tile_binding: Option<crate::session::OfmBindingInput>,
    context_tiles: Option<Vec<CachedOfmBusTile>>,
) -> Result<Vec<(crate::session::OfmRawInput, crate::session::OfmBindingInput)>, String> {
    let tiles = context_tiles.unwrap_or_default();
    if tiles.len() > rosenfu_core::ofm_evidence::MAX_TILES {
        return Err("道路の参照範囲が広すぎます。".into());
    }
    let mut cached: Vec<_> = tiles
        .into_iter()
        .map(|t| (t.raw_evidence, t.tile_binding))
        .collect();
    match (raw_evidence, tile_binding) {
        (None, None) => {}
        (Some(raw), Some(binding)) => cached.push((raw, binding)),
        _ => return Err("道路のキャッシュ情報が揃っていません。".into()),
    }
    Ok(cached)
}

// Hook up to the typed storage format only after the exact chain save/reopen
// regression has been validated. Never expose a live writer under an old tag.
pub(crate) const TRAM_CHAIN_SAVE_ENABLED: bool = rosenfu_storage::STREET_TRAM_CHAIN_SAVE_ENABLED;

fn authored_tram_proof<'a>(
    guide: Option<&'a rosenfu_core::ofm_evidence::AuthoredTramGuide>,
    chain: Option<&'a rosenfu_core::ofm_evidence::AuthoredTramChainGuide>,
) -> Result<rosenfu_core::ofm_evidence::AuthoredTramProof<'a>, String> {
    match (guide, chain) {
        (Some(guide), None) => Ok(guide.into()),
        (None, Some(chain)) if TRAM_CHAIN_SAVE_ENABLED => Ok(chain.into()),
        (None, Some(_)) => Err("この版では複数境界の併用軌道を保存できません。".into()),
        _ => Err("道路に沿う軌道の元データを一つ指定してください。".into()),
    }
}

/// Geometry for the game track guide, not permission to operate a real tram.
#[tauri::command]
pub async fn preview_authored_tram_road_guide(
    editor: State<'_, Mutex<WorldEditor>>,
    expected_world_token: String,
    guide: Option<rosenfu_core::ofm_evidence::AuthoredTramGuide>,
    chain_guide: Option<rosenfu_core::ofm_evidence::AuthoredTramChainGuide>,
    raw_evidence: Option<crate::session::OfmRawInput>,
    tile_binding: Option<crate::session::OfmBindingInput>,
    context_tiles: Option<Vec<CachedOfmBusTile>>,
) -> Result<Vec<rosenfu_core::world::GeoPoint>, String> {
    let cached = cached_tiles(raw_evidence, tile_binding, context_tiles)?;
    let snapshot = editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .clone();
    tauri::async_runtime::spawn_blocking(move || {
        let proof = authored_tram_proof(guide.as_ref(), chain_guide.as_ref())?;
        snapshot.preview_authored_tram_road_guide(&expected_world_token, proof, cached)
    })
    .await
    .map_err(|_| "道路に沿う軌道の経路を確認できません。".to_owned())?
}

#[tauri::command]
#[allow(clippy::too_many_arguments)] // Keep the existing IPC fields compatible.
pub async fn build_authored_street_tram(
    app: tauri::AppHandle,
    expected_world_token: String,
    guide: Option<rosenfu_core::ofm_evidence::AuthoredTramGuide>,
    chain_guide: Option<rosenfu_core::ofm_evidence::AuthoredTramChainGuide>,
    raw_evidence: Option<crate::session::OfmRawInput>,
    tile_binding: Option<crate::session::OfmBindingInput>,
    context_tiles: Option<Vec<CachedOfmBusTile>>,
    alignment: crate::session::PlannedAlignment,
    roads: Option<Vec<rosenfu_core::world::RoadSnapshot>>,
) -> Result<WorldDraft, String> {
    let cached = cached_tiles(raw_evidence, tile_binding, context_tiles)?;
    tauri::async_runtime::spawn_blocking(move || {
        use tauri::Manager;
        let editor = app.state::<Mutex<WorldEditor>>();
        let mut editor = editor.lock().map_err(|_| "world state unavailable")?;
        let proof = authored_tram_proof(guide.as_ref(), chain_guide.as_ref())?;
        editor.build_authored_street_tram(
            &expected_world_token,
            proof,
            cached,
            alignment,
            roads.unwrap_or_default(),
        )
    })
    .await
    .map_err(|_| "併用軌道を建設できません。".to_owned())?
}

#[tauri::command]
pub async fn preview_gameplay_ofm_bus(
    editor: State<'_, Mutex<WorldEditor>>,
    expected_world_token: String,
    route_id: String,
    name: String,
    path: rosenfu_core::ofm_evidence::BusPathBinding,
    raw_evidence: Option<crate::session::OfmRawInput>,
    tile_binding: Option<crate::session::OfmBindingInput>,
    context_tiles: Option<Vec<CachedOfmBusTile>>,
    request_stop_targets: Option<bool>,
    request_auto_stop_suggestions: Option<bool>,
) -> Result<crate::session::GameplayBusPathPreview, String> {
    let cached = cached_tiles(raw_evidence, tile_binding, context_tiles)?;
    // Clone the immutable draft inputs/cache under the lock only. Geometry
    // search never holds up live edits or runs on the WebView command thread.
    let snapshot = editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .clone();
    tauri::async_runtime::spawn_blocking(move || {
        snapshot.preview_gameplay_ofm_bus_details(
            &expected_world_token,
            route_id,
            name,
            path,
            cached,
            crate::session::GameplayBusPreviewOptions {
                request_stop_targets: request_stop_targets.unwrap_or(false),
                request_auto_stop_suggestions: request_auto_stop_suggestions.unwrap_or(false),
            },
        )
    })
    .await
    .map_err(|_| "バス経路のプレビューを取得できません。".to_owned())?
}

/// Use the normal OFM cache data (or already retained world evidence) to stage
/// and commit an ordinary Bus route. This does not claim legal road permission.
#[tauri::command]
pub fn put_gameplay_ofm_bus(
    editor: State<'_, Mutex<WorldEditor>>,
    expected_world_token: String,
    route_id: String,
    name: String,
    path: rosenfu_core::ofm_evidence::BusPathBinding,
    raw_evidence: Option<crate::session::OfmRawInput>,
    tile_binding: Option<crate::session::OfmBindingInput>,
    context_tiles: Option<Vec<CachedOfmBusTile>>,
) -> Result<WorldDraft, String> {
    let cached = cached_tiles(raw_evidence, tile_binding, context_tiles)?;
    let mut editor = editor.lock().map_err(|_| "world state unavailable")?;
    editor.put_gameplay_ofm_bus(&expected_world_token, route_id, name, path, cached)
}

#[tauri::command]
pub fn installed_seam_bus_options(
    editor: State<'_, Mutex<WorldEditor>>,
) -> Result<crate::session::InstalledSeamBusOptions, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .installed_seam_bus_options()
}

#[tauri::command]
pub fn retain_installed_seam_tiles(
    editor: State<'_, Mutex<WorldEditor>>,
    expected_world_token: String,
    catalog_key: String,
    raw: Vec<crate::session::OfmRawInput>,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .retain_installed_seam_tiles(&expected_world_token, catalog_key, raw)
}

#[tauri::command]
pub fn put_installed_seam_bus(
    editor: State<'_, Mutex<WorldEditor>>,
    expected_world_token: String,
    catalog_key: String,
    name: String,
) -> Result<WorldDraft, String> {
    editor
        .lock()
        .map_err(|_| "world state unavailable")?
        .put_installed_seam_bus(&expected_world_token, catalog_key, name)
}

#[tauri::command]
pub fn remove_gameplay_ofm_bus(
    editor: State<'_, Mutex<WorldEditor>>,
    expected_world_token: String,
    route_id: String,
) -> Result<WorldDraft, String> {
    let mut editor = editor.lock().map_err(|_| "world state unavailable")?;
    editor.remove_gameplay_ofm_bus(&expected_world_token, &route_id)
}
