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
    /// Spatial validity of this version; None is legacy whole-snapshot coverage.
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaterPolygon {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<Vec<CoverageBounds>>,
    pub rings: Vec<Vec<GeoPoint>>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentSnapshot {
    pub revision: String,
    pub road_coverage: Vec<CoverageBounds>,
    pub roads: Vec<RoadWay>,
    pub water_coverage: Vec<CoverageBounds>,
    pub water: Vec<WaterPolygon>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomaticCrossing {
    pub road_id: String,
    pub point: GeoPoint,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentAssessment {
    pub crossings: Vec<AutomaticCrossing>,
    pub roads_ready: bool,
    pub water_ready: bool,
    pub unknown_road_height: bool,
    pub ground_water: bool,
    pub longest_water_m: f64,
    pub water_limit_exceeded: bool,
}
impl EnvironmentAssessment {
    pub fn buildable(&self) -> bool {
        self.roads_ready
            && self.water_ready
            && !self.unknown_road_height
            && !self.ground_water
            && !self.water_limit_exceeded
    }
}
fn valid_point(p: GeoPoint) -> bool {
    p.latitude.is_finite()
        && p.longitude.is_finite()
        && p.latitude.abs() <= 85.0
        && p.longitude.abs() <= 180.0
}
impl EnvironmentSnapshot {
    pub fn validate(&self) -> Result<(), WorldError> {
        if self.revision.len() > 200
            || self.roads.len() > 10000
            || self.water.len() > 10000
            || self.road_coverage.len() > 128
            || self.water_coverage.len() > 128
        {
            return Err(WorldError::InvalidEnvironment);
        }
        let scopes = self
            .roads
            .iter()
            .filter_map(|r| r.coverage.as_ref())
            .chain(self.water.iter().filter_map(|p| p.coverage.as_ref()));
        if scopes.clone().map(Vec::len).sum::<usize>() > 10000 {
            return Err(WorldError::InvalidEnvironment);
        }
        for b in self
            .road_coverage
            .iter()
            .chain(&self.water_coverage)
            .chain(scopes.flatten())
        {
            if ![b.west, b.south, b.east, b.north]
                .iter()
                .all(|n| n.is_finite())
                || b.west >= b.east
                || b.south >= b.north
                || b.west < -180.0
                || b.east > 180.0
                || b.south < -85.0
                || b.north > 85.0
            {
                return Err(WorldError::InvalidEnvironment);
            }
        }
        let mut count = 0;

        for r in &self.roads {
            count += r.points.len();
            if r.id.is_empty()
                || r.id.len() > 100
                || r.points.len() < 2
                || r.points.iter().any(|p| !valid_point(*p))
                || [&r.layer, &r.level, &r.bridge, &r.tunnel]
                    .iter()
                    .any(|v| v.as_ref().is_some_and(|s| s.len() > 100))
            {
                return Err(WorldError::InvalidEnvironment);
            }
        }
        for p in &self.water {
            if p.rings.is_empty() {
                return Err(WorldError::InvalidEnvironment);
            }
            for ring in &p.rings {
                count += ring.len();
                if ring.len() < 4
                    || ring.first() != ring.last()
                    || ring.iter().any(|p| !valid_point(*p))
                {
                    return Err(WorldError::InvalidEnvironment);
                }
            }
        }
        if count > 100000 {
            return Err(WorldError::InvalidEnvironment);
        }
        Ok(())
    }
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
fn ring_contains(p: GeoPoint, ring: &[GeoPoint]) -> bool {
    let mut inside = false;
    for pair in ring.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let ap = sub(p, a);
        let ab = sub(b, a);
        if cross(ap, ab).abs() < 1e-13
            && ap.0 * ab.0 + ap.1 * ab.1 >= 0.0
            && ap.0 * ab.0 + ap.1 * ab.1 <= ab.0 * ab.0 + ab.1 * ab.1
        {
            return true;
        }
        if (a.latitude > p.latitude) != (b.latitude > p.latitude)
            && p.longitude
                < (b.longitude - a.longitude) * (p.latitude - a.latitude)
                    / (b.latitude - a.latitude)
                    + a.longitude
        {
            inside = !inside;
        }
    }
    inside
}
fn in_scope(p: GeoPoint, scope: Option<&[CoverageBounds]>) -> bool {
    scope.is_none_or(|bounds| {
        bounds.iter().any(|b| {
            p.longitude >= b.west
                && p.longitude < b.east
                && p.latitude >= b.south
                && p.latitude < b.north
        })
    })
}
fn water_contains(p: GeoPoint, polygons: &[WaterPolygon]) -> bool {
    polygons.iter().any(|poly| {
        in_scope(p, poly.coverage.as_deref())
            && poly
                .rings
                .iter()
                .fold(false, |inside, ring| inside ^ ring_contains(p, ring))
    })
}
fn surface(road: &RoadWay) -> bool {
    let zero = |v: &Option<String>| {
        v.as_ref()
            .is_none_or(|v| v.parse::<f64>().is_ok_and(|n| n == 0.0))
    };
    zero(&road.layer)
        && zero(&road.level)
        && [&road.bridge, &road.tunnel]
            .iter()
            .all(|v| v.as_ref().is_none_or(|v| v == "no"))
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
    data: &EnvironmentSnapshot,
) -> bool {
    if crosses_at(&road.points, index, t, a, b) {
        return true;
    }
    let point = mix(road.points[index], road.points[index + 1], t);
    let mut positive = false;
    let mut negative = false;
    for other in &data.roads {
        if !in_scope(point, other.coverage.as_deref())
            || other.layer != road.layer
            || other.level != road.level
            || other.bridge != road.bridge
            || other.tunnel != road.tunnel
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
/// `flat_ground` is supplied from the vertical profile: false for any non-flat profile,
/// even if categorical infrastructure says Ground. OSM layer is never interpreted as metres.
pub fn assess_environment(
    points: &[GeoPoint],
    level: InfrastructureLevel,
    flat_ground: bool,
    data: &EnvironmentSnapshot,
) -> EnvironmentAssessment {
    assess_environment_sections(
        points,
        &vec![level; points.len().saturating_sub(1)],
        flat_ground,
        data,
    )
}
fn assess_environment_sections(
    points: &[GeoPoint],
    levels: &[InfrastructureLevel],
    flat_ground: bool,
    data: &EnvironmentSnapshot,
) -> EnvironmentAssessment {
    let mut result = EnvironmentAssessment {
        crossings: vec![],
        roads_ready: true,
        water_ready: true,
        unknown_road_height: false,
        ground_water: false,
        longest_water_m: 0.0,
        water_limit_exceeded: false,
    };
    let mut run = 0.0;
    let mut limited_run = 0.0;
    for (i, pair) in points.windows(2).enumerate() {
        let level = levels[i];
        let (a, b) = (pair[0], pair[1]);
        result.roads_ready &= coverage(a, b, &data.road_coverage);
        result.water_ready &= coverage(a, b, &data.water_coverage);
        let mut cuts = vec![0.0, 1.0];
        for polygon in &data.water {
            // Validity boundaries can divide water even when geometry spans the refresh.
            for bnd in polygon.coverage.iter().flatten() {
                let corners = [
                    GeoPoint {
                        longitude: bnd.west,
                        latitude: bnd.south,
                    },
                    GeoPoint {
                        longitude: bnd.east,
                        latitude: bnd.south,
                    },
                    GeoPoint {
                        longitude: bnd.east,
                        latitude: bnd.north,
                    },
                    GeoPoint {
                        longitude: bnd.west,
                        latitude: bnd.north,
                    },
                    GeoPoint {
                        longitude: bnd.west,
                        latitude: bnd.south,
                    },
                ];
                for edge in corners.windows(2) {
                    if let Some((t, _)) = intersection(a, b, edge[0], edge[1]) {
                        cuts.push(t);
                    }
                }
            }
            for ring in &polygon.rings {
                for edge in ring.windows(2) {
                    if let Some((t, _)) = intersection(a, b, edge[0], edge[1]) {
                        cuts.push(t);
                    }
                }
            }
        }
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
        for span in cuts.windows(2) {
            if water_contains(mix(a, b, (span[0] + span[1]) / 2.0), &data.water) {
                let length = metres(a, b) * (span[1] - span[0]);
                run += length;
                if matches!(
                    level,
                    InfrastructureLevel::Underground2 | InfrastructureLevel::Underground3
                ) {
                    limited_run = 0.0;
                } else {
                    limited_run += length;
                    if limited_run > 30000.0 + 1e-6 {
                        result.water_limit_exceeded = true;
                    }
                }
                result.longest_water_m = result.longest_water_m.max(run);
                if level == InfrastructureLevel::Ground {
                    result.ground_water = true;
                }
            } else {
                run = 0.0;
                limited_run = 0.0;
            }
        }
        if level != InfrastructureLevel::Ground || !flat_ground {
            continue;
        }
        for road in &data.roads {
            for (j, edge) in road.points.windows(2).enumerate() {
                if let Some((t, u)) = intersection(a, b, edge[0], edge[1]) {
                    if !in_scope(mix(a, b, t), road.coverage.as_deref())
                        || !crosses_at(points, i, t, edge[0], edge[1])
                        || !crosses_road_at(road, j, u, a, b, data)
                    {
                        continue;
                    }
                    if !surface(road) {
                        // Explicit nonzero layers / bridges / tunnels are grade-separated; malformed tags remain unknown.
                        let known_nonzero = [&road.layer, &road.level].iter().any(|v| {
                            v.as_ref().is_some_and(|s| {
                                s.parse::<f64>().is_ok_and(|n| n.is_finite() && n != 0.0)
                            })
                        });
                        let separated = known_nonzero
                            || [&road.bridge, &road.tunnel]
                                .iter()
                                .any(|v| v.as_ref().is_some_and(|s| s == "yes"));
                        if !separated {
                            result.unknown_road_height = true;
                        }
                        continue;
                    }
                    let point = mix(a, b, t);
                    if !result
                        .crossings
                        .iter()
                        .any(|c| same_crossing_point(c.point, point))
                    {
                        result.crossings.push(AutomaticCrossing {
                            road_id: road.id.clone(),
                            point,
                        });
                    }
                }
            }
        }
    }

    result
}

/// Uses full geometry across anchor boundaries, retaining continuous water runs.
/// The caller supplies flat_ground from the physical vertical profile when present.
pub fn assess_alignment_environment(
    alignment: &Alignment,
    flat_ground: bool,
    data: &EnvironmentSnapshot,
) -> EnvironmentAssessment {
    let anchors = alignment
        .anchor_indices
        .clone()
        .unwrap_or_else(|| vec![0, alignment.points.len() - 1]);
    let mut levels = vec![InfrastructureLevel::Ground; alignment.points.len() - 1];
    for (i, pair) in anchors.windows(2).enumerate() {
        for level in &mut levels[pair[0]..pair[1]] {
            *level = alignment
                .section_infrastructure(i)
                .unwrap_or_default()
                .level;
        }
    }
    assess_environment_sections(&alignment.points, &levels, flat_ground, data)
}

/// Check a physical non-branching chain, so splitting a water crossing into separately
/// built alignments cannot evade the continuous 30 km limit.
pub fn validate_connected_water(
    world: &WorldDraft,
    changed: &[AlignmentId],
    data: &EnvironmentSnapshot,
) -> Result<(), WorldError> {
    let mut visited: BTreeSet<AlignmentId> = BTreeSet::new();
    for seed in &world.alignments {
        if visited.contains(&seed.id) {
            continue;
        }
        let mut component = BTreeSet::new();
        let mut pending = vec![seed.id];
        while let Some(id) = pending.pop() {
            if !component.insert(id) {
                continue;
            }
            for c in &world.alignment_connections {
                if c.a.alignment_id == id {
                    pending.push(c.b.alignment_id);
                }
                if c.b.alignment_id == id {
                    pending.push(c.a.alignment_id);
                }
            }
        }
        visited.extend(component.iter().copied());
        if !changed.iter().any(|id| component.contains(id)) {
            continue;
        }
        // This model supports endpoint chains/rings. Ambiguous branches must not
        // silently select one outgoing edge and omit another water run.
        for id in &component {
            for endpoint in [AlignmentEndpoint::Start, AlignmentEndpoint::End] {
                if world
                    .alignment_connections
                    .iter()
                    .filter(|c| {
                        (c.a.alignment_id == *id && c.a.endpoint == endpoint)
                            || (c.b.alignment_id == *id && c.b.endpoint == endpoint)
                    })
                    .count()
                    > 1
                {
                    return Err(WorldError::InvalidEnvironment);
                }
            }
        }
        let link = |id, endpoint| {
            world.alignment_connections.iter().find_map(|c| {
                if c.a.alignment_id == id && c.a.endpoint == endpoint {
                    Some(c.b)
                } else if c.b.alignment_id == id && c.b.endpoint == endpoint {
                    Some(c.a)
                } else {
                    None
                }
            })
        };
        let mut start = (seed.id, AlignmentEndpoint::Start);
        for id in &component {
            for endpoint in [AlignmentEndpoint::Start, AlignmentEndpoint::End] {
                if link(*id, endpoint).is_none() {
                    start = (*id, endpoint);
                }
            }
        }
        let mut id = start.0;
        let mut entry = start.1;
        let mut seen = BTreeSet::new();
        let mut points = vec![];
        let mut levels = vec![];
        let mut closed = false;
        loop {
            if !seen.insert(id) {
                closed = (id, entry) == start;
                break;
            }
            let a = world
                .alignments
                .iter()
                .find(|a| a.id == id)
                .ok_or(WorldError::InvalidEnvironment)?;
            let anchors = a
                .anchor_indices
                .clone()
                .unwrap_or_else(|| vec![0, a.points.len() - 1]);
            let mut attrs = vec![];
            for (i, pair) in anchors.windows(2).enumerate() {
                attrs.extend(std::iter::repeat_n(
                    a.section_infrastructure(i).unwrap_or_default().level,
                    pair[1] - pair[0],
                ));
            }
            let mut line = a.points.clone();
            if entry == AlignmentEndpoint::End {
                line.reverse();
                attrs.reverse();
            }
            if points.is_empty() {
                points.extend(line);
            } else {
                points.extend(line.into_iter().skip(1));
            }
            levels.extend(attrs);
            let exit = if entry == AlignmentEndpoint::Start {
                AlignmentEndpoint::End
            } else {
                AlignmentEndpoint::Start
            };
            let Some(next) = link(id, exit) else {
                break;
            };
            id = next.alignment_id;
            entry = next.endpoint;
        }
        if seen != component {
            return Err(WorldError::InvalidEnvironment);
        }
        // Traverse a ring's join as well. Only rings longer than the limit need
        // doubling: a short fully-wet ring must not be counted twice.
        let length: f64 = points.windows(2).map(|p| metres(p[0], p[1])).sum();
        if closed && length > 30000.0 + 1e-6 {
            points.extend(points.clone().into_iter().skip(1));
            levels.extend(levels.clone());
        }
        let assessment = assess_environment_sections(&points, &levels, false, data);
        if !assessment.water_ready || assessment.ground_water || assessment.water_limit_exceeded {
            return Err(WorldError::InvalidEnvironment);
        }
    }
    Ok(())
}

pub fn same_crossing_point(a: GeoPoint, b: GeoPoint) -> bool {
    metres(a, b) < 0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: f64, y: f64) -> GeoPoint {
        GeoPoint {
            longitude: x,
            latitude: y,
        }
    }
    fn data() -> EnvironmentSnapshot {
        EnvironmentSnapshot {
            revision: "fixture".into(),
            road_coverage: vec![CoverageBounds {
                west: -1.0,
                south: -1.0,
                east: 1.0,
                north: 1.0,
            }],
            water_coverage: vec![CoverageBounds {
                west: -1.0,
                south: -1.0,
                east: 1.0,
                north: 1.0,
            }],
            roads: vec![RoadWay {
                coverage: None,
                id: "1".into(),
                points: vec![p(0.0, -0.1), p(0.0, 0.1)],
                layer: None,
                level: None,
                bridge: None,
                tunnel: None,
            }],
            water: vec![],
        }
    }
    fn assess(points: &[GeoPoint], d: &EnvironmentSnapshot) -> EnvironmentAssessment {
        assess_environment(points, InfrastructureLevel::Ground, true, d)
    }
    fn water(left: f64, right: f64) -> WaterPolygon {
        WaterPolygon {
            coverage: None,
            rings: vec![vec![
                p(left, -0.5),
                p(right, -0.5),
                p(right, 0.5),
                p(left, 0.5),
                p(left, -0.5),
            ]],
        }
    }
    #[test]
    fn proper_crossing_and_source_split_are_one() {
        let mut d = data();
        d.roads[0].points.insert(1, p(0.0, 0.0));
        assert_eq!(
            assess(&[p(-0.1, 0.0), p(0.0, 0.0), p(0.1, 0.0)], &d)
                .crossings
                .len(),
            1
        );
    }
    #[test]
    fn crossing_at_two_way_boundary_is_one() {
        let mut d = data();
        d.roads[0].points[1] = p(0.0, 0.0);
        let mut other = d.roads[0].clone();
        other.id = "2".into();
        other.points = vec![p(0.0, 0.0), p(0.0, 0.1)];
        d.roads.push(other);
        assert_eq!(assess(&[p(-0.1, 0.0), p(0.1, 0.0)], &d).crossings.len(), 1);
    }
    #[test]
    fn tangent_collinear_and_endpoint_contact_are_not_crossings() {
        let d = data();
        for points in [
            vec![p(-0.1, 0.0), p(0.0, 0.0), p(-0.1, 0.1)],
            vec![p(0.0, -0.1), p(0.0, 0.1)],
            vec![p(-0.1, 0.0), p(0.0, 0.0)],
        ] {
            assert!(assess(&points, &d).crossings.is_empty());
        }
    }
    #[test]
    fn grade_separation_and_slopes_are_not_crossings() {
        let mut d = data();
        let points = [p(-0.1, 0.0), p(0.1, 0.0)];
        assert!(
            assess_environment(&points, InfrastructureLevel::Elevated, true, &d)
                .crossings
                .is_empty()
        );
        assert!(
            assess_environment(&points, InfrastructureLevel::Ground, false, &d)
                .crossings
                .is_empty()
        );
        d.roads[0].bridge = Some("yes".into());
        assert!(assess(&points, &d).crossings.is_empty());
    }
    #[test]
    fn malformed_height_is_unknown() {
        let mut d = data();
        d.roads[0].layer = Some("unknown".into());
        let a = assess(&[p(-0.1, 0.0), p(0.1, 0.0)], &d);
        assert!(a.unknown_road_height);
        assert!(!a.buildable());
    }
    #[test]
    fn coverage_cannot_be_inferred_from_empty_data() {
        let d = EnvironmentSnapshot::default();
        let a = assess(&[p(-0.1, 0.0), p(0.1, 0.0)], &d);
        assert!(!a.roads_ready && !a.water_ready && !a.buildable());
    }
    #[test]
    fn coverage_checks_between_vertices_and_unions_adjacent_tiles() {
        let mut d = data();
        d.road_coverage = vec![
            CoverageBounds {
                west: -1.0,
                south: -1.0,
                east: -0.01,
                north: 1.0,
            },
            CoverageBounds {
                west: 0.01,
                south: -1.0,
                east: 1.0,
                north: 1.0,
            },
        ];
        let points = [p(-0.1, 0.0), p(0.1, 0.0)];
        assert!(!assess(&points, &d).roads_ready);
        d.road_coverage[0].east = 0.01;
        assert!(assess(&points, &d).roads_ready);
    }
    #[test]
    fn ground_partial_water_is_rejected_but_hole_is_land() {
        let mut d = data();
        d.water = vec![water(-0.05, 0.05)];
        let a = assess(&[p(-0.1, 0.0), p(0.1, 0.0)], &d);
        assert!(a.ground_water && !a.buildable());
        assert!((a.longest_water_m - 11119.4927).abs() < 0.1);
        d.water[0].rings.push(water(-0.02, 0.02).rings.remove(0));
        assert!(!assess(&[p(-0.01, 0.0), p(0.01, 0.0)], &d).ground_water);
    }
    #[test]
    fn thirty_km_boundary_and_deep_tunnel_exception() {
        let mut d = data();
        d.water = vec![water(-0.5, 0.5)];
        let longitude = 30000.0 / 6371000.0 * 180.0 / std::f64::consts::PI;
        let points = [p(0.0, 0.0), p(longitude, 0.0)];
        assert!(
            !assess_environment(&points, InfrastructureLevel::Elevated, true, &d)
                .water_limit_exceeded
        );
        let points = [p(0.0, 0.0), p(longitude + 0.00001, 0.0)];
        assert!(
            assess_environment(&points, InfrastructureLevel::Underground, true, &d)
                .water_limit_exceeded
        );
        assert!(
            !assess_environment(&points, InfrastructureLevel::Underground2, true, &d)
                .water_limit_exceeded
        );
    }
    #[test]
    fn overlapping_polygons_do_not_double_count_and_land_resets_run() {
        let mut d = data();
        d.water = vec![water(-0.3, -0.2), water(0.2, 0.3), water(0.2, 0.3)];
        let a = assess_environment(
            &[p(-0.4, 0.0), p(0.4, 0.0)],
            InfrastructureLevel::Elevated,
            true,
            &d,
        );
        assert!((a.longest_water_m - 11119.4927).abs() < 0.1);
    }
    #[test]
    fn separate_connected_alignments_cannot_evade_water_limit() {
        let mut w = World::default();
        let player = w.add_player("p".into()).unwrap();
        let owner = w.add_company(player, "c".into()).unwrap();
        let a = w
            .build_alignment(owner, "a".into(), vec![p(0.0, 0.0), p(0.18, 0.0)])
            .unwrap();
        let b = w
            .build_alignment(owner, "b".into(), vec![p(0.18, 0.0), p(0.36, 0.0)])
            .unwrap();
        let mut draft = w.snapshot().clone();
        for alignment in &mut draft.alignments {
            alignment.infrastructure = vec![SectionInfrastructure {
                track_count: 1,
                level: InfrastructureLevel::Elevated,
            }];
        }
        w.replace(draft).unwrap();
        w.connect_alignment_endpoints(
            owner,
            AlignmentEndpointRef {
                alignment_id: a,
                endpoint: AlignmentEndpoint::End,
            },
            AlignmentEndpointRef {
                alignment_id: b,
                endpoint: AlignmentEndpoint::Start,
            },
        )
        .unwrap();
        let mut d = data();
        d.water = vec![water(-0.5, 0.5)];
        assert!(validate_connected_water(w.snapshot(), &[b], &d).is_err());
    }
    fn ring_world(size: f64) -> (World, Vec<AlignmentId>) {
        let mut w = World::default();
        let player = w.add_player("p".into()).unwrap();
        let owner = w.add_company(player, "c".into()).unwrap();
        let lines = [
            vec![p(0.0, size / 2.0), p(0.0, size), p(size / 2.0, size)],
            vec![p(size / 2.0, size), p(size, size), p(size, size / 2.0)],
            vec![p(size, size / 2.0), p(size, 0.0), p(size / 2.0, 0.0)],
            vec![p(size / 2.0, 0.0), p(0.0, 0.0), p(0.0, size / 2.0)],
        ];
        let mut ids = vec![];
        for line in lines {
            ids.push(w.build_alignment(owner, "ring".into(), line).unwrap());
        }
        let mut d = w.snapshot().clone();
        for a in &mut d.alignments {
            a.infrastructure = vec![SectionInfrastructure {
                track_count: 1,
                level: InfrastructureLevel::Elevated,
            }];
        }
        w.replace(d).unwrap();
        for i in 0..ids.len() {
            w.connect_alignment_endpoints(
                owner,
                AlignmentEndpointRef {
                    alignment_id: ids[i],
                    endpoint: AlignmentEndpoint::End,
                },
                AlignmentEndpointRef {
                    alignment_id: ids[(i + 1) % ids.len()],
                    endpoint: AlignmentEndpoint::Start,
                },
            )
            .unwrap();
        }
        (w, ids)
    }
    #[test]
    fn circular_water_run_across_seed_exceeds_thirty_km() {
        let (w, ids) = ring_world(0.2);
        let mut d = data();
        d.water = vec![water(-0.1, 0.04)];
        // 22.2 km west edge + two 4.4 km partial edges: each linearized
        // prefix/suffix is below 30 km but the continuous circular run is not.
        assert!(validate_connected_water(w.snapshot(), &ids, &d).is_err());
        d.water = vec![water(-0.1, 0.03)];
        assert!(validate_connected_water(w.snapshot(), &ids, &d).is_ok());
        d.water = vec![water(-0.1, 0.04)];
        d.water[0].rings.push(vec![
            p(-0.01, 0.09),
            p(0.05, 0.09),
            p(0.05, 0.11),
            p(-0.01, 0.11),
            p(-0.01, 0.09),
        ]);
        assert!(validate_connected_water(w.snapshot(), &ids, &d).is_ok());
    }
    #[test]
    fn short_fully_wet_ring_is_counted_once_and_ambiguous_branch_is_rejected() {
        let (w, ids) = ring_world(0.05);
        let mut d = data();
        d.water = vec![water(-0.1, 0.1)];
        assert!(validate_connected_water(w.snapshot(), &ids, &d).is_ok());
        let mut malformed = w.snapshot().clone();
        malformed
            .alignment_connections
            .push(malformed.alignment_connections[0].clone());
        assert!(validate_connected_water(&malformed, &ids, &d).is_err());
    }
    #[test]
    fn saved_snapshot_reproduces_assets_and_geometry_edit_removes_them() {
        let d = data();
        let restored: EnvironmentSnapshot =
            serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        assert_eq!(
            assess(&[p(-0.1, 0.0), p(0.1, 0.0)], &d),
            assess(&[p(-0.1, 0.0), p(0.1, 0.0)], &restored)
        );
        assert!(assess(&[p(-0.1, 0.0), p(-0.01, 0.0)], &restored)
            .crossings
            .is_empty());
    }
}
