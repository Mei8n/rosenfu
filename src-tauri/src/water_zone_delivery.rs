//! Blocking, water-only fixed-host delivery. Run on spawn_blocking, never the UI thread.
//! Caller supplies audited immutable pins at startup, never pins/URLs from IPC.
//! Cancellation rejects late results; a blocked socket is bounded by the remaining
//! 30-second request deadline, not synchronously interrupted by the cancel flag.
use rosenfu_core::water_zone::{acquisition::Acquisition, WaterZone, ZonePin};
use std::{
    collections::VecDeque,
    io::Read,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

const HEADER_LIMIT: usize = 1024 * 1024;
const CACHE_LIMIT: usize = 64 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(30);
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryError {
    UnknownZone,
    InvalidPin,
    Busy,
    Cancelled,
    Timeout,
    Transport,
    Response,
    Limit,
    Integrity,
}
#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    // Production delivery currently uses per-request deadlines; tests exercise
    // cancellation without exposing an unused control API in the desktop build.
    #[cfg(test)]
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    fn check(&self, deadline: Instant) -> Result<(), DeliveryError> {
        if self.0.load(Ordering::Acquire) {
            Err(DeliveryError::Cancelled)
        } else if Instant::now() >= deadline {
            Err(DeliveryError::Timeout)
        } else {
            Ok(())
        }
    }
}
pub struct TransportResponse {
    pub status: u16,
    pub content_range: Option<String>,
    pub content_length: Option<u64>,
    pub body: Box<dyn Read + Send>,
}
/// DI implementations must enforce the passed deadline on all socket I/O.
pub trait Transport: Send + Sync {
    fn get(
        &self,
        url: &str,
        range: Option<&str>,
        remaining: Duration,
    ) -> Result<TransportResponse, DeliveryError>;
}
pub struct HttpTransport(reqwest::blocking::Client);
impl HttpTransport {
    pub fn new() -> Result<Self, DeliveryError> {
        reqwest::blocking::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(DEADLINE)
            .user_agent("Rosenfu/0.1 water-only on-demand")
            .build()
            .map(Self)
            .map_err(|_| DeliveryError::Transport)
    }
}
impl Transport for HttpTransport {
    fn get(
        &self,
        url: &str,
        range: Option<&str>,
        remaining: Duration,
    ) -> Result<TransportResponse, DeliveryError> {
        // Defense in depth; URLs are generated only from validated registered pins.
        let parsed = reqwest::Url::parse(url).map_err(|_| DeliveryError::InvalidPin)?;
        if parsed.scheme() != "https"
            || parsed.host_str() != Some("data.anita.garden")
            || parsed.port().is_some()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || !parsed.path().starts_with("/trainworld/")
        {
            return Err(DeliveryError::InvalidPin);
        }
        let mut request = self
            .0
            .get(url)
            .timeout(remaining)
            .header("Accept-Encoding", "identity");
        if let Some(range) = range {
            request = request.header("Range", range);
        }
        let response = request.send().map_err(|e| {
            if e.is_timeout() {
                DeliveryError::Timeout
            } else {
                DeliveryError::Transport
            }
        })?;
        let headers = response.headers();
        if headers.get_all("content-range").iter().count() > 1
            || headers.get_all("content-length").iter().count() > 1
            || headers
                .get("content-encoding")
                .is_some_and(|v| v.as_bytes() != b"identity")
        {
            return Err(DeliveryError::Response);
        }
        let content_range = headers
            .get("content-range")
            .map(|v| v.to_str().map(str::to_owned))
            .transpose()
            .map_err(|_| DeliveryError::Response)?;
        let content_length = headers
            .get("content-length")
            .map(|v| {
                v.to_str()
                    .ok()
                    .and_then(|s| s.parse::<u64>().ok())
                    .ok_or(DeliveryError::Response)
            })
            .transpose()?;
        Ok(TransportResponse {
            status: response.status().as_u16(),
            content_range,
            content_length,
            body: Box::new(response),
        })
    }
}
struct Entry {
    id: String,
    bytes: usize,
    zone: Arc<WaterZone>,
}
pub struct WaterZoneDelivery<T: Transport = HttpTransport> {
    pins: Vec<ZonePin>,
    transport: T,
    cache: Mutex<VecDeque<Entry>>,
    active: AtomicUsize,
}
struct Permit<'a>(&'a AtomicUsize);
impl Drop for Permit<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
impl<T: Transport> WaterZoneDelivery<T> {
    pub fn new(pins: Vec<ZonePin>, transport: T) -> Result<Self, DeliveryError> {
        if pins.len() > 128 {
            return Err(DeliveryError::Limit);
        }
        for (i, pin) in pins.iter().enumerate() {
            pin.validate().map_err(|_| DeliveryError::InvalidPin)?;
            if pins[..i].iter().any(|p| p.zone_id == pin.zone_id) {
                return Err(DeliveryError::InvalidPin);
            }
        }
        Ok(Self {
            pins,
            transport,
            cache: Mutex::new(VecDeque::new()),
            active: AtomicUsize::new(0),
        })
    }
    /// Cache-owned memory is bounded. Callers retaining Arc results own that memory.
    /// Refresh failure never removes an existing validated entry.
    pub fn acquire(
        &self,
        zone_id: &str,
        cancel: &Cancellation,
    ) -> Result<Arc<WaterZone>, DeliveryError> {
        self.acquire_inner(zone_id, cancel, false)
    }
    // Pinned production data is immutable and acquired through the cache.
    // Forced refresh is only used to test failure atomicity of cache replacement.
    #[cfg(test)]
    pub fn refresh(
        &self,
        zone_id: &str,
        cancel: &Cancellation,
    ) -> Result<Arc<WaterZone>, DeliveryError> {
        self.acquire_inner(zone_id, cancel, true)
    }
    fn acquire_inner(
        &self,
        id: &str,
        cancel: &Cancellation,
        refresh: bool,
    ) -> Result<Arc<WaterZone>, DeliveryError> {
        let deadline = Instant::now() + DEADLINE;
        cancel.check(deadline)?;
        let pin = self
            .pins
            .iter()
            .find(|p| p.zone_id == id)
            .ok_or(DeliveryError::UnknownZone)?;
        if !refresh {
            let mut cache = self.cache.lock().map_err(|_| DeliveryError::Busy)?;
            if let Some(i) = cache.iter().position(|e| e.id == id) {
                let entry = cache.remove(i).ok_or(DeliveryError::Busy)?;
                let result = entry.zone.clone();
                cache.push_back(entry);
                cancel.check(deadline)?;
                return Ok(result);
            }
        }
        let mut active = self.active.load(Ordering::Acquire);
        loop {
            if active >= 2 {
                return Err(DeliveryError::Busy);
            }
            match self.active.compare_exchange_weak(
                active,
                active + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(current) => active = current,
            }
        }
        let _permit = Permit(&self.active);
        let (header_url, _) = pin.urls().map_err(|_| DeliveryError::InvalidPin)?;
        let mut header = self.transport.get(
            &header_url,
            None,
            deadline.saturating_duration_since(Instant::now()),
        )?;
        cancel.check(deadline)?;
        if header.status != 200 || header.content_range.is_some() {
            return Err(DeliveryError::Response);
        }
        if header
            .content_length
            .is_some_and(|n| n > HEADER_LIMIT as u64)
        {
            return Err(DeliveryError::Limit);
        }
        let mut bytes = Vec::new();
        read_chunks(&mut *header.body, HEADER_LIMIT, cancel, deadline, |chunk| {
            bytes.extend_from_slice(chunk);
            Ok(())
        })?;
        if header
            .content_length
            .is_some_and(|n| n != bytes.len() as u64)
        {
            return Err(DeliveryError::Response);
        }
        let mut acquisition =
            Acquisition::new(pin.clone()).map_err(|_| DeliveryError::InvalidPin)?;
        let request = acquisition
            .begin(&bytes, Instant::now())
            .map_err(|_| DeliveryError::Integrity)?;
        drop(bytes);
        drop(header);
        cancel.check(deadline)?;
        let mut response = self.transport.get(
            &request.url,
            Some(&request.range),
            deadline.saturating_duration_since(Instant::now()),
        )?;
        cancel.check(deadline)?;
        let expected_range = format!(
            "bytes {}-{}/{}",
            pin.range_start,
            pin.range_start + request.expected_bytes as u64 - 1,
            pin.container_length
        );
        if response.status != 206
            || response.content_range.as_deref() != Some(&expected_range)
            || response
                .content_length
                .is_some_and(|n| n != request.expected_bytes as u64)
        {
            return Err(DeliveryError::Response);
        }
        read_chunks(
            &mut *response.body,
            request.expected_bytes,
            cancel,
            deadline,
            |chunk| {
                acquisition
                    .push(request.token, chunk, Instant::now())
                    .map_err(|_| DeliveryError::Integrity)
            },
        )?;
        cancel.check(deadline)?;
        let zone = acquisition
            .finish(
                request.token,
                response.status,
                &expected_range,
                Instant::now(),
            )
            .map_err(|_| DeliveryError::Integrity)?;
        let mut cache = self.cache.lock().map_err(|_| DeliveryError::Busy)?;
        cancel.check(deadline)?;
        cache.retain(|e| e.id != id);
        while cache.len() >= 4
            || cache.iter().map(|e| e.bytes).sum::<usize>() + request.expected_bytes > CACHE_LIMIT
        {
            cache.pop_front();
        }
        cache.push_back(Entry {
            id: id.into(),
            bytes: request.expected_bytes,
            zone: zone.clone(),
        });
        Ok(zone)
    }
}
fn read_chunks(
    reader: &mut dyn Read,
    limit: usize,
    cancel: &Cancellation,
    deadline: Instant,
    mut accept: impl FnMut(&[u8]) -> Result<(), DeliveryError>,
) -> Result<(), DeliveryError> {
    let mut total = 0usize;
    let mut buffer = [0u8; 8192];
    loop {
        cancel.check(deadline)?;
        let len = reader
            .read(&mut buffer)
            .map_err(|_| DeliveryError::Transport)?;
        cancel.check(deadline)?;
        if len == 0 {
            return Ok(());
        }
        total = total.checked_add(len).ok_or(DeliveryError::Limit)?;
        if total > limit {
            return Err(DeliveryError::Limit);
        }
        accept(&buffer[..len])?;
    }
}

