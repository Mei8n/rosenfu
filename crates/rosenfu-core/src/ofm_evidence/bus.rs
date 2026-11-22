//! Ground-only, bounded OFM bus gameplay graph. This is not traffic authorization,
//! a coverage guarantee, or a way to install a native road catalog.
use super::{
    number, Error, EvidenceStore, RawValue, Result, SourceSpan, TileBinding, TileLine, MAX_TILES,
};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

pub const BUS_GRAPH_RULE: &str = "ofm_ground_bus_z14_v1";
pub const BUS_RECTANGLE_RULE: &str = "ofm_ground_bus_rectangle_z14_v2";
const MAX_GRAPH_POINTS: usize = 100_000;
const MAX_VIAS: usize = 30;
const MAX_MULTI_RELAXATIONS: usize = 1_000_000;
const MAX_MULTI_ATOMS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusTileProof {
    pub tile_key: String,
    pub tile_raw_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusVertex {
    pub feature: u32,
    pub line: u32,
    pub vertex: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BusPattern {
    OpenRoundTrip,
    ClosedOneWay,
}

/// Authored geometry control. An intermediate path vertex is not a passenger
/// stop unless separately selected against its approved route occurrence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusVia {
    pub id: String,
    pub tile_key: String,
    pub vertex: BusVertex,
}

/// Manual passenger stop on one already resolved outward route occurrence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusInteriorStop {
    pub id: String,
    pub name: String,
    pub occurrence_key: String,
    pub fraction_q: u32,
}
pub(crate) const BUS_STOP_DENOMINATOR: u32 = 1_000_000;

/// A reproducible request, not a proof or an independently trusted saved route.
/// Call the solver again against the matching verified tile to reconstruct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusPathBinding {
    pub rule: String,
    pub tile_key: String,
    pub tile_raw_sha256: String,
    pub respect_oneway: bool,
    pub start: BusVertex,
    pub end: BusVertex,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_tiles: Vec<BusTileProof>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_tile_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<BusPattern>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub via: Vec<BusVia>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interior_stops: Vec<BusInteriorStop>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BusPath {
    pub binding: BusPathBinding,
    pub spans: Vec<SourceSpan>,
    /// Geographic longitude/latitude, exactly from the traversed tile vertices.
    pub points: Vec<[f64; 2]>,
    pub length_m: f64,
    pub(crate) operation_length_mm: u64,
    pub(crate) atom_operation_micrometres: Vec<u64>,
    /// Derived point indices in exact authored via order, not a stop decision.
    pub via_point_indices: Vec<usize>,
    pub(crate) steps: Vec<BusSourceStep>,
    pub(crate) points_grid: Vec<BusGridPoint>,
}

impl BusPath {
    /// Directed materialized identity shared by every ordinary route that
    /// traverses this exact verified raw segment (or owned seam-clipped part).
    pub fn source_atom_id(&self, index: usize) -> Option<String> {
        self.atom_id(index, false)
    }
    pub(crate) fn physical_atom_id(&self, index: usize) -> Option<String> {
        self.atom_id(index, true)
    }
    fn atom_id(&self, index: usize, physical: bool) -> Option<String> {
        let step = self.steps.get(index)?;
        let span = self.spans.get(step.span)?;
        let digest = self.source_atom_raw_sha(index)?;
        let (a, b) = if physical && step.from > step.to {
            (step.to, step.from)
        } else {
            (step.from, step.to)
        };
        let key = format!(
            "ofm-bus-atom-v1\0{}\0{}\0{}\0{}\0{:016x}\0{:016x}",
            span.binding_key,
            digest,
            span.fragment_id,
            step.raw_ordinal,
            a.to_bits(),
            b.to_bits()
        );
        Some(format!(
            "ofm-bus:edge:{}",
            super::raw_sha256(key.as_bytes())
        ))
    }
    pub(crate) fn control_pair(&self, index: usize) -> (&str, &str) {
        let leg = self.via_point_indices.partition_point(|&i| i <= index);
        let a = leg
            .checked_sub(1)
            .and_then(|i| self.binding.via.get(i))
            .map_or("@start", |v| v.id.as_str());
        let b = self.binding.via.get(leg).map_or("@end", |v| v.id.as_str());
        (a, b)
    }
    pub(crate) fn on_return_control(id: &str) -> &str {
        match id {
            "@start" => "@end",
            "@end" => "@start",
            other => other,
        }
    }
    /// One token per outward drawn segment. Control IDs disambiguate revisits
    /// to the same directed atom without depending on editor array indices.
    pub fn stop_occurrence_keys(&self) -> Vec<String> {
        (0..self.steps.len())
            .filter_map(|i| {
                let atom = self.source_atom_id(i)?;
                let (a, b) = self.control_pair(i);
                let key = format!("ofm-bus-stop-occurrence-v1\0{a}\0{b}\0{atom}");
                Some(super::raw_sha256(key.as_bytes()))
            })
            .collect()
    }
    pub(crate) fn source_atom_raw_sha(&self, index: usize) -> Option<&str> {
        let span = self.spans.get(self.steps.get(index)?.span)?;
        if span.binding_key == self.binding.tile_key {
            Some(&self.binding.tile_raw_sha256)
        } else {
            self.binding
                .additional_tiles
                .iter()
                .find(|p| p.tile_key == span.binding_key)
                .map(|p| p.tile_raw_sha256.as_str())
        }
    }
}
/// Directional, potentially seam-clipped atom of one original raw segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BusSourceStep {
    pub span: usize,
    pub raw_ordinal: usize,
    pub from: f64,
    pub to: f64,
}

