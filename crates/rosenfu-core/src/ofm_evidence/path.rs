//! Pure derived geometry, not construction or physical validation. Every call
//! regenerates boundary candidates from immutable raw evidence, never claims.
use super::{boundary_candidates, Error, EvidenceStore, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
type P = [f64; 2];
const RULE: &str = "ofm_overlap_blend_v1";
const METHOD: &str = "ofm_tile_path_v1";
const TRAM_METHOD: &str = "ofm_authored_street_tram_v1";
const TRAM_RULE: &str = "ofm_ground_tram_guide_v1";
const ADJACENT_TRAM_RULE: &str = "ofm_ground_tram_adjacent_v2";
const SEGMENTS: usize = 256;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PathPick {
    pub fragment_id: String,
    pub segment_index: usize,
    pub fraction: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceSpan {
    pub binding_key: String,
    pub fragment_id: String,
    pub from: f64,
    pub to: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PathJoin {
    pub rule: String,
    pub a_id: String,
    pub b_id: String,
    pub axis: u8,
    pub from_axis: f64,
    pub to_axis: f64,
    pub a_range: [f64; 2],
    pub b_range: [f64; 2],
}
/// Untrusted lightweight persistence proposal. Deserializing this never validates it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PathDescriptor {
    pub method: String,
    pub algorithm: String,
    pub geometry_derived: bool,
    pub coverage_verified: bool,
    pub required_binding_keys: Vec<String>,
    pub start: PathPick,
    pub end: PathPick,
    pub ordered_spans: Vec<SourceSpan>,
    pub joins: Vec<PathJoin>,
    /// Present only for a game-authored street tram; not a real lane right.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_tram_guide"
    )]
    pub tram_guide: Option<super::AuthoredTramGuide>,
}
fn present_tram_guide<'de, D>(
    input: D,
) -> std::result::Result<Option<super::AuthoredTramGuide>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    super::AuthoredTramGuide::deserialize(input).map(Some)
}
impl PathDescriptor {
    /// Wire capability marker; its presence doesn't certify any geometry.
    pub fn carries_street_tram_proof(&self) -> bool {
        self.method == TRAM_METHOD || self.tram_guide.is_some()
    }

