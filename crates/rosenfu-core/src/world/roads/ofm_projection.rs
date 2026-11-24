//! Advisory crossing view of immutable OFM bytes already retained by the world.
//! It is not a routing graph, a coverage certificate, or a saved snapshot.
use super::{
    ofm_tile_revision, overlaps, union_bounds, valid_point, CoverageBounds, GeoPoint,
    RoadClassification, RoadSnapshot, RoadWay,
};
use crate::ofm_evidence::{
    number, raw_road_fragment_id, road_grid_lon_lat, DecodedTile, EvidenceStore, RawValue,
    TileBinding, TileLine, MAX_REQUEST_BYTES, MAX_TILES,
};
use std::collections::{BTreeSet, VecDeque};
use std::sync::Arc;

const MAX_PROJECTED_POINTS: usize = 20_000;

// Buffered raw fragments can describe the same road in both adjacent cells.
// Their direct crossing is only advisory and they have no verified seam pair:
// do not claim an exact boundary hit from either side. The epsilon only allows
// for coordinate conversion/intersection rounding, not a geographic gap.
pub(super) fn owns_crossing_point(point: GeoPoint, scope: &[CoverageBounds]) -> bool {
    const EDGE_EPSILON_DEG: f64 = 1e-10;
    scope.iter().any(|b| {
        point.longitude > b.west + EDGE_EPSILON_DEG
            && point.longitude < b.east - EDGE_EPSILON_DEG
            && point.latitude > b.south + EDGE_EPSILON_DEG
            && point.latitude < b.north - EDGE_EPSILON_DEG
    })
}

fn one_revision_distinct_cells<'a>(tiles: impl Iterator<Item = &'a TileBinding>) -> bool {
    let mut tiles = tiles.peekable();
    let Some(first) = tiles.peek() else {
        return false;
    };
    let revision = first.revision.as_str();
    let mut coordinates = BTreeSet::new();
    tiles.all(|b| b.revision.as_str() == revision && coordinates.insert((b.x, b.y)))
}

fn tile_bounds(binding: &TileBinding) -> CoverageBounds {
    let x = f64::from(binding.x);
    let y = f64::from(binding.y);
    let [west, north] = road_grid_lon_lat(x, y);
    let [east, south] = road_grid_lon_lat(x + 1., y + 1.);
    CoverageBounds {
        west,
        south,
        east,
        north,
    }
}

fn text<'a>(line: &'a TileLine, key: &str) -> Option<&'a str> {
    match line.attributes.get(key) {
        Some(RawValue::String(value)) => Some(value),
        _ => None,
    }
}

fn observed_surface_class(line: &TileLine) -> Option<&str> {
    let class = text(line, "class")?;
    // Leave minor access paths and service spurs to the existing road snapshot
    // path; a crossing is more selective than a playable Bus/tram guide.
    if !matches!(
        class,
        "motorway" | "trunk" | "primary" | "secondary" | "tertiary" | "minor" | "busway"
    ) || line.attributes.contains_key("brunnel")
        || !line.attributes.get("layer").is_none_or(|value| match value {
            RawValue::String(s) => s.parse::<f64>() == Ok(0.),
            other => number(other) == Some(0.),
        })
    {
        return None;
    }
    Some(class)
}

fn project(tile: &DecodedTile, bounds: CoverageBounds, points: &mut usize) -> Option<RoadSnapshot> {
    let mut roads = Vec::new();
    let binding = &tile.binding;
    for line in &tile.lines {
        let Some(class) = observed_surface_class(line) else {
            continue;
        };
        if line.coordinates.len() < 2 {
            continue;
        }
        *points = points.checked_add(line.coordinates.len())?;
        if *points > MAX_PROJECTED_POINTS {
            return None;
        }
        let extent = f64::from(line.extent);
        let shape: Vec<_> = line
            .coordinates
            .iter()
            .map(|[x, y]| {
                let [longitude, latitude] = road_grid_lon_lat(
                    f64::from(binding.x) + f64::from(*x) / extent,
                    f64::from(binding.y) + f64::from(*y) / extent,
                );
                GeoPoint {
                    latitude,
                    longitude,
                }
            })
            .collect();
        if shape.iter().any(|p| !valid_point(*p)) {
            continue;
        }
        roads.push(RoadWay {
            id: raw_road_fragment_id(binding, line),
            points: shape,
            coverage: None,
            layer: text(line, "layer")
                .map(str::to_owned)
                .or_else(|| {
                    line.attributes
                        .get("layer")
                        .and_then(number)
                        .map(|n| n.to_string())
                }),
            level: None,
            bridge: Some("no".into()),
            tunnel: Some("no".into()),
            classification: Some(RoadClassification {
                class: class.into(),
                subclass: text(line, "subclass").map(str::to_owned),
                access: text(line, "access").map(str::to_owned),
                oneway: text(line, "oneway")
                    .map(str::to_owned)
                    .or_else(|| {
                        line.attributes
                            .get("oneway")
                            .and_then(number)
                            .map(|n| n.to_string())
                    }),
            }),
        });
    }
    let snapshot = RoadSnapshot {
        revision: binding.revision.clone(),
        source: binding.canonical_url.clone(),
        source_url: binding.canonical_url.clone(),
        source_sha256: binding.raw_sha256.clone(),
        attribution: "© OpenMapTiles © OpenStreetMap contributors".into(),
        license: "ODbL-1.0".into(),
        coverage_verified: false,
        road_coverage: vec![bounds],
        roads,
    };
    snapshot.validate().ok()?;
    Some(snapshot)
}