/// Exact rational global tile-grid position, independent of projected latitude.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BusGridPoint([(i128, u64); 2]);
impl BusGridPoint {
    fn new(binding: &TileBinding, extent: u32, offset: [(i64, u64); 2]) -> Self {
        let mut result = [(0, 1); 2];
        for (i, (origin, (n, d))) in [binding.x, binding.y].into_iter().zip(offset).enumerate() {
            let den = u64::from(extent) * d;
            let num = i128::from(n) + i128::from(origin) * i128::from(den);
            let (mut a, mut b) = ((num.unsigned_abs() % u128::from(den)) as u64, den);
            while b != 0 {
                (a, b) = (b, a % b);
            }
            result[i] = (num / i128::from(a), den / a);
        }
        Self(result)
    }
    pub(crate) fn id(self) -> String {
        let [(xn, xd), (yn, yd)] = self.0;
        format!("ofm-bus:node:{xn}:{xd}:{yn}:{yd}")
    }
    fn point(self) -> [f64; 2] {
        let [(xn, xd), (yn, yd)] = self.0;
        let x = xn as f64 / xd as f64;
        let y = yn as f64 / yd as f64;
        [
            x / 16384. * 360. - 180.,
            (std::f64::consts::PI * (1. - 2. * y / 16384.))
                .sinh()
                .atan()
                .to_degrees(),
        ]
    }
}

#[derive(Clone, Copy)]
enum Direction {
    Both,
    Forward,
    Reverse,
}

fn oneway(value: Option<&RawValue>) -> Option<Direction> {
    match value {
        None => Some(Direction::Both),
        Some(RawValue::String(s)) => match s.as_str() {
            "no" | "0" => Some(Direction::Both),
            "yes" | "1" => Some(Direction::Forward),
            "-1" => Some(Direction::Reverse),
            _ => None,
        },
        Some(v) => match number(v)? {
            0. => Some(Direction::Both),
            1. => Some(Direction::Forward),
            -1. => Some(Direction::Reverse),
            _ => None,
        },
    }
}

fn eligible(line: &TileLine, respect_oneway: bool) -> Option<Direction> {
    let attrs = &line.attributes;
    if !matches!(attrs.get("class"), Some(RawValue::String(s)) if matches!(
        s.as_str(), "primary" | "secondary" | "tertiary" | "minor" | "service" |
            "busway" | "bus_guideway"
    )) || attrs.contains_key("brunnel")
        || !match attrs.get("layer") {
            None => true,
            Some(RawValue::String(s)) => s.parse::<f64>() == Ok(0.),
            Some(v) => number(v) == Some(0.),
        }
        || !match attrs.get("access") {
            None => true, // simulated eligibility of a supported street class
            Some(RawValue::String(s)) => matches!(s.as_str(), "yes" | "permissive" | "designated"),
            _ => false,
        }
    {
        return None;
    }
    let tagged = oneway(attrs.get("oneway"))?;
    Some(if respect_oneway {
        tagged
    } else {
        Direction::Both
    })
}

