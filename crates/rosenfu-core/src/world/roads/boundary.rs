//! Conservative correspondence hypotheses for buffered OFM fragments, not real
//! road identity or topology. Nothing here changes saved source geometry.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
type P = [f64; 2];
const BAND: f64 = 10.;
const TOLERANCE: f64 = 0.8;
const MAX_COMPARISONS: usize = 10_000;
// Sample-to-edge distance evaluations, not just candidate pair count. Dense
// source vertices cannot multiply the bounded pair count into billions of work.
const MAX_GEOMETRY_WORK: usize = 1_000_000;
fn comparison_work(a: &[P], b: &[P]) -> usize {
    let samples = |p: &[P]| {
        p.windows(2)
            .map(|e| (distance(e[0], e[1]) / 2.).ceil().max(1.) as usize + 1)
            .sum::<usize>()
    };
    2 * (samples(a) * (b.len() - 1) + samples(b) * (a.len() - 1))
}

pub(super) struct Pair {
    a: String,
    b: String,
    axis: usize,
    seam: f64,
    scale: P,
}
struct Clipped<'a> {
    road: &'a RoadWay,
    points: Vec<P>,
    low: f64,
    high: f64,
    direction: f64,
}
fn lat(y: u32) -> f64 {
    (std::f64::consts::PI * (1. - 2. * f64::from(y) / 16384.))
        .sinh()
        .atan()
        .to_degrees()
}
fn distance(a: P, b: P) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn point_segment(p: P, a: P, b: P) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let length = d[0] * d[0] + d[1] * d[1];
    let t = if length > 0. {
        ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / length
    } else {
        0.
    }
    .clamp(0., 1.);
    distance(p, [a[0] + t * d[0], a[1] + t * d[1]])
}
// For each source subsegment, find ONE target segment within tolerance at
// BOTH endpoints. Distance to a fixed convex segment is convex, so this bounds
// the entire subsegment, not merely samples. It may conservatively refuse a
// valid bend where the closest target segment changes between endpoints.
fn within_distance(a: &[P], b: &[P]) -> bool {
    for edge in a.windows(2) {
        let n = (distance(edge[0], edge[1]) / 2.).ceil().max(1.) as usize;
        for i in 0..n {
            let at = |i: usize| {
                let t = i as f64 / n as f64;
                [
                    edge[0][0] + t * (edge[1][0] - edge[0][0]),
                    edge[0][1] + t * (edge[1][1] - edge[0][1]),
                ]
            };
            let (p, q) = (at(i), at(i + 1));
            if !b.windows(2).any(|e| {
                point_segment(p, e[0], e[1]) <= TOLERANCE
                    && point_segment(q, e[0], e[1]) <= TOLERANCE
            }) {
                return false;
            }
        }
    }
    true
}

