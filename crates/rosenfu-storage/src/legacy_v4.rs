//! Frozen top-level schema 4 shape; never accept newer sidecars under an old version.
use super::legacy_alignment::LegacyAlignment;
use rosenfu_core::world::{
    Company, OperatingService, Player, Route, RoutePath, Station, StationPlacement, Track,
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
    route_paths: Vec<RoutePath>,
    #[serde(default)]
    alignments: Vec<LegacyAlignment>,
    #[serde(default)]
    station_placements: Vec<StationPlacement>,
}
pub(super) fn migrate(value: serde_json::Value) -> Result<serde_json::Value, serde_json::Error> {
    let old: Document = serde_json::from_value(value)?;
    let mut current = serde_json::to_value(old)?;
    current["schema_version"] = serde_json::json!(5);
    current["world"]["service_paths"] = serde_json::json!([]);
    Ok(current)
}
