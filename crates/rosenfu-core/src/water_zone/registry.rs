//! Audited fixed-version water ranges, not a claim of complete Japanese coverage.
use super::ZonePin;
use std::sync::OnceLock;

pub const PROVIDER_REVISION: &str = "2026-11-25";
const AUDITED_ZONE_COUNT: usize = 41;

/// All verified pins. Hashes record observed bytes, not an author signature.
pub fn pins() -> Result<&'static [ZonePin], String> {
    static PINS: OnceLock<Result<Vec<ZonePin>, String>> = OnceLock::new();
    PINS.get_or_init(|| {
        let pins: Vec<ZonePin> = serde_json::from_str(include_str!("registry.json"))
            .map_err(|e| format!("water registry: {e}"))?;
        let mut ids = std::collections::BTreeSet::new();
        if pins.len() != AUDITED_ZONE_COUNT {
            return Err("water registry must contain 41 audited zones".into());
        }
        for pin in &pins {
            pin.validate()
                .map_err(|e| format!("water registry {}: {e:?}", pin.zone_id))?;
            if pin.provider_revision != PROVIDER_REVISION || !ids.insert(&pin.zone_id) {
                return Err("water registry version or duplicate zone".into());
            }
        }
        Ok(pins)
    })
    .as_ref()
    .map(Vec::as_slice)
    .map_err(Clone::clone)
}

/// Grid bounds in [west, south, east, north], using the reader's exact projection.
pub fn bounds(pin: &ZonePin) -> [f64; 4] {
    let (kx, ky) = pin.scale();
    [
        pin.origin_longitude + pin.x0 / kx,
        pin.origin_latitude + pin.y0 / ky,
        pin.origin_longitude + (pin.x0 + f64::from(pin.width) * pin.cell_m) / kx,
        pin.origin_latitude + (pin.y0 + f64::from(pin.height) * pin.cell_m) / ky,
    ]
}

/// Returns every touching grid; callers must resolve overlaps with wet priority.
/// An empty result means no registered fine grid, never a successful dry lookup.
/// A registered grid whose fetch fails remains unknown, not normal non-provision.
pub fn intersecting(query: [f64; 4]) -> Result<Vec<&'static ZonePin>, String> {
    validate_query(query)?;
    Ok(pins()?
        .iter()
        .filter(|pin| touches(bounds(pin), query))
        .collect())
}
fn validate_query(query: [f64; 4]) -> Result<(), String> {
    if !query.iter().all(|v| v.is_finite())
        || query[0] > query[2]
        || query[1] > query[3]
        || query[0] < -180.
        || query[2] > 180.
        || query[1] < -85.
        || query[3] > 85.
    {
        return Err("invalid water query bounds".into());
    }
    Ok(())
}
fn touches(b: [f64; 4], query: [f64; 4]) -> bool {
    b[0] <= query[2] && b[2] >= query[0] && b[1] <= query[3] && b[3] >= query[1]
}

