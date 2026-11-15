//! Immutable reference geometry. Assets are derived from the saved snapshot and current
//! alignment geometry, so edits, deletions and Undo cannot leave stale crossing records.
use super::*;
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageBounds {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoadWay {
    /// Spatial validity of this version; None uses whole-snapshot coverage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<Vec<CoverageBounds>>,
    pub id: String,
    pub points: Vec<GeoPoint>,
    /// Source tags, not metres. Unknown/unsupported structures must not become surface roads.
    pub layer: Option<String>,
    pub level: Option<String>,
    pub bridge: Option<String>,
    pub tunnel: Option<String>,
}

/// Loaded, bounded road reference data. This is not a claim of nationwide completeness.
/// No terrain/water facts are represented or inferred here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoadSnapshot {
    pub revision: String,
    /// Dataset identity, origin and attribution supplied by the importer.
    pub source: String,
    pub source_url: String,
    pub source_sha256: String,
    pub attribution: String,
    pub license: String,
    /// Explicit operator verification that the clipping polygon belongs to this source.
    /// Unverified geometry can supply road facts, but never complete-area coverage.
    pub coverage_verified: bool,
    pub road_coverage: Vec<CoverageBounds>,
    pub roads: Vec<RoadWay>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomaticCrossing {
    pub road_id: String,
    pub point: GeoPoint,
    pub position: f64,
}
/// Advisory only: incomplete reference data must not disable normal construction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoadAssessment {
    pub crossings: Vec<AutomaticCrossing>,
    pub roads_ready: bool,
    pub unknown_road_height: bool,
}
impl RoadSnapshot {
    pub fn validate(&self) -> Result<(), WorldError> {
        let invalid = || WorldError::InvalidRoadSnapshot;
        if self.revision.trim().is_empty()
            || self.revision.len() > 200
            || self.source.trim().is_empty()
            || self.source.len() > 2000
            || !self.source_url.starts_with("https://")
            || self.source_url.len() > 2000
            || self.source_sha256.len() != 64
            || !self.source_sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || self.attribution.trim().is_empty()
            || self.attribution.len() > 2000
            || self.license.trim().is_empty()
            || self.license.len() > 200
            || self.roads.len() > 100000
            || self.road_coverage.len() > 10000
        {
            return Err(invalid());
        }
        let scopes = self.roads.iter().filter_map(|r| r.coverage.as_ref());
        if scopes.clone().map(Vec::len).sum::<usize>() > 100000 {
            return Err(invalid());
        }
        for b in self.road_coverage.iter().chain(scopes.flatten()) {
            if ![b.west, b.south, b.east, b.north]
                .iter()
                .all(|n| n.is_finite())
                || b.west >= b.east
                || b.south >= b.north
                || b.west < -180.
                || b.east > 180.
                || b.south < -85.
                || b.north > 85.
            {
                return Err(invalid());
            }
        }
        let mut count = 0;
        let mut ids = BTreeSet::new();
        for r in &self.roads {
            count += r.points.len();
            if r.id.is_empty()
                || r.id.len() > 100
                || !ids.insert(&r.id)
                || r.points.len() < 2
                || r.points.iter().any(|p| !valid_point(*p))
                || [&r.layer, &r.level, &r.bridge, &r.tunnel]
                    .iter()
                    .any(|v| v.as_ref().is_some_and(|s| s.len() > 100))
            {
                return Err(invalid());
            }
        }
        if count > 2000000 {
            return Err(invalid());
        }
        Ok(())
    }
}

