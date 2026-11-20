//! Pinned construction masks. Valid decoded dry cells are game-model land; missing data is not.
use super::water_model::{bundled, validate_with_model, WaterField, UNKNOWN};
use super::*;
use sha2::{Digest, Sha256};
use std::sync::Arc;
const ROOT: &str = "https://raw.githubusercontent.com/Mei8n/rosenfu/108195ed2fe57b5ab8ea4f787a560a5fa30362b4/data/water-mask-trial-20261114/";
#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    lon: f64,
    lat: f64,
    x: f64,
    y: f64,
    width: usize,
    height: usize,
    bytes: usize,
    hash: &'static str,
}
const SPECS: [Spec; 3] = [
    Spec {
        key: "fuji",
        lon: 138.73000000000002,
        lat: 35.36,
        x: -1825.,
        y: -2225.,
        width: 146,
        height: 178,
        bytes: 716,
        hash: "9d07d0612ffec02a518f9613569f2b8756d54bfd1e576140d278df39eff347a9",
    },
    Spec {
        key: "kanmon",
        lon: 130.95,
        lat: 33.96,
        x: -2775.,
        y: -3350.,
        width: 222,
        height: 268,
        bytes: 2620,
        hash: "906aaa77adb59cb7baef692646564c59498f5d26e4c0789f177ea1f8122c3b92",
    },
    Spec {
        key: "takamatsu",
        lon: 134.05,
        lat: 34.345,
        x: -2775.,
        y: -2800.,
        width: 222,
        height: 224,
        bytes: 3844,
        hash: "86f1c4500422aa7e845ac0fdd0d5ebb4f56da73fa307ed7552fb3ce1505bf341",
    },
];
#[derive(Serialize)]
pub struct WaterMaskRequest {
    pub id: &'static str,
    pub url: String,
    pub bytes: usize,
    pub sha256: &'static str,
}
pub fn water_mask_requests() -> Vec<WaterMaskRequest> {
    SPECS
        .iter()
        .map(|s| WaterMaskRequest {
            id: s.key,
            url: format!("{ROOT}{}/water.bin", s.key),
            bytes: s.bytes,
            sha256: s.hash,
        })
        .collect()
}
impl Spec {
    fn scale(self) -> (f64, f64) {
        let ky = 6371008.8 * std::f64::consts::PI / 180.;
        (ky * self.lat.to_radians().cos(), ky)
    }
    fn bounds(self) -> [f64; 4] {
        let (k, ky) = self.scale();
        [
            self.lon + self.x / k,
            self.lat + self.y / ky,
            self.lon + (self.x + self.width as f64 * 25.) / k,
            self.lat + (self.y + self.height as f64 * 25.) / ky,
        ]
    }
    fn cell(self, p: GeoPoint) -> Option<(usize, usize)> {
        let [k, ky] = [self.scale().0, self.scale().1];
        let x = ((p.longitude - self.lon) * k - self.x) / 25.;
        let y = ((p.latitude - self.lat) * ky - self.y) / 25.;
        if x >= 0. && y >= 0. && x < self.width as f64 && y < self.height as f64 {
            Some((x.floor() as usize, y.floor() as usize))
        } else {
            None
        }
    }
}
#[derive(Clone)]
struct Fine {
    rows: Vec<usize>,
    edges: Vec<usize>,
}
impl Fine {
    fn decode(s: Spec, bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != s.bytes || format!("{:x}", Sha256::digest(bytes)) != s.hash {
            return Err("水域データの版または内容が一致しません。".into());
        }
        let row_bytes = (s.height + 1) * 4;
        if row_bytes > bytes.len() || !(bytes.len() - row_bytes).is_multiple_of(2) {
            return Err(UNKNOWN.into());
        }
        let rows: Vec<_> = bytes[..row_bytes]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b) as usize)
            .collect();
        let edges: Vec<_> = bytes[row_bytes..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes(*b) as usize)
            .collect();
        if rows.first() != Some(&0) || rows.last() != Some(&edges.len()) {
            return Err(UNKNOWN.into());
        }
        for pair in rows.windows(2) {
            if pair[0] > pair[1] || pair[1] > edges.len() || (pair[1] - pair[0]) % 2 != 0 {
                return Err(UNKNOWN.into());
            }
            let row = &edges[pair[0]..pair[1]];
            if row.iter().any(|v| *v > s.width) || row.windows(2).any(|p| p[0] >= p[1]) {
                return Err(UNKNOWN.into());
            }
        }
        Ok(Self { rows, edges })
    }
    fn water(&self, col: usize, row: usize) -> bool {
        self.edges[self.rows[row]..self.rows[row + 1]].partition_point(|v| *v <= col) % 2 == 1
    }
}
/// Per-session immutable fixed-version cache. Failed installs never replace good evidence.
#[derive(Clone, Default)]
pub struct ConstructionWaterModel {
    fine: [Option<Arc<Fine>>; 3],
    national: Option<super::water_national::NationalWaterModel>,
}
impl ConstructionWaterModel {
    pub fn enable_national(&mut self) -> Result<(), String> {
        self.national = Some(super::water_national::NationalWaterModel::new()?);
        Ok(())
    }
    pub fn national_enabled(&self) -> bool {
        self.national.is_some()
    }
    pub fn missing_zones(&self, bounds: [f64; 4]) -> Result<Vec<String>, String> {
        self.national.as_ref().ok_or(UNKNOWN)?.missing(bounds)
    }
    /// Batch independent regions without repeated per-segment allocations.
    pub fn missing_zones_many(
        &self,
        bounds: impl IntoIterator<Item = [f64; 4]>,
    ) -> Result<Vec<String>, String> {
        self.national.as_ref().ok_or(UNKNOWN)?.missing_many(bounds)
    }
    pub fn install_zone(&mut self, zone: Arc<crate::water_zone::WaterZone>) -> Result<(), String> {
        self.national.as_mut().ok_or(UNKNOWN)?.install(zone)
    }