#[derive(Debug, Clone, Copy)]
struct Arc {
    to: usize,
    line: usize,
    ordinal: usize,
    from_pos: f64,
    to_pos: f64,
    weight: u64,
}

#[derive(Clone, Copy)]
struct Step(u64, usize);
impl PartialEq for Step {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Step {}
impl PartialOrd for Step {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Step {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.cmp(&self.0).then_with(|| other.1.cmp(&self.1))
    }
}

fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    let lon = (b[0] - a[0]).to_radians();
    let lat = (b[1] - a[1]).to_radians();
    let h = (lat / 2.).sin().powi(2)
        + a[1].to_radians().cos() * b[1].to_radians().cos() * (lon / 2.).sin().powi(2);
    2. * 6_371_000. * h.clamp(0., 1.).sqrt().asin()
}

fn fragment(tile: &TileBinding, line: &TileLine) -> String {
    format!(
        "ofm:{}:14:{}:{}:{}:{}",
        tile.revision, tile.x, tile.y, line.feature_index, line.line_index
    )
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Cell(u32, u32);

#[derive(Clone, Copy)]
struct Boundary {
    axis: usize,
    /// The lower-coordinate tile owns the first side (left/top).
    lower_tile: bool,
}

impl Boundary {
    fn offset(self, extent: i32) -> i32 {
        if self.lower_tile {
            extent
        } else {
            0
        }
    }
    fn inner(self, extent: i32, coordinate: [i32; 2]) -> bool {
        let q = coordinate[self.axis];
        if self.lower_tile {
            q < extent
        } else {
            q > 0
        }
    }
}

/// Exact ratio along an original segment; used both for power-of-two tile
/// grids and for deterministic arbitrary valid MVT extent.
#[derive(Clone, Copy)]
struct Fraction(u64, u64);
impl Fraction {
    fn less(self, rhs: Self) -> bool {
        u128::from(self.0) * u128::from(rhs.1) < u128::from(rhs.0) * u128::from(self.1)
    }
    fn as_f64(self) -> f64 {
        self.0 as f64 / self.1 as f64
    }
    fn weight_to(self, rhs: Self, whole: u64) -> u64 {
        let (a, b, c, d) = (
            u128::from(self.0),
            u128::from(self.1),
            u128::from(rhs.0),
            u128::from(rhs.1),
        );
        let parts = (a * d).abs_diff(c * b);
        (u128::from(whole) * parts / (b * d)).max(1) as u64
    }
}

struct Cut {
    segment: usize,
    fraction: Fraction,
    orthogonal: Fraction,
}
fn seam_cut(line: &TileLine, boundary: Boundary) -> Option<Cut> {
    let extent = line.extent as i32;
    let axis = boundary.axis;
    let seam = boundary.offset(extent);
    let mut cut = None;
    for (i, w) in line.coordinates.windows(2).enumerate() {
        let [a, b] = [w[0], w[1]];
        if boundary.inner(extent, a) == boundary.inner(extent, b) {
            continue;
        }
        let den = i64::from(a[axis]).abs_diff(i64::from(b[axis]));
        let num = i64::from(a[axis]).abs_diff(i64::from(seam));
        if den == 0 || num > den {
            return None;
        }
        let orth = (den as i64 - num as i64) * i64::from(a[1 - axis])
            + num as i64 * i64::from(b[1 - axis]);
        // No diagonal corner edges. The existing candidate verifier further
        // excludes the surrounding corner band, near-parallel aliases, etc.
        if orth <= 0 || orth >= i64::from(extent) * den as i64 {
            continue;
        }
        if cut.is_some() {
            return None;
        }
        cut = Some(Cut {
            segment: i,
            fraction: Fraction(num, den),
            orthogonal: Fraction(orth as u64, den),
        });
    }
    cut
}

#[derive(Clone, Copy)]
struct Port {
    fraction: Fraction,
    node: usize,
}

struct Line<'a> {
    tile: &'a TileBinding,
    raw: &'a TileLine,
    base: usize,
    direction: Direction,
    ports: BTreeMap<usize, Vec<Port>>,
}
type Groups = BTreeMap<(Cell, [i32; 2]), Vec<(usize, usize, bool)>>;