// Missing OSM structure tags use the conventional surface-road default, not a
// measured elevation. Explicit malformed/unsupported tags remain unverified.
#[derive(Clone, Copy, PartialEq)]
enum RoadHeight {
    Surface,
    Separated,
    Unknown,
}
fn road_height(road: &RoadWay) -> RoadHeight {
    let mut separated = false;
    let mut unknown = false;
    for value in [&road.layer, &road.level].into_iter().flatten() {
        match value.parse::<f64>() {
            Ok(n) if n.is_finite() => separated |= n != 0.,
            _ => unknown = true,
        }
    }
    for value in [&road.bridge, &road.tunnel].into_iter().flatten() {
        match value.as_str() {
            "no" => (),
            "yes" | "viaduct" | "aqueduct" | "boardwalk" | "cantilever" | "covered"
            | "building_passage" | "culvert" => separated = true,
            _ => unknown = true,
        }
    }
    if separated {
        RoadHeight::Separated
    } else if unknown {
        RoadHeight::Unknown
    } else {
        RoadHeight::Surface
    }
}
fn valid_point(p: GeoPoint) -> bool {
    p.latitude.is_finite()
        && p.longitude.is_finite()
        && p.latitude.abs() <= 85.0
        && p.longitude.abs() <= 180.0
}
fn cross(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.1 - a.1 * b.0
}
fn sub(a: GeoPoint, b: GeoPoint) -> (f64, f64) {
    (a.longitude - b.longitude, a.latitude - b.latitude)
}
fn mix(a: GeoPoint, b: GeoPoint, t: f64) -> GeoPoint {
    GeoPoint {
        longitude: a.longitude + (b.longitude - a.longitude) * t,
        latitude: a.latitude + (b.latitude - a.latitude) * t,
    }
}
fn metres(a: GeoPoint, b: GeoPoint) -> f64 {
    let d = sub(a, b);
    6371000.0
        * (d.0.to_radians() * ((a.latitude + b.latitude) / 2.0).to_radians().cos())
            .hypot(d.1.to_radians())
}
fn intersection(a: GeoPoint, b: GeoPoint, c: GeoPoint, d: GeoPoint) -> Option<(f64, f64)> {
    let r = sub(b, a);
    let s = sub(d, c);
    let den = cross(r, s);
    if den.abs() < 1e-15 {
        return None;
    }
    let t = cross(sub(c, a), s) / den;
    let u = cross(sub(c, a), r) / den;
    if (-1e-9..=1.0 + 1e-9).contains(&t) && (-1e-9..=1.0 + 1e-9).contains(&u) {
        Some((t.clamp(0.0, 1.0), u.clamp(0.0, 1.0)))
    } else {
        None
    }
}
fn merge(mut spans: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut result: Vec<(f64, f64)> = vec![];
    for (a, b) in spans {
        if let Some(last) = result.last_mut() {
            if a <= last.1 + 1e-9 {
                last.1 = last.1.max(b);
                continue;
            }
        }
        result.push((a, b));
    }
    result
}
fn coverage(a: GeoPoint, b: GeoPoint, boxes: &[CoverageBounds]) -> bool {
    let spans = boxes
        .iter()
        .filter_map(|r| {
            let mut lo: f64 = 0.0;
            let mut hi: f64 = 1.0;
            for (x, y, min, max) in [
                (a.longitude, b.longitude, r.west, r.east),
                (a.latitude, b.latitude, r.south, r.north),
            ] {
                if (y - x).abs() < 1e-15 {
                    if x < min || x > max {
                        return None;
                    }
                } else {
                    let p = (min - x) / (y - x);
                    let q = (max - x) / (y - x);
                    lo = lo.max(p.min(q));
                    hi = hi.min(p.max(q));
                }
            }
            (lo <= hi).then_some((lo, hi))
        })
        .collect();
    let spans = merge(spans);
    spans.len() == 1 && spans[0].0 <= 1e-9 && spans[0].1 >= 1.0 - 1e-9
}
fn in_scope(p: GeoPoint, scope: Option<&[CoverageBounds]>) -> bool {
    scope.is_none_or(|bounds| {
        bounds.iter().any(|b| {
            p.longitude >= b.west
                && p.longitude <= b.east
                && p.latitude >= b.south
                && p.latitude <= b.north
        })
    })
}
/// A crossing at a source split vertex must cross the local line, not merely touch it.
fn crosses_at(points: &[GeoPoint], index: usize, t: f64, a: GeoPoint, b: GeoPoint) -> bool {
    let before = if t <= 1e-9 {
        index.checked_sub(1).map(|i| points[i])
    } else {
        Some(points[index])
    };
    let after = if t >= 1.0 - 1e-9 {
        points.get(index + 2).copied()
    } else {
        Some(points[index + 1])
    };
    let side = |p| cross(sub(b, a), sub(p, a));
    match (before, after) {
        (Some(p), Some(q)) => side(p) * side(q) < -1e-24,
        _ => false,
    }
}
fn crosses_road_at(
    road: &RoadWay,
    index: usize,
    t: f64,
    a: GeoPoint,
    b: GeoPoint,
    roads: &[(&RoadWay, &[CoverageBounds])],
) -> bool {
    if crosses_at(&road.points, index, t, a, b) {
        return true;
    }
    let point = mix(road.points[index], road.points[index + 1], t);
    let mut positive = false;
    let mut negative = false;
    for &(other, scope) in roads {
        if !in_scope(point, Some(scope))
            || !in_scope(point, other.coverage.as_deref())
            || !((road_height(road) == RoadHeight::Surface
                && road_height(other) == RoadHeight::Surface)
                || (other.layer == road.layer
                    && other.level == road.level
                    && other.bridge == road.bridge
                    && other.tunnel == road.tunnel))
        {
            continue;
        }
        for edge in other.points.windows(2) {
            for (p, q) in [(edge[0], edge[1]), (edge[1], edge[0])] {
                if metres(p, point) < 0.001 {
                    let side = cross(sub(b, a), sub(q, a));
                    positive |= side > 1e-12;
                    negative |= side < -1e-12;
                }
            }
        }
    }
    positive && negative
}