    pub fn carries_adjacent_street_tram_proof(&self) -> bool {
        self.algorithm == ADJACENT_TRAM_RULE
            || self
                .tram_guide
                .as_ref()
                .is_some_and(|guide| guide.adjacent_end.is_some())
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct DerivedPath {
    pub descriptor: PathDescriptor,
    pub points: Vec<P>,
    pub max_correction_meters: f64,
}
struct Fragment {
    id: String,
    binding: String,
    x: u32,
    y: u32,
    points: Vec<P>,
}
struct Blend {
    record: PathJoin,
    points: Vec<P>,
    maximum: f64,
}
fn lat(y: f64) -> f64 {
    (std::f64::consts::PI * (1. - 2. * y / 16384.))
        .sinh()
        .atan()
        .to_degrees()
}
fn at(l: &Fragment, pos: f64) -> Result<P> {
    if !pos.is_finite() || pos < 0. || pos > (l.points.len() - 1) as f64 {
        return Err(Error::Binding);
    }
    let i = (pos.floor() as usize).min(l.points.len() - 2);
    let t = pos - i as f64;
    let a = l.points[i];
    let b = l.points[i + 1];
    Ok([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t])
}
fn location(l: &Fragment, axis: usize, value: f64) -> Result<f64> {
    let mut hits: Vec<f64> = Vec::new();
    for (i, e) in l.points.windows(2).enumerate() {
        let a = e[0][axis];
        let b = e[1][axis];
        if a == b {
            if a == value {
                return Err(Error::Binding);
            }
            continue;
        }
        let t = (value - a) / (b - a);
        if (0. ..=1.).contains(&t) && !hits.iter().any(|p| (*p - (i as f64 + t)).abs() < 1e-8) {
            hits.push(i as f64 + t);
            if hits.len() > 1 {
                return Err(Error::Binding);
            }
        }
    }
    if hits.len() != 1 {
        return Err(Error::Binding);
    }
    Ok(hits[0])
}
fn slice(l: &Fragment, from: f64, to: f64) -> Result<Vec<P>> {
    let mut out = vec![at(l, from)?];
    if to > from {
        let mut i = from.floor() as usize + 1;
        while (i as f64) < to {
            out.push(l.points[i]);
            i += 1;
        }
    } else {
        let mut i = from.ceil() as i64 - 1;
        while (i as f64) > to {
            out.push(l.points[i as usize]);
            i -= 1;
        }
    }
    out.push(at(l, to)?);
    Ok(out)
}
fn blend(a: &Fragment, b: &Fragment) -> Result<Blend> {
    let axis = usize::from(a.x == b.x);
    let mid = (lat(f64::from(a.y.max(b.y) + 1)) + lat(f64::from(a.y.min(b.y)))) / 2.;
    let sx = 111320. * mid.to_radians().cos();
    let sy = 111320.;
    let scale = if axis == 0 { sx } else { sy };
    let center = if axis == 0 {
        f64::from(a.x.max(b.x)) / 16384. * 360. - 180.
    } else {
        lat(f64::from(a.y.max(b.y)))
    };
    let direction = if axis == 0 {
        (f64::from(b.x) - f64::from(a.x)).signum()
    } else {
        (f64::from(a.y) - f64::from(b.y)).signum()
    };
    let from = center - direction * 10. / scale;
    let to = center + direction * 10. / scale;
    let ar = [location(a, axis, from)?, location(a, axis, to)?];
    let br = [location(b, axis, from)?, location(b, axis, to)?];
    let mut parameters: Vec<f64> = (0..=20).map(|i| i as f64 / 20.).collect();
    for l in [a, b] {
        for p in &l.points {
            let t = (p[axis] - from) / (to - from);
            if t > 0. && t < 1. {
                parameters.push(t);
            }
        }
    }
    parameters.sort_by(f64::total_cmp);
    parameters.dedup();
    if parameters.len() > SEGMENTS + 1 {
        return Err(Error::Limit);
    }
    let mut points = Vec::with_capacity(parameters.len());
    let mut maximum: f64 = 0.;
    for t in parameters {
        let v = from + (to - from) * t;
        let pa = at(a, location(a, axis, v)?)?;
        let pb = at(b, location(b, axis, v)?)?;
        let d = ((pa[0] - pb[0]) * sx).hypot((pa[1] - pb[1]) * sy);
        if !d.is_finite() || d > 0.8 + 1e-7 {
            return Err(Error::Binding);
        }
        maximum = maximum.max(d);
        let w = t * t * (3. - 2. * t);
        points.push(if t == 0. {
            pa
        } else if t == 1. {
            pb
        } else {
            [pa[0] + (pb[0] - pa[0]) * w, pa[1] + (pb[1] - pa[1]) * w]
        });
    }
    Ok(Blend {
        record: PathJoin {
            rule: RULE.into(),
            a_id: a.id.clone(),
            b_id: b.id.clone(),
            axis: axis as u8,
            from_axis: from,
            to_axis: to,
            a_range: ar,
            b_range: br,
        },
        points,
        maximum,
    })
}
fn pick(l: &Fragment, p: &PathPick) -> Result<f64> {
    if p.segment_index >= l.points.len() - 1
        || !p.fraction.is_finite()
        || !(0. ..=1.).contains(&p.fraction)
    {
        return Err(Error::Binding);
    }
    Ok(p.segment_index as f64 + p.fraction)
}
fn append(points: &mut Vec<P>, part: impl IntoIterator<Item = P>) -> Result<()> {
    for p in part {
        if points.last() != Some(&p) {
            points.push(p);
        }
        if points.len() > SEGMENTS + 1 {
            return Err(Error::Limit);
        }
    }
    Ok(())
}
/// Complete independent reconstruction. This output remains subject to later
/// physical alignment/height/grade/curvature and atomic save validation.
pub fn reconstruct_path(
    store: &EvidenceStore,
    required_keys: &[String],
    start: &PathPick,
    end: &PathPick,
) -> Result<DerivedPath> {
    if required_keys.len() > 16 {
        return Err(Error::Limit);
    }
    let required: BTreeSet<_> = required_keys.iter().map(String::as_str).collect();
    let available: BTreeSet<_> = store.bindings().map(|b| b.canonical_url.as_str()).collect();
    if required.len() != required_keys.len() || required != available {
        return Err(Error::Binding);
    }
    let scoped;
    let store = if store.is_decoded_scope() {
        store
    } else {
        scoped = store.scope(required_keys)?;
        &scoped
    };
    let tiles: Vec<_> = store.bindings().map(|b| (b.x, b.y)).collect();
    let pairs = boundary_candidates(store, &tiles)?;
    let mut adjacent: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in pairs {
        adjacent
            .entry(p.a_fragment.clone())
            .or_default()
            .push(p.b_fragment.clone());
        adjacent.entry(p.b_fragment).or_default().push(p.a_fragment);
    }
    let chain = select_chain(&adjacent, &start.fragment_id, &end.fragment_id)?;
    let wanted: BTreeSet<_> = chain.iter().collect();
    let mut by_id = BTreeMap::new();
    for b in store.bindings() {
        for line in &store.decoded(&b.canonical_url)?.lines {
            let id = format!(
                "ofm:{}:14:{}:{}:{}:{}",
                b.revision, b.x, b.y, line.feature_index, line.line_index
            );
            if !wanted.contains(&id) {
                continue;
            }
            let points = line
                .coordinates
                .iter()
                .map(|p| {
                    [
                        (f64::from(b.x) + f64::from(p[0]) / f64::from(line.extent)) / 16384. * 360.
                            - 180.,
                        lat(f64::from(b.y) + f64::from(p[1]) / f64::from(line.extent)),
                    ]
                })
                .collect();
            by_id.insert(
                id.clone(),
                Fragment {
                    id,
                    binding: b.canonical_url.clone(),
                    x: b.x,
                    y: b.y,
                    points,
                },
            );
        }
    }
    let fragments: Vec<_> = chain
        .iter()
        .map(|id| by_id.get(id).ok_or(Error::Binding))
        .collect::<Result<_>>()?;
    let start_pos = pick(fragments[0], start)?;
    let end_pos = pick(fragments[fragments.len() - 1], end)?;
    let blends: Vec<_> = fragments
        .windows(2)
        .map(|p| blend(p[0], p[1]))
        .collect::<Result<_>>()?;
    let mut points = Vec::new();
    let mut spans = Vec::new();
    let mut covered = 0usize;
    for (i, l) in fragments.iter().enumerate() {
        let before = i.checked_sub(1).map(|j| &blends[j]);
        let after = blends.get(i);
        let from = before.map_or(start_pos, |b| b.record.b_range[1]);
        let to = after.map_or(end_pos, |b| b.record.a_range[0]);
        let direction = (to - from).signum();
        if to == from
            || before
                .is_some_and(|b| (b.record.b_range[1] - b.record.b_range[0]).signum() != direction)
            || after
                .is_some_and(|b| (b.record.a_range[1] - b.record.a_range[0]).signum() != direction)
        {
            return Err(Error::Binding);
        }
        let span_from = before.map_or(from, |b| b.record.b_range[0]);
        let span_to = after.map_or(to, |b| b.record.a_range[1]);
        covered += (span_from.max(span_to).ceil() - span_from.min(span_to).floor()) as usize;
        if covered > SEGMENTS {
            return Err(Error::Limit);
        }
        spans.push(SourceSpan {
            binding_key: l.binding.clone(),
            fragment_id: l.id.clone(),
            from: span_from,
            to: span_to,
        });
        append(&mut points, slice(l, from, to)?)?;
        if let Some(b) = after {
            append(&mut points, b.points.iter().copied())?;
        }
    }
    if points.len() < 2 || points.iter().flatten().any(|v| !v.is_finite()) {
        return Err(Error::Binding);
    }
    let maximum = blends.iter().map(|b| b.maximum).fold(0., f64::max);
    Ok(DerivedPath {
        descriptor: PathDescriptor {
            method: METHOD.into(),
            algorithm: RULE.into(),
            geometry_derived: true,
            coverage_verified: false,
            required_binding_keys: available.into_iter().map(str::to_owned).collect(),
            start: start.clone(),
            end: end.clone(),
            ordered_spans: spans,
            joins: blends.into_iter().map(|b| b.record).collect(),
            tram_guide: None,
        },
        points,
        max_correction_meters: maximum,
    })
}
/// Use exactly the native guide solver used at point placement. The binding
/// records the raw byte identity as well as the original fractional endpoints.
pub(crate) fn reconstruct_street_tram_path(
    store: &EvidenceStore,
    guide: &super::AuthoredTramGuide,
) -> Result<DerivedPath> {
    let points = super::reconstruct_authored_tram_guide(store, guide)?
        .filter(|line| (2..=4096).contains(&line.len()))
        .ok_or(Error::Binding)?;
    let pick = |key: &str, vertex: super::BusVertex, q: Option<u32>| -> Result<PathPick> {
        let fragment_prefix = key
            .split_once("/planet/")
            .and_then(|(_, tile)| tile.strip_suffix(".pbf"))
            .ok_or(Error::Binding)?
            .replace('/', ":");
        Ok(PathPick {
            fragment_id: format!("ofm:{fragment_prefix}:{}:{}", vertex.feature, vertex.line),
            segment_index: vertex.vertex as usize,
            fraction: f64::from(q.unwrap_or(0)) / 1_000_000.,
        })
    };
    let end_key = guide
        .adjacent_end
        .as_ref()
        .map_or(guide.tile_key.as_str(), |end| end.tile_key.as_str());
    Ok(DerivedPath {
        descriptor: PathDescriptor {
            method: TRAM_METHOD.into(),
            algorithm: if guide.adjacent_end.is_some() {
                ADJACENT_TRAM_RULE.into()
            } else {
                TRAM_RULE.into()
            },
            geometry_derived: true,
            coverage_verified: false,
            required_binding_keys: guide.evidence_request().tile_keys()?,
            start: pick(&guide.tile_key, guide.start, guide.start_fraction_q)?,
            end: pick(end_key, guide.end, guide.end_fraction_q)?,
            ordered_spans: vec![],
            joins: vec![],
            tram_guide: Some(guide.clone()),
        },
        points,
        max_correction_meters: 0.,
    })
}

fn close(a: f64, b: f64, tolerance: f64) -> bool {
    b.is_finite() && (a - b).abs() <= tolerance
}
/// Rebuilds geometry, then checks all lightweight claims. Never trusts graph JSON,
/// claimed derived coordinates, source hashes without bytes, or a prior DTO decode.
pub fn validate_path_descriptor(
    store: &EvidenceStore,
    claim: &PathDescriptor,
) -> Result<DerivedPath> {
    if let Some(guide) = claim.tram_guide.as_ref() {
        if claim.method != TRAM_METHOD
            || claim.required_binding_keys != guide.evidence_request().tile_keys()?
            || !claim.ordered_spans.is_empty()
            || !claim.joins.is_empty()
        {
            return Err(Error::Binding);
        }
        let actual = reconstruct_street_tram_path(store, guide)?;
        return (actual.descriptor == *claim)
            .then_some(actual)
            .ok_or(Error::Binding);
    }
    if claim.method != METHOD
        || claim.algorithm != RULE
        || !claim.geometry_derived
        || claim.coverage_verified
        || claim.ordered_spans.len() > 32
        || claim.joins.len() > 31
    {
        return Err(Error::Binding);
    }
    let actual = reconstruct_path(
        store,
        &claim.required_binding_keys,
        &claim.start,
        &claim.end,
    )?;
    let a = &actual.descriptor;
    if a.ordered_spans.len() != claim.ordered_spans.len() || a.joins.len() != claim.joins.len() {
        return Err(Error::Binding);
    }
    for (a, b) in a.ordered_spans.iter().zip(&claim.ordered_spans) {
        if a.binding_key != b.binding_key
            || a.fragment_id != b.fragment_id
            || !close(a.from, b.from, 1e-8)
            || !close(a.to, b.to, 1e-8)
        {
            return Err(Error::Binding);
        }
    }
    for (a, b) in a.joins.iter().zip(&claim.joins) {
        if a.rule != b.rule
            || a.a_id != b.a_id
            || a.b_id != b.b_id
            || a.axis != b.axis
            || !close(a.from_axis, b.from_axis, 1e-10)
            || !close(a.to_axis, b.to_axis, 1e-10)
            || a.a_range
                .iter()
                .zip(b.a_range)
                .chain(a.b_range.iter().zip(b.b_range))
                .any(|(a, b)| !close(*a, b, 1e-8))
        {
            return Err(Error::Binding);
        }
    }
    Ok(actual)
}
fn select_chain(
    adjacent: &BTreeMap<String, Vec<String>>,
    start: &str,
    end: &str,
) -> Result<Vec<String>> {
    if adjacent.values().any(|v| v.len() > 2) {
        return Err(Error::Binding);
    }
    if start == end {
        return Err(Error::Unsupported);
    }
    let mut parents: BTreeMap<String, Option<String>> = BTreeMap::from([(start.to_owned(), None)]);
    let mut queue = vec![start.to_owned()];
    let mut cursor = 0;
    while cursor < queue.len() {
        let current = queue[cursor].clone();
        cursor += 1;
        for next in adjacent.get(&current).into_iter().flatten() {
            if parents[&current].as_ref() == Some(next) {
                continue;
            }
            if parents.contains_key(next) {
                return Err(Error::Binding);
            }
            parents.insert(next.clone(), Some(current.clone()));
            queue.push(next.clone());
            if queue.len() > 32 {
                return Err(Error::Limit);
            }
        }
    }
    if !parents.contains_key(end) {
        return Err(Error::Binding);
    }
    let mut chain = Vec::new();
    let mut current = Some(end.to_owned());
    while let Some(id) = current {
        current = parents.get(&id).ok_or(Error::Binding)?.clone();
        chain.push(id);
    }
    chain.reverse();
    Ok(chain)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn graph(edges: &[(&str, &str)]) -> BTreeMap<String, Vec<String>> {
        let mut g: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (a, b) in edges {
            g.entry((*a).into()).or_default().push((*b).into());
            g.entry((*b).into()).or_default().push((*a).into());
        }
        g
    }
    #[test]
    fn chain_rejects_branches_cycles_and_component_limit() {
        assert_eq!(
            select_chain(&graph(&[("a", "b"), ("b", "c")]), "c", "a").unwrap(),
            vec!["c", "b", "a"]
        );
        for edges in [
            vec![("a", "b"), ("b", "c"), ("c", "a")],
            vec![("a", "b"), ("b", "c"), ("b", "d")],
        ] {
            assert!(select_chain(&graph(&edges), "a", "c").is_err());
        }
        let mut large: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for i in 0..33 {
            large
                .entry(i.to_string())
                .or_default()
                .push((i + 1).to_string());
            large
                .entry((i + 1).to_string())
                .or_default()
                .push(i.to_string());
        }
        assert_eq!(select_chain(&large, "0", "1"), Err(Error::Limit));
    }
}
