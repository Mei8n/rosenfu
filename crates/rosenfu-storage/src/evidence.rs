//! One complete save envelope, not sidecar files. Existing storage transactions
//! atomically publish the document only after core revalidation and byte limits.
use crate::StorageError;
use base64::{engine::general_purpose::STANDARD, Engine};
use rosenfu_core::{
    ofm_evidence::{
        EvidenceStore, RawEvidence, TileBinding, MAX_REQUEST_BYTES, MAX_TILES, MAX_TILE_BYTES,
    },
    world::WorldError,
};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EvidenceEnvelope {
    raw_blobs: Vec<Blob>,
    bindings: Vec<TileBinding>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Blob {
    sha256: String,
    byte_length: usize,
    base64: String,
}
fn invalid() -> StorageError {
    WorldError::InvalidRoadEvidence.into()
}
impl EvidenceEnvelope {
    pub(super) fn from_store(store: &EvidenceStore) -> Self {
        Self {
            raw_blobs: store
                .blobs()
                .map(|(sha256, raw)| Blob {
                    sha256: sha256.into(),
                    byte_length: raw.len(),
                    base64: STANDARD.encode(raw),
                })
                .collect(),
            bindings: store.bindings().cloned().collect(),
        }
    }
    pub(super) fn restore(self) -> Result<EvidenceStore, StorageError> {
        if self.raw_blobs.len() > MAX_TILES || self.bindings.len() > MAX_TILES {
            return Err(invalid());
        }
        let mut total = 0usize;
        // Preflight every declared length before any decoded raw allocation.
        for b in &self.raw_blobs {
            total = total.checked_add(b.byte_length).ok_or_else(invalid)?;
            if b.byte_length == 0
                || b.byte_length > MAX_TILE_BYTES
                || total > MAX_REQUEST_BYTES
                || b.base64.len() != 4 * b.byte_length.div_ceil(3)
            {
                return Err(invalid());
            }
        }
        let raw = self
            .raw_blobs
            .into_iter()
            .map(|b| {
                let bytes = STANDARD.decode(&b.base64).map_err(|_| invalid())?;
                if bytes.len() != b.byte_length || STANDARD.encode(&bytes) != b.base64 {
                    return Err(invalid());
                }
                Ok(RawEvidence {
                    sha256: b.sha256,
                    bytes,
                })
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        EvidenceStore::from_parts(raw, self.bindings).map_err(|_| invalid())
    }
}
