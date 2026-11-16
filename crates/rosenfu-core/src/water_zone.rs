//! Independent, offline reader for pinned water-only row runs. No mixed pack is retained.
//! Successful dry cells describe an adopted game model, not surveyed completeness.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const RULE: &str = "water-zone-row-runs-v1";
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_HEADER_BYTES: usize = 1024 * 1024;
const R: f64 = 6_371_008.8;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ZonePin {
    pub provider_revision: String,
    pub zone_id: String,
    pub rule: String,
    pub origin_longitude: f64,
    pub origin_latitude: f64,
    pub x0: f64,
    pub y0: f64,
    pub cell_m: f64,
    pub width: u32,
    pub height: u32,
    pub run_count: u32,
    pub range_start: u64,
    /// Zero-filled bytes between row offsets and runs, included in the payload hash.
    #[serde(default)]
    pub run_padding: u8,
    pub container_length: u64,
    pub header_sha256: String,
    pub payload_sha256: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidPin,
    Limit,
    Hash,
    Range,
    Decode,
    Coordinate,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    ModelWater,
    ModelDry,
    OutsideGrid,
}
#[derive(Debug, Clone, PartialEq)]
pub struct WaterZone {
    pin: ZonePin,
    rows: Vec<u32>,
    runs: Vec<u16>,
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl ZonePin {
    pub fn payload_length(&self) -> Result<usize, Error> {
        let bytes = (u64::from(self.height) + 1)
            .checked_mul(4)
            .and_then(|n| n.checked_add(u64::from(self.run_count) * 2))
            .and_then(|n| n.checked_add(u64::from(self.run_padding)))
            .ok_or(Error::Limit)?;
        if bytes > MAX_BYTES as u64 {
            return Err(Error::Limit);
        }
        Ok(bytes as usize)
    }
    pub fn validate(&self) -> Result<(), Error> {
        let r = self.provider_revision.as_bytes();
        if r.len() != 10
            || r[4] != b'-'
            || r[7] != b'-'
            || r.iter()
                .enumerate()
                .any(|(i, b)| i != 4 && i != 7 && !b.is_ascii_digit())
            || self.zone_id.is_empty()
            || self.zone_id.len() > 32
            || !self
                .zone_id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || self.rule != RULE
            || !hash(&self.header_sha256)
            || !hash(&self.payload_sha256)
            || self.width == 0
            || self.width > 65535
            || self.height == 0
            || self.height > 65535
            || !self.run_count.is_multiple_of(2)
            || self.run_padding > 7
            || ![
                self.origin_longitude,
                self.origin_latitude,
                self.x0,
                self.y0,
                self.cell_m,
            ]
            .iter()
            .all(|n| n.is_finite())
            || self.cell_m <= 0.
            || self.cell_m > 1000.
            || self.origin_longitude.abs() > 180.
            || self.origin_latitude.abs() > 85.
        {
            return Err(Error::InvalidPin);
        }
        let length = self.payload_length()? as u64;
        if self
            .range_start
            .checked_add(length)
            .is_none_or(|end| end > self.container_length)
        {
            return Err(Error::Range);
        }
        let (kx, ky) = self.scale();
        for (x, y) in [
            (self.x0, self.y0),
            (
                self.x0 + f64::from(self.width) * self.cell_m,
                self.y0 + f64::from(self.height) * self.cell_m,
            ),
        ] {
            if (self.origin_longitude + x / kx).abs() > 180.
                || (self.origin_latitude + y / ky).abs() > 85.
            {
                return Err(Error::InvalidPin);
            }
        }
        Ok(())
    }
    fn scale(&self) -> (f64, f64) {
        let ky = R * std::f64::consts::PI / 180.;
        (ky * self.origin_latitude.to_radians().cos(), ky)
    }
    /// Only fixed-version provider URLs can be formed. The caller must additionally
    /// select this pin from its audited registry, never accept arbitrary IPC pins.
    pub fn urls(&self) -> Result<(String, String), Error> {
        self.validate()?;
        let root = format!(
            "https://data.anita.garden/trainworld/{}/{}",
            self.provider_revision, self.zone_id
        );
        Ok((format!("{root}.json"), format!("{root}.bin")))
    }
    /// Integrity only: this does not parse or authenticate the provider's header,
    /// nor prove the caller's specification was extracted correctly from it.
    pub fn verify_header(&self, bytes: &[u8]) -> Result<(), Error> {
        self.validate()?;
        if bytes.is_empty() || bytes.len() > MAX_HEADER_BYTES {
            return Err(Error::Limit);
        }
        if digest(bytes) != self.header_sha256 {
            return Err(Error::Hash);
        }
        Ok(())
    }
}
impl WaterZone {
    /// Complete exact range response required; a server ignoring Range is never
    /// interpreted as an empty mask or a successful absence of water.
    pub fn from_range(
        pin: ZonePin,
        status: u16,
        content_range: &str,
        bytes: &[u8],
    ) -> Result<Self, Error> {
        pin.validate()?;
        let len = pin.payload_length()?;
        let end = pin
            .range_start
            .checked_add(len as u64)
            .and_then(|n| n.checked_sub(1))
            .ok_or(Error::Range)?;
        if status != 206
            || content_range != format!("bytes {}-{end}/{}", pin.range_start, pin.container_length)
        {
            return Err(Error::Range);
        }
        Self::decode(pin, bytes)
    }
    /// Offline water-only payload, checked before any allocation. No dense raster.
    pub fn decode(pin: ZonePin, bytes: &[u8]) -> Result<Self, Error> {
        pin.validate()?;
        if bytes.len() != pin.payload_length()? {
            return Err(Error::Limit);
        }
        if digest(bytes) != pin.payload_sha256 {
            return Err(Error::Hash);
        }
        let split = (pin.height as usize + 1) * 4;
        let rows: Vec<u32> = bytes[..split]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
            .collect();
        let run_start = split + usize::from(pin.run_padding);
        if bytes[split..run_start].iter().any(|b| *b != 0) {
            return Err(Error::Decode);
        }
        let runs: Vec<u16> = bytes[run_start..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes(*b))
            .collect();
        if rows.first() != Some(&0) || rows.last() != Some(&pin.run_count) {
            return Err(Error::Decode);
        }
        for pair in rows.windows(2) {
            if pair[0] > pair[1] || pair[1] > pin.run_count || pair[0] % 2 != 0 || pair[1] % 2 != 0
            {
                return Err(Error::Decode);
            }
            let mut end = 0;
            for run in runs[pair[0] as usize..pair[1] as usize].as_chunks::<2>().0 {
                if run[0] < end || run[0] >= run[1] || u32::from(run[1]) > pin.width {
                    return Err(Error::Decode);
                }
                end = run[1];
            }
        }
        Ok(Self { pin, rows, runs })
    }
    pub fn pin(&self) -> &ZonePin {
        &self.pin
    }
    pub fn at(&self, longitude: f64, latitude: f64) -> Result<Cell, Error> {
        if !longitude.is_finite()
            || !latitude.is_finite()
            || longitude.abs() > 180.
            || latitude.abs() > 85.
        {
            return Err(Error::Coordinate);
        }
        let (kx, ky) = self.pin.scale();
        let x = ((longitude - self.pin.origin_longitude) * kx - self.pin.x0) / self.pin.cell_m;
        let y = ((latitude - self.pin.origin_latitude) * ky - self.pin.y0) / self.pin.cell_m;
        if x < 0. || y < 0. || x >= f64::from(self.pin.width) || y >= f64::from(self.pin.height) {
            return Ok(Cell::OutsideGrid);
        }
        let row = y.floor() as usize;
        let col = x.floor() as u16;
        let edges = &self.runs[self.rows[row] as usize..self.rows[row + 1] as usize];
        Ok(if edges.partition_point(|v| *v <= col) % 2 == 1 {
            Cell::ModelWater
        } else {
            Cell::ModelDry
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (ZonePin, Vec<u8>) {
        let bytes: Vec<u8> = [0u32, 2, 4]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .chain([1u16, 3, 0, 1].into_iter().flat_map(u16::to_le_bytes))
            .collect();
        (
            ZonePin {
                provider_revision: "2026-11-13".into(),
                zone_id: "synthetic".into(),
                rule: RULE.into(),
                origin_longitude: 139.,
                origin_latitude: 35.,
                x0: 0.,
                y0: 0.,
                cell_m: 25.,
                width: 4,
                height: 2,
                run_count: 4,
                range_start: 100,
                run_padding: 0,
                container_length: 200,
                header_sha256: digest(b"header"),
                payload_sha256: digest(&bytes),
            },
            bytes,
        )
    }
    #[test]
    fn explicit_padding_is_hashed_bounded_and_zero() {
        let (mut pin, mut bytes) = fixture();
        bytes.splice(12..12, [0; 4]);
        pin.run_padding = 4;
        pin.payload_sha256 = digest(&bytes);
        assert_eq!(pin.payload_length().unwrap(), 24);
        WaterZone::decode(pin.clone(), &bytes).unwrap();
        bytes[12] = 1;
        assert_eq!(WaterZone::decode(pin.clone(), &bytes), Err(Error::Hash));
        pin.payload_sha256 = digest(&bytes);
        assert_eq!(WaterZone::decode(pin.clone(), &bytes), Err(Error::Decode));
        pin.run_padding = 8;
        assert_eq!(pin.validate(), Err(Error::InvalidPin));
    }
    #[test]
    fn half_open_cells_and_fixed_binding() {
        let (pin, bytes) = fixture();
        let (kx, ky) = pin.scale();
        pin.verify_header(b"header").unwrap();
        assert_eq!(pin.verify_header(b"changed"), Err(Error::Hash));
        let z = WaterZone::from_range(pin, 206, "bytes 100-119/200", &bytes).unwrap();
        for (x, y, expected) in [
            (0.5, 0.5, Cell::ModelDry),
            (1.5, 0.5, Cell::ModelWater),
            (3.5, 0.5, Cell::ModelDry),
            (0.5, 1.5, Cell::ModelWater),
            (4.5, 0.5, Cell::OutsideGrid),
            (-0.5, 0.5, Cell::OutsideGrid),
        ] {
            assert_eq!(
                z.at(139. + x * 25. / kx, 35. + y * 25. / ky).unwrap(),
                expected
            );
        }
        assert_eq!(z.at(f64::NAN, 35.), Err(Error::Coordinate));
        assert_eq!(z.pin().rule, RULE);
    }
    #[test]
    fn ignored_range_wrong_total_truncation_and_hash_reject() {
        let (pin, bytes) = fixture();
        for (status, range) in [
            (200, "bytes 100-119/200"),
            (206, "bytes 100-119/201"),
            (206, "bytes 101-120/200"),
            (206, "bytes 100-119/*"),
        ] {
            assert_eq!(
                WaterZone::from_range(pin.clone(), status, range, &bytes),
                Err(Error::Range)
            );
        }
        assert_eq!(
            WaterZone::decode(pin.clone(), &bytes[..19]),
            Err(Error::Limit)
        );
        let mut corrupt = bytes.clone();
        corrupt[0] = 1;
        assert_eq!(WaterZone::decode(pin, &corrupt), Err(Error::Hash));
    }
    #[test]
    fn correctly_hashed_malformed_rows_and_runs_reject() {
        for (offset, value) in [(0, 2), (4, 3), (8, 2), (12, 4), (14, 0), (16, 4)] {
            let (mut pin, mut bytes) = fixture();
            bytes[offset] = value;
            pin.payload_sha256 = digest(&bytes);
            assert_eq!(WaterZone::decode(pin, &bytes), Err(Error::Decode));
        }
    }
    #[test]
    fn unsafe_pins_and_budget_reject_before_payload_read() {
        let (pin, bytes) = fixture();
        for i in 0..8 {
            let mut p = pin.clone();
            match i {
                0 => p.zone_id = "../tyo".into(),
                1 => p.provider_revision = "latest".into(),
                2 => p.width = 65536,
                3 => p.cell_m = f64::INFINITY,
                4 => p.range_start = u64::MAX,
                5 => p.run_count = u32::MAX - 1,
                6 => p.rule = "other".into(),
                _ => p.origin_latitude = 90.,
            }
            assert!(WaterZone::decode(p, &bytes).is_err());
        }
    }
    #[test]
    fn large_grid_stays_sparse_and_empty_is_valid_only_after_hash() {
        let (mut p, _) = fixture();
        p.width = 8002;
        p.height = 9261;
        p.run_count = 0;
        p.cell_m = 25.;
        let bytes = vec![0; (p.height as usize + 1) * 4];
        p.payload_sha256 = digest(&bytes);
        p.container_length = p.range_start + bytes.len() as u64;
        let z = WaterZone::decode(p, &bytes).unwrap();
        assert!(z.runs.is_empty());
        assert_eq!(z.rows.len(), 9262);
        assert_eq!(z.at(139., 35.).unwrap(), Cell::ModelDry);
    }
}

pub mod acquisition;
mod header;

pub mod registry;
