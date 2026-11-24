//! Read-only geometry projection for the opt-in retained reference overlay.
//! This never supplies authority, connectivity, or completeness to gameplay.
use super::{road_grid_lon_lat, Error, EvidenceStore, Result, MAX_TILES};

const Z14_EDGE: u32 = 1 << 14;
const MAX_SEGMENTS: usize = 4_000;

#[derive(Debug, PartialEq, serde::Serialize)]
pub struct RetainedRoadView {
    pub requested_tiles: usize,
    pub retained_tiles: usize,
    pub segments: Vec<[[f64; 2]; 2]>,
    pub truncated: bool,
}

/// Project only the retained z14 tiles intersecting an inclusive tile window.
/// Every returned coordinate is [longitude, latitude]; lines are clipped to
/// their owning tile to avoid showing provider buffers outside the window.
pub fn retained_road_view(store: &EvidenceStore, rect: [u32; 4]) -> Result<RetainedRoadView> {
    let [west, north, east, south] = rect;
    if west > east || north > south || east >= Z14_EDGE || south >= Z14_EDGE {
        return Err(Error::Binding);
    }
    let requested_tiles = ((east - west + 1) * (south - north + 1)) as usize;
    if requested_tiles > MAX_TILES {
        return Err(Error::Limit);
    }
    // Discovery scans the existing bounded manifest, not the world's geometry.
    let keys: Vec<_> = store
        .bindings()
        .filter(|tile| (west..=east).contains(&tile.x) && (north..=south).contains(&tile.y))
        .map(|tile| tile.canonical_url.clone())
        .collect();
    let mut result = RetainedRoadView {
        requested_tiles,
        retained_tiles: keys.len(),
        segments: Vec::new(),
        truncated: false,
    };
    if keys.is_empty() {
        return Ok(result);
    }
    let scope = store.scope(&keys)?;
    for key in keys {
        let tile = scope.decoded(&key)?;
        for line in &tile.lines {
            for part in line.coordinates.windows(2) {
                let local = |p: [i32; 2]| p.map(|n| f64::from(n) / f64::from(line.extent));
                let Some(segment) = clip_tile(local(part[0]), local(part[1])) else {
                    continue;
                };
                if result.segments.len() == MAX_SEGMENTS {
                    result.truncated = true;
                    return Ok(result);
                }
                result.segments.push(segment.map(|[x, y]| {
                    road_grid_lon_lat(f64::from(tile.binding.x) + x, f64::from(tile.binding.y) + y)
                }));
            }
        }
    }
    Ok(result)
}

fn clip_tile(a: [f64; 2], b: [f64; 2]) -> Option<[[f64; 2]; 2]> {
    let mut low = 0.0_f64;
    let mut high = 1.0_f64;
    let delta = [b[0] - a[0], b[1] - a[1]];
    for axis in 0..2 {
        if delta[axis] == 0. {
            if !(0.0..=1.0).contains(&a[axis]) {
                return None;
            }
            continue;
        }
        let start = -a[axis] / delta[axis];
        let end = (1. - a[axis]) / delta[axis];
        low = low.max(start.min(end));
        high = high.min(start.max(end));
    }
    if low >= high || delta == [0.; 2] {
        return None;
    }
    Some([low, high].map(|t| {
        [
            (a[0] + t * delta[0]).clamp(0., 1.),
            (a[1] + t * delta[1]).clamp(0., 1.),
        ]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ofm_evidence::{raw_sha256, RawEvidence, TileBinding, DECODER_RULE};

    #[test]
    fn decoded_view_reads_only_selected_tiles_and_does_not_rewrite_the_archive() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/ofm_session_synthetic.json"
        ))
        .unwrap();
        let bytes: Vec<u8> =
            serde_json::from_value(fixture["rawEvidence"][0]["bytes"].clone()).unwrap();
        let sha = raw_sha256(&bytes);
        let rows: Vec<_> = [(14552, 6451), (14624, 6016)]
            .into_iter()
            .map(|(x, y)| TileBinding {
                canonical_url: format!("https://tiles.openfreemap.org/planet/test/14/{x}/{y}.pbf"),
                revision: "test".into(),
                decoder_rule: DECODER_RULE.into(),
                raw_sha256: sha.clone(),
                z: 14,
                x,
                y,
            })
            .collect();
        let store = EvidenceStore::from_archive_parts(
            vec![RawEvidence {
                sha256: sha.clone(),
                bytes: bytes.clone(),
            }],
            rows.clone(),
        )
        .unwrap();
        let manifest = store.descriptors();
        assert!(store.cached_view_tiles_for_test().is_empty());
        let near = retained_road_view(&store, [14552, 6451, 14553, 6451]).unwrap();
        assert_eq!((near.requested_tiles, near.retained_tiles), (2, 1));
        assert!(!near.segments.is_empty());
        assert_eq!(store.cached_view_tiles_for_test(), [(14552, 6451)]);
        let no_coverage = retained_road_view(&store, [100, 100, 100, 100]).unwrap();
        assert!(no_coverage.segments.is_empty());
        assert_eq!(store.cached_view_tiles_for_test(), [(14552, 6451)]);
        assert_eq!(
            retained_road_view(&store, [14552, 6016, 14624, 6451]),
            Err(Error::Limit)
        );
        let far = retained_road_view(&store, [14624, 6016, 14624, 6016]).unwrap();
        assert!(!far.segments.is_empty());
        assert_ne!(near.segments, far.segments);
        assert_eq!(
            store.cached_view_tiles_for_test(),
            [(14552, 6451), (14624, 6016)]
        );
        assert_eq!(store.descriptors(), manifest);
        assert_eq!(store.raw(&sha).unwrap(), bytes.as_slice());
    }

    #[test]
    fn tile_buffer_no_longer_draws_beyond_loaded_cell() {
        assert_eq!(
            clip_tile([-0.25, 0.5], [1.25, 0.5]),
            Some([[0., 0.5], [1., 0.5]])
        );
        assert_eq!(clip_tile([1.1, 0.], [1.1, 1.]), None);
        assert_eq!(clip_tile([0.5, 0.5], [0.5, 0.5]), None);
    }
}
