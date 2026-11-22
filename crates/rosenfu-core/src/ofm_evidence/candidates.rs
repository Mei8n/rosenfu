//! Conservative boundary correspondence from complete decoded evidence, not road
//! identity, topology, access permission, or the derived blend/path validator.
use super::{number, Error, EvidenceStore, RawValue, Result, TileBinding, TileLine};
use std::collections::{BTreeMap, BTreeSet};
type Point = [f64; 2];
const BAND: f64 = 10.;
const TOLERANCE: f64 = 0.8;
const MAX_LINES: usize = 2000;
const MAX_POINTS: usize = 100_000;
const MAX_PAIRS: usize = 10_000;
const MAX_WORK: usize = 1_000_000;
#[derive(Debug, Clone, PartialEq)]
pub struct BoundaryCandidate {
    pub a_fragment: String,
    pub b_fragment: String,
    pub overlap_meters: f64,
    pub max_separation_meters: f64,
}
struct Line<'a> {
    id: String,
    tile: &'a TileBinding,
    raw: &'a TileLine,
    points: Vec<Point>,
}
struct Clip<'a> {
    line: &'a Line<'a>,
    points: Vec<Point>,
    length: f64,
    direction: f64,
}
fn latitude(y: f64) -> f64 {
    (std::f64::consts::PI * (1. - 2. * y / 16384.))
        .sinh()
        .atan()
        .to_degrees()
}
fn distance(a: Point, b: Point) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn string<'a>(l: &'a TileLine, k: &str) -> Option<&'a str> {
    match l.attributes.get(k) {
        Some(RawValue::String(s)) => Some(s),
        _ => None,
    }
}
fn candidate_class(l: &TileLine) -> bool {
    matches!(
        string(l, "class"),
        Some("primary" | "secondary" | "tertiary" | "minor" | "service")
    )
}
fn eligible(l: &TileLine) -> bool {
    !l.attributes.contains_key("brunnel")
        && l.attributes.get("layer").is_none_or(|v| match v {
            RawValue::String(s) => s.trim().parse::<f64>().ok() == Some(0.),
            _ => number(v) == Some(0.),
        })
        && l.attributes
            .get("oneway")
            .is_none_or(|v| number(v).is_some_and(|n| n == 0. || n == 1. || n == -1.))
}
// Numeric protobuf wire encodings share JS Number semantics, but string tags
// must not silently become numeric direction/access claims.
fn same_value(a: Option<&RawValue>, b: Option<&RawValue>, key: &str) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) if key == "layer" => {
            let layer = |v: &RawValue| match v {
                RawValue::String(s) => s.trim().parse::<f64>().ok(),
                _ => number(v),
            };
            layer(a) == layer(b)
        }
        (Some(a), Some(b)) => match (number(a), number(b)) {
            (Some(a), Some(b)) => a == b,
            _ => a == b,
        },
        _ => false,
    }
}
fn same(a: &TileLine, b: &TileLine) -> bool {
    ["class", "subclass", "access", "brunnel", "layer", "oneway"]
        .iter()
        .all(|k| same_value(a.attributes.get(*k), b.attributes.get(*k), k))
}
fn clip<'a>(
    line: &'a Line<'a>,
    axis: usize,
    seam: f64,
    scale: Point,
    low: f64,
    high: f64,
) -> Option<Clip<'a>> {
    let mut points: Vec<Point> = Vec::new();
    let mut direction = 0.;
    for e in line.points.windows(2) {
        let a = [e[0][0] * scale[0], e[0][1] * scale[1]];
        let b = [e[1][0] * scale[0], e[1][1] * scale[1]];
        let d = b[axis] - a[axis];
        if d == 0. {
            if (a[axis] - seam).abs() <= BAND {
                return None;
            }
            continue;
        }
        let mut lo = (seam - BAND - a[axis]) / d;
        let mut hi = (seam + BAND - a[axis]) / d;
        if lo > hi {
            std::mem::swap(&mut lo, &mut hi);
        }
        lo = lo.max(0.);
        hi = hi.min(1.);
        if hi <= lo {
            continue;
        }
        if direction != 0. && direction != d.signum() {
            return None;
        }
        direction = d.signum();
        let at = |t: f64| [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])];
        let p = at(lo);
        let q = at(hi);
        if points.last().is_some_and(|last| distance(*last, p) > 1e-5) {
            return None;
        }
        if points.is_empty() {
            points.push(p);
        }
        points.push(q);
    }
    if points.len() < 2 {
        return None;
    }
    let first = points[0];
    let last = *points.last()?;
    if ((first[axis] - seam).abs() - BAND).abs() > 1e-5
        || ((last[axis] - seam).abs() - BAND).abs() > 1e-5
        || first[axis] == last[axis]
        || points
            .iter()
            .any(|p| p[1 - axis] < low + BAND || p[1 - axis] > high - BAND)
    {
        return None;
    }
    let length = points.windows(2).map(|e| distance(e[0], e[1])).sum();
    if length > 100. {
        return None;
    }
    if first[axis] > last[axis] {
        points.reverse();
    }
    Some(Clip {
        line,
        points,
        length,
        direction,
    })
}
fn point_segment(p: Point, a: Point, b: Point) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let den = d[0] * d[0] + d[1] * d[1];
    let t = if den == 0. {
        0.
    } else {
        ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / den
    }
    .clamp(0., 1.);
    distance(p, [a[0] + t * d[0], a[1] + t * d[1]])
}
fn upper(p: Point, q: Point, target: &[Point], depth: u8, budget: &mut usize) -> Result<f64> {
    let mut bound = f64::INFINITY;
    for e in target.windows(2) {
        *budget = budget.checked_sub(1).ok_or(Error::Limit)?;
        bound = bound.min(point_segment(p, e[0], e[1]).max(point_segment(q, e[0], e[1])));
    }
    if bound <= TOLERANCE || depth == 8 {
        return Ok(bound);
    }
    let m = [(p[0] + q[0]) / 2., (p[1] + q[1]) / 2.];
    Ok(upper(p, m, target, depth + 1, budget)?.max(upper(m, q, target, depth + 1, budget)?))
}
fn separation(a: &[Point], b: &[Point], budget: &mut usize) -> Result<f64> {
    let mut worst: f64 = 0.;
    for e in a.windows(2) {
        worst = worst.max(upper(e[0], e[1], b, 0, budget)?);
        if worst > TOLERANCE {
            break;
        }
    }
    Ok(worst)
}
/// Recomputes candidates from every candidate-class line in every required tile.
/// `required_tiles` must exactly match this evidence store, including unused roads.
/// The caller cannot supply a road subset, claimed graph hash or candidate geometry.
/// Missing external tiles cannot be inferred; the required acquisition scope must
/// be determined by a later trusted path request, not claimed as global coverage.
pub fn boundary_candidates(
    store: &EvidenceStore,
    required_tiles: &[(u32, u32)],
) -> Result<Vec<BoundaryCandidate>> {
    boundary_candidates_impl(store, required_tiles, false, false)
}