impl BusPathBinding {
    pub(crate) fn swapped_endpoints(&self) -> Result<Self> {
        let mut reverse = self.clone();
        std::mem::swap(&mut reverse.start, &mut reverse.end);
        if let Some(key) = self
            .end_tile_key
            .as_deref()
            .filter(|key| *key != self.tile_key)
        {
            let target = reverse
                .additional_tiles
                .iter_mut()
                .find(|tile| tile.tile_key == key)
                .ok_or(Error::Binding)?;
            std::mem::swap(&mut reverse.tile_key, &mut target.tile_key);
            std::mem::swap(&mut reverse.tile_raw_sha256, &mut target.tile_raw_sha256);
            reverse.end_tile_key = Some(self.tile_key.clone());
            reverse
                .additional_tiles
                .sort_by(|a, b| a.tile_key.cmp(&b.tile_key));
        }
        reverse.via.reverse();
        Ok(reverse)
    }

    /// Syntactic source request only; exact digests are resolved from the store.
    pub(crate) fn tile_keys(&self) -> Result<Vec<String>> {
        match self.rule.as_str() {
            BUS_GRAPH_RULE if self.additional_tiles.is_empty() && self.end_tile_key.is_none() => {}
            BUS_RECTANGLE_RULE
                if !self.additional_tiles.is_empty()
                    && self.additional_tiles.len() < MAX_TILES
                    && self.end_tile_key.as_ref().is_some_and(|end| {
                        end == &self.tile_key
                            || self.additional_tiles.iter().any(|t| &t.tile_key == end)
                    }) => {}
            _ => return Err(Error::Unsupported),
        }
        let mut keys = vec![self.tile_key.clone()];
        keys.extend(self.additional_tiles.iter().map(|p| p.tile_key.clone()));
        keys.sort_unstable();
        if keys.windows(2).any(|k| k[0] == k[1]) {
            return Err(Error::Binding);
        }
        if self.via.len() > MAX_VIAS {
            return Err(Error::Limit);
        }
        let mut ids = BTreeSet::new();
        for via in &self.via {
            if via.id.is_empty()
                || via.id.len() > 80
                || !via
                    .id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
                || !ids.insert(via.id.as_str())
                || keys.binary_search(&via.tile_key).is_err()
            {
                return Err(Error::Binding);
            }
        }
        Ok(keys)
    }
}

/// One-cell compatibility entry point; multi-cell worlds call reconstruct with
/// a bounded rectangle. Never receives any asserted topology from a caller.
pub fn find_gameplay_bus_path(
    store: &EvidenceStore,
    tile_key: &str,
    start: BusVertex,
    end: BusVertex,
    respect_oneway: bool,
) -> Result<Option<BusPath>> {
    let tile = store.decoded(tile_key)?;
    let request = BusPathBinding {
        rule: BUS_GRAPH_RULE.into(),
        tile_key: tile_key.into(),
        tile_raw_sha256: tile.binding.raw_sha256.clone(),
        respect_oneway,
        start,
        end,
        additional_tiles: vec![],
        end_tile_key: None,
        pattern: None,
        interior_stops: vec![],
        via: vec![],
    };
    solve_gameplay_bus_scope(store, &request)
}

#[derive(Debug)]
struct BusGraphLine {
    tile_key: String,
    fragment_id: String,
    extent: u32,
    metric_scale: u128,
}
#[derive(Debug)]
pub(super) struct GameplayBusGraph {
    lines: Vec<BusGraphLine>,
    offsets: BTreeMap<String, BTreeMap<(u32, u32), (usize, usize)>>,
    representative: Vec<usize>,
    geometry: Vec<BusGridPoint>,
    adj: Vec<Vec<Arc>>,
}
impl GameplayBusGraph {
    fn node_at(&self, key: &str, v: BusVertex) -> Option<usize> {
        let &(base, n) = self.offsets.get(key)?.get(&(v.feature, v.line))?;
        let i = v.vertex as usize;
        (i < n).then(|| self.representative[base + i])
    }
    pub(super) fn fits_cache(&self) -> bool {
        // Bound the one retained compiled graph independently from tile/path
        // caches. Candidate graph buffers are still subject to the graph cap.
        let bound = self.adj.capacity() * std::mem::size_of::<Vec<Arc>>()
            + self
                .adj
                .iter()
                .map(|a| a.capacity() * std::mem::size_of::<Arc>())
                .sum::<usize>()
            + self.geometry.capacity() * std::mem::size_of::<BusGridPoint>()
            + self.representative.capacity() * std::mem::size_of::<usize>()
            + self
                .offsets
                .iter()
                .map(|(key, map)| key.capacity() + 128 + map.len() * 64)
                .sum::<usize>()
            + self
                .lines
                .iter()
                .map(|line| {
                    line.tile_key.capacity()
                        + line.fragment_id.capacity()
                        + std::mem::size_of::<BusGraphLine>()
                })
                .sum::<usize>();
        bound <= 24 * 1024 * 1024
    }
}

