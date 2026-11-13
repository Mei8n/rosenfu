//! Frozen schema 6 document, validated before upgrading alignment kinds.
use super::legacy_alignment::LegacyAlignment;
use rosenfu_core::world::{
    AlignmentProvenance, Company, OperatingService, Player, Route, RoutePath, ServicePath,
    SourceRecord, Station, StationPlacement, Track,
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
    service_paths: Vec<ServicePath>,
    source_records: Vec<SourceRecord>,
    alignment_provenance: Vec<AlignmentProvenance>,
    #[serde(default)]
    alignments: Vec<LegacyAlignment>,
    #[serde(default)]
    station_placements: Vec<StationPlacement>,
}
pub(super) fn migrate(value: serde_json::Value) -> Result<serde_json::Value, serde_json::Error> {
    let old: Document = serde_json::from_value(value)?;
    let mut current = serde_json::to_value(old)?;
    current["schema_version"] = serde_json::json!(7);
    for alignment in current["world"]["alignments"]
        .as_array_mut()
        .expect("serialized array")
    {
        alignment["track_type"] = serde_json::json!("normal");
    }
    Ok(current)
}
