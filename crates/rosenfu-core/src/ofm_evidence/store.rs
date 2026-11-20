//! In-memory immutable evidence table. Persistence must reconstruct this through
//! validated constructors; neither descriptors nor caller-supplied decoded objects are proof.
use super::{
    decode_request, decode_tile, raw_sha256, validate_request, DecodedTile, Error, EvidenceInput,
    Result, TileBinding, MAX_REQUEST_BYTES, MAX_TILES, MAX_TILE_BYTES,
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{Arc, Mutex},
};

// The retained-world archive is distinct from one bounded transport or path request.
pub const MAX_PINNED_BINDINGS: usize = 256;
pub const MAX_PINNED_RAW_BYTES: usize = 8 * 1024 * 1024;
const MAX_PINNED_BOUND_BYTES: usize = 64 * 1024 * 1024;
const CACHE_WEIGHT_LIMIT: usize = 16 * 1024 * 1024;

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
    canonical: BTreeMap<DecodeKey, String>,
    decoded: BTreeMap<DecodeKey, Arc<DecodedTile>>,
}
/// Best-effort derived tile cache. Independent of the immutable archive: eviction
/// or a contended/unavailable cache can never remove proof or change a result.
#[derive(Debug, Default)]
struct ScopeCache {
    entries: VecDeque<(DecodeKey, Arc<DecodedTile>, usize)>,
    weight: usize,
}
impl ScopeCache {
    fn get(&mut self, key: &DecodeKey) -> Option<Arc<DecodedTile>> {
        let pos = self.entries.iter().position(|entry| &entry.0 == key)?;
        let entry = self.entries.remove(pos)?;
        let tile = Arc::clone(&entry.1);
        self.entries.push_back(entry);
        Some(tile)
    }
    fn insert(&mut self, key: DecodeKey, tile: Arc<DecodedTile>) {
        // Deliberately overcharge inlined line/attribute and map-node storage;
        // separately count heap strings and coordinates. This is a cache weight,
        // not a claim about native process RSS (the decoder also has hard limits).
        let mut weight = 1024usize;
        for line in &tile.lines {
            weight = weight
                .saturating_add(256)
                .saturating_add(line.coordinates.capacity().saturating_mul(8));
            for (name, value) in &line.attributes {
                weight = weight.saturating_add(256).saturating_add(name.capacity());
                if let super::RawValue::String(value) = value {
                    weight = weight.saturating_add(value.capacity());
                }
            }
        }
        if weight > CACHE_WEIGHT_LIMIT || self.get(&key).is_some() {
            return;
        }
        while self.entries.len() >= MAX_TILES
            || self.weight.saturating_add(weight) > CACHE_WEIGHT_LIMIT
        {
            let Some((_, _, evicted)) = self.entries.pop_front() else {
                return;
            };
            self.weight -= evicted;
        }
        self.entries.push_back((key, tile, weight));
        self.weight += weight;
    }
}
/// Cheap clones share immutable raw and decoded data. No Deserialize implementation:
/// reloading arbitrary DTOs must go through `from_parts` and the bounded decoder.
#[derive(Debug, Clone, Default)]
pub struct EvidenceStore {
    state: Arc<State>,
    cache: Arc<Mutex<ScopeCache>>,
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
        self.add(raw, bindings, false, false)
    }
    /// Grow a retained archive by one bounded request; do not keep decoded geometry
    /// for the whole world. Existing immutable proofs remain reachable by key.
    pub(crate) fn with_pinned_additions(
        &self,
        raw: Vec<RawEvidence>,
        bindings: Vec<TileBinding>,
    ) -> Result<Self> {
        self.add(raw, bindings, true, false)
    }
    /// Restore a persisted archive via the same content checks, never trusting digests
    /// from a descriptor or increasing the budget of a subsequent path calculation.
    pub fn from_archive_parts(raw: Vec<RawEvidence>, bindings: Vec<TileBinding>) -> Result<Self> {
        Self::empty().add(raw, bindings, true, true)
    }
    fn add(
        &self,
        raw: Vec<RawEvidence>,
        bindings: Vec<TileBinding>,
        archive: bool,
        restore: bool,
    ) -> Result<Self> {
        let input_count = if restore {
            MAX_PINNED_BINDINGS
        } else {
            MAX_TILES
        };
        let input_bytes = if restore {
            MAX_PINNED_RAW_BYTES
        } else {
            MAX_REQUEST_BYTES
        };
        if raw.len() > input_count || bindings.len() > input_count {
            return Err(Error::Limit);
        }
        let supplied: BTreeSet<_> = bindings.iter().map(|b| b.canonical_url.clone()).collect();
        if archive && supplied.len() != bindings.len() {
            return Err(Error::Binding);
        }
        let mut submitted = 0usize;
        for r in &raw {
            submitted = submitted.checked_add(r.bytes.len()).ok_or(Error::Limit)?;
            if r.bytes.len() > MAX_TILE_BYTES || submitted > input_bytes {
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
        let cap = if archive {
            MAX_PINNED_BINDINGS
        } else {
            MAX_TILES
        };
        if candidate.len() > cap || blobs.len() > cap {
            return Err(Error::Limit);
        }
        if archive {
            let bytes = blobs.values().try_fold(0usize, |sum, raw| {
                sum.checked_add(raw.len()).ok_or(Error::Limit)
            })?;
            if bytes > MAX_PINNED_RAW_BYTES {
                return Err(Error::Limit);
            }
        }

        let mut used = BTreeSet::new();
        let mut inputs = Vec::with_capacity(candidate.len());
        let mut total = 0usize;
        let mut submitted_bound = 0usize;
        let revision = candidate.values().next().map(|b| &b.revision);
        for binding in candidate.values() {
            // Do not mix geography revisions within a world or silently rebind a tile.
            if Some(&binding.revision) != revision {
                return Err(Error::Binding);
            }
            let raw = blobs.get(&binding.raw_sha256).ok_or(Error::Hash)?;
            used.insert(binding.raw_sha256.as_str());
            total = total.checked_add(raw.len()).ok_or(Error::Limit)?;
            if archive && !restore && supplied.contains(&binding.canonical_url) {
                submitted_bound = submitted_bound.checked_add(raw.len()).ok_or(Error::Limit)?;
            }
            inputs.push(EvidenceInput { binding, raw });
        }
        if archive && (total > MAX_PINNED_BOUND_BYTES || submitted_bound > MAX_REQUEST_BYTES) {
            return Err(Error::Limit);
        }
        if used.len() != blobs.len() {
            return Err(Error::Binding);
        }
        if inputs.is_empty() {
            return Ok(Self::empty());
        }

        let mut canonical = BTreeMap::new();
        let mut decoded = BTreeMap::new();
        if archive {
            // The initial admission and every restore validate real bytes. For
            // immutable old bindings, reuse only the already-derived digest.
            for input in inputs {
                let key = DecodeKey::from(input.binding);
                let digest = match self.state.canonical.get(&key) {
                    Some(old) => old.clone(),
                    None => decode_tile(input.binding, input.raw)?.canonical_sha256,
                };
                canonical.insert(key, digest);
            }
        } else {
            for value in decode_request(&inputs)? {
                let key = DecodeKey::from(&value.binding);
                canonical.insert(key.clone(), value.canonical_sha256.clone());
                let shared = match self.state.decoded.get(&key) {
                    Some(old) if old.as_ref() == &value => Arc::clone(old),
                    _ => Arc::new(value),
                };
                decoded.insert(key, shared);
            }
        }
        Ok(Self {
            state: Arc::new(State {
                blobs,
                bindings: candidate,
                canonical,
                decoded,
            }),
            cache: Arc::clone(&self.cache),
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
            let canonical = self.state.canonical.get(&cache_key).ok_or(Error::Binding)?;
            state.canonical.insert(cache_key.clone(), canonical.clone());
            if let Some(decoded) = self.state.decoded.get(&cache_key) {
                state.decoded.insert(cache_key, Arc::clone(decoded));
            }
        }
        // Preflight the entire requested scope, including repeated blob usage,
        // before decoding anything from the persistent archive.
        if state.decoded.len() != state.bindings.len() {
            let inputs: Vec<_> = state
                .bindings
                .values()
                .map(|binding| EvidenceInput {
                    binding,
                    raw: &state.blobs[&binding.raw_sha256],
                })
                .collect();
            validate_request(&inputs)?;
            for input in inputs {
                let key = DecodeKey::from(input.binding);
                let cached = self.cache.lock().ok().and_then(|mut cache| cache.get(&key));
                let was_cached = cached.is_some();
                let decoded = match cached {
                    Some(tile) => tile,
                    None => Arc::new(decode_tile(input.binding, input.raw)?),
                };
                if state.canonical.get(&key) != Some(&decoded.canonical_sha256) {
                    return Err(Error::Hash);
                }
                if !was_cached {
                    if let Ok(mut cache) = self.cache.lock() {
                        cache.insert(key.clone(), Arc::clone(&decoded));
                    }
                }
                state.decoded.insert(key, decoded);
            }
        }
        Ok(Self {
            state: Arc::new(state),
            cache: Arc::clone(&self.cache),
        })
    }
    pub(super) fn is_decoded_scope(&self) -> bool {
        self.state.decoded.len() == self.state.bindings.len()
    }
    pub fn descriptors(&self) -> Vec<EvidenceDescriptor> {
        self.state
            .bindings
            .values()
            .map(|binding| {
                // Internal state is built only by the validated constructor above.
                let canonical = &self.state.canonical[&DecodeKey::from(binding)];
                EvidenceDescriptor {
                    binding: binding.clone(),
                    byte_length: self.state.blobs[&binding.raw_sha256].len(),
                    canonical_sha256: canonical.clone(),
                }
            })
            .collect()
    }
    /// Resolve a tile in a decoded request/store scope. For a retained archive,
    /// obtain a bounded `scope` first. Absence is not an empty road context.
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
                && self.state.canonical == other.state.canonical)
    }
}
impl Eq for EvidenceStore {}