struct Candidate<'a> {
    binding: &'a TileBinding,
    bounds: CoverageBounds,
    bytes: usize,
}

struct ProjectedTile {
    index: usize,
    snapshot: Arc<RoadSnapshot>,
    points: usize,
}

/// One assessment pass shares immutable source projections only. Inspect the
/// bounded archive once, then resolve each alignment against its own scope.
/// Committed snapshots still win and alone can attest coverage.
pub(super) struct RetainedObservations<'a> {
    store: &'a EvidenceStore,
    candidates: Vec<Candidate<'a>>,
    cache: VecDeque<ProjectedTile>,
    cached_points: usize,
}

impl<'a> RetainedObservations<'a> {
    pub(super) fn new(store: &'a EvidenceStore, saved: &[RoadSnapshot]) -> Self {
        let saved_urls: BTreeSet<_> = saved.iter().map(|s| s.source_url.as_str()).collect();
        let saved_revision = saved
            .iter()
            .find_map(|s| ofm_tile_revision(&s.source_url).ok().flatten());
        let verified: Vec<_> = saved
            .iter()
            .filter(|s| s.coverage_verified)
            .filter_map(|s| s.road_coverage.iter().copied().reduce(union_bounds))
            .collect();
        let candidates = store
            .bindings()
            .filter_map(|binding| {
                if saved_urls.contains(binding.canonical_url.as_str())
                    || saved_revision.is_some_and(|revision| revision != binding.revision.as_str())
                {
                    return None;
                }
                let bounds = tile_bounds(binding);
                if verified.iter().any(|scope| overlaps(bounds, *scope)) {
                    return None;
                }
                let bytes = store.raw(&binding.raw_sha256).ok()?.len();
                Some(Candidate {
                    binding,
                    bounds,
                    bytes,
                })
            })
            .collect();
        Self {
            store,
            candidates,
            cache: VecDeque::new(),
            cached_points: 0,
        }
    }

    pub(super) fn for_alignment(&mut self, bounds: CoverageBounds) -> Vec<Arc<RoadSnapshot>> {
        self.try_alignment(bounds).unwrap_or_default()
    }

    fn try_alignment(&mut self, bounds: CoverageBounds) -> Option<Vec<Arc<RoadSnapshot>>> {
        let selected: Vec<_> = self
            .candidates
            .iter()
            .enumerate()
            .filter(|(_, tile)| overlaps(tile.bounds, bounds))
            .take(MAX_TILES + 1)
            .map(|(index, _)| index)
            .collect();
        if selected.len() > MAX_TILES
            || !one_revision_distinct_cells(selected.iter().map(|&i| self.candidates[i].binding))
        {
            return None;
        }
        let bytes = selected.iter().try_fold(0usize, |sum, &i| {
            sum.checked_add(self.candidates[i].bytes)
        })?;
        if bytes > MAX_REQUEST_BYTES {
            return None;
        }
        let mut points = 0usize;
        selected
            .into_iter()
            .map(|index| {
                let (snapshot, count) = self.cached_or_project(index)?;
                points = points.checked_add(count)?;
                (points <= MAX_PROJECTED_POINTS).then_some(snapshot)
            })
            .collect()
    }

    fn cached_or_project(&mut self, index: usize) -> Option<(Arc<RoadSnapshot>, usize)> {
        if let Some(position) = self.cache.iter().position(|tile| tile.index == index) {
            let tile = self.cache.remove(position)?;
            let result = (Arc::clone(&tile.snapshot), tile.points);
            self.cache.push_back(tile);
            return Some(result);
        }
        let candidate = &self.candidates[index];
        let key = &candidate.binding.canonical_url;
        let scope = self.store.scope(std::slice::from_ref(key)).ok()?;
        let mut points = 0;
        let snapshot = Arc::new(project(scope.decoded(key).ok()?, candidate.bounds, &mut points)?);
        while self.cache.len() >= MAX_TILES || self.cached_points + points > MAX_PROJECTED_POINTS {
            self.cached_points -= self.cache.pop_front()?.points;
        }
        self.cached_points += points;
        self.cache.push_back(ProjectedTile {
            index,
            snapshot: Arc::clone(&snapshot),
            points,
        });
        Some((snapshot, points))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_scope_requires_one_revision_and_distinct_cells() {
        let first = TileBinding {
            canonical_url: "https://tiles.openfreemap.org/planet/a/14/14552/6451.pbf".into(),
            revision: "a".into(),
            z: 14,
            x: 14552,
            y: 6451,
            raw_sha256: "a".repeat(64),
            decoder_rule: crate::ofm_evidence::DECODER_RULE.into(),
        };
        let mut adjacent = first.clone();
        adjacent.x += 1;
        adjacent.canonical_url = "https://tiles.openfreemap.org/planet/a/14/14553/6451.pbf".into();
        assert!(one_revision_distinct_cells([&first, &adjacent].into_iter()));
        assert!(!one_revision_distinct_cells([&first, &first].into_iter()));

        adjacent.revision = "b".into();
        adjacent.canonical_url = "https://tiles.openfreemap.org/planet/b/14/14553/6451.pbf".into();
        assert!(!one_revision_distinct_cells([&first, &adjacent].into_iter()));
    }
}
