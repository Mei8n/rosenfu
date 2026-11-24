//! Bounded OFM bus gameplay graph: legacy ground-only and explicitly observed
//! grade continuations. Not traffic authorization, guaranteed coverage or a road catalog.
use super::bus_grade::{self, Grade};
use super::{
    number, Error, EvidenceStore, RawValue, Result, SourceSpan, TileBinding, TileLine, MAX_TILES,
};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

mod tram_chain;
pub use tram_chain::{reconstruct_authored_tram_chain, AuthoredTramChainGuide};
mod fractional;
pub use fractional::{resolve_fractional_bus_traversal, FractionalBusTraversal};

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

/// Opted-in structure-aware topology. Absent uses the original ground-only
/// routing rule to reproduce already authored and saved paths exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BusGradePolicy {
    ObservedConnectors,
}

/// A reproducible request, not a proof or an independently trusted saved route.
/// Call the solver again against the matching verified tile to reconstruct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusPathBinding {
    pub rule: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grade_policy: Option<BusGradePolicy>,
    pub tile_key: String,
    pub tile_raw_sha256: String,
    pub respect_oneway: bool,
    pub start: BusVertex,
    pub end: BusVertex,
    /// Optional interior boarding cuts along one complete directed raw atom.
    /// Raw vertex endpoints and physical resource identity are unchanged.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "read_same_atom_stops"
    )]
    pub same_atom_stops: Option<[u32; 2]>,
    /// Independently chosen lower raw atoms with forward-source millionths.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "read_same_atom_stops"
    )]
    pub fractional_terminals: Option<[u32; 2]>,
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

fn read_same_atom_stops<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<[u32; 2]>, D::Error> {
    <[u32; 2]>::deserialize(deserializer).map(Some)
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
    pub(crate) resolved_terminal_cuts: Option<[u32; 2]>,
    /// Derived point indices in exact authored via order, not a stop decision.
    pub via_point_indices: Vec<usize>,
    pub(crate) steps: Vec<BusSourceStep>,
    pub(crate) points_grid: Vec<BusGridPoint>,
    node_suffixes: Vec<String>,
}