/// Scope contains every requested tile checked against its exact digest.
/// Build once from the scoped evidence, independent of leg count.
pub(super) fn build_gameplay_bus_graph(
    store: &EvidenceStore,
    query: &BusPathBinding,
) -> Result<GameplayBusGraph> {
    let keys = query.tile_keys()?;
    let mut tiles = keys
        .iter()
        .map(|k| store.decoded(k))
        .collect::<Result<Vec<_>>>()?;
    tiles.sort_by_key(|tile| (tile.binding.x, tile.binding.y));
    let cells: BTreeSet<_> = tiles
        .iter()
        .map(|t| Cell(t.binding.x, t.binding.y))
        .collect();
    let min_x = cells.iter().map(|c| c.0).min().ok_or(Error::Binding)?;
    let min_y = cells.iter().map(|c| c.1).min().ok_or(Error::Binding)?;
    let max_x = cells.iter().map(|c| c.0).max().ok_or(Error::Binding)?;
    let max_y = cells.iter().map(|c| c.1).max().ok_or(Error::Binding)?;
    if cells.len() != keys.len()
        || (max_x - min_x + 1) * (max_y - min_y + 1) != cells.len() as u32
        || tiles
            .iter()
            .any(|t| t.binding.revision != tiles[0].binding.revision)
    {
        return Err(Error::Binding);
    }
    let mut extents = tiles.iter().flat_map(|t| t.lines.iter()).map(|l| l.extent);
    if let Some(first) = extents.next() {
        if query.rule == BUS_RECTANGLE_RULE && extents.any(|extent| extent != first) {
            return Err(Error::Unsupported);
        }
    }

    let owned = |tile: &TileBinding, extent: i32, [x, y]: [i32; 2]| {
        x >= 0
            && x < extent
            && y >= 0
            && y < extent
            && !(x == 0 && tile.x > min_x)
            && !(y == 0 && tile.y > min_y)
    };
    let mut groups: Groups = BTreeMap::new();
    let mut lines = Vec::<Line<'_>>::new();
    let mut offsets: BTreeMap<String, BTreeMap<(u32, u32), (usize, usize)>> = BTreeMap::new();
    let mut geometry = Vec::new();
    let mut ids = BTreeMap::new();
    for tile in tiles {
        for raw in &tile.lines {
            let Some(direction) = eligible(raw, query.respect_oneway) else {
                continue;
            };
            if raw.coordinates.windows(2).any(|p| p[0] == p[1]) {
                continue;
            }
            let base = geometry.len();
            let n = raw.coordinates.len();
            if base + n > MAX_GRAPH_POINTS {
                return Err(Error::Limit);
            }
            for (i, &p) in raw.coordinates.iter().enumerate() {
                if owned(&tile.binding, raw.extent as i32, p) {
                    groups
                        .entry((Cell(tile.binding.x, tile.binding.y), p))
                        .or_default()
                        .push((lines.len(), base + i, i == 0 || i + 1 == n));
                }
                geometry.push(BusGridPoint::new(
                    &tile.binding,
                    raw.extent,
                    p.map(|q| (i64::from(q), 1)),
                ));
            }
            ids.insert(fragment(&tile.binding, raw), lines.len());
            offsets
                .entry(tile.binding.canonical_url.clone())
                .or_default()
                .insert((raw.feature_index, raw.line_index), (base, n));
            lines.push(Line {
                tile: &tile.binding,
                raw,
                base,
                direction,
                ports: BTreeMap::new(),
            });
        }
    }

    let mut representative: Vec<_> = (0..geometry.len()).collect();
    for group in groups.values().filter(|g| g.len() > 1 && g.len() <= 4) {
        let mut distinct = BTreeSet::new();
        if group.iter().filter(|(_, _, e)| !e).count() > 1
            || !group.iter().all(|(line, _, _)| distinct.insert(*line))
        {
            continue;
        }
        for &(_, id, _) in group {
            representative[id] = group[0].1;
        }
    }

    if query.rule == BUS_RECTANGLE_RULE {
        let coords: Vec<_> = cells.iter().map(|c| (c.0, c.1)).collect();
        for seam in super::candidates::boundary_candidates_for_bus(store, &coords)? {
            let (Some(&a), Some(&b)) = (ids.get(&seam.a_fragment), ids.get(&seam.b_fragment))
            else {
                continue;
            };
            let axis = usize::from(lines[a].tile.x == lines[b].tile.x);
            let a_direction = if axis == 0 {
                lines[a].tile.x < lines[b].tile.x
            } else {
                lines[a].tile.y < lines[b].tile.y
            };
            let (low, high) = if a_direction { (a, b) } else { (b, a) };
            let a_line = &lines[low];
            let b_line = &lines[high];
            let (Some(a_cut), Some(b_cut)) = (
                seam_cut(
                    a_line.raw,
                    Boundary {
                        axis,
                        lower_tile: true,
                    },
                ),
                seam_cut(
                    b_line.raw,
                    Boundary {
                        axis,
                        lower_tile: false,
                    },
                ),
            ) else {
                continue;
            };
            if a_cut.orthogonal.less(b_cut.orthogonal) || b_cut.orthogonal.less(a_cut.orthogonal) {
                continue;
            }
            let extent = a_line.raw.extent;
            let orth = (a_cut.orthogonal.0 as i64, a_cut.orthogonal.1);
            let edge = (i64::from(extent), 1);
            let offsets = if axis == 0 {
                [edge, orth]
            } else {
                [orth, edge]
            };
            let node = geometry.len();
            if node >= MAX_GRAPH_POINTS {
                return Err(Error::Limit);
            }
            geometry.push(BusGridPoint::new(a_line.tile, extent, offsets));
            for (index, cut) in [(low, a_cut), (high, b_cut)] {
                lines[index]
                    .ports
                    .entry(cut.segment)
                    .or_default()
                    .push(Port {
                        node,
                        fraction: cut.fraction,
                    });
            }
        }
    }

    let mut adj = vec![Vec::new(); geometry.len()];
    for (line_index, line) in lines.iter().enumerate() {
        for i in 0..line.raw.coordinates.len() - 1 {
            let [a, b] = [line.raw.coordinates[i], line.raw.coordinates[i + 1]];
            let extent = line.raw.extent as i32;
            let a_inside = owned(line.tile, extent, a);
            let b_inside = owned(line.tile, extent, b);
            let mut ports = line.ports.get(&i).cloned().unwrap_or_default();
            if ports.is_empty() && !(a_inside && b_inside) {
                continue;
            }
            if a_inside {
                ports.push(Port {
                    node: representative[line.base + i],
                    fraction: Fraction(0, 1),
                });
            }
            if b_inside {
                ports.push(Port {
                    node: representative[line.base + i + 1],
                    fraction: Fraction(1, 1),
                });
            }
            if ports.len() != 2 {
                continue;
            }
            if ports[1].fraction.less(ports[0].fraction) {
                ports.swap(0, 1)
            }
            let [from, to] = [ports[0], ports[1]];
            if !from.fraction.less(to.fraction) || from.node == to.node {
                continue;
            }
            let dx = i128::from(a[0]) - i128::from(b[0]);
            let dy = i128::from(a[1]) - i128::from(b[1]);
            let whole = (((dx * dx + dy * dy) as u128) * 1_000_000_000_000).isqrt() as u64;
            let arc = Arc {
                to: to.node,
                line: line_index,
                ordinal: i,
                from_pos: i as f64 + from.fraction.as_f64(),
                to_pos: i as f64 + to.fraction.as_f64(),
                weight: from.fraction.weight_to(to.fraction, whole),
            };
            if !matches!(line.direction, Direction::Reverse) {
                adj[from.node].push(arc);
            }
            if !matches!(line.direction, Direction::Forward) {
                adj[to.node].push(Arc {
                    to: from.node,
                    from_pos: arc.to_pos,
                    to_pos: arc.from_pos,
                    ..arc
                });
            }
        }
    }
    let scales: BTreeMap<_, _> = cells.iter().map(|c| (c.1, bus_metric_scale(c.1))).collect();
    let lines = lines
        .into_iter()
        .map(|line| BusGraphLine {
            tile_key: line.tile.canonical_url.clone(),
            fragment_id: fragment(line.tile, line.raw),
            extent: line.raw.extent,
            metric_scale: scales[&line.tile.y],
        })
        .collect();
    Ok(GameplayBusGraph {
        lines,
        offsets,
        representative,
        geometry,
        adj,
    })
}