fn eligible(r: &RoadWay) -> bool {
    road_height(r) == RoadHeight::Surface
        && r.points.len() <= 512
        && r.classification.as_ref().is_some_and(|c| {
            matches!(
                c.class.as_str(),
                "primary" | "secondary" | "tertiary" | "minor" | "service"
            ) && c
                .oneway
                .as_deref()
                .is_none_or(|v| matches!(v, "0" | "1" | "-1" | "yes" | "no"))
        })
}
fn same(a: &RoadWay, b: &RoadWay) -> bool {
    a.classification == b.classification
        && a.layer == b.layer
        && a.level == b.level
        && a.bridge == b.bridge
        && a.tunnel == b.tunnel
}
fn clip(r: &RoadWay, axis: usize, seam: f64, scale: P, low: f64, high: f64) -> Option<Clipped<'_>> {
    if !eligible(r) {
        return None;
    }
    let mut out: Vec<P> = vec![];
    let mut direction: f64 = 0.;
    for edge in r.points.windows(2) {
        let a = [edge[0].longitude * scale[0], edge[0].latitude * scale[1]];
        let b = [edge[1].longitude * scale[0], edge[1].latitude * scale[1]];
        let d = b[axis] - a[axis];
        if d == 0. {
            if (a[axis] - seam).abs() <= BAND {
                return None;
            }
            continue;
        }
        let mut t0 = (seam - BAND - a[axis]) / d;
        let mut t1 = (seam + BAND - a[axis]) / d;
        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
        }
        t0 = t0.max(0.);
        t1 = t1.min(1.);
        if t1 <= t0 {
            continue;
        }
        if direction != 0. && direction != d.signum() {
            return None;
        }
        direction = d.signum();
        let p = [a[0] + t0 * (b[0] - a[0]), a[1] + t0 * (b[1] - a[1])];
        let q = [a[0] + t1 * (b[0] - a[0]), a[1] + t1 * (b[1] - a[1])];
        if out.last().is_some_and(|last| distance(*last, p) > 1e-5) {
            return None;
        }
        if out.is_empty() {
            out.push(p);
        }
        out.push(q);
    }
    if out.len() < 2
        || ((out[0][axis] - seam).abs() - BAND).abs() > 1e-5
        || ((out.last()?[axis] - seam).abs() - BAND).abs() > 1e-5
        || out[0][axis] == out.last()?[axis]
        || out
            .iter()
            .any(|p| p[1 - axis] < low + BAND || p[1 - axis] > high - BAND)
        || out.windows(2).map(|e| distance(e[0], e[1])).sum::<f64>() > 100.
    {
        return None;
    }
    if out[0][axis] > out.last()?[axis] {
        out.reverse();
    }
    let lo = out
        .iter()
        .map(|p| p[1 - axis])
        .fold(f64::INFINITY, f64::min);
    let hi = out
        .iter()
        .map(|p| p[1 - axis])
        .fold(f64::NEG_INFINITY, f64::max);
    Some(Clipped {
        road: r,
        points: out,
        low: lo,
        high: hi,
        direction,
    })
}
fn compatible(a: &Clipped<'_>, b: &Clipped<'_>) -> bool {
    if !same(a.road, b.road) {
        return false;
    }
    if a.road
        .classification
        .as_ref()
        .and_then(|c| c.oneway.as_deref())
        .is_some_and(|s| !matches!(s, "0" | "no"))
        && a.direction != b.direction
    {
        return false;
    }
    let tangent = |p: &[P]| {
        let a = p[0];
        let b = p[p.len() - 1];
        let n = distance(a, b);
        [(b[0] - a[0]) / n, (b[1] - a[1]) / n]
    };
    let ta = tangent(&a.points);
    let tb = tangent(&b.points);
    ta[0] * tb[0] + ta[1] * tb[1] >= 0.995
        && within_distance(&a.points, &b.points)
        && within_distance(&b.points, &a.points)
}