/// Only remove lines that cannot intersect both sides of any required seam.
/// Unlike filtering by gameplay permission or nearest geometry, this preserves
/// every possible competing boundary fragment in the authorized context.
pub(super) fn boundary_candidates_for_bus(
    store: &EvidenceStore,
    required_tiles: &[(u32, u32)],
    structured: bool,
) -> Result<Vec<BoundaryCandidate>> {
    boundary_candidates_impl(store, required_tiles, true, structured)
}

fn crosses_context_boundary(
    line: &TileLine,
    tile: &TileBinding,
    cells: &BTreeSet<(u32, u32)>,
) -> bool {
    let mut lo = [i32::MAX; 2];
    let mut hi = [i32::MIN; 2];
    for point in &line.coordinates {
        for axis in 0..2 {
            lo[axis] = lo[axis].min(point[axis]);
            hi[axis] = hi[axis].max(point[axis]);
        }
    }
    let crosses = |axis: usize, boundary: i32| lo[axis] < boundary && hi[axis] > boundary;
    let extent = line.extent as i32;
    let (x, y) = (tile.x, tile.y);
    (x > 0 && cells.contains(&(x - 1, y)) && crosses(0, 0))
        || (cells.contains(&(x + 1, y)) && crosses(0, extent))
        || (y > 0 && cells.contains(&(x, y - 1)) && crosses(1, 0))
        || (cells.contains(&(x, y + 1)) && crosses(1, extent))
}