struct SearchScratch {
    distances: Vec<u64>,
    generation: Vec<u32>,
    active: u32,
    parent: Vec<Option<(usize, Arc)>>,
    queue: BinaryHeap<Step>,
}
impl SearchScratch {
    fn new(n: usize) -> Self {
        Self {
            distances: vec![0; n],
            generation: vec![0; n],
            active: 0,
            parent: vec![None; n],
            queue: BinaryHeap::new(),
        }
    }
    fn distance(&self, node: usize) -> u64 {
        if self.generation[node] == self.active {
            self.distances[node]
        } else {
            u64::MAX
        }
    }
    fn update(&mut self, node: usize, value: u64) {
        self.generation[node] = self.active;
        self.distances[node] = value;
        self.queue.push(Step(value, node));
    }
    fn leg(
        &mut self,
        graph: &GameplayBusGraph,
        source: usize,
        target: usize,
        budget: &mut usize,
        reversed_atoms: &mut Vec<Arc>,
    ) -> Result<bool> {
        if source == target {
            return Ok(false);
        }
        self.active = self.active.checked_add(1).ok_or(Error::Limit)?;
        self.queue.clear();
        self.update(source, 0);
        while let Some(Step(cost, u)) = self.queue.pop() {
            if cost > self.distance(u) {
                continue;
            }
            if u == target {
                break;
            }
            for &arc in &graph.adj[u] {
                *budget = budget.checked_sub(1).ok_or(Error::Limit)?;
                let next = cost.checked_add(arc.weight).ok_or(Error::Limit)?;
                if next < self.distance(arc.to) {
                    self.parent[arc.to] = Some((u, arc));
                    self.update(arc.to, next);
                }
            }
        }
        if self.distance(target) == u64::MAX {
            return Ok(false);
        }
        let begin = reversed_atoms.len();
        let mut u = target;
        while u != source {
            let (p, arc) = self.parent[u].ok_or(Error::Malformed)?;
            reversed_atoms.push(arc);
            u = p;
        }
        reversed_atoms[begin..].reverse();
        Ok(true)
    }
}