    pub fn missing_requests(&self) -> Vec<WaterMaskRequest> {
        water_mask_requests()
            .into_iter()
            .enumerate()
            .filter(|(i, _)| self.fine[*i].is_none())
            .map(|(_, r)| r)
            .collect()
    }
    pub fn install(&mut self, id: &str, bytes: &[u8]) -> Result<(), String> {
        let index = SPECS
            .iter()
            .position(|s| s.key == id)
            .ok_or("未登録の水域データです。")?;
        let mask = Fine::decode(SPECS[index], bytes)?;
        self.fine[index] = Some(Arc::new(mask));
        Ok(())
    }
    pub fn validate(&self, before: &WorldDraft, after: &WorldDraft) -> Result<(), String> {
        validate_with_model(before, after, self)
    }
}
impl WaterField for ConstructionWaterModel {
    fn water(&self, p: GeoPoint) -> Result<bool, String> {
        if let Some(model) = &self.national {
            return model.water(p);
        }
        // Scope remains authoritative even if a future fine rectangle extends beyond it.
        let coarse = bundled()?.water(p)?;
        for (i, s) in SPECS.iter().enumerate() {
            if let Some((x, y)) = s.cell(p) {
                let fine = self.fine[i]
                    .as_ref()
                    .ok_or("水域の細密データを取得できていないため着工できません。")?;
                // A successful fixed mask defines the construction game model, not
                // surveyed completeness. Missing/invalid installs never reach here.
                return Ok(fine.water(x, y));
            }
        }
        // Complete compiled provider listing: no fine applies here, so coarse is used.
        Ok(coarse)
    }
    fn cuts(&self, a: GeoPoint, b: GeoPoint) -> Result<Vec<f64>, String> {
        if let Some(model) = &self.national {
            return model.cuts(a, b);
        }
        let mut cuts = bundled()?.cuts(a, b)?;
        for s in SPECS {
            let [w, south, e, n] = s.bounds();
            if a.longitude.max(b.longitude) < w
                || a.longitude.min(b.longitude) > e
                || a.latitude.max(b.latitude) < south
                || a.latitude.min(b.latitude) > n
            {
                continue;
            }
            for (from, to, low, high, count) in [
                (a.longitude, b.longitude, w, e, s.width),
                (a.latitude, b.latitude, south, n, s.height),
            ] {
                if from == to {
                    continue;
                }
                for i in 0..=count {
                    let edge = low + (high - low) * i as f64 / count as f64;
                    let t = (edge - from) / (to - from);
                    if t > 0. && t < 1. {
                        cuts.push(t);
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
    const KANMON: &[u8] =
        include_bytes!("../../../../data/water-mask-trial-20261114/kanmon/water.bin");
    fn point() -> GeoPoint {
        GeoPoint {
            longitude: 130.92709487964845,
            latitude: 33.930210012951626,
        }
    }
    #[test]
    fn fine_water_overrides_coarse_land_and_failed_install_does_not_erase_it() {
        let mut m = ConstructionWaterModel::default();
        let p = point();
        assert!(!bundled().unwrap().water(p).unwrap());
        assert!(m.water(p).is_err());
        assert!(m.install("kanmon", &KANMON[..KANMON.len() - 1]).is_err());
        assert_eq!(m.missing_requests().len(), 3);
        m.install("kanmon", KANMON).unwrap();
        assert!(m.water(p).unwrap());
        assert_eq!(m.missing_requests().len(), 2);
        let mut corrupt = KANMON.to_vec();
        corrupt[0] ^= 1;
        assert!(m.install("kanmon", &corrupt).is_err());
        assert!(m.water(p).unwrap());
        assert!(m.install("unknown", KANMON).is_err());
    }
    #[test]
    fn acquired_fine_dry_is_model_land_while_normal_nonprovision_uses_coarse() {
        let mut m = ConstructionWaterModel::default();
        m.install(
            "fuji",
            include_bytes!("../../../../data/water-mask-trial-20261114/fuji/water.bin"),
        )
        .unwrap();
        assert!(!m
            .water(GeoPoint {
                longitude: 138.73,
                latitude: 35.36
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
                longitude: 136.1,
                latitude: 35.25
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
    fn acquired_fine_dry_overrides_coarse_water_without_claiming_survey_completeness() {
        let mut m = ConstructionWaterModel::default();
        m.install("kanmon", KANMON).unwrap();
        let s = SPECS[1];
        let (kx, ky) = s.scale();
        let mut checked = 0;
        for row in 0..s.height {
            for col in 0..s.width {
                let p = GeoPoint {
                    longitude: s.lon + (s.x + (col as f64 + 0.5) * 25.) / kx,
                    latitude: s.lat + (s.y + (row as f64 + 0.5) * 25.) / ky,
                };
                if bundled().unwrap().water(p).unwrap()
                    && !m.fine[1].as_ref().unwrap().water(col, row)
                {
                    assert!(!m.water(p).unwrap());
                    checked += 1;
                }
            }
        }
        assert!(
            checked > 0,
            "real fixed mask must exercise fine-dry/coarse-wet precedence"
        );
    }
    #[test]
    fn fine_cell_boundaries_are_included_even_with_endpoints_outside_fine() {
        let m = ConstructionWaterModel::default();
        let a = GeoPoint {
            longitude: 130.9,
            latitude: 33.96,
        };
        let b = GeoPoint {
            longitude: 131.,
            latitude: 33.96,
        };
        assert!(m.cuts(a, b).unwrap().len() > SPECS[1].width);
    }
    #[test]
    fn native_pins_and_cells_match_published_manifests() {
        for (index, (manifest, bytes)) in [
            (
                include_str!("../../../../data/water-mask-trial-20261114/fuji/manifest.json"),
                include_bytes!("../../../../data/water-mask-trial-20261114/fuji/water.bin")
                    .as_slice(),
            ),
            (
                include_str!("../../../../data/water-mask-trial-20261114/kanmon/manifest.json"),
                KANMON,
            ),
            (
                include_str!("../../../../data/water-mask-trial-20261114/takamatsu/manifest.json"),
                include_bytes!("../../../../data/water-mask-trial-20261114/takamatsu/water.bin")
                    .as_slice(),
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let value: serde_json::Value = serde_json::from_str(manifest).unwrap();
            let s = SPECS[index];
            let fine = Fine::decode(s, bytes).unwrap();
            assert_eq!(value["payload"]["sha256"], s.hash);
            assert_eq!(value["spec"]["width"], s.width);
            assert_eq!(value["spec"]["height"], s.height);
            assert_eq!(value["spec"]["originLongitude"].as_f64().unwrap(), s.lon);
            assert_eq!(value["spec"]["originLatitude"].as_f64().unwrap(), s.lat);
            assert_eq!(value["spec"]["x0"].as_f64().unwrap(), s.x);
            assert_eq!(value["spec"]["y0"].as_f64().unwrap(), s.y);
            assert_eq!(value["spec"]["cellM"], 25.0);
            assert_eq!(value["spec"]["coverage"], "observations_only");
            let count: usize = (0..s.height)
                .map(|row| (0..s.width).filter(|col| fine.water(*col, row)).count())
                .sum();
            assert_eq!(value["water_cells"], count);
        }
    }
}
