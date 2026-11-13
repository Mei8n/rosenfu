//! Frozen provenance shape for schema 6/7. Source snapshots remain unchanged.
use rosenfu_core::world::{AlignmentId, AlignmentProvenanceInput};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LegacyAlignmentProvenance {
    pub alignment_id: AlignmentId,
    pub source: AlignmentProvenanceInput,
}
pub(super) fn migrate(
    mut value: serde_json::Value,
) -> Result<serde_json::Value, serde_json::Error> {
    use serde::de::Error;
    let old: Vec<LegacyAlignmentProvenance> =
        serde_json::from_value(value["world"]["alignment_provenance"].clone())?;
    // Schema 7 allowed omitted alignment arrays (but not explicit null).
    let empty = Vec::new();
    let alignments = value["world"]
        .get("alignments")
        .map_or(Some(&empty), |v| v.as_array())
        .ok_or_else(|| serde_json::Error::custom("missing alignments"))?;
    let mut upgraded = Vec::new();
    for p in old {
        let id = serde_json::to_value(p.alignment_id)?;
        let len = alignments
            .iter()
            .find(|a| a["id"] == id)
            .and_then(|a| a["points"].as_array())
            .map(Vec::len)
            .filter(|len| *len >= 2)
            .ok_or_else(|| serde_json::Error::custom("missing provenance alignment"))?;
        let mut entry = serde_json::to_value(p)?;
        entry["start_point_index"] = serde_json::json!(0);
        entry["end_point_index"] = serde_json::json!(len - 1);
        upgraded.push(entry);
    }
    value["world"]["alignment_provenance"] = serde_json::Value::Array(upgraded);
    value["schema_version"] = serde_json::json!(8);
    Ok(value)
}