pub(super) fn solve_gameplay_bus_scope(
    store: &EvidenceStore,
    query: &BusPathBinding,
) -> Result<Option<BusPath>> {
    solve_gameplay_bus_graph(&build_gameplay_bus_graph(store, query)?, query)
}

pub(super) fn solve_gameplay_bus_graph(
    graph: &GameplayBusGraph,
    query: &BusPathBinding,
) -> Result<Option<BusPath>> {
    let failed = |i| {
        if query.via.is_empty() {
            Ok(None)
        } else {
            Err(Error::NoBusLeg(i))
        }
    };
    let Some(source) = graph.node_at(&query.tile_key, query.start) else {
        return failed(0);
    };
    let end_key = query.end_tile_key.as_deref().unwrap_or(&query.tile_key);
    let mut nodes = Vec::with_capacity(query.via.len() + 2);
    nodes.push(source);
    for (i, via) in query.via.iter().enumerate() {
        let Some(node) = graph.node_at(&via.tile_key, via.vertex) else {
            return failed(i);
        };
        nodes.push(node);
    }
    let Some(target) = graph.node_at(end_key, query.end) else {
        return failed(query.via.len());
    };
    if query.pattern == Some(BusPattern::ClosedOneWay) && (source != target || query.via.is_empty())
    {
        return Err(Error::InvalidBusCircuit);
    }
    nodes.push(target);
    let mut via_point_indices = Vec::with_capacity(query.via.len());
    let mut scratch = SearchScratch::new(graph.geometry.len());
    let mut edges = Vec::new();
    let mut budget = if query.via.is_empty() {
        usize::MAX
    } else {
        MAX_MULTI_RELAXATIONS
    };
    for (i, leg) in nodes.windows(2).enumerate() {
        if !scratch.leg(graph, leg[0], leg[1], &mut budget, &mut edges)? {
            return failed(i);
        }
        if !query.via.is_empty() && edges.len() > MAX_MULTI_ATOMS {
            return Err(Error::Limit);
        }
        if i < query.via.len() {
            via_point_indices.push(edges.len());
        }
    }
    let mut spans: Vec<SourceSpan> = Vec::new();
    let mut steps = Vec::with_capacity(edges.len());
    let mut points_grid = vec![graph.geometry[source]];
    let mut length_um = 0u128;
    let mut atom_operation_micrometres = Vec::new();
    let round_trip = query.pattern == Some(BusPattern::OpenRoundTrip);
    if round_trip && edges.len() > 4096 {
        return Err(Error::Limit);
    }
    for arc in edges {
        let line = &graph.lines[arc.line];
        let (from, to) = (arc.from_pos, arc.to_pos);
        let fragment_id = &line.fragment_id;
        let part = bus_arc_micrometres(arc.weight, line.extent, line.metric_scale);
        length_um = length_um.checked_add(part).ok_or(Error::Limit)?;
        atom_operation_micrometres.push(u64::try_from(part).map_err(|_| Error::Limit)?);
        if let Some(tail) = spans.last_mut().filter(|s| {
            &s.fragment_id == fragment_id
                && s.to == from
                && (s.to - s.from).signum() == (to - from).signum()
        }) {
            tail.to = to;
        } else {
            spans.push(SourceSpan {
                binding_key: line.tile_key.clone(),
                fragment_id: fragment_id.clone(),
                from,
                to,
            });
        }
        steps.push(BusSourceStep {
            span: spans.len() - 1,
            raw_ordinal: arc.ordinal,
            from,
            to,
        });
        points_grid.push(graph.geometry[arc.to]);
    }
    let points: Vec<_> = points_grid.iter().map(|p| p.point()).collect();
    let length_m = points.windows(2).map(|p| distance(p[0], p[1])).sum();
    Ok(Some(BusPath {
        binding: query.clone(),
        spans,
        steps,
        points_grid,
        points,
        length_m,
        operation_length_mm: u64::try_from((length_um + 500) / 1000).map_err(|_| Error::Limit)?,
        atom_operation_micrometres,
        via_point_indices,
    }))
}

