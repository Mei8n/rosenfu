//! Immutable raw evidence can be shared by SHA, but decoding is bound to tile coordinates
//! and a pinned rule. Hashes prove byte consistency, not supplier authenticity or coverage.
//! No network, decompression, world mutation, or implicit repair is performed here.
mod bus;
mod bus_grade;
pub(crate) use bus::BUS_STOP_DENOMINATOR;
pub use bus::{
    find_gameplay_bus_path, reconstruct_authored_tram_chain, reconstruct_authored_tram_guide,
    reconstruct_gameplay_bus_path, resolve_fractional_bus_traversal, AuthoredTramChainGuide,
    AuthoredTramGuide, BusGradePolicy, BusInteriorStop, BusPath, BusPathBinding, BusPattern,
    BusTileProof, BusVertex, BusVia, FractionalBusTraversal, BUS_GRAPH_RULE, BUS_RECTANGLE_RULE,
};
mod candidates;
mod path;
mod seam_witness;
pub(crate) use path::reconstruct_street_tram_path;
pub use path::{
    reconstruct_path, validate_path_descriptor, AuthoredTramProof, DerivedPath, PathDescriptor,
    PathJoin, PathPick, SourceSpan,
};
pub use seam_witness::{
    replay_westbound_seam_traversal, verify_westbound_seam_receipt, WestboundSeamReceipt,
    WestboundSeamTraversal, WestboundSeamWitness, WestboundSourceAtom, WESTBOUND_SEAM_WITNESS_RULE,
};
mod store;
mod viewport;
pub use candidates::{boundary_candidates, validate_boundary_candidates, BoundaryCandidate};
pub use viewport::{retained_road_view, RetainedRoadView};
mod wire;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
pub use store::{
    EvidenceDescriptor, EvidenceStore, RawEvidence, MAX_PINNED_BINDINGS, MAX_PINNED_RAW_BYTES,
};
use wire::{bytes, once, var, Field, Reader};

