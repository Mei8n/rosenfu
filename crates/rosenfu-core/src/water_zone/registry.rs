//! Audited fixed-version water ranges, not a claim of complete Japanese coverage.
use super::ZonePin;
use std::sync::OnceLock;

/// All verified pins. Hashes record observed bytes, not an author signature.
pub fn pins() -> Result<&'static [ZonePin], String> {
    static PINS: OnceLock<Result<Vec<ZonePin>, String>> = OnceLock::new();
    PINS.get_or_init(|| {
        let pins: Vec<ZonePin> = serde_json::from_str(include_str!("registry.json"))
            .map_err(|e| format!("water registry: {e}"))?;
        let mut ids = std::collections::BTreeSet::new();
        if pins.len() != 45 {
            return Err("water registry must contain 45 audited zones".into());
        }
        for pin in &pins {
            pin.validate()
                .map_err(|e| format!("water registry {}: {e:?}", pin.zone_id))?;
            if pin.provider_revision != "2026-11-13" || !ids.insert(&pin.zone_id) {
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
    Ok(pins()?
        .iter()
        .filter(|pin| {
            let b = bounds(pin);
            b[0] <= query[2] && b[2] >= query[0] && b[1] <= query[3] && b[3] >= query[1]
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(all.len(), 45);
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