/// Web-Mercator's local ground scale is 1/cosh(y). Use tile-centre
/// scale and fixed integer arithmetic; portable save replay cannot depend
/// on platform sinh/haversine. This approximates physical distances across
/// an individual z14 tile, not travel speed or real-road access.
fn bus_metric_scale(y: u32) -> u128 {
    const SCALE: u128 = 1_000_000_000_000;
    const PI: u128 = 3_141_592_653_590;
    let x = PI * u128::from((16_384i64 - (2 * i64::from(y) + 1)).unsigned_abs()) / 16_384;
    let x2 = x * x;
    let mut term = SCALE;
    let mut cosh = SCALE;
    for n in 1..=12u128 {
        term = term * x2 / (SCALE * SCALE * (2 * n - 1) * (2 * n));
        cosh += term;
    }
    cosh
}

fn bus_arc_micrometres(weight: u64, extent: u32, cosh: u128) -> u128 {
    const EARTH_CIRCUMFERENCE_MM: u128 = 40_030_173_592;
    // Cancel the weight's microgrid scale before multiplying. Even
    // the maximum accepted input extent leaves ample headroom in u128.
    u128::from(weight) * EARTH_CIRCUMFERENCE_MM * 1_000_000_000
        / (16_384 * u128::from(extent) * cosh)
}

/// Reconstruct only from evidence in the world archive, never from caller paths.
pub fn reconstruct_gameplay_bus_path(
    store: &EvidenceStore,
    proposed: &BusPathBinding,
) -> Result<Option<BusPath>> {
    match store.gameplay_bus_path(proposed) {
        Err(Error::NoBusLeg(_)) => Ok(None),
        result => result.map(|path| path.as_deref().cloned()),
    }
}
