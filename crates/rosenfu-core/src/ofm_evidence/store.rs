//! In-memory immutable evidence table. Persistence must reconstruct this through
//! validated constructors; neither descriptors nor caller-supplied decoded objects are proof.
use super::{
    decode_request, decode_tile_with_verified_sha, raw_sha256, validate_request, DecodedTile,
    Error, EvidenceInput, Result, TileBinding, MAX_REQUEST_BYTES, MAX_TILES, MAX_TILE_BYTES,
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
#[derive(Debug, PartialEq, Eq)]
struct BusCompiledGraphKey {
    rule: String,
    grade_policy: Option<super::BusGradePolicy>,
    respect_oneway: bool,
    tile_keys_and_hashes: Vec<(String, String)>,
}
#[derive(Debug, Default)]
struct ScopeCache {
    entries: VecDeque<(DecodeKey, Arc<DecodedTile>, usize)>,
    // Stored proofs have their own capped cohort; candidate previews do not
    // evict unrelated validated routes during normal edits.
    bus_paths: VecDeque<Arc<super::BusPath>>,
    bus_steps: usize,
    bus_path: Option<Arc<super::BusPath>>,
    bus_graph: Option<(BusCompiledGraphKey, Arc<super::bus::GameplayBusGraph>)>,
    weight: usize,
}
impl ScopeCache {
    fn get_bus_path(&mut self, binding: &super::BusPathBinding) -> Option<Arc<super::BusPath>> {
        if let Some(i) = self.bus_paths.iter().position(|p| &p.binding == binding) {
            let hit = self.bus_paths.remove(i)?;
            self.bus_paths.push_back(Arc::clone(&hit));
            Some(hit)
        } else {
            self.bus_path
                .as_ref()
                .filter(|p| &p.binding == binding)
                .cloned()
        }
    }
    fn admit_bus_path(&mut self, path: Arc<super::BusPath>) {
        const MAX_STEPS: usize = 32_768 + 4_096;
        // Each admitted ordinary route may have independently searched
        // directions; the existing total atomic-step budget stays fixed.
        const MAX_PATHS: usize = 2 * 65;
        if path.steps.len() > 4096 || self.bus_paths.iter().any(|p| p.binding == path.binding) {
            return;
        }
        while self.bus_paths.len() >= MAX_PATHS || self.bus_steps + path.steps.len() > MAX_STEPS {
            let Some(evicted) = self.bus_paths.pop_front() else {
                return;
            };
            self.bus_steps -= evicted.steps.len();
        }
        self.bus_steps += path.steps.len();
        self.bus_paths.push_back(path);
    }
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
                    // Every new blob was SHA-checked above; old bytes are
                    // immutable. Inputs select blobs by the binding's SHA.
                    None => {
                        let cached = (!restore)
                            .then(|| self.cache.lock().ok().and_then(|mut cache| cache.get(&key)))
                            .flatten();
                        if let Some(tile) = cached {
                            // Submitted bytes were rechecked against this immutable
                            // DecodeKey's raw SHA; preview never grants new trust.
                            tile.canonical_sha256.clone()
                        } else {
                            let tile = decode_tile_with_verified_sha(input.binding, input.raw)?;
                            let digest = tile.canonical_sha256.clone();
                            if !restore {
                                if let Ok(mut cache) = self.cache.lock() {
                                    cache.insert(key.clone(), Arc::new(tile));
                                }
                            }
                            digest
                        }
                    }
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
                    // State is private and admitted by add() above.
                    None => Arc::new(decode_tile_with_verified_sha(input.binding, input.raw)?),
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
    pub(crate) fn gameplay_bus_path(
        &self,
        proposed: &super::BusPathBinding,
    ) -> Result<Option<Arc<super::BusPath>>> {
        self.gameplay_bus_path_cached(proposed, false)
    }
    pub(crate) fn stored_gameplay_bus_path(
        &self,
        proposed: &super::BusPathBinding,
    ) -> Result<Option<Arc<super::BusPath>>> {
        self.gameplay_bus_path_cached(proposed, true)
    }
    fn gameplay_bus_path_cached(
        &self,
        proposed: &super::BusPathBinding,
        stored: bool,
    ) -> Result<Option<Arc<super::BusPath>>> {
        let geometry_only;
        let proposed = if proposed.interior_stops.is_empty() {
            proposed
        } else {
            geometry_only = super::BusPathBinding {
                interior_stops: vec![],
                ..proposed.clone()
            };
            &geometry_only
        };
        let keys = proposed.tile_keys()?;
        for (key, digest) in std::iter::once((&proposed.tile_key, &proposed.tile_raw_sha256)).chain(
            proposed
                .additional_tiles
                .iter()
                .map(|p| (&p.tile_key, &p.tile_raw_sha256)),
        ) {
            let binding = self.state.bindings.get(key).ok_or(Error::Binding)?;
            if &binding.raw_sha256 != digest {
                return Err(Error::Hash);
            }
        }
        if let Ok(mut cache) = self.cache.lock() {
            if let Some(hit) = cache.get_bus_path(proposed) {
                if stored {
                    cache.admit_bus_path(Arc::clone(&hit));
                }
                return Ok(Some(hit));
            }
        }
        let key = BusCompiledGraphKey {
            rule: proposed.rule.clone(),
            grade_policy: proposed.grade_policy,
            respect_oneway: proposed.respect_oneway,
            tile_keys_and_hashes: keys
                .iter()
                .map(|key| (key.clone(), self.state.bindings[key].raw_sha256.clone()))
                .collect(),
        };
        let hit = self.cache.lock().ok().and_then(|cache| {
            cache
                .bus_graph
                .as_ref()
                .filter(|(have, _)| have == &key)
                .map(|(_, graph)| Arc::clone(graph))
        });
        let graph = if let Some(hit) = hit {
            hit
        } else {
            let scope = self.scope(&keys)?;
            let graph = Arc::new(super::bus::build_gameplay_bus_graph(&scope, proposed)?);
            if graph.fits_cache() {
                if let Ok(mut cache) = self.cache.lock() {
                    cache.bus_graph = Some((key, Arc::clone(&graph)));
                }
            }
            graph
        };
        let path = super::bus::solve_gameplay_bus_graph(&graph, proposed)?.map(Arc::new);
        if let (Some(path), Ok(mut cache)) = (path.as_ref(), self.cache.lock()) {
            if stored {
                cache.admit_bus_path(Arc::clone(path));
            } else if path.steps.len() <= 4096 {
                cache.bus_path = Some(Arc::clone(path));
            }
        }
        Ok(path)
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

#[cfg(test)]
mod bus_preview_cache_tests {
    use super::*;
    use crate::ofm_evidence::{BusPathBinding, BusVertex, BusVia, BUS_GRAPH_RULE};

    #[test]
    fn moving_a_via_reuses_the_compiled_graph_within_exact_scope() {
        // Generated class=minor MVT: an eligible three-point line and branch.
        let raw = vec![
            26, 81, 10, 14, 116, 114, 97, 110, 115, 112, 111, 114, 116, 97, 116, 105, 111, 110,
            120, 2, 40, 128, 32, 18, 20, 18, 2, 0, 0, 24, 2, 34, 12, 9, 200, 1, 200, 1, 18, 200, 1,
            0, 200, 1, 0, 18, 20, 18, 2, 0, 0, 24, 2, 34, 12, 9, 144, 3, 200, 1, 18, 0, 200, 1, 0,
            200, 1, 26, 5, 99, 108, 97, 115, 115, 34, 7, 10, 5, 109, 105, 110, 111, 114,
        ];
        let sha = super::super::raw_sha256(&raw);
        let key = "https://tiles.openfreemap.org/planet/20261101_080013_pt/14/14552/6451.pbf";
        let binding = TileBinding {
            canonical_url: key.into(),
            revision: "20261101_080013_pt".into(),
            z: 14,
            x: 14552,
            y: 6451,
            raw_sha256: sha.clone(),
            decoder_rule: super::super::DECODER_RULE.into(),
        };
        let stored = EvidenceStore::from_parts(
            vec![RawEvidence {
                sha256: sha.clone(),
                bytes: raw,
            }],
            vec![binding],
        )
        .unwrap();
        let v = |feature, vertex| BusVertex {
            feature,
            line: 0,
            vertex,
        };
        let mut q = BusPathBinding {
            rule: BUS_GRAPH_RULE.into(),
            grade_policy: None,
            tile_key: key.into(),
            tile_raw_sha256: sha,
            respect_oneway: false,
            start: v(0, 0),
            end: v(1, 2),
            additional_tiles: vec![],
            end_tile_key: None,
            via: vec![BusVia {
                id: "v".into(),
                tile_key: key.into(),
                vertex: v(1, 0),
            }],
            pattern: None,
            interior_stops: vec![],
        };
        let first = stored.gameplay_bus_path(&q).unwrap().unwrap();
        let graph = |store: &EvidenceStore| {
            Arc::clone(&store.cache.lock().unwrap().bus_graph.as_ref().unwrap().1)
        };
        let initialized = graph(&stored);
        q.via[0].vertex = v(1, 1);
        let moved = stored.clone().gameplay_bus_path(&q).unwrap().unwrap();
        assert_ne!(first.via_point_indices, moved.via_point_indices);
        assert!(Arc::ptr_eq(&initialized, &graph(&stored)));
        q.interior_stops.push(crate::ofm_evidence::BusInteriorStop {
            id: "stop".into(),
            name: "停留所".into(),
            occurrence_key: moved.stop_occurrence_keys()[0].clone(),
            fraction_q: 500_000,
        });
        let same_path = stored.gameplay_bus_path(&q).unwrap().unwrap();
        assert!(Arc::ptr_eq(&moved, &same_path));
        q.respect_oneway = true;
        assert!(stored.gameplay_bus_path(&q).unwrap().is_some());
        assert!(!Arc::ptr_eq(&initialized, &graph(&stored)));
    }
}