pub const DECODER_RULE: &str = "ofm_transportation_z14_v1";
pub const MAX_TILE_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_REQUEST_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_TILES: usize = 16;
const MAX_FEATURES: usize = 10_000;
const MAX_POINTS: usize = 20_000;
const MAX_DICTIONARY: usize = 20_000;
const MAX_GEOMETRY_WORDS: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Binding,
    Hash,
    Limit,
    Malformed,
    MissingTransportation,
    Unsupported,
    InvalidAttribute,
    NoBusLeg(usize),
    InvalidBusCircuit,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OpenFreeMap evidence: {self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TileBinding {
    pub canonical_url: String,
    pub revision: String,
    pub z: u8,
    pub x: u32,
    pub y: u32,
    pub raw_sha256: String,
    pub decoder_rule: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawValue {
    String(String),
    Float32(u32),
    Float64(u64),
    Int(i64),
    UInt(u64),
    SInt(i64),
    Bool(bool),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileLine {
    /// Original opaque MVT id, not a provenance claim or stable road identity.
    pub raw_feature_id: Option<u64>,
    pub feature_index: u32,
    pub line_index: u32,
    pub extent: u32,
    /// MVT integer coordinates, including the supplier's tile buffer.
    pub coordinates: Vec<[i32; 2]>,
    /// Known raw fields only, with original protobuf value types preserved.
    pub attributes: BTreeMap<String, RawValue>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedTile {
    pub binding: TileBinding,
    pub lines: Vec<TileLine>,
    pub canonical_sha256: String,
    duplicate_feature_ids: BTreeSet<u64>,
}
impl DecodedTile {
    /// Return an unambiguous, single-part admitted road line for a nonzero
    /// supplier ID. The ID alone does not prove continuity or trust the
    /// producer; callers must independently establish both.
    pub fn unique_raw_road_line(&self, raw_feature_id: u64) -> Option<&TileLine> {
        if raw_feature_id == 0 || self.duplicate_feature_ids.contains(&raw_feature_id) {
            return None;
        }
        let mut matches = self
            .lines
            .iter()
            .filter(|line| line.raw_feature_id == Some(raw_feature_id));
        let first = matches.next()?;
        (first.line_index == 0 && matches.next().is_none()).then_some(first)
    }
}
pub struct EvidenceInput<'a> {
    pub binding: &'a TileBinding,
    pub raw: &'a [u8],
}

/// Canonical raw fragment identity shared by the route and crossing views.
pub(crate) fn raw_road_fragment_id(binding: &TileBinding, line: &TileLine) -> String {
    format!(
        "ofm:{}:14:{}:{}:{}:{}",
        binding.revision, binding.x, binding.y, line.feature_index, line.line_index
    )
}

pub(crate) fn road_grid_lon_lat(x: f64, y: f64) -> [f64; 2] {
    [
        x / 16384. * 360. - 180.,
        (std::f64::consts::PI * (1. - 2. * y / 16384.))
            .sinh()
            .atan()
            .to_degrees(),
    ]
}

pub fn raw_sha256(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}
impl TileBinding {
    pub fn validate(&self) -> Result<()> {
        if self.z != 14
            || self.x >= 16384
            || self.y >= 16384
            || self.revision.is_empty()
            || self.revision.len() > 64
            || !self
                .revision
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || self.decoder_rule != DECODER_RULE
            || self.raw_sha256.len() != 64
            || !self
                .raw_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || self.canonical_url
                != format!(
                    "https://tiles.openfreemap.org/planet/{}/14/{}/{}.pbf",
                    self.revision, self.x, self.y
                )
        {
            return Err(Error::Binding);
        }
        Ok(())
    }
}
/// Checks the whole request before decoding any tile, including reused evidence.
/// Duplicate tile bindings and mixed provider revisions are rejected atomically.
fn validate_request(inputs: &[EvidenceInput<'_>]) -> Result<()> {
    if inputs.is_empty() || inputs.len() > MAX_TILES {
        return Err(Error::Limit);
    }
    let mut total = 0usize;
    let mut tiles = BTreeSet::new();
    let revision = &inputs[0].binding.revision;
    for input in inputs {
        input.binding.validate()?;
        if &input.binding.revision != revision || !tiles.insert((input.binding.x, input.binding.y))
        {
            return Err(Error::Binding);
        }
        total = total.checked_add(input.raw.len()).ok_or(Error::Limit)?;
        if input.raw.len() > MAX_TILE_BYTES || total > MAX_REQUEST_BYTES {
            return Err(Error::Limit);
        }
    }
    Ok(())
}
pub fn decode_request(inputs: &[EvidenceInput<'_>]) -> Result<Vec<DecodedTile>> {
    validate_request(inputs)?;
    inputs
        .iter()
        .map(|input| decode_tile(input.binding, input.raw))
        .collect()
}
pub fn decode_tile(binding: &TileBinding, raw: &[u8]) -> Result<DecodedTile> {
    binding.validate()?;
    if raw.len() > MAX_TILE_BYTES {
        return Err(Error::Limit);
    }
    if raw_sha256(raw) != binding.raw_sha256 {
        return Err(Error::Hash);
    }
    decode_tile_with_verified_sha(binding, raw)
}

// The caller must already have checked the raw hash against the validated
// binding. Archive callers obtain bytes only by SHA from the immutable store.
fn decode_tile_with_verified_sha(binding: &TileBinding, raw: &[u8]) -> Result<DecodedTile> {
    let mut reader = Reader::new(raw);
    let mut transportation = None;
    let mut layers = 0;
    while let Some((tag, field)) = reader.next()? {
        if tag != 3 {
            continue;
        }
        layers += 1;
        if layers > 64 {
            return Err(Error::Limit);
        }
        let data = bytes(field)?;
        let mut r = Reader::new(data);
        let mut name = None;
        while let Some((tag, value)) = r.next()? {
            if tag == 1 {
                once(&mut name, text(bytes(value)?, 256)?)?;
            }
        }
        if name == Some("transportation") {
            once(&mut transportation, data)?;
        }
    }
    let (lines, duplicate_feature_ids) =
        decode_layer(transportation.ok_or(Error::MissingTransportation)?)?;
    let canonical_sha256 = canonical_hash(binding, &lines);
    Ok(DecodedTile {
        binding: binding.clone(),
        lines,
        canonical_sha256,
        duplicate_feature_ids,
    })
}
fn text(raw: &[u8], limit: usize) -> Result<&str> {
    if raw.len() > limit {
        return Err(Error::Limit);
    }
    std::str::from_utf8(raw).map_err(|_| Error::Malformed)
}
fn decode_layer(raw: &[u8]) -> Result<(Vec<TileLine>, BTreeSet<u64>)> {
    let mut reader = Reader::new(raw);
    let mut features = Vec::new();
    let mut keys = Vec::new();
    let mut values = Vec::new();
    let mut extent = None;
    let mut version = None;
    let mut key_set = BTreeSet::new();
    while let Some((tag, field)) = reader.next()? {
        match tag {
            1 => {
                text(bytes(field)?, 256)?;
            }
            2 => {
                if features.len() >= MAX_FEATURES {
                    return Err(Error::Limit);
                }
                features.push(bytes(field)?);
            }
            3 => {
                if keys.len() >= MAX_DICTIONARY {
                    return Err(Error::Limit);
                }
                let k = text(bytes(field)?, 1024)?;
                if !key_set.insert(k) {
                    return Err(Error::Malformed);
                }
                keys.push(k);
            }
            4 => {
                if values.len() >= MAX_DICTIONARY {
                    return Err(Error::Limit);
                }
                values.push(decode_value(bytes(field)?)?);
            }
            5 => once(
                &mut extent,
                u32::try_from(var(field)?).map_err(|_| Error::Malformed)?,
            )?,
            15 => once(&mut version, var(field)?)?,
            _ => {}
        }
    }
    if version != Some(2) {
        return Err(Error::Unsupported);
    }
    let extent = extent.unwrap_or(4096);
    if extent == 0 || extent > 1 << 24 {
        return Err(Error::Limit);
    }
    let mut lines = Vec::new();
    let mut points = 0;
    let mut words = 0;
    let mut feature_ids = BTreeSet::new();
    let mut duplicate_feature_ids = BTreeSet::new();
    for (index, data) in features.iter().enumerate() {
        let mut r = Reader::new(data);
        let mut tags = None;
        let mut geometry = None;
        let mut kind = None;
        let mut id = None;
        while let Some((tag, f)) = r.next()? {
            match tag {
                1 => once(&mut id, var(f)?)?,
                2 => once(&mut tags, bytes(f)?)?,
                3 => once(&mut kind, var(f)?)?,
                4 => once(&mut geometry, bytes(f)?)?,
                _ => {}
            }
        }
        if let Some(id) = id.filter(|&id| id != 0) {
            if !feature_ids.insert(id) {
                duplicate_feature_ids.insert(id);
            }
        }
        let mut attrs = BTreeMap::new();
        let mut seen = BTreeSet::new();
        let mut t = Reader::new(tags.unwrap_or_default());
        let mut count = 0;
        while !t.empty() {
            count += 1;
            if count > 256 {
                return Err(Error::Limit);
            }
            let k = usize::try_from(t.var()?).map_err(|_| Error::Malformed)?;
            let v = usize::try_from(t.var()?).map_err(|_| Error::Malformed)?;
            let k = *keys.get(k).ok_or(Error::Malformed)?;
            let v = values.get(v).ok_or(Error::Malformed)?;
            if !seen.insert(k) {
                return Err(Error::Malformed);
            }
            if matches!(
                k,
                "class" | "subclass" | "access" | "brunnel" | "layer" | "oneway"
            ) {
                attrs.insert(k.to_owned(), v.clone());
            }
        }
        if kind != Some(2) {
            continue;
        }
        let Some(RawValue::String(class)) = attrs.get("class") else {
            continue;
        };
        if !matches!(
            class.as_str(),
            "motorway"
                | "trunk"
                | "primary"
                | "secondary"
                | "tertiary"
                | "minor"
                | "service"
                | "track"
                | "busway"
                | "bus_guideway"
                | "path"
        ) {
            continue;
        }
        validate_attributes(&attrs)?;
        let decoded = decode_geometry(
            geometry.ok_or(Error::Malformed)?,
            extent,
            &mut points,
            &mut words,
        )?;
        for (j, coordinates) in decoded.into_iter().enumerate() {
            lines.push(TileLine {
                raw_feature_id: id,
                feature_index: index as u32,
                line_index: j as u32,
                extent,
                coordinates,
                attributes: attrs.clone(),
            });
        }
    }
    Ok((lines, duplicate_feature_ids))
}
fn decode_value(raw: &[u8]) -> Result<RawValue> {
    let mut r = Reader::new(raw);
    let mut out = None;
    while let Some((tag, f)) = r.next()? {
        let value = match (tag, f) {
            (1, Field::Bytes(v)) => RawValue::String(text(v, 4096)?.to_owned()),
            (2, Field::Fixed32(v)) if f32::from_bits(v).is_finite() => RawValue::Float32(v),
            (3, Field::Fixed64(v)) if f64::from_bits(v).is_finite() => RawValue::Float64(v),
            (4, Field::Var(v)) => RawValue::Int(v as i64),
            (5, Field::Var(v)) => RawValue::UInt(v),
            (6, Field::Var(v)) => RawValue::SInt(zigzag(v)),
            (7, Field::Var(v)) if v <= 1 => RawValue::Bool(v != 0),
            (1..=7, _) => return Err(Error::Malformed),
            _ => continue,
        };
        once(&mut out, value)?;
    }
    out.ok_or(Error::Malformed)
}
fn zigzag(v: u64) -> i64 {
    ((v >> 1) as i64) ^ -((v & 1) as i64)
}
pub(crate) fn number(v: &RawValue) -> Option<f64> {
    match v {
        RawValue::Float32(v) => Some(f32::from_bits(*v) as f64),
        RawValue::Float64(v) => Some(f64::from_bits(*v)),
        RawValue::Int(v) | RawValue::SInt(v) if v.unsigned_abs() <= 9_007_199_254_740_991 => {
            Some(*v as f64)
        }
        RawValue::UInt(v) if *v <= 9_007_199_254_740_991 => Some(*v as f64),
        _ => None,
    }
}
fn forbidden(c: char) -> bool {
    // Unicode Cc and Cf, matching the frontend attribute gate (no normalization).
    c.is_control()
        || matches!(c as u32,0xAD|0x600..=0x605|0x61C|0x6DD|0x70F|0x890..=0x891|0x8E2|0x180E|0x200B..=0x200F|0x202A..=0x202E|0x2060..=0x2064|0x2066..=0x206F|0xFEFF|0xFFF9..=0xFFFB|0x110BD|0x110CD|0x13430..=0x1343F|0x1BCA0..=0x1BCA3|0x1D173..=0x1D17A|0xE0001|0xE0020..=0xE007F)
}
fn validate_attributes(attrs: &BTreeMap<String, RawValue>) -> Result<()> {
    for (key, v) in attrs {
        match v {
            RawValue::String(s) if !s.is_empty() && s.len() <= 100 && !s.chars().any(forbidden) => {
            }
            _ if matches!(key.as_str(), "layer" | "oneway") && number(v).is_some() => {}
            _ => return Err(Error::InvalidAttribute),
        }
        if key == "brunnel" {
            let valid_structure = match v {
                RawValue::String(value) => matches!(value.as_str(), "bridge" | "tunnel" | "ford"),
                _ => false,
            };
            if !valid_structure {
                return Err(Error::InvalidAttribute);
            }
        }
        if key == "layer" {
            let n = match v {
                RawValue::String(s) => s.trim().parse::<f64>().ok(),
                _ => number(v),
            };
            if !n.is_some_and(f64::is_finite) {
                return Err(Error::InvalidAttribute);
            }
        }
    }
    Ok(())
}
fn decode_geometry(
    raw: &[u8],
    extent: u32,
    points: &mut usize,
    words: &mut usize,
) -> Result<Vec<Vec<[i32; 2]>>> {
    let mut r = Reader::new(raw);
    let mut lines = Vec::new();
    let mut line = Vec::new();
    let mut pos = [0i64; 2];
    while !r.empty() {
        *words += 1;
        if *words > MAX_GEOMETRY_WORDS {
            return Err(Error::Limit);
        }
        let command = r.var()?;
        let count = usize::try_from(command >> 3).map_err(|_| Error::Limit)?;
        let op = command & 7;
        if count == 0 || (op == 1 && count != 1) || !matches!(op, 1 | 2) {
            return Err(Error::Malformed);
        }
        if op == 1 && !line.is_empty() {
            if line.len() < 2 {
                return Err(Error::Malformed);
            }
            lines.push(std::mem::take(&mut line));
        }
        if op == 2 && line.is_empty() {
            return Err(Error::Malformed);
        }
        if count > MAX_POINTS.saturating_sub(*points) {
            return Err(Error::Limit);
        }
        *points += count;
        *words = words.checked_add(count * 2).ok_or(Error::Limit)?;
        if *words > MAX_GEOMETRY_WORDS {
            return Err(Error::Limit);
        }
        for _ in 0..count {
            for axis in &mut pos {
                let delta = r.var()?;
                if delta > u32::MAX as u64 {
                    return Err(Error::Malformed);
                }
                *axis = axis.checked_add(zigzag(delta)).ok_or(Error::Malformed)?;
                if axis.abs() > i64::from(extent) * 16 {
                    return Err(Error::Limit);
                }
            }
            line.push([
                i32::try_from(pos[0]).map_err(|_| Error::Limit)?,
                i32::try_from(pos[1]).map_err(|_| Error::Limit)?,
            ]);
        }
    }
    if line.len() < 2 {
        return Err(Error::Malformed);
    }
    lines.push(line);
    Ok(lines)
}
// Keep the v1 hash format: opaque feature IDs are already covered by the
// raw byte digests, but do not change logical fragment/route identity.
fn canonical_hash(binding: &TileBinding, lines: &[TileLine]) -> String {
    let mut h = Sha256::new();
    h.update(b"rosenfu-ofm-integer-context-v1\0");
    fn s(h: &mut Sha256, v: &str) {
        h.update((v.len() as u32).to_le_bytes());
        h.update(v.as_bytes());
    }
    s(&mut h, &binding.decoder_rule);
    s(&mut h, &binding.canonical_url);
    s(&mut h, &binding.revision);
    s(&mut h, &binding.raw_sha256);
    h.update([binding.z]);
    h.update(binding.x.to_le_bytes());
    h.update(binding.y.to_le_bytes());
    h.update((lines.len() as u32).to_le_bytes());
    for l in lines {
        for n in [
            l.feature_index,
            l.line_index,
            l.extent,
            l.coordinates.len() as u32,
        ] {
            h.update(n.to_le_bytes());
        }
        for p in &l.coordinates {
            h.update(p[0].to_le_bytes());
            h.update(p[1].to_le_bytes());
        }
        h.update((l.attributes.len() as u32).to_le_bytes());
        for (k, v) in &l.attributes {
            s(&mut h, k);
            match v {
                RawValue::String(v) => {
                    h.update([1]);
                    s(&mut h, v)
                }
                RawValue::Float32(v) => {
                    h.update([2]);
                    h.update(v.to_le_bytes())
                }
                RawValue::Float64(v) => {
                    h.update([3]);
                    h.update(v.to_le_bytes())
                }
                RawValue::Int(v) => {
                    h.update([4]);
                    h.update(v.to_le_bytes())
                }
                RawValue::UInt(v) => {
                    h.update([5]);
                    h.update(v.to_le_bytes())
                }
                RawValue::SInt(v) => {
                    h.update([6]);
                    h.update(v.to_le_bytes())
                }
                RawValue::Bool(v) => h.update([7, u8::from(*v)]),
            }
        }
    }
    format!("{:x}", h.finalize())
}
