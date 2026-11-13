//! Frozen schema-1 wire types. Do not add fields here when the live model changes.
//! Reject unknown legacy fields rather than silently discarding newer data.
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    format: String,
    schema_version: u64,
    world: World,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct World {
    next_id: String,
    players: Vec<Player>,
    companies: Vec<Company>,
    stations: Vec<Station>,
    tracks: Vec<Track>,
    routes: Vec<Route>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Player {
    id: String,
    name: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Company {
    id: String,
    owner: String,
    name: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Station {
    id: String,
    owner: String,
    name: String,
    latitude: f64,
    longitude: f64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Track {
    id: String,
    owner: String,
    from: String,
    to: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Route {
    id: String,
    owner: String,
    name: String,
    start: String,
    tracks: Vec<String>,
}

pub(super) fn migrate(value: serde_json::Value) -> Result<serde_json::Value, serde_json::Error> {
    // Deserialize a frozen, strict legacy schema before adding current fields.
    let old: Document = serde_json::from_value(value)?;
    let mut migrated = serde_json::to_value(old)?;
    migrated["schema_version"] = serde_json::json!(2);
    migrated["world"]["services"] = serde_json::json!([]);
    Ok(migrated)
}
