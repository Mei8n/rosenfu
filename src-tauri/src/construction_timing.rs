//! Opt-in native phase logging, without snapshot serialization or saved fields.
pub struct Phase {
    name: &'static str,
    start: Option<std::time::Instant>,
}
impl Phase {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            start: (std::env::var_os("ROSENFU_CONSTRUCTION_TIMING").as_deref()
                == Some(std::ffi::OsStr::new("1")))
            .then(std::time::Instant::now),
        }
    }
}
impl Drop for Phase {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            eprintln!(
                "construction phase={} ms={:.3} thread={:?}",
                self.name,
                start.elapsed().as_secs_f64() * 1000.0,
                std::thread::current().id()
            );
        }
    }
}
