//! Frozen top-level schema 2/3 shape; rejects schema 4 sidecars in old files.
use super::legacy_alignment::LegacyAlignment;
use rosenfu_core::world::{
    Company, OperatingService, Player, Route, Station, StationPlacement, Track,
};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    format: String,
    schema_version: u64,
    world: LegacyWorld,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyWorld {
    next_id: String,
    players: Vec<Player>,
    companies: Vec<Company>,
    stations: Vec<Station>,
    tracks: Vec<Track>,
    routes: Vec<Route>,
    services: Vec<OperatingService>,
    #[serde(default)]
    alignments: Vec<LegacyAlignment>,
    #[serde(default)]
    station_placements: Vec<StationPlacement>,
}
pub(super) fn migrate(value: serde_json::Value) -> Result<serde_json::Value, serde_json::Error> {
    let old: Document = serde_json::from_value(value)?;
    let mut current = serde_json::to_value(old)?;
    current["schema_version"] = serde_json::json!(4);
    current["world"]["route_paths"] = serde_json::json!([]);
    Ok(current)
}
