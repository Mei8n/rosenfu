//! Native transport boundary, not an HTTP client or IPC command. Caller selects
//! an audited pin; never construct this from arbitrary untrusted UI pins.
use super::*;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
const DEADLINE: Duration = Duration::from_secs(30);
#[derive(Debug, Clone)]
pub struct Request {
    pub token: u64,
    pub url: String,
    pub range: String,
    pub expected_bytes: usize,
}
struct Pending {
    token: u64,
    deadline: Instant,
    bytes: Vec<u8>,
}
pub struct Acquisition {
    pin: ZonePin,
    serial: u64,
    pending: Option<Pending>,
    cached: Option<Arc<WaterZone>>,
}
impl Acquisition {
    pub fn new(pin: ZonePin) -> Result<Self, Error> {
        pin.validate()?;
        Ok(Self {
            pin,
            serial: 0,
            pending: None,
            cached: None,
        })
    }
    pub fn cached(&self) -> Option<Arc<WaterZone>> {
        self.cached.clone()
    }
    /// Full header must already be obtained with a separate bounded request.
    /// Replacing an in-flight attempt invalidates its token, not the good cache.
    pub fn begin(&mut self, header: &[u8], now: Instant) -> Result<Request, Error> {
        self.pin.verify_header_spec(header)?;
        let serial = self.serial.checked_add(1).ok_or(Error::Limit)?;
        let deadline = now.checked_add(DEADLINE).ok_or(Error::Limit)?;
        let len = self.pin.payload_length()?;
        let (_, url) = self.pin.urls()?;
        let end = self.pin.range_start + len as u64 - 1;
        self.serial = serial;
        self.pending = Some(Pending {
            token: serial,
            deadline,
            bytes: Vec::new(),
        });
        Ok(Request {
            token: serial,
            url,
            range: format!("bytes={}-{end}", self.pin.range_start),
            expected_bytes: len,
        })
    }
    pub fn cancel(&mut self, token: u64) {
        if self.pending.as_ref().is_some_and(|p| p.token == token) {
            self.pending = None;
        }
    }
    pub fn push(&mut self, token: u64, chunk: &[u8], now: Instant) -> Result<(), Error> {
        let p = self
            .pending
            .as_mut()
            .filter(|p| p.token == token)
            .ok_or(Error::Range)?;
        if now >= p.deadline
            || p.bytes
                .len()
                .checked_add(chunk.len())
                .is_none_or(|n| n > self.pin.payload_length().unwrap_or(0))
        {
            self.pending = None;
            return Err(Error::Limit);
        }
        p.bytes.extend_from_slice(chunk);
        Ok(())
    }
    /// Transport must reject redirects before reading, send only the request URL,
    /// propagate cancellation to socket I/O, and call this with actual headers.
    pub fn finish(
        &mut self,
        token: u64,
        status: u16,
        range: &str,
        now: Instant,
    ) -> Result<Arc<WaterZone>, Error> {
        if !self.pending.as_ref().is_some_and(|p| p.token == token) {
            return Err(Error::Range);
        }
        let p = self.pending.take().unwrap();
        if now >= p.deadline {
            return Err(Error::Limit);
        }
        let result = Arc::new(WaterZone::from_range(
            self.pin.clone(),
            status,
            range,
            &p.bytes,
        )?);
        self.cached = Some(result.clone());
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (ZonePin, Vec<u8>, Vec<u8>) {
        let header=br#"{"origin":{"lon":139.0,"lat":35.0},"water":{"cell_m":25.0,"origin_m":[0,0],"size":[4,1]},"arrays":[{"name":"water_row","dtype":"u32","per":"water","offset":100,"count":2},{"name":"water_x","dtype":"u16","per":"water","offset":108,"count":2}]}"#.to_vec();
        let bytes: Vec<u8> = [0u32, 2]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .chain([1u16, 3].into_iter().flat_map(u16::to_le_bytes))
            .collect();
        let pin = ZonePin {
            provider_revision: "2026-11-13".into(),
            zone_id: "synthetic".into(),
            rule: RULE.into(),
            origin_longitude: 139.,
            origin_latitude: 35.,
            x0: 0.,
            y0: 0.,
            cell_m: 25.,
            width: 4,
            height: 1,
            run_count: 2,
            range_start: 100,
            run_padding: 0,
            container_length: 200,
            header_sha256: digest(&header),
            payload_sha256: digest(&bytes),
        };
        (pin, header, bytes)
    }
    #[test]
    fn header_geometry_offsets_types_duplicates_and_hash_are_bound() {
        let (pin, header, _) = setup();
        pin.verify_header_spec(&header).unwrap();
        for i in 0..4 {
            let mut h: serde_json::Value = serde_json::from_slice(&header).unwrap();
            match i {
                0 => h["water"]["size"][0] = 5.into(),
                1 => h["arrays"][1]["offset"] = 109.into(),
                2 => h["arrays"][0]["dtype"] = "u16".into(),
                _ => {
                    let x = h["arrays"][0].clone();
                    h["arrays"].as_array_mut().unwrap().push(x);
                }
            }
            let bytes = serde_json::to_vec(&h).unwrap();
            let mut p = pin.clone();
            p.header_sha256 = digest(&bytes);
            assert!(p.verify_header_spec(&bytes).is_err());
        }
        let duplicate = String::from_utf8(header.clone()).unwrap().replacen(
            "\"lon\":139.0",
            "\"lon\":139.0,\"lon\":139.0",
            1,
        );
        let mut p = pin.clone();
        p.header_sha256 = digest(duplicate.as_bytes());
        assert_eq!(
            p.verify_header_spec(duplicate.as_bytes()),
            Err(Error::Decode)
        );
    }
    #[test]
    fn explicit_padding_must_match_header_offsets() {
        let (mut pin, header, _) = setup();
        let mut h: serde_json::Value = serde_json::from_slice(&header).unwrap();
        let arrays = h["arrays"].as_array_mut().unwrap();
        let run = arrays.iter_mut().find(|a| a["name"] == "water_x").unwrap();
        run["offset"] = serde_json::json!(run["offset"].as_u64().unwrap() + 4);
        let bytes = serde_json::to_vec(&h).unwrap();
        pin.header_sha256 = digest(&bytes);
        assert_eq!(pin.verify_header_spec(&bytes), Err(Error::InvalidPin));
        pin.run_padding = 4;
        pin.verify_header_spec(&bytes).unwrap();
    }
    #[test]
    fn ignored_fields_duplicate_keys_deep_nesting_and_noninteger_spec_reject() {
        let (pin, header, _) = setup();
        let text = String::from_utf8(header).unwrap();
        let cases = [
            text.replacen("{", "{\"unused\":{\"a\":1,\"a\":2},", 1),
            text.replacen("\"size\":[4,1]", "\"size\":[4.5,1]", 1),
            format!(
                "{{\"unused\":{}0{},{}",
                "[".repeat(200),
                "]".repeat(200),
                &text[1..]
            ),
        ];
        for text in cases {
            let mut p = pin.clone();
            p.header_sha256 = digest(text.as_bytes());
            assert!(p.verify_header_spec(text.as_bytes()).is_err());
        }
    }
    #[test]
    fn cancellation_replacement_timeout_and_failure_keep_prior_cache() {
        let (pin, header, bytes) = setup();
        let mut a = Acquisition::new(pin).unwrap();
        let now = Instant::now();
        let r = a.begin(&header, now).unwrap();
        a.push(r.token, &bytes[..4], now).unwrap();
        a.push(r.token, &bytes[4..], now).unwrap();
        let good = a.finish(r.token, 206, "bytes 100-111/200", now).unwrap();
        let old = a.begin(&header, now).unwrap();
        let new = a.begin(&header, now).unwrap();
        assert!(a.push(old.token, &bytes, now).is_err());
        a.cancel(old.token);
        a.push(new.token, &bytes, now).unwrap();
        assert!(a.finish(new.token, 200, "bytes 100-111/200", now).is_err());
        assert!(Arc::ptr_eq(&good, &a.cached().unwrap()));
        let r = a.begin(&header, now).unwrap();
        assert!(a.push(r.token, &bytes, now + DEADLINE).is_err());
        assert!(Arc::ptr_eq(&good, &a.cached().unwrap()));
        let r = a.begin(&header, now).unwrap();
        a.cancel(r.token);
        assert!(a.finish(r.token, 206, "bytes 100-111/200", now).is_err());
        let r = a.begin(&header, now).unwrap();
        assert!(a.push(r.token, &[0; 13], now).is_err());
        assert!(Arc::ptr_eq(&good, &a.cached().unwrap()));
    }
    #[test]
    #[ignore = "External audited Tokyo water-only bytes; no network or real payload in CI"]
    fn real_tokyo_header_and_water_only_payload() {
        let header = std::fs::read(std::env::var("TW_WATER_HEADER").unwrap()).unwrap();
        let bytes = std::fs::read(std::env::var("TW_WATER_PAYLOAD").unwrap()).unwrap();
        let p = ZonePin {
            provider_revision: "2026-11-13".into(),
            zone_id: "tyo".into(),
            rule: RULE.into(),
            origin_longitude: 139.7671,
            origin_latitude: 35.6812,
            x0: -97375.,
            y0: -88950.,
            cell_m: 25.,
            width: 8002,
            height: 9261,
            run_count: 203244,
            range_start: 2518712,
            run_padding: 0,
            container_length: 6696000,
            header_sha256: "9120fa6c925372d9e4dfca76b4e9ba3e65a6efe186ff62872baab6b56cba8afd"
                .into(),
            payload_sha256: "588d24c63b44ebe4cb004583b2c22cdc3691dc7fec101966adffb605f2fcb0b4"
                .into(),
        };
        p.verify_header_spec(&header).unwrap();
        let z = WaterZone::from_range(p, 206, "bytes 2518712-2962247/6696000", &bytes).unwrap();
        let wet: u64 = z.runs.chunks_exact(2).map(|p| u64::from(p[1] - p[0])).sum();
        let total = u64::from(z.pin.width) * u64::from(z.pin.height);
        assert!((wet as f64 / total as f64 - 0.2647).abs() < 0.00005);
        println!(
            "cells={total} wet={wet} runs={} bytes={}",
            z.runs.len(),
            bytes.len()
        );
    }
}
