//! Production-only fixed registry model. Cache absence is never land.
use super::water_model::{bundled, WaterField, UNKNOWN};
use super::*;
use crate::water_zone::{registry, Cell, WaterZone, ZonePin};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone)]
pub(super) struct NationalWaterModel {
    pins: Arc<Vec<ZonePin>>,
    zones: BTreeMap<String, Arc<WaterZone>>,
}
fn bounds(p: &ZonePin) -> [f64; 4] {
    registry::bounds(p)
}
fn contains(p: &ZonePin, q: GeoPoint) -> bool {
    let [w, s, e, n] = bounds(p);
    q.longitude >= w && q.longitude < e && q.latitude >= s && q.latitude < n
}
impl NationalWaterModel {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            pins: Arc::new(registry::pins()?.to_vec()),
            zones: BTreeMap::new(),
        })
    }
    pub fn missing(&self, area: [f64; 4]) -> Result<Vec<String>, String> {
        Ok(registry::intersecting(area)?
            .into_iter()
            .filter(|p| !self.zones.contains_key(&p.zone_id))
            .map(|p| p.zone_id.clone())
            .collect())
    }
    pub fn install(&mut self, zone: Arc<WaterZone>) -> Result<(), String> {
        let trusted = self
            .pins
            .iter()
            .find(|p| p.zone_id == zone.pin().zone_id)
            .ok_or(UNKNOWN)?;
        if trusted != zone.pin() {
            return Err(UNKNOWN.into());
        }
        let total: usize = self
            .zones
            .values()
            .filter(|z| z.pin().zone_id != zone.pin().zone_id)
            .map(|z| z.pin().payload_length().unwrap_or(usize::MAX))
            .try_fold(0usize, |a, b| a.checked_add(b))
            .ok_or(UNKNOWN)?;
        if total
            .checked_add(zone.pin().payload_length().map_err(|_| UNKNOWN)?)
            .is_none_or(|n| n > 64 * 1024 * 1024)
        {
            return Err("水域キャッシュの上限を超えています。".into());
        }
        self.zones.insert(zone.pin().zone_id.clone(), zone);
        Ok(())
    }
}
impl WaterField for NationalWaterModel {
    fn water(&self, p: GeoPoint) -> Result<bool, String> {
        let coarse = bundled()?.water(p)?;
        let mut matched = false;
        let mut wet = false;
        for pin in self.pins.iter().filter(|pin| contains(pin, p)) {
            matched = true;
            let zone = self
                .zones
                .get(&pin.zone_id)
                .ok_or("水域データを取得中、または取得できていないため着工できません。")?;
            match zone.at(p.longitude, p.latitude).map_err(|_| UNKNOWN)? {
                Cell::ModelWater => wet = true,
                Cell::ModelDry => {}
                Cell::OutsideGrid => return Err(UNKNOWN.into()),
            }
        }
        Ok(if matched { wet } else { coarse })
    }
    fn cuts(&self, a: GeoPoint, b: GeoPoint) -> Result<Vec<f64>, String> {
        let mut cuts = bundled()?.cuts(a, b)?;
        for pin in self.pins.iter().filter(|pin| {
            let [w, s, e, n] = bounds(pin);
            w <= a.longitude.max(b.longitude)
                && e >= a.longitude.min(b.longitude)
                && s <= a.latitude.max(b.latitude)
                && n >= a.latitude.min(b.latitude)
        }) {
            let [w, s, e, n] = bounds(pin);
            for (from, to, lo, hi, count) in [
                (a.longitude, b.longitude, w, e, pin.width),
                (a.latitude, b.latitude, s, n, pin.height),
            ] {
                if from == to {
                    continue;
                }
                let step = (hi - lo) / f64::from(count);
                let start = ((from.min(to) - lo) / step).ceil().max(0.) as u32;
                let end = ((from.max(to) - lo) / step).floor().min(f64::from(count)) as u32;
                if start > end {
                    continue;
                }
                if cuts.len().saturating_add((end - start) as usize + 1) > 100_000 {
                    return Err("水域の境界が多すぎるため区間を分けてください。".into());
                }
                for i in start..=end {
                    let t = (lo + f64::from(i) * step - from) / (to - from);
                    if t > 0. && t < 1. {
                        cuts.push(t)
                    }
                }
            }
        }
        Ok(cuts)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    fn fixture(id: &str, wet: bool) -> Arc<WaterZone> {
        let rows: Vec<u32> = if wet { vec![0, 2, 4] } else { vec![0, 0, 0] };
        let mut raw = rows
            .iter()
            .flat_map(|n| n.to_le_bytes())
            .collect::<Vec<_>>();
        if wet {
            for n in [0u16, 2, 0, 2] {
                raw.extend(n.to_le_bytes())
            }
        }
        let pin = ZonePin {
            provider_revision: "2026-11-13".into(),
            zone_id: id.into(),
            rule: crate::water_zone::RULE.into(),
            origin_longitude: 139.75,
            origin_latitude: 35.67,
            x0: -100.,
            y0: -100.,
            cell_m: 100.,
            width: 2,
            height: 2,
            run_count: if wet { 4 } else { 0 },
            run_padding: 0,
            range_start: 0,
            container_length: raw.len() as u64,
            header_sha256: "0".repeat(64),
            payload_sha256: format!("{:x}", Sha256::digest(&raw)),
        };
        Arc::new(WaterZone::decode(pin, &raw).unwrap())
    }
    #[test]
    fn overlapping_fine_requires_all_data_and_uses_wet_priority() {
        let dry = fixture("dry", false);
        let wet = fixture("wet", true);
        let p = GeoPoint {
            longitude: 139.75,
            latitude: 35.67,
        };
        let mut m = NationalWaterModel {
            pins: Arc::new(vec![dry.pin().clone(), wet.pin().clone()]),
            zones: BTreeMap::new(),
        };
        assert!(m.water(p).is_err());
        m.install(dry.clone()).unwrap();
        assert!(m.water(p).is_err());
        m.install(wet).unwrap();
        assert!(m.water(p).unwrap());
        assert!(m.install(fixture("foreign", false)).is_err());
        assert!(m.water(p).unwrap());
    }
    #[test]
    fn acquired_fine_dry_is_model_dry_and_outside_registry_uses_coarse() {
        let dry = fixture("dry", false);
        let mut m = NationalWaterModel {
            pins: Arc::new(vec![dry.pin().clone()]),
            zones: BTreeMap::new(),
        };
        m.install(dry).unwrap();
        assert!(!m
            .water(GeoPoint {
                longitude: 139.75,
                latitude: 35.67
            })
            .unwrap());
        assert!(!m
            .water(GeoPoint {
                longitude: 139.7671,
                latitude: 35.6812
            })
            .unwrap());
        assert!(m
            .water(GeoPoint {
                longitude: 0.,
                latitude: 0.
            })
            .is_err());
    }
    #[test]
    fn fine_grid_boundaries_are_checked_even_when_endpoints_are_outside() {
        let dry = fixture("dry", false);
        let m = NationalWaterModel {
            pins: Arc::new(vec![dry.pin().clone()]),
            zones: BTreeMap::new(),
        };
        let a = GeoPoint {
            longitude: 139.748,
            latitude: 35.67,
        };
        let b = GeoPoint {
            longitude: 139.752,
            latitude: 35.67,
        };
        let cuts = m.cuts(a, b).unwrap();
        assert!(cuts.iter().any(|t| (*t - 0.5).abs() < 1e-8));
        assert!(m
            .water(GeoPoint {
                longitude: 139.75,
                latitude: 35.67
            })
            .is_err());
        assert!(m.water(a).is_ok());
    }
}