impl std::fmt::Display for DeliveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "water zone acquisition: {self:?}")
    }
}
impl std::error::Error for DeliveryError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    const HEADER: &[u8] = br#"{"origin":{"lon":139.0,"lat":35.0},"water":{"cell_m":25.0,"origin_m":[0,0],"size":[4,1]},"arrays":[{"name":"water_row","dtype":"u32","per":"water","offset":100,"count":2},{"name":"water_x","dtype":"u16","per":"water","offset":108,"count":2}]}"#;
    fn payload() -> Vec<u8> {
        [0u32, 2]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .chain([1u16, 3].into_iter().flat_map(u16::to_le_bytes))
            .collect()
    }
    fn pin(id: &str) -> ZonePin {
        ZonePin {
            provider_revision: "2026-11-13".into(),
            zone_id: id.into(),
            rule: "water-zone-row-runs-v1".into(),
            origin_longitude: 139.,
            origin_latitude: 35.,
            x0: 0.,
            y0: 0.,
            cell_m: 25.,
            width: 4,
            height: 1,
            run_count: 2,
            run_padding: 0,
            range_start: 100,
            container_length: 200,
            header_sha256: "c8b8a7c98d36452a3e67920d05e3f8ae81516a2aadd3e7650dea587e53d2a7f8"
                .into(),
            payload_sha256: "09aaf450392b44bd3fed92306ce965b338eed14b48398c0cdf8849525a6773dd"
                .into(),
        }
    }
    fn response(status: u16, range: Option<&str>, bytes: Vec<u8>) -> TransportResponse {
        TransportResponse {
            status,
            content_range: range.map(str::to_owned),
            content_length: Some(bytes.len() as u64),
            body: Box::new(Cursor::new(bytes)),
        }
    }
    struct Mock {
        queue: Mutex<VecDeque<TransportResponse>>,
        urls: Mutex<Vec<(String, Option<String>)>>,
    }
    impl Mock {
        fn good(n: usize) -> Self {
            let mut queue = VecDeque::new();
            for _ in 0..n {
                queue.push_back(response(200, None, HEADER.to_vec()));
                queue.push_back(response(206, Some("bytes 100-111/200"), payload()));
            }
            Self {
                queue: Mutex::new(queue),
                urls: Mutex::new(Vec::new()),
            }
        }
    }
    impl Transport for Mock {
        fn get(
            &self,
            url: &str,
            range: Option<&str>,
            remaining: Duration,
        ) -> Result<TransportResponse, DeliveryError> {
            assert!(remaining <= DEADLINE && !remaining.is_zero());
            self.urls
                .lock()
                .unwrap()
                .push((url.into(), range.map(str::to_owned)));
            self.queue
                .lock()
                .unwrap()
                .pop_front()
                .ok_or(DeliveryError::Transport)
        }
    }
    #[test]
    fn fixed_registered_urls_range_and_cache() {
        let d = WaterZoneDelivery::new(vec![pin("test")], Mock::good(1)).unwrap();
        let c = Cancellation::default();
        let first = d.acquire("test", &c).unwrap();
        assert!(Arc::ptr_eq(&first, &d.acquire("test", &c).unwrap()));
        assert_eq!(
            *d.transport.urls.lock().unwrap(),
            vec![
                (
                    "https://data.anita.garden/trainworld/2026-11-13/test.json".into(),
                    None
                ),
                (
                    "https://data.anita.garden/trainworld/2026-11-13/test.bin".into(),
                    Some("bytes=100-111".into())
                )
            ]
        );
        assert!(matches!(
            d.acquire("../../other", &c),
            Err(DeliveryError::UnknownZone)
        ));
    }
    #[test]
    fn rejects_duplicate_invalid_registry_and_cancelled_cache_access() {
        assert!(WaterZoneDelivery::new(vec![pin("a"), pin("a")], Mock::good(0)).is_err());
        assert!(WaterZoneDelivery::new(vec![pin("../a")], Mock::good(0)).is_err());
        let d = WaterZoneDelivery::new(vec![pin("a")], Mock::good(1)).unwrap();
        d.acquire("a", &Cancellation::default()).unwrap();
        let c = Cancellation::default();
        c.cancel();
        assert!(matches!(d.acquire("a", &c), Err(DeliveryError::Cancelled)));
    }
    #[test]
    fn failures_preserve_validated_cache() {
        for case in 0..7 {
            let d = WaterZoneDelivery::new(vec![pin("a")], Mock::good(2)).unwrap();
            let c = Cancellation::default();
            let old = d.acquire("a", &c).unwrap();
            {
                let mut queue = d.transport.queue.lock().unwrap();
                match case {
                    0 => queue[0].status = 302,
                    1 => queue[0].body = Box::new(Cursor::new(vec![b'x'; HEADER.len()])),
                    2 => queue[1].status = 200,
                    3 => queue[1].content_range = Some("bytes 100-111/201".into()),
                    4 => queue[1].body = Box::new(Cursor::new(vec![0; 12])),
                    5 => queue[1].body = Box::new(Cursor::new(vec![0; 11])),
                    _ => queue[1].body = Box::new(Cursor::new(vec![0; 13])),
                }
            }
            assert!(d.refresh("a", &c).is_err());
            assert!(Arc::ptr_eq(&old, &d.acquire("a", &c).unwrap()));
            assert_eq!(d.active.load(Ordering::Acquire), 0);
        }
    }
    struct CancelRead {
        cancel: Cancellation,
        body: Cursor<Vec<u8>>,
    }
    impl Read for CancelRead {
        fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
            self.cancel.cancel();
            self.body.read(b)
        }
    }
    #[test]
    fn cancellation_during_last_read_prevents_cache_commit() {
        let d = WaterZoneDelivery::new(vec![pin("a")], Mock::good(1)).unwrap();
        let c = Cancellation::default();
        d.transport.queue.lock().unwrap()[1].body = Box::new(CancelRead {
            cancel: c.clone(),
            body: Cursor::new(payload()),
        });
        assert!(matches!(d.acquire("a", &c), Err(DeliveryError::Cancelled)));
        assert!(d.cache.lock().unwrap().is_empty());
    }
    #[test]
    fn oversized_header_deadline_and_concurrency_are_bounded() {
        let d = WaterZoneDelivery::new(vec![pin("a")], Mock::good(1)).unwrap();
        d.transport.queue.lock().unwrap()[0].content_length = Some(HEADER_LIMIT as u64 + 1);
        assert!(matches!(
            d.acquire("a", &Cancellation::default()),
            Err(DeliveryError::Limit)
        ));
        d.active.store(2, Ordering::Release);
        assert!(matches!(
            d.acquire("a", &Cancellation::default()),
            Err(DeliveryError::Busy)
        ));
        let mut body = Cursor::new(vec![0; 10]);
        assert_eq!(
            read_chunks(
                &mut body,
                10,
                &Cancellation::default(),
                Instant::now(),
                |_| Ok(())
            ),
            Err(DeliveryError::Timeout)
        );
        assert_eq!(body.position(), 0);
    }
    #[test]
    fn lru_caps_entries_and_never_evicts_on_failure() {
        let pins = ["a", "b", "c", "d", "e"].into_iter().map(pin).collect();
        let d = WaterZoneDelivery::new(pins, Mock::good(5)).unwrap();
        for id in ["a", "b", "c", "d", "e"] {
            d.acquire(id, &Cancellation::default()).unwrap();
        }
        let cache = d.cache.lock().unwrap();
        assert_eq!(cache.len(), 4);
        assert_eq!(cache.front().unwrap().id, "b");
    }
    #[test]
    fn cancellation_is_per_consumer_and_does_not_clear_shared_cache() {
        let d = WaterZoneDelivery::new(vec![pin("a")], Mock::good(1)).unwrap();
        let old = d.acquire("a", &Cancellation::default()).unwrap();
        let cancelled = Cancellation::default();
        cancelled.cancel();
        assert!(matches!(
            d.refresh("a", &cancelled),
            Err(DeliveryError::Cancelled)
        ));
        assert!(Arc::ptr_eq(
            &old,
            &d.acquire("a", &Cancellation::default()).unwrap()
        ));
    }
    #[test]
    fn native_transport_rejects_arbitrary_hosts_schemes_and_query_before_io() {
        let t = HttpTransport::new().unwrap();
        for url in [
            "http://data.anita.garden/trainworld/x",
            "https://example.com/trainworld/x",
            "https://data.anita.garden/trainworld/x?q=1",
            "https://user@data.anita.garden/trainworld/x",
            "https://data.anita.garden/other",
        ] {
            assert!(matches!(
                t.get(url, None, Duration::from_secs(1)),
                Err(DeliveryError::InvalidPin)
            ));
        }
    }
}