/// Derive crossings from current physical height at each intersection. Never cache
/// these records in a save: alignment edits, removals and Undo change them immediately.
pub fn assess_road_crossings(alignment: &Alignment, snapshots: &[RoadSnapshot]) -> RoadAssessment {
    assess_connected_road_crossings(alignment, snapshots, &[], &[])
}

fn assess_connected_road_crossings(
    alignment: &Alignment,
    snapshots: &[RoadSnapshot],
    start_neighbors: &[GeoPoint],
    end_neighbors: &[GeoPoint],
) -> RoadAssessment {
    let mut result = RoadAssessment {
        crossings: vec![],
        roads_ready: false,
        unknown_road_height: false,
    };
    if snapshots.is_empty() {
        return result;
    }
    let bounds: Vec<_> = snapshots
        .iter()
        .filter(|snapshot| snapshot.coverage_verified)
        .flat_map(|s| s.road_coverage.iter().copied())
        .collect();
    let roads: Vec<_> = snapshots
        .iter()
        .flat_map(|s| s.roads.iter().map(|r| (r, s.road_coverage.as_slice())))
        .collect();
    let total = alignment.length_m();
    if alignment.points.len() < 2 || !total.is_finite() || total <= 0. {
        return result;
    }
    result.roads_ready = true;
    let mut cursor = 0.;
    for (i, pair) in alignment.points.windows(2).enumerate() {
        let (a, b) = (pair[0], pair[1]);
        let length = super::alignment::distance(a, b);
        result.roads_ready &= coverage(a, b, &bounds);
        for &(road, scope) in &roads {
            let height = road_height(road);
            if height == RoadHeight::Separated {
                continue;
            }
            for (j, edge) in road.points.windows(2).enumerate() {
                let Some((t, u)) = intersection(a, b, edge[0], edge[1]) else {
                    continue;
                };
                let point = mix(a, b, t);
                if !in_scope(point, Some(scope))
                    || !in_scope(point, road.coverage.as_deref())
                    || !(crosses_at(&alignment.points, i, t, edge[0], edge[1])
                        || connected_endpoint_crosses(
                            alignment,
                            i,
                            t,
                            edge[0],
                            edge[1],
                            start_neighbors,
                            end_neighbors,
                        ))
                    || !crosses_road_at(road, j, u, a, b, &roads)
                {
                    continue;
                }
                let position = (cursor + length * t) / total;
                let Some(rail_height) = alignment.height_at_position_m(position) else {
                    result.unknown_road_height = true;
                    continue;
                };
                if rail_height.abs() > 1e-6 {
                    continue;
                }
                if height == RoadHeight::Unknown {
                    result.unknown_road_height = true;
                    continue;
                }
                // Duplicate mapped ways and source vertices produce one physical asset.
                if !result.crossings.iter().any(|c| {
                    metres(c.point, point) < 0.01 && (c.position - position).abs() * total < 0.01
                }) {
                    result.crossings.push(AutomaticCrossing {
                        road_id: road.id.clone(),
                        point,
                        position,
                    });
                }
            }
        }
        cursor += length;
    }
    result
        .crossings
        .sort_by(|a, b| a.position.total_cmp(&b.position));
    result
}
impl World {
    /// Add immutable reference coverage without discarding earlier loaded regions.
    pub fn add_road_snapshot(&mut self, snapshot: RoadSnapshot) -> Result<(), WorldError> {
        snapshot.validate()?;
        if self.data.road_snapshots.contains(&snapshot) {
            return Ok(());
        }
        let mut candidate = self.data.road_snapshots.clone();
        candidate.push(snapshot);
        validate_road_snapshots(&candidate)?;
        self.data.road_snapshots = candidate;
        Ok(())
    }
    /// Derive all road assets against explicit topology. A shared endpoint asset
    /// belongs to the lowest alignment ID, so splitting a line never duplicates it.
    pub fn road_assessments(&self) -> std::collections::BTreeMap<AlignmentId, RoadAssessment> {
        let mut result = std::collections::BTreeMap::new();
        for alignment in &self.data.alignments {
            let mut starts = vec![];
            let mut ends = vec![];
            for connection in &self.data.alignment_connections {
                for (here, other) in [(connection.a, connection.b), (connection.b, connection.a)] {
                    if here.alignment_id != alignment.id {
                        continue;
                    }
                    let Some(neighbor) = self
                        .data
                        .alignments
                        .iter()
                        .find(|a| a.id == other.alignment_id)
                    else {
                        continue;
                    };
                    // Use physical profiles, not categorical level labels. The world
                    // validates equal endpoint positions, grades and explicit links.
                    if alignment.endpoint_height_m(here.endpoint).abs() > 1e-6
                        || neighbor.endpoint_height_m(other.endpoint).abs() > 1e-6
                    {
                        continue;
                    }
                    let inside = match other.endpoint {
                        AlignmentEndpoint::Start => neighbor.points[1],
                        AlignmentEndpoint::End => neighbor.points[neighbor.points.len() - 2],
                    };
                    match here.endpoint {
                        AlignmentEndpoint::Start => starts.push(inside),
                        AlignmentEndpoint::End => ends.push(inside),
                    }
                }
            }
            result.insert(
                alignment.id,
                assess_connected_road_crossings(
                    alignment,
                    &self.data.road_snapshots,
                    &starts,
                    &ends,
                ),
            );
        }
        let mut shared_endpoints: Vec<GeoPoint> = vec![];
        for assessment in result.values_mut() {
            assessment.crossings.retain(|crossing| {
                if crossing.position > 1e-9 && crossing.position < 1. - 1e-9 {
                    return true;
                }
                if shared_endpoints
                    .iter()
                    .any(|p| metres(*p, crossing.point) < 0.01)
                {
                    return false;
                }
                shared_endpoints.push(crossing.point);
                true
            });
        }
        result
    }
    pub fn road_assessment(&self, id: AlignmentId) -> Result<RoadAssessment, WorldError> {
        self.road_assessments()
            .remove(&id)
            .ok_or(WorldError::MissingReference("alignment"))
    }
}

