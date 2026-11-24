//! One complete save envelope, not sidecar files. Existing storage transactions
//! atomically publish the document only after core revalidation and byte limits.
use crate::StorageError;
use base64::{display::Base64Display, engine::general_purpose::STANDARD, Engine};
use rosenfu_core::ofm_evidence::{MAX_REQUEST_BYTES, MAX_TILES};
use rosenfu_core::{
    ofm_evidence::{
        EvidenceStore, RawEvidence, TileBinding, MAX_PINNED_BINDINGS, MAX_PINNED_RAW_BYTES,
        MAX_TILE_BYTES,
    },
    world::WorldError,
};
use serde::{ser::SerializeSeq, Deserialize, Serialize, Serializer};
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
/// Borrow the immutable proof store. Serialize standard base64 via serde_json's
/// string formatter instead of building a copy of every encoded raw blob.
#[derive(Serialize)]
pub(super) struct BorrowedEvidenceEnvelope<'a> {
    raw_blobs: BorrowedRawRows<'a>,
    bindings: BorrowedBindingRows<'a>,
}
impl<'a> BorrowedEvidenceEnvelope<'a> {
    pub(super) fn from_store(store: &'a EvidenceStore) -> Self {
        Self {
            raw_blobs: BorrowedRawRows(store),
            bindings: BorrowedBindingRows(store),
        }
    }
}
struct BorrowedRawRows<'a>(&'a EvidenceStore);
struct BorrowedBindingRows<'a>(&'a EvidenceStore);
struct BorrowedBase64<'a>(&'a [u8]);
#[derive(Serialize)]
struct BorrowedBlob<'a> {
    sha256: &'a str,
    byte_length: usize,
    base64: BorrowedBase64<'a>,
}
impl Serialize for BorrowedBase64<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&Base64Display::new(self.0, &STANDARD))
    }
}
impl Serialize for BorrowedRawRows<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.blobs().count()))?;
        for (sha256, raw) in self.0.blobs() {
            sequence.serialize_element(&BorrowedBlob {
                sha256,
                byte_length: raw.len(),
                base64: BorrowedBase64(raw),
            })?;
        }
        sequence.end()
    }
}
impl Serialize for BorrowedBindingRows<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.bindings().count()))?;
        for binding in self.0.bindings() {
            sequence.serialize_element(binding)?;
        }
        sequence.end()
    }
}
fn invalid() -> StorageError {
    WorldError::InvalidRoadEvidence.into()
}
impl EvidenceEnvelope {
    pub(super) fn restore(self, legacy_limits: bool) -> Result<EvidenceStore, StorageError> {
        let (count, budget) = if legacy_limits {
            (MAX_TILES, MAX_REQUEST_BYTES)
        } else {
            (MAX_PINNED_BINDINGS, MAX_PINNED_RAW_BYTES)
        };
        if self.raw_blobs.len() > count || self.bindings.len() > count {
            return Err(invalid());
        }
        let mut total = 0usize;
        // Preflight every declared length before any decoded raw allocation.
        for b in &self.raw_blobs {
            total = total.checked_add(b.byte_length).ok_or_else(invalid)?;
            if b.byte_length == 0
                || b.byte_length > MAX_TILE_BYTES
                || total > budget
                || b.base64.len() != 4 * b.byte_length.div_ceil(3)
            {
                return Err(invalid());
            }
        }
        let raw = self
            .raw_blobs
            .into_iter()
            .map(|b| {
                // STANDARD rejects noncanonical padding, spare bits, and
                // non-alphabet characters. Do not allocate an encoded copy
                // solely to compare it with the input.
                let bytes = STANDARD.decode(&b.base64).map_err(|_| invalid())?;
                if bytes.len() != b.byte_length {
                    return Err(invalid());
                }
                Ok(RawEvidence {
                    sha256: b.sha256,
                    bytes,
                })
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        if legacy_limits {
            EvidenceStore::from_parts(raw, self.bindings)
        } else {
            EvidenceStore::from_archive_parts(raw, self.bindings)
        }
        .map_err(|_| invalid())
    }
}

#[cfg(test)]
mod streaming_tests {
    use super::*;

    #[test]
    fn split_base64_formatting_is_identical_to_owned_evidence() {
        // Minimal valid transportation layer. Additional protobuf fields keep
        // output large enough to span separate Base64Display chunks.
        let mut raw = vec![26, 21, 10, 14];
        raw.extend(b"transportation");
        raw.extend([120, 2, 40, 128, 32]);
        for _ in 0..4096 {
            raw.extend([8, 0]);
        }
        let sha = rosenfu_core::ofm_evidence::raw_sha256(&raw);
        let binding = TileBinding {
            canonical_url: "https://tiles.openfreemap.org/planet/test/14/1/2.pbf".into(),
            revision: "test".into(),
            z: 14,
            x: 1,
            y: 2,
            raw_sha256: sha.clone(),
            decoder_rule: rosenfu_core::ofm_evidence::DECODER_RULE.into(),
        };
        let store = EvidenceStore::from_parts(
            vec![RawEvidence {
                sha256: sha.clone(),
                bytes: raw.clone(),
            }],
            vec![binding.clone()],
        )
        .unwrap();
        let old = EvidenceEnvelope {
            raw_blobs: vec![Blob {
                sha256: sha,
                byte_length: raw.len(),
                base64: STANDARD.encode(&raw),
            }],
            bindings: vec![binding],
        };
        assert_eq!(
            serde_json::to_vec_pretty(&BorrowedEvidenceEnvelope::from_store(&store)).unwrap(),
            serde_json::to_vec_pretty(&old).unwrap(),
        );
    }
}