impl BusPath {
    /// Directed materialized identity shared by every ordinary route that
    /// traverses this exact verified raw segment (or owned seam-clipped part).
    pub(crate) fn node_id_at(&self, i: usize) -> Option<String> {
        let grid = self.points_grid.get(i)?;
        let suffix = self.node_suffixes.get(i)?;
        Some(format!("{}{suffix}", grid.id()))
    }
    pub(crate) fn atom_is_ground(&self, i: usize) -> bool {
        self.steps.get(i).is_some_and(|step| step.ground)
    }
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
    /// Bounds on the full oriented first/last atoms when the preview clips
    /// those atoms for a fractional Bus route.
    pub fn stop_target_terminal_cuts(&self) -> Option<[u32; 2]> {
        self.resolved_terminal_cuts
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
    ground: bool,
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
        super::road_grid_lon_lat(x, y)
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

fn supports_ground_road_geometry(line: &TileLine) -> bool {
    let attrs = &line.attributes;
    matches!(attrs.get("class"), Some(RawValue::String(s)) if matches!(
        s.as_str(), "primary" | "secondary" | "tertiary" | "minor" | "service" |
            "busway" | "bus_guideway"
    )) && !attrs.contains_key("brunnel")
        && match attrs.get("layer") {
            None => true,
            Some(RawValue::String(s)) => s.parse::<f64>() == Ok(0.),
            Some(v) => number(v) == Some(0.),
        }
        && match attrs.get("access") {
            None => true, // simulated eligibility of a supported street class
            Some(RawValue::String(s)) => matches!(s.as_str(), "yes" | "permissive" | "designated"),
            _ => false,
        }
}

pub(super) fn supports_authored_ground_tram(line: &TileLine) -> bool {
    supports_ground_road_geometry(line)
        && !matches!(line.attributes.get("class"), Some(RawValue::String(s)) if s == "busway" || s == "bus_guideway")
}

fn eligible(line: &TileLine, respect_oneway: bool, structured: bool) -> Option<(Direction, Grade)> {
    if structured {
        if !bus_grade::supported_class(line) || !bus_grade::allowed_access(line) {
            return None;
        }
        let grade = bus_grade::grade(line)?;
        let tagged = oneway(line.attributes.get("oneway"))?;
        return Some((
            if respect_oneway {
                tagged
            } else {
                Direction::Both
            },
            grade,
        ));
    }
    if !supports_ground_road_geometry(line) {
        return None;
    }
    let tagged = oneway(line.attributes.get("oneway"))?;
    Some((
        if respect_oneway {
            tagged
        } else {
            Direction::Both
        },
        Grade::Ground,
    ))
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
    grade: Grade,
    ports: BTreeMap<usize, Vec<Port>>,
}
type Groups = BTreeMap<(Cell, [i32; 2]), Vec<(usize, usize, bool)>>;

impl BusPathBinding {
    pub fn has_fractional_stops(&self) -> bool {
        self.same_atom_stops.is_some() || self.fractional_terminals.is_some()
    }

    pub(crate) fn swapped_endpoints(&self) -> Result<Self> {
        if self.fractional_terminals.is_some() {
            return Err(Error::Unsupported);
        }
        let mut reverse = self.clone();
        std::mem::swap(&mut reverse.start, &mut reverse.end);
        if let Some([a, z]) = self.same_atom_stops {
            reverse.same_atom_stops = Some([
                BUS_STOP_DENOMINATOR.checked_sub(z).ok_or(Error::Binding)?,
                BUS_STOP_DENOMINATOR.checked_sub(a).ok_or(Error::Binding)?,
            ]);
        }
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
        if let Some(cuts) = self.fractional_terminals {
            if self.same_atom_stops.is_some()
                || self.pattern.is_some()
                || !self.via.is_empty()
                || cuts.iter().any(|q| !(1..BUS_STOP_DENOMINATOR).contains(q))
                || (self.end_tile_key.as_deref().unwrap_or(&self.tile_key) == self.tile_key
                    && self.start == self.end
                    && cuts[0] == cuts[1])
            {
                return Err(Error::Binding);
            }
        }
        if let Some([from, to]) = self.same_atom_stops {
            if self.rule != BUS_GRAPH_RULE
                || self.grade_policy.is_some()
                || self.pattern == Some(BusPattern::ClosedOneWay)
                || !self.via.is_empty()
                || !self.interior_stops.is_empty()
                || self.start.feature != self.end.feature
                || self.start.line != self.end.line
                || self.start.vertex.abs_diff(self.end.vertex) != 1
                || from == 0
                || from >= to
                || to >= BUS_STOP_DENOMINATOR
            {
                return Err(Error::Binding);
            }
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
        same_atom_stops: None,
        fractional_terminals: None,
        grade_policy: None,
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
    untyped_unreachable(solve_gameplay_bus_scope(store, &request))
}

#[derive(Debug)]
struct BusGraphLine {
    boardable_ground: bool,
    tile_key: String,
    fragment_id: String,
    extent: u32,
    metric_scale: u128,
    grade: Grade,
}
#[derive(Debug, Clone, Copy)]
enum NodeSource {
    Surface,
    Vertex {
        grade: Grade,
        line: usize,
        raw_vertex: usize,
    },
    Seam {
        grade: Grade,
        one: usize,
        two: usize,
    },
}

#[derive(Debug)]
pub(super) struct GameplayBusGraph {
    lines: Vec<BusGraphLine>,
    offsets: BTreeMap<String, BTreeMap<(u32, u32), (usize, usize)>>,
    representative: Vec<usize>,
    geometry: Vec<BusGridPoint>,
    node_sources: Vec<NodeSource>,
    adj: Vec<Vec<Arc>>,
}
impl GameplayBusGraph {
    fn node_at(&self, key: &str, v: BusVertex) -> Option<usize> {
        let &(base, n) = self.offsets.get(key)?.get(&(v.feature, v.line))?;
        let i = v.vertex as usize;
        (i < n).then(|| self.representative[base + i])
    }
    fn node_suffix(&self, index: usize) -> String {
        let (grade, source) = match self.node_sources[index] {
            NodeSource::Surface => return String::new(),
            NodeSource::Vertex {
                grade,
                line,
                raw_vertex,
            } => (
                grade,
                format!("v\0{}\0{raw_vertex}", self.lines[line].fragment_id),
            ),
            NodeSource::Seam { grade, one, two } => (
                grade,
                format!(
                    "s\0{}\0{}",
                    self.lines[one].fragment_id, self.lines[two].fragment_id
                ),
            ),
        };
        format!(
            "{}:{}",
            grade.node_suffix(),
            super::raw_sha256(source.as_bytes())
        )
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
            + self.node_sources.capacity() * std::mem::size_of::<NodeSource>()
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
    build_observed_road_graph(store, query, false, None)
}

fn build_observed_road_graph(
    store: &EvidenceStore,
    query: &BusPathBinding,
    authored_tram_guide: bool,
    tram_chain: Option<&BTreeMap<Cell, usize>>,
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
        || (tram_chain.is_none() && (max_x - min_x + 1) * (max_y - min_y + 1) != cells.len() as u32)
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

    let has_west =
        |x, y| tram_chain.map_or(x > min_x, |_| x > 0 && cells.contains(&Cell(x - 1, y)));
    let has_north =
        |x, y| tram_chain.map_or(y > min_y, |_| y > 0 && cells.contains(&Cell(x, y - 1)));
    let owned = |tile: &TileBinding, extent: i32, [x, y]: [i32; 2]| {
        x >= 0
            && x < extent
            && y >= 0
            && y < extent
            && !(x == 0 && has_west(tile.x, tile.y))
            && !(y == 0 && has_north(tile.x, tile.y))
    };
    let structured = query.grade_policy == Some(BusGradePolicy::ObservedConnectors);
    let mut groups: Groups = BTreeMap::new();
    let mut observed_counts: BTreeMap<_, usize> = BTreeMap::new();
    let mut node_sources = Vec::new();
    let mut lines = Vec::<Line<'_>>::new();
    let mut offsets: BTreeMap<String, BTreeMap<(u32, u32), (usize, usize)>> = BTreeMap::new();
    let mut geometry = Vec::new();
    let mut ids = BTreeMap::new();
    for tile in tiles {
        for raw in &tile.lines {
            if structured || authored_tram_guide {
                for &p in &raw.coordinates {
                    if owned(&tile.binding, raw.extent as i32, p) {
                        *observed_counts
                            .entry((Cell(tile.binding.x, tile.binding.y), p))
                            .or_default() += 1;
                    }
                }
            }
            let accepted = if authored_tram_guide {
                // Source motor direction is not an in-game rail design rule.
                // Do not enter explicit bus-only, unavailable or non-ground
                // roads, and do not grant the graph any transport authority.
                supports_authored_ground_tram(raw).then_some((Direction::Both, Grade::Ground))
            } else {
                eligible(raw, query.respect_oneway, structured)
            };
            let Some((direction, grade)) = accepted else {
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
                node_sources.push(
                    if grade.is_ground() && (!structured || !bus_grade::fast_class(raw)) {
                        NodeSource::Surface
                    } else {
                        NodeSource::Vertex {
                            grade,
                            line: lines.len(),
                            raw_vertex: i,
                        }
                    },
                );
                geometry.push(BusGridPoint::new(
                    &tile.binding,
                    raw.extent,
                    p.map(|q| (i64::from(q), 1)),
                ));
            }
            ids.insert(super::raw_road_fragment_id(&tile.binding, raw), lines.len());
            offsets
                .entry(tile.binding.canonical_url.clone())
                .or_default()
                .insert((raw.feature_index, raw.line_index), (base, n));
            lines.push(Line {
                tile: &tile.binding,
                raw,
                base,
                direction,
                grade,
                ports: BTreeMap::new(),
            });
        }
    }

    let mut representative: Vec<_> = (0..geometry.len()).collect();
    let join_surface = |group: &[(usize, usize, bool)], representative: &mut [usize]| {
        if !(2..=4).contains(&group.len()) {
            return;
        }
        let mut distinct = BTreeSet::new();
        if group.iter().filter(|(_, _, e)| !e).count() > 1
            || !group.iter().all(|(line, _, _)| distinct.insert(*line))
        {
            return;
        }
        for &(_, id, _) in group {
            representative[id] = group[0].1;
        }
    };
    for (coordinate, group) in &groups {
        if !structured {
            // An ineligible/unknown arm at the exact source vertex makes
            // an authored Tram junction unresolved. Bank no new authority.
            if !authored_tram_guide || observed_counts.get(coordinate) == Some(&group.len()) {
                join_surface(group, &mut representative);
            }
            continue;
        }
        let ground: Vec<_> = group
            .iter()
            .copied()
            .filter(|&(line, _, _)| {
                lines[line].grade.is_ground() && !bus_grade::fast_class(lines[line].raw)
            })
            .collect();
        join_surface(&ground, &mut representative);
        if observed_counts.get(coordinate) != Some(&2) {
            continue;
        }
        let [(a, one, true), (b, two, true)] = group.as_slice() else {
            continue;
        };
        if a == b
            || (lines[*a].grade.is_ground()
                && lines[*b].grade.is_ground()
                && !bus_grade::fast_class(lines[*a].raw)
                && !bus_grade::fast_class(lines[*b].raw))
        {
            continue;
        }
        let left = &lines[*a];
        let right = &lines[*b];
        if bus_grade::exact_continuation(
            left.raw,
            *one - left.base,
            left.grade,
            right.raw,
            *two - right.base,
            right.grade,
        ) {
            let chosen = if right.grade.is_ground() && !left.grade.is_ground() {
                *two
            } else {
                *one
            };
            representative[*one] = chosen;
            representative[*two] = chosen;
        }
    }

    let mut forward_seams = BTreeMap::new();
    if query.rule == BUS_RECTANGLE_RULE {
        let coords: Vec<_> = cells.iter().map(|c| (c.0, c.1)).collect();
        let seams = if authored_tram_guide {
            super::candidates::boundary_candidates_for_tram(store, &coords)?
        } else {
            super::candidates::boundary_candidates_for_bus(store, &coords, structured)?
        };
        for seam in seams {
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
            if a_line.grade != b_line.grade {
                continue;
            }
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
            if let Some(rank) = tram_chain {
                let a = Cell(a_line.tile.x, a_line.tile.y);
                let b = Cell(b_line.tile.x, b_line.tile.y);
                // Only leave a seam towards the next requested tile. The
                // caller validates that there are no nonconsecutive neighbors.
                let a_rank = rank.get(&a).ok_or(Error::Binding)?;
                let b_rank = rank.get(&b).ok_or(Error::Binding)?;
                forward_seams.insert(node, if a_rank < b_rank { b } else { a });
            }
            node_sources.push(
                if a_line.grade.is_ground() && (!structured || !bus_grade::fast_class(a_line.raw)) {
                    NodeSource::Surface
                } else {
                    NodeSource::Seam {
                        grade: a_line.grade,
                        one: low,
                        two: high,
                    }
                },
            );
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
        let may_depart = |node| {
            forward_seams
                .get(&node)
                .is_none_or(|cell| *cell == Cell(line.tile.x, line.tile.y))
        };
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
            if !matches!(line.direction, Direction::Reverse) && may_depart(from.node) {
                adj[from.node].push(arc);
            }
            if !matches!(line.direction, Direction::Forward) && may_depart(to.node) {
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
            boardable_ground: supports_ground_road_geometry(line.raw),
            tile_key: line.tile.canonical_url.clone(),
            fragment_id: super::raw_road_fragment_id(line.tile, line.raw),
            extent: line.raw.extent,
            metric_scale: scales[&line.tile.y],
            grade: line.grade,
        })
        .collect();
    Ok(GameplayBusGraph {
        lines,
        offsets,
        representative,
        geometry,
        node_sources,
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
    if let Some(cuts) = query.fractional_terminals {
        return Ok(fractional::solve_fractional(graph, query, cuts)?.map(|result| result.path));
    }
    let missing_anchor = |i| {
        if query.via.is_empty() {
            Ok(None)
        } else {
            Err(Error::NoBusLeg(i))
        }
    };
    let Some(source) = graph.node_at(&query.tile_key, query.start) else {
        return missing_anchor(0);
    };
    let end_key = query.end_tile_key.as_deref().unwrap_or(&query.tile_key);
    let mut nodes = Vec::with_capacity(query.via.len() + 2);
    nodes.push(source);
    for (i, via) in query.via.iter().enumerate() {
        let Some(node) = graph.node_at(&via.tile_key, via.vertex) else {
            return missing_anchor(i);
        };
        nodes.push(node);
    }
    let Some(target) = graph.node_at(end_key, query.end) else {
        return missing_anchor(query.via.len());
    };
    if query.pattern == Some(BusPattern::ClosedOneWay) && (source != target || query.via.is_empty())
    {
        return Err(Error::InvalidBusCircuit);
    }
    nodes.push(target);
    let mut via_point_indices = Vec::with_capacity(query.via.len());
    let mut edges = Vec::new();
    if query.same_atom_stops.is_some() {
        // A path search could take a detour through other roads to reach both
        // vertices. A fractional route is valid only on the exact atom the
        // player clicked, independently present in the requested direction.
        let fragment = format!(
            "ofm:{}:{}:{}",
            query
                .tile_key
                .split("/planet/")
                .nth(1)
                .and_then(|part| part.strip_suffix(".pbf"))
                .ok_or(Error::Binding)?
                .replace('/', ":"),
            query.start.feature,
            query.start.line,
        );
        let mut direct = graph.adj[source].iter().filter(|arc| {
            let line = &graph.lines[arc.line];
            arc.to == target
                && line.tile_key == query.tile_key
                && line.fragment_id == fragment
                && line.grade.is_ground()
                && arc.ordinal == query.start.vertex.min(query.end.vertex) as usize
                && arc.from_pos == f64::from(query.start.vertex)
                && arc.to_pos == f64::from(query.end.vertex)
        });
        let arc = direct.next().ok_or(Error::NoBusLeg(0))?;
        if direct.next().is_some() {
            return Err(Error::Binding);
        }
        edges.push(*arc);
    } else {
        let mut scratch = SearchScratch::new(graph.geometry.len());
        let mut budget = if query.via.is_empty() {
            usize::MAX
        } else {
            MAX_MULTI_RELAXATIONS
        };
        for (i, leg) in nodes.windows(2).enumerate() {
            if leg[0] == leg[1] {
                return missing_anchor(i);
            }
            if !scratch.leg(graph, leg[0], leg[1], &mut budget, &mut edges)? {
                // Report a genuine disconnection even without waypoints.
                return Err(Error::NoBusLeg(i));
            }
            if !query.via.is_empty() && edges.len() > MAX_MULTI_ATOMS {
                return Err(Error::Limit);
            }
            if i < query.via.len() {
                via_point_indices.push(edges.len());
            }
        }
    }
    Ok(Some(materialize_bus_path(
        graph,
        query,
        source,
        edges,
        via_point_indices,
    )?))
}

fn materialize_bus_path(
    graph: &GameplayBusGraph,
    query: &BusPathBinding,
    source: usize,
    edges: Vec<Arc>,
    via_point_indices: Vec<usize>,
) -> Result<BusPath> {
    let mut spans: Vec<SourceSpan> = Vec::new();
    let mut steps = Vec::with_capacity(edges.len());
    let mut points_grid = vec![graph.geometry[source]];
    let mut node_suffixes = vec![graph.node_suffix(source)];
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
            ground: line.grade.is_ground(),
            span: spans.len() - 1,
            raw_ordinal: arc.ordinal,
            from,
            to,
        });
        points_grid.push(graph.geometry[arc.to]);
        node_suffixes.push(graph.node_suffix(arc.to));
    }
    let points: Vec<_> = points_grid.iter().map(|p| p.point()).collect();
    let length_m = points.windows(2).map(|p| distance(p[0], p[1])).sum();
    Ok(BusPath {
        binding: query.clone(),
        spans,
        steps,
        points_grid,
        node_suffixes,
        points,
        length_m,
        operation_length_mm: u64::try_from((length_um + 500) / 1000).map_err(|_| Error::Limit)?,
        atom_operation_micrometres,
        resolved_terminal_cuts: None,
        via_point_indices,
    })
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

/// Same-cell reproducible geometry query for a game-authored track. It has
/// no flags for Bus direction, grades, stop placement or real rail operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthoredTramGuide {
    /// Exactly one cardinal-neighbor cell. The omitted field keeps one-cell
    /// commands and save geometry byte-compatible.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_adjacent_tram_proof"
    )]
    pub adjacent_end: Option<BusTileProof>,
    pub tile_key: String,
    pub tile_raw_sha256: String,
    pub start: BusVertex,
    pub end: BusVertex,
    /// Measured along the verified segment starting at the given vertex. A
    /// missing fraction means the original vertex, not guessed geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_fraction_q: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_fraction_q: Option<u32>,
}

fn present_adjacent_tram_proof<'de, D>(
    input: D,
) -> std::result::Result<Option<BusTileProof>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    BusTileProof::deserialize(input).map(Some)
}

impl AuthoredTramGuide {
    pub(crate) fn evidence_request(&self) -> BusPathBinding {
        BusPathBinding {
            rule: if self.adjacent_end.is_some() {
                BUS_RECTANGLE_RULE.into()
            } else {
                BUS_GRAPH_RULE.into()
            },
            same_atom_stops: None,
            fractional_terminals: None,
            grade_policy: None,
            tile_key: self.tile_key.clone(),
            tile_raw_sha256: self.tile_raw_sha256.clone(),
            respect_oneway: false,
            start: self.start,
            end: self.end,
            additional_tiles: self.adjacent_end.iter().cloned().collect(),
            end_tile_key: self.adjacent_end.as_ref().map(|end| end.tile_key.clone()),
            pattern: None,
            interior_stops: vec![],
            via: vec![],
        }
    }
}

pub fn reconstruct_authored_tram_guide(
    store: &EvidenceStore,
    proposed: &AuthoredTramGuide,
) -> Result<Option<Vec<[f64; 2]>>> {
    let query = proposed.evidence_request();
    let scope = store.scope(&query.tile_keys()?)?;
    let start_binding = &scope.decoded(&query.tile_key)?.binding;
    if start_binding.raw_sha256 != query.tile_raw_sha256 {
        return Err(Error::Hash);
    }
    let end_key = if let Some(end) = &proposed.adjacent_end {
        let end_binding = &scope.decoded(&end.tile_key)?.binding;
        if start_binding.x.abs_diff(end_binding.x) + start_binding.y.abs_diff(end_binding.y) != 1
            || start_binding.revision != end_binding.revision
        {
            return Err(Error::Binding);
        }
        if end_binding.raw_sha256 != end.tile_raw_sha256 {
            return Err(Error::Hash);
        }
        end.tile_key.as_str()
    } else {
        &query.tile_key
    };
    let graph = build_observed_road_graph(&scope, &query, true, None)?;
    if proposed.start_fraction_q.is_none() && proposed.end_fraction_q.is_none() {
        return Ok(
            untyped_unreachable(solve_gameplay_bus_graph(&graph, &query))?.map(|p| p.points),
        );
    }
    let anchor: fn(&GameplayBusGraph, &str, BusVertex, Option<u32>) -> Option<TramAnchor> =
        if proposed.adjacent_end.is_some() {
            adjacent_tram_anchor
        } else {
            authored_anchor
        };
    let Some((start, from)) = anchor(
        &graph,
        &query.tile_key,
        query.start,
        proposed.start_fraction_q,
    ) else {
        return Ok(None);
    };
    let Some((end, to)) = anchor(&graph, end_key, query.end, proposed.end_fraction_q) else {
        return Ok(None);
    };
    // Two independently admitted positions on the same raw atom are
    // directly connected. Searching via the atom's endpoints would insert
    // a false turnback on a completely straight stretch.
    if proposed.adjacent_end.is_none() && query.start == query.end {
        return Ok((start != end).then_some(vec![start, end]));
    }
    // Both fractional picks attach only to a verified ordinary raw atom.
    // At most four bounded searches share one graph and one work allowance.
    let mut scratch = SearchScratch::new(graph.geometry.len());
    let mut budget = MAX_MULTI_RELAXATIONS;
    let mut best: Option<(f64, Vec<[f64; 2]>)> = None;
    for &f in &from {
        for &t in &to {
            let mut legs = vec![];
            if f != t && !scratch.leg(&graph, f, t, &mut budget, &mut legs)? {
                continue;
            }
            if legs.len() > MAX_MULTI_ATOMS - 3 {
                return Err(Error::Limit);
            }
            let mut points = Vec::with_capacity(legs.len() + 3);
            points.extend([start, graph.geometry[f].point()]);
            points.extend(legs.iter().map(|arc| graph.geometry[arc.to].point()));
            points.push(end);
            points.dedup();
            if points.len() < 2 {
                continue;
            }
            let length = points.windows(2).map(|p| distance(p[0], p[1])).sum();
            if best.as_ref().is_none_or(|(shortest, _)| length < *shortest) {
                best = Some((length, points));
            }
        }
    }
    Ok(best.map(|(_, points)| points))
}

type TramAnchor = ([f64; 2], Vec<usize>);

fn adjacent_tram_anchor(
    graph: &GameplayBusGraph,
    key: &str,
    vertex: BusVertex,
    q: Option<u32>,
) -> Option<TramAnchor> {
    let Some(q) = q.filter(|q| *q > 0) else {
        return authored_anchor(graph, key, vertex, q);
    };
    if q > 1_000_000 {
        return None;
    }
    let &(base, n) = graph
        .offsets
        .get(key)?
        .get(&(vertex.feature, vertex.line))?;
    let i = vertex.vertex as usize;
    if i.checked_add(1)? >= n {
        return None;
    }
    let raw_pair = [base.checked_add(i)?, base.checked_add(i + 1)?];
    let fragment = format!(
        "ofm:{}:{}:{}",
        key.split_once("/planet/")?
            .1
            .strip_suffix(".pbf")?
            .replace('/', ":"),
        vertex.feature,
        vertex.line
    );
    let fraction = f64::from(q) / 1_000_000.;
    let position = i as f64 + fraction;
    // A seam arc has one owned raw endpoint and one verified cut port.
    // Inspect only incident arcs; points in the duplicated buffer past
    // the cut must not attach to the other cell by coordinate coincidence.
    for raw in raw_pair {
        let u = graph.representative[raw];
        for arc in &graph.adj[u] {
            if arc.ordinal != i
                || graph.lines[arc.line].fragment_id != fragment
                || position < arc.from_pos.min(arc.to_pos)
                || position > arc.from_pos.max(arc.to_pos)
            {
                continue;
            }
            if position == arc.from_pos {
                return Some((graph.geometry[u].point(), vec![u]));
            }
            if position == arc.to_pos {
                return Some((graph.geometry[arc.to].point(), vec![arc.to]));
            }
            let [a, b] = raw_pair.map(|r| graph.geometry[r].point());
            return Some((
                [
                    a[0] + (b[0] - a[0]) * fraction,
                    a[1] + (b[1] - a[1]) * fraction,
                ],
                vec![u, arc.to],
            ));
        }
    }
    None
}

fn authored_anchor(
    graph: &GameplayBusGraph,
    tile_key: &str,
    v: BusVertex,
    fraction_q: Option<u32>,
) -> Option<TramAnchor> {
    const FRACTION_DENOMINATOR: u32 = 1_000_000;
    let a = graph.node_at(tile_key, v)?;
    let Some(q) = fraction_q.filter(|q| *q != 0) else {
        return Some((graph.geometry[a].point(), vec![a]));
    };
    if q > FRACTION_DENOMINATOR {
        return None;
    }
    let b = graph.node_at(
        tile_key,
        BusVertex {
            vertex: v.vertex.checked_add(1)?,
            ..v
        },
    )?;
    let fragment = format!(
        "ofm:{}:{}:{}",
        tile_key
            .split("/planet/")
            .nth(1)?
            .strip_suffix(".pbf")?
            .replace('/', ":"),
        v.feature,
        v.line
    );
    // An exact point on a clipped/denied/unowned atom cannot be forged from
    // nearby nodes or a common XY. The whole original segment must be linked.
    if !graph.adj[a].iter().any(|arc| {
        arc.to == b
            && arc.ordinal == v.vertex as usize
            && graph.lines[arc.line].fragment_id == fragment
            && arc.from_pos == f64::from(v.vertex)
            && arc.to_pos == f64::from(v.vertex) + 1.
    }) {
        return None;
    }
    let [p, z] = [graph.geometry[a].point(), graph.geometry[b].point()];
    let fraction = f64::from(q) / f64::from(FRACTION_DENOMINATOR);
    Some(if q == FRACTION_DENOMINATOR {
        (z, vec![b])
    } else {
        (
            [
                p[0] + (z[0] - p[0]) * fraction,
                p[1] + (z[1] - p[1]) * fraction,
            ],
            vec![a, b],
        )
    })
}

/// Reconstruct only from evidence in the world archive, never from caller paths.
pub fn reconstruct_gameplay_bus_path(
    store: &EvidenceStore,
    proposed: &BusPathBinding,
) -> Result<Option<BusPath>> {
    untyped_unreachable(store.gameplay_bus_path(proposed)).map(|path| path.as_deref().cloned())
}

fn untyped_unreachable<T>(result: Result<Option<T>>) -> Result<Option<T>> {
    match result {
        Err(Error::NoBusLeg(_)) => Ok(None),
        other => other,
    }
}