/// Once per world assessment. Only adjacent tile boundaries are compared, with
/// an along-boundary interval sweep and fail-closed global budgets. No all-road
/// Cartesian scan. Duplicate snapshots do not create false competing fragments.
pub(super) fn pairs(snapshots: &[RoadSnapshot]) -> Vec<Pair> {
    let mut tiles: BTreeMap<(u32, u32), BTreeMap<&str, &RoadWay>> = BTreeMap::new();
    let mut revision = None;
    for s in snapshots {
        let Ok(Some(rev)) = ofm_tile_revision(&s.source_url) else {
            continue;
        };
        if rev != s.revision || revision.is_some_and(|r| r != rev) {
            return vec![];
        }
        revision = Some(rev);
        let parts: Vec<_> = s.source_url.split('/').collect();
        let (Ok(x), Some(y)) = (
            parts[6].parse::<u32>(),
            parts[7]
                .strip_suffix(".pbf")
                .and_then(|v| v.parse::<u32>().ok()),
        ) else {
            return vec![];
        };
        let prefix = format!("ofm:{rev}:14:{x}:{y}:");
        for r in &s.roads {
            // Identity must belong to this exact source tile, not a display ID.
            let Some(suffix) = r.id.strip_prefix(&prefix) else {
                continue;
            };
            let id: Vec<_> = suffix.split(':').collect();
            let index = |v: &str, max: u64| {
                !v.is_empty()
                    && v.bytes().all(|b| b.is_ascii_digit())
                    && (v.len() == 1 || !v.starts_with('0'))
                    && v.parse::<u64>().is_ok_and(|n| n <= max)
            };
            if id.len() != 2 || !index(id[0], 9999) || !index(id[1], 9_007_199_254_740_991) {
                continue;
            }
            tiles.entry((x, y)).or_default().insert(&r.id, r);
        }
    }
    let mut matches = vec![];
    let mut comparisons = 0usize;
    let mut geometry_work = 0usize;
    let mut clipped_points = 0usize;
    let mut clipped_lines = 0usize;
    for (&(x, y), left) in &tiles {
        for (nx, ny, axis) in [(x + 1, y, 0usize), (x, y + 1, 1usize)] {
            let Some(right) = tiles.get(&(nx, ny)) else {
                continue;
            };
            let south = lat(y.max(ny) + 1);
            let north = lat(y.min(ny));
            let scale = [111320. * ((south + north) / 2.).to_radians().cos(), 111320.];
            let seam = if axis == 0 {
                (f64::from(nx) / 16384. * 360. - 180.) * scale[0]
            } else {
                lat(ny) * scale[1]
            };
            let (low, high) = if axis == 0 {
                (south * scale[1], north * scale[1])
            } else {
                (
                    (f64::from(x) / 16384. * 360. - 180.) * scale[0],
                    (f64::from(x + 1) / 16384. * 360. - 180.) * scale[0],
                )
            };
            let mut a: Vec<_> = left
                .values()
                .filter_map(|r| clip(r, axis, seam, scale, low, high))
                .collect();
            let mut b: Vec<_> = right
                .values()
                .filter_map(|r| clip(r, axis, seam, scale, low, high))
                .collect();
            clipped_lines += a.len() + b.len();
            clipped_points += a.iter().chain(&b).map(|v| v.points.len()).sum::<usize>();
            if clipped_lines > 2000 || clipped_points > 100000 {
                return vec![];
            }
            a.sort_by(|a, b| a.low.total_cmp(&b.low));
            b.sort_by(|a, b| a.low.total_cmp(&b.low));
            let mut candidates = vec![];
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            let mut begin = 0;
            for aa in &a {
                while begin < b.len() && b[begin].high < aa.low - TOLERANCE {
                    begin += 1;
                }
                for bb in b[begin..]
                    .iter()
                    .take_while(|bb| bb.low <= aa.high + TOLERANCE)
                {
                    comparisons += 1;
                    if comparisons > MAX_COMPARISONS {
                        return vec![];
                    }
                    if bb.high < aa.low - TOLERANCE || !same(aa.road, bb.road) {
                        continue;
                    }
                    geometry_work += comparison_work(&aa.points, &bb.points);
                    if geometry_work > MAX_GEOMETRY_WORK {
                        return vec![];
                    }
                    if !compatible(aa, bb) {
                        continue;
                    }
                    *counts.entry(&aa.road.id).or_default() += 1;
                    *counts.entry(&bb.road.id).or_default() += 1;
                    candidates.push((&aa.road.id, &bb.road.id));
                }
            }
            for (a, b) in candidates {
                if counts[a.as_str()] == 1 && counts[b.as_str()] == 1 {
                    matches.push(Pair {
                        a: a.clone(),
                        b: b.clone(),
                        axis,
                        seam,
                        scale,
                    });
                }
            }
        }
    }
    matches
}
fn local_rail(alignment: &Alignment, a: &AutomaticCrossing, b: &AutomaticCrossing) -> bool {
    let lo = a.position.min(b.position);
    let hi = a.position.max(b.position);
    let total = alignment.length_m();
    if (hi - lo) * total > 1.
        || (hi - lo) * total - super::super::alignment::distance(a.point, b.point) > 0.01
        || !alignment.is_flat_over_position(lo, hi)
    {
        return false;
    }
    let scale = [
        111320.
            * ((a.point.latitude + b.point.latitude) / 2.)
                .to_radians()
                .cos(),
        111320.,
    ];
    let project = |p: GeoPoint| [p.longitude * scale[0], p.latitude * scale[1]];
    let start = project(a.point);
    let end = project(b.point);
    let mut cursor = 0.;
    for edge in alignment.points.windows(2) {
        let length = super::super::alignment::distance(edge[0], edge[1]);
        let finish = cursor + length;
        if finish >= lo * total && cursor <= hi * total {
            let d = [
                (edge[1].longitude - edge[0].longitude) * scale[0],
                (edge[1].latitude - edge[0].latitude) * scale[1],
            ];
            let chord = [end[0] - start[0], end[1] - start[1]];
            let n = d[0].hypot(d[1]) * chord[0].hypot(chord[1]);
            // Equal-position candidates still require correspondence, but no arc
            // lies between them. Otherwise reject bends and reversed local rails.
            if n > 1e-12 && (d[0] * chord[0] + d[1] * chord[1]).abs() / n < 0.99995 {
                return false;
            }
            if finish > lo * total
                && finish < hi * total
                && point_segment(project(edge[1]), start, end) > 0.01
            {
                return false;
            }
        }
        cursor = finish;
    }
    true
}
/// Only observed crossings on both members of a conservative correspondence pair are
/// eligible. Fixed tile ordering chooses source geometry, never an average.
pub(super) fn consolidate(
    alignment: &Alignment,
    crossings: &mut Vec<AutomaticCrossing>,
    pairs: &[Pair],
) {
    if pairs.is_empty() || crossings.len() < 2 {
        return;
    }
    let mut by_road: BTreeMap<&str, Vec<(usize, &AutomaticCrossing)>> = BTreeMap::new();
    for (index, crossing) in crossings.iter().enumerate() {
        by_road
            .entry(&crossing.road_id)
            .or_default()
            .push((index, crossing));
    }
    let mut remove = BTreeSet::new();
    for pair in pairs {
        let in_band = |c: &AutomaticCrossing| {
            let p = [
                c.point.longitude * pair.scale[0],
                c.point.latitude * pair.scale[1],
            ];
            (p[pair.axis] - pair.seam).abs() <= BAND
        };
        let a: Vec<_> = by_road
            .get(pair.a.as_str())
            .into_iter()
            .flatten()
            .filter(|(_, c)| in_band(c))
            .copied()
            .collect();
        let b: Vec<_> = by_road
            .get(pair.b.as_str())
            .into_iter()
            .flatten()
            .filter(|(_, c)| in_band(c))
            .copied()
            .collect();
        if let ([(ai, ac)], [(bi, bc)]) = (a.as_slice(), b.as_slice()) {
            if local_rail(alignment, ac, bc) && !remove.contains(ai) {
                remove.insert(*bi);
            }
        }
    }
    let mut i = 0;
    crossings.retain(|_| {
        let keep = !remove.contains(&i);
        i += 1;
        keep
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoints_near_different_target_segments_do_not_bound_the_interval() {
        // The old 2 m endpoint sampling sees distance zero at both ends, but
        // distance at the midpoint is sqrt(0.8), exceeding the 0.8 m limit.
        let source = [[-1., 0.], [1., 0.]];
        let bent = [[-1., 0.], [0., 2.], [1., 0.]];
        assert!(!within_distance(&source, &bent));
        assert!(within_distance(&source, &[[-1., 0.634], [1., 0.634]]));
    }
}
