//! In-memory immutable evidence table. Persistence must reconstruct this through
//! `from_parts`; neither descriptors nor caller-supplied decoded objects are proof.
use super::{
    decode_request, raw_sha256, DecodedTile, Error, EvidenceInput, Result, TileBinding,
    MAX_REQUEST_BYTES, MAX_TILES, MAX_TILE_BYTES,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// Untrusted transport input. Base64 and document-size checks belong to storage.
#[derive(Debug)]
pub struct RawEvidence {
    pub sha256: String,
    pub bytes: Vec<u8>,
}
/// Small output metadata, without raw blobs or decoded geometry.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceDescriptor {
    pub binding: TileBinding,
    pub byte_length: usize,
    /// Computed by Rust from raw evidence, never accepted as constructor input.
    pub canonical_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct DecodeKey {
    raw_sha256: String,
    revision: String,
    z: u8,
    x: u32,
    y: u32,
    decoder_rule: String,
}
impl From<&TileBinding> for DecodeKey {
    fn from(b: &TileBinding) -> Self {
        Self {
            raw_sha256: b.raw_sha256.clone(),
            revision: b.revision.clone(),
            z: b.z,
            x: b.x,
            y: b.y,
            decoder_rule: b.decoder_rule.clone(),
        }
    }
}
#[derive(Debug, Default)]
struct State {
    blobs: BTreeMap<String, Arc<[u8]>>,
    bindings: BTreeMap<String, TileBinding>,
    decoded: BTreeMap<DecodeKey, Arc<DecodedTile>>,
}
/// Exact accounting of validated evidence, not a capacity promise or a heap estimate.
/// Per-binding bytes retain the decoder work charge even when raw blobs are shared.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EvidenceUsage {
    pub binding_count: usize,
    pub unique_blob_count: usize,
    pub unique_raw_bytes: usize,
    pub binding_raw_bytes: usize,
    /// Base64 payload bytes only; excludes JSON metadata and the rest of the world.
    pub base64_payload_bytes: usize,
}
/// Cheap clones share immutable raw and decoded data. No Deserialize implementation:
/// reloading arbitrary DTOs must go through `from_parts` and the bounded decoder.
#[derive(Debug, Clone, Default)]
pub struct EvidenceStore {
    state: Arc<State>,
}
impl EvidenceStore {
    /// Empty evidence cannot attest to any tile or coverage.
    pub fn empty() -> Self {
        Self::default()
    }
    /// Validates all supplied bindings and raw bytes before publishing any state.
    pub fn from_parts(raw: Vec<RawEvidence>, bindings: Vec<TileBinding>) -> Result<Self> {
        Self::empty().with_additions(raw, bindings)
    }
    /// Returns a new store, never mutating self, including on conflict or decode failure.
    /// Reused inputs count toward the complete candidate's decoder request budget.
    /// The initial store capacity is deliberately <=16 bindings and <=8 MiB including
    /// repeated blob use; larger persisted-world stores need a separately reviewed API.
    pub fn with_additions(
        &self,
        raw: Vec<RawEvidence>,
        bindings: Vec<TileBinding>,
    ) -> Result<Self> {
        if raw.len() > MAX_TILES || bindings.len() > MAX_TILES {
            return Err(Error::Limit);
        }
        let mut submitted = 0usize;
        for r in &raw {
            submitted = submitted.checked_add(r.bytes.len()).ok_or(Error::Limit)?;
            if r.bytes.len() > MAX_TILE_BYTES || submitted > MAX_REQUEST_BYTES {
                return Err(Error::Limit);
            }
            if raw_sha256(&r.bytes) != r.sha256 {
                return Err(Error::Hash);
            }
        }
        let mut blobs = self.state.blobs.clone();
        for r in raw {
            if let Some(old) = blobs.get(&r.sha256) {
                if old.as_ref() != r.bytes.as_slice() {
                    return Err(Error::Hash);
                }
            } else {
                blobs.insert(r.sha256, Arc::from(r.bytes));
            }
        }
        let mut candidate = self.state.bindings.clone();
        for binding in bindings {
            binding.validate()?;
            if let Some(old) = candidate.get(&binding.canonical_url) {
                if old != &binding {
                    return Err(Error::Binding);
                }
            } else {
                candidate.insert(binding.canonical_url.clone(), binding);
            }
        }
        if candidate.len() > MAX_TILES || blobs.len() > MAX_TILES {
            return Err(Error::Limit);
        }
        let mut used = BTreeSet::new();
        let mut inputs = Vec::with_capacity(candidate.len());
        for binding in candidate.values() {
            let raw = blobs.get(&binding.raw_sha256).ok_or(Error::Hash)?;
            used.insert(binding.raw_sha256.as_str());
            inputs.push(EvidenceInput { binding, raw });
        }
        // Reject orphan material rather than accumulating invisible payloads.
        if used.len() != blobs.len() {
            return Err(Error::Binding);
        }
        if inputs.is_empty() {
            return Ok(Self::empty());
        }
        // Mandatory whole-context validation, also for reused/cached evidence.
        let validated = decode_request(&inputs)?;
        let mut decoded = BTreeMap::new();
        for value in validated {
            let key = DecodeKey::from(&value.binding);
            let shared = match self.state.decoded.get(&key) {
                Some(old) if old.as_ref() == &value => Arc::clone(old),
                _ => Arc::new(value),
            };
            decoded.insert(key, shared);
        }
        Ok(Self {
            state: Arc::new(State {
                blobs,
                bindings: candidate,
                decoded,
            }),
        })
    }
    /// Read-only whole-tile view of an already validated immutable store. This
    /// accepts keys only, never external bytes or DTOs, and shares validated Arcs.
    /// It does not infer acquisition-scope completeness or allow road filtering.
    pub fn scope(&self, keys: &[String]) -> Result<Self> {
        if keys.is_empty() || keys.len() > MAX_TILES {
            return Err(Error::Limit);
        }
        let mut state = State::default();
        for key in keys {
            let binding = self.state.bindings.get(key).ok_or(Error::Binding)?;
            if state
                .bindings
                .insert(key.clone(), binding.clone())
                .is_some()
            {
                return Err(Error::Binding);
            }
            let raw = self
                .state
                .blobs
                .get(&binding.raw_sha256)
                .ok_or(Error::Hash)?;
            state
                .blobs
                .insert(binding.raw_sha256.clone(), Arc::clone(raw));
            let cache_key = DecodeKey::from(binding);
            let decoded = self.state.decoded.get(&cache_key).ok_or(Error::Binding)?;
            state.decoded.insert(cache_key, Arc::clone(decoded));
        }
        Ok(Self {
            state: Arc::new(state),
        })
    }
    /// Read-only accounting without decoding again or allocating encoded blobs.
    pub fn usage(&self) -> EvidenceUsage {
        EvidenceUsage {
            binding_count: self.state.bindings.len(),
            unique_blob_count: self.state.blobs.len(),
            unique_raw_bytes: self.state.blobs.values().map(|raw| raw.len()).sum(),
            binding_raw_bytes: self
                .state
                .bindings
                .values()
                .map(|binding| self.state.blobs[&binding.raw_sha256].len())
                .sum(),
            base64_payload_bytes: self
                .state
                .blobs
                .values()
                .map(|raw| raw.len().div_ceil(3) * 4)
                .sum(),
        }
    }
    pub fn descriptors(&self) -> Vec<EvidenceDescriptor> {
        self.state
            .bindings
            .values()
            .map(|binding| {
                // Internal state is built only by the validated constructor above.
                let decoded = &self.state.decoded[&DecodeKey::from(binding)];
                EvidenceDescriptor {
                    binding: binding.clone(),
                    byte_length: self.state.blobs[&binding.raw_sha256].len(),
                    canonical_sha256: decoded.canonical_sha256.clone(),
                }
            })
            .collect()
    }
    /// Resolve a required binding; absence is an error, not an empty road context.
    pub fn decoded(&self, canonical_url: &str) -> Result<&DecodedTile> {
        let b = self
            .state
            .bindings
            .get(canonical_url)
            .ok_or(Error::Binding)?;
        self.state
            .decoded
            .get(&DecodeKey::from(b))
            .map(AsRef::as_ref)
            .ok_or(Error::Binding)
    }
    pub fn raw(&self, sha256: &str) -> Result<&[u8]> {
        self.state
            .blobs
            .get(sha256)
            .map(AsRef::as_ref)
            .ok_or(Error::Hash)
    }
    /// Read-only export for a future bounded, atomic save envelope.
    pub fn blobs(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.state
            .blobs
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_ref()))
    }
    pub fn bindings(&self) -> impl Iterator<Item = &TileBinding> {
        self.state.bindings.values()
    }
}

// Pointer fast path for ordinary snapshots; content equality after independently
// decoded reloads. Compare raw bytes too, never treating claimed hashes as equality.
impl PartialEq for EvidenceStore {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.state, &other.state)
            || (self.state.bindings == other.state.bindings
                && self.state.blobs == other.state.blobs
                && self.state.decoded == other.state.decoded)
    }
}
impl Eq for EvidenceStore {}