/// Union of the grids touched by independent query bounds. Never merge the
/// queries into a nationwide bounding box: gaps between regions stay unloaded.
/// Geometry is immutable; the temporary match set is not world/session state.
pub fn intersecting_many(
    queries: impl IntoIterator<Item = [f64; 4]>,
) -> Result<Vec<&'static ZonePin>, String> {
    let pins = pins()?;
    let bounds: Vec<_> = pins.iter().map(bounds).collect();
    let mut matched = vec![false; pins.len()];
    for query in queries {
        // Validate every query even after every region has matched.
        validate_query(query)?;
        for (i, b) in bounds.iter().enumerate() {
            if !matched[i] {
                matched[i] =
                    b[0] <= query[2] && b[2] >= query[0] && b[1] <= query[3] && b[3] >= query[1];
            }
        }
    }
    Ok(pins
        .iter()
        .zip(matched)
        .filter_map(|(pin, hit)| hit.then_some(pin))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reference(queries: &[[f64; 4]]) -> Vec<&'static ZonePin> {
        pins()
            .unwrap()
            .iter()
            .filter(|pin| {
                queries.iter().any(|q| {
                    let b = bounds(pin);
                    b[0] <= q[2] && b[2] >= q[0] && b[1] <= q[3] && b[3] >= q[1]
                })
            })
            .collect()
    }
    #[test]
    fn batched_regions_match_independent_queries_without_filling_gaps() {
        let queries = [
            [139.75, 35.67, 139.76, 35.68],
            [141.35, 43.068, 141.351, 43.069],
        ];
        assert_eq!(intersecting_many(queries).unwrap(), reference(&queries));
        assert_eq!(
            intersecting_many([queries[1], queries[0], queries[1]]).unwrap(),
            reference(&queries)
        );
        assert!(intersecting_many([]).unwrap().is_empty());
        for pin in pins().unwrap() {
            let [w, s, e, n] = bounds(pin);
            for query in [[w, s, w, s], [e, n, e, n], [w - 1., s, e + 1., n]] {
                assert_eq!(intersecting_many([query]).unwrap(), reference(&[query]));
            }
        }
        // Invalid trailing input remains an error even after all grids matched.
        assert!(intersecting_many([[-180., -85., 180., 85.], [f64::NAN, 0., 0., 0.]]).is_err());
    }
    #[test]
    #[ignore = "manual synthetic performance measurement, no wall-clock assertion"]
    fn measure_batched_region_queries() {
        let queries: Vec<_> = (0..100_000)
            .map(|i| {
                let x = 139.70 + (i % 1000) as f64 * 0.00001;
                [x, 35.67, x + 0.00001, 35.67001]
            })
            .collect();
        let pins = pins().unwrap();
        let start = std::time::Instant::now();
        let mut old = std::collections::BTreeSet::new();
        for q in &queries {
            let hits: Vec<_> = pins
                .iter()
                .filter(|p| {
                    let b = bounds(p);
                    b[0] <= q[2] && b[2] >= q[0] && b[1] <= q[3] && b[3] >= q[1]
                })
                .collect();
            old.extend(hits.into_iter().map(|p| p.zone_id.clone()));
        }
        let old_time = start.elapsed();
        let start = std::time::Instant::now();
        let new = intersecting_many(queries).unwrap();
        let new_time = start.elapsed();
        assert_eq!(old, new.into_iter().map(|p| p.zone_id.clone()).collect());
        eprintln!("100000 segment region requests: old={old_time:?}, batched={new_time:?}");
    }
    #[test]
    #[ignore = "requires locally acquired water-only evidence; never redistributed"]
    fn external_registry_headers_and_all_row_runs() {
        let root = std::path::PathBuf::from(
            std::env::var("WATER_ZONE_EVIDENCE").expect("evidence directory"),
        );
        for pin in pins().unwrap() {
            let header = std::fs::read(root.join(format!("{}.json", pin.zone_id))).unwrap();
            pin.verify_header_spec(&header).unwrap();
            let raw = std::fs::read(root.join(format!("{}.water", pin.zone_id))).unwrap();
            super::super::WaterZone::decode(pin.clone(), &raw).unwrap();
        }
    }
    #[test]
    fn registry_validates_all_pins_and_projection() {
        let all = pins().unwrap();
        assert_eq!(all.len(), AUDITED_ZONE_COUNT);
        for pin in all {
            let b = bounds(pin);
            assert!(intersecting(b)
                .unwrap()
                .iter()
                .any(|p| p.zone_id == pin.zone_id));
            let q = [(b[0] + b[2]) / 2., (b[1] + b[3]) / 2.];
            assert!(intersecting([q[0], q[1], q[0], q[1]])
                .unwrap()
                .iter()
                .any(|p| p.zone_id == pin.zone_id));
        }
        assert!(intersecting([0., 0., 0., 0.]).unwrap().is_empty());
        assert!(intersecting([f64::NAN, 0., 0., 0.]).is_err());
        assert!(intersecting([1., 0., 0., 1.]).is_err());
    }
}