/// Conflicting revisions/geometries never silently replace saved reference facts.
pub(super) fn validate_road_snapshots(snapshots: &[RoadSnapshot]) -> Result<(), WorldError> {
    use std::collections::BTreeMap;
    if snapshots.len() > 512 {
        return Err(WorldError::InvalidRoadSnapshot);
    }
    let mut sources = BTreeMap::new();
    let mut roads: BTreeMap<&str, &RoadWay> = BTreeMap::new();
    let mut point_count = 0;
    for snapshot in snapshots {
        snapshot.validate()?;
        let identity = (
            &snapshot.revision,
            &snapshot.source_url,
            &snapshot.source_sha256,
            &snapshot.attribution,
            &snapshot.license,
        );
        if sources
            .insert(&snapshot.source, identity)
            .is_some_and(|old| old != identity)
        {
            return Err(WorldError::InvalidRoadSnapshot);
        }
        for road in &snapshot.roads {
            point_count += road.points.len();
            if point_count > 2000000 {
                return Err(WorldError::InvalidRoadSnapshot);
            }
            if let Some(old) = roads.insert(&road.id, road) {
                if old.points != road.points
                    || old.layer != road.layer
                    || old.level != road.level
                    || old.bridge != road.bridge
                    || old.tunnel != road.tunnel
                {
                    return Err(WorldError::InvalidRoadSnapshot);
                }
            }
        }
    }
    Ok(())
}

/// Only a declared continuation on the opposite side turns an endpoint touch
/// into a through-crossing. Nearby but disconnected rail geometry is irrelevant.
fn connected_endpoint_crosses(
    alignment: &Alignment,
    index: usize,
    t: f64,
    a: GeoPoint,
    b: GeoPoint,
    starts: &[GeoPoint],
    ends: &[GeoPoint],
) -> bool {
    let (inside, neighbors) = if index == 0 && t <= 1e-9 {
        (alignment.points[1], starts)
    } else if index + 2 == alignment.points.len() && t >= 1. - 1e-9 {
        (alignment.points[index], ends)
    } else {
        return false;
    };
    let side = |p| cross(sub(b, a), sub(p, a));
    neighbors.iter().any(|p| side(inside) * side(*p) < -1e-24)
}
