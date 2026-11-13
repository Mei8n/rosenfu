//! Frozen pre-schema-7 alignment. New fields must never be accepted under an old header.
use rosenfu_core::world::{AlignmentId, CompanyId, GeoPoint};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LegacyAlignment {
    id: AlignmentId,
    owner: CompanyId,
    name: String,
    points: Vec<GeoPoint>,
}