fn boundary_candidates_impl(
    store: &EvidenceStore,
    required_tiles: &[(u32, u32)],
    prefilter_interior_only: bool,
    structured_bus: bool,
) -> Result<Vec<BoundaryCandidate>> {
    let required: BTreeSet<_> = required_tiles.iter().copied().collect();
    let available: BTreeSet<_> = store.bindings().map(|b| (b.x, b.y)).collect();
    if required.len() != required_tiles.len() || required != available || required.is_empty() {
        return Err(Error::Binding);
    }
    let scoped;
    let store = if store.is_decoded_scope() {
        store
    } else {
        let keys: Vec<_> = store.bindings().map(|b| b.canonical_url.clone()).collect();
        scoped = store.scope(&keys)?;
        &scoped
    };
    let grade_context = |l: &TileLine| {
        super::bus_grade::supported_class(l)
            && super::bus_grade::grade(l).is_some()
            && super::bus_grade::one_way(l).is_some()
            && super::bus_grade::allowed_access(l)
    };
    let mut lines = Vec::new();
    let mut points = 0;
    for b in store.bindings() {
        for l in &store.decoded(&b.canonical_url)?.lines {
            if !(if structured_bus {
                super::bus_grade::supported_class(l)
            } else {
                candidate_class(l)
            }) || (prefilter_interior_only && !crosses_context_boundary(l, b, &required))
            {
                continue;
            }
            points += l.coordinates.len();
            if lines.len() >= MAX_LINES || points > MAX_POINTS || l.coordinates.len() > 512 {
                return Err(Error::Limit);
            }
            let geometry: Vec<_> = l
                .coordinates
                .iter()
                .map(|p| {
                    [
                        (f64::from(b.x) + f64::from(p[0]) / f64::from(l.extent)) / 16384. * 360.
                            - 180.,
                        latitude(f64::from(b.y) + f64::from(p[1]) / f64::from(l.extent)),
                    ]
                })
                .collect();
            if geometry
                .iter()
                .any(|p| p[0].abs() > 180. || p[1].abs() > 85.)
            {
                return Err(Error::Unsupported);
            }
            lines.push(Line {
                id: format!(
                    "ofm:{}:14:{}:{}:{}:{}",
                    b.revision, b.x, b.y, l.feature_index, l.line_index
                ),
                tile: b,
                raw: l,
                points: geometry,
            });
        }
    }
    let mut groups: BTreeMap<(u32, u32), Vec<&Line<'_>>> = BTreeMap::new();
    for l in &lines {
        groups.entry((l.tile.x, l.tile.y)).or_default().push(l);
    }
    let mut pairs = Vec::new();
    let mut checked = 0;
    let mut budget = MAX_WORK;
    for (&(x, y), a) in &groups {
        for next in [(x + 1, y), (x, y + 1)] {
            let Some(b) = groups.get(&next) else {
                continue;
            };
            let vertical = x != next.0;
            let axis = if vertical { 0 } else { 1 };
            let south = latitude(f64::from(y.max(next.1) + 1));
            let north = latitude(f64::from(y.min(next.1)));
            let scale = [111320. * ((south + north) / 2.).to_radians().cos(), 111320.];
            let seam = if vertical {
                (f64::from(next.0) / 16384. * 360. - 180.) * scale[0]
            } else {
                latitude(f64::from(next.1)) * scale[1]
            };
            let (low, high) = if vertical {
                (south * scale[1], north * scale[1])
            } else {
                (
                    (f64::from(x) / 16384. * 360. - 180.) * scale[0],
                    (f64::from(x + 1) / 16384. * 360. - 180.) * scale[0],
                )
            };
            let ac: Vec<_> = a
                .iter()
                .filter(|l| {
                    if structured_bus {
                        grade_context(l.raw)
                    } else {
                        eligible(l.raw)
                    }
                })
                .filter_map(|l| clip(l, axis, seam, scale, low, high))
                .collect();
            let bc: Vec<_> = b
                .iter()
                .filter(|l| {
                    if structured_bus {
                        grade_context(l.raw)
                    } else {
                        eligible(l.raw)
                    }
                })
                .filter_map(|l| clip(l, axis, seam, scale, low, high))
                .collect();
            let mut found = Vec::new();
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for a in &ac {
                for b in &bc {
                    if !same(a.line.raw, b.line.raw) {
                        continue;
                    }
                    let directed = if structured_bus {
                        super::bus_grade::one_way(a.line.raw).is_some_and(|v| v != 0)
                    } else {
                        a.line
                            .raw
                            .attributes
                            .get("oneway")
                            .and_then(number)
                            .is_some_and(|v| v != 0.)
                    };
                    if directed && a.direction != b.direction {
                        continue;
                    }
                    checked += 1;
                    if checked > MAX_PAIRS {
                        return Err(Error::Limit);
                    }
                    let tangent = |p: &[Point]| {
                        let d = [p[p.len() - 1][0] - p[0][0], p[p.len() - 1][1] - p[0][1]];
                        let n = d[0].hypot(d[1]);
                        [d[0] / n, d[1] / n]
                    };
                    let ta = tangent(&a.points);
                    let tb = tangent(&b.points);
                    if ta[0] * tb[0] + ta[1] * tb[1] < 0.995 {
                        continue;
                    }
                    let error = separation(&a.points, &b.points, &mut budget)?.max(separation(
                        &b.points,
                        &a.points,
                        &mut budget,
                    )?);
                    if error > TOLERANCE {
                        continue;
                    }
                    found.push(BoundaryCandidate {
                        a_fragment: a.line.id.clone(),
                        b_fragment: b.line.id.clone(),
                        overlap_meters: a.length.min(b.length),
                        max_separation_meters: error,
                    });
                    *counts.entry(&a.line.id).or_default() += 1;
                    *counts.entry(&b.line.id).or_default() += 1;
                }
            }
            pairs.extend(found.into_iter().filter(|p| {
                counts[p.a_fragment.as_str()] == 1 && counts[p.b_fragment.as_str()] == 1
            }));
        }
    }
    pairs.sort_by(|a, b| (&a.a_fragment, &a.b_fragment).cmp(&(&b.a_fragment, &b.b_fragment)));
    Ok(pairs)
}
/// Rejects missing, extra, duplicated, reordered or numerically forged candidate
/// claims. This is NOT approval to save a seam path; reconstruction is separate.
pub fn validate_boundary_candidates(
    store: &EvidenceStore,
    required_tiles: &[(u32, u32)],
    claimed: &[BoundaryCandidate],
) -> Result<Vec<BoundaryCandidate>> {
    let actual = boundary_candidates(store, required_tiles)?;
    if actual.len() != claimed.len()
        || actual.iter().zip(claimed).any(|(a, b)| {
            a.a_fragment != b.a_fragment
                || a.b_fragment != b.b_fragment
                || !b.overlap_meters.is_finite()
                || !b.max_separation_meters.is_finite()
                || (a.overlap_meters - b.overlap_meters).abs() > 1e-6
                || (a.max_separation_meters - b.max_separation_meters).abs() > 1e-6
        })
    {
        return Err(Error::Binding);
    }
    Ok(actual)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn continuous_upper_bound_and_shared_work_budget() {
        let target = [[0., 0.], [1., 2.], [2., 0.]];
        let mut budget = MAX_WORK;
        assert!(separation(&[[0., 0.], [2., 0.]], &target, &mut budget).unwrap() > TOLERANCE);
        assert_eq!(
            separation(&[[0., 0.], [2., 0.]], &target, &mut 0),
            Err(Error::Limit)
        );
        assert!(
            (separation(
                &[[0., 0.], [20., 0.]],
                &[[0., 0.634], [20., 0.634]],
                &mut budget
            )
            .unwrap()
                - 0.634)
                .abs()
                < 1e-10
        );
    }
}
