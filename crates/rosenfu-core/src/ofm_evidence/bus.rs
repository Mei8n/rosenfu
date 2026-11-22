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
}

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
}

#[derive(Debug, Clone, PartialEq)]
pub struct BusPath {
    pub binding: BusPathBinding,
    pub spans: Vec<SourceSpan>,
    /// Geographic longitude/latitude, exactly from the traversed tile vertices.
    pub points: Vec<[f64; 2]>,
    pub length_m: f64,
    pub(crate) operation_length_mm: u64,
    pub(crate) steps: Vec<BusSourceStep>,
    pub(crate) points_grid: Vec<BusGridPoint>,
}

impl BusPath {
    /// Directed materialized identity shared by every ordinary route that
    /// traverses this exact verified raw segment (or owned seam-clipped part).
    pub fn source_atom_id(&self, index: usize) -> Option<String> {
        let step = self.steps.get(index)?;
        let span = self.spans.get(step.span)?;
        let digest = self.source_atom_raw_sha(index)?;
        let key = format!(
            "ofm-bus-atom-v1\0{}\0{}\0{}\0{}\0{:016x}\0{:016x}",
            span.binding_key,
            digest,
            span.fragment_id,
            step.raw_ordinal,
            step.from.to_bits(),
            step.to.to_bits()
        );
        Some(format!(
            "ofm-bus:edge:{}",
            super::raw_sha256(key.as_bytes())
        ))
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

#[derive(Clone, Copy)]
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
    };
    solve_gameplay_bus_scope(store, &request)
}

/// Scope contains every requested tile, already checked against its exact
/// digest. Other archive tiles cannot add candidate seams or change the result.
pub(super) fn solve_gameplay_bus_scope(
    store: &EvidenceStore,
    query: &BusPathBinding,
) -> Result<Option<BusPath>> {
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
    let mut offsets = BTreeMap::new();
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
            offsets.insert(
                (
                    tile.binding.canonical_url.as_str(),
                    raw.feature_index,
                    raw.line_index,
                ),
                (base, n),
            );
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

    let node_at = |key: &str, v: BusVertex| -> Option<usize> {
        let &(base, n) = offsets.get(&(key, v.feature, v.line))?;
        let i = v.vertex as usize;
        (i < n).then(|| representative[base + i])
    };
    let (Some(source), Some(target)) = (
        node_at(&query.tile_key, query.start),
        node_at(
            query.end_tile_key.as_deref().unwrap_or(&query.tile_key),
            query.end,
        ),
    ) else {
        return Ok(None);
    };
    if source == target {
        return Ok(None);
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
    let mut d = vec![u64::MAX; geometry.len()];
    let mut parent = vec![None; geometry.len()];
    let mut queue = BinaryHeap::from([Step(0, source)]);
    d[source] = 0;
    while let Some(Step(cost, u)) = queue.pop() {
        if cost > d[u] {
            continue;
        }
        if u == target {
            break;
        }
        for &arc in &adj[u] {
            let next = cost.checked_add(arc.weight).ok_or(Error::Limit)?;
            if next < d[arc.to] {
                d[arc.to] = next;
                parent[arc.to] = Some((u, arc));
                queue.push(Step(next, arc.to));
            }
        }
    }
    if d[target] == u64::MAX {
        return Ok(None);
    }
    let mut edges = Vec::new();
    let mut u = target;
    while u != source {
        let (p, arc) = parent[u].ok_or(Error::Malformed)?;
        edges.push(arc);
        u = p;
    }
    edges.reverse();
    let mut spans: Vec<SourceSpan> = Vec::new();
    let mut steps = Vec::with_capacity(edges.len());
    let mut points_grid = vec![geometry[source]];
    let mut length_um = 0u128;
    let round_trip = query.pattern == Some(BusPattern::OpenRoundTrip);
    if round_trip && edges.len() > 4096 {
        return Err(Error::Limit);
    }
    let scales: BTreeMap<_, _> = if round_trip {
        cells.iter().map(|c| (c.1, bus_metric_scale(c.1))).collect()
    } else {
        BTreeMap::new()
    };
    for arc in edges {
        let line = &lines[arc.line];
        let (from, to) = (arc.from_pos, arc.to_pos);
        let fragment_id = fragment(line.tile, line.raw);
        if round_trip {
            length_um = length_um
                .checked_add(bus_arc_micrometres(
                    arc.weight,
                    line.raw.extent,
                    scales[&line.tile.y],
                ))
                .ok_or(Error::Limit)?;
        }
        if let Some(tail) = spans.last_mut().filter(|s| {
            s.fragment_id == fragment_id
                && s.to == from
                && (s.to - s.from).signum() == (to - from).signum()
        }) {
            tail.to = to;
        } else {
            spans.push(SourceSpan {
                binding_key: line.tile.canonical_url.clone(),
                fragment_id,
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
        points_grid.push(geometry[arc.to]);
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
    store
        .gameplay_bus_path(proposed)
        .map(|path| path.as_deref().cloned())
}
