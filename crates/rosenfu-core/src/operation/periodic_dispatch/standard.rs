//! Standard named interval bands, as an explicit off-default rail source.
use super::*;
use crate::operation::{
    service_planning::{
        periodic::{MAX_PERIODIC_DEPARTURES, MAX_PERIODIC_WINDOW_MS},
        FleetPlanningRequest, MAX_SAFE_MS,
    },
    Timetable,
};

const MIN: u64 = 60_000;
const MAX_INTERVAL_MINUTES: u16 = 240;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NamedBandOverrides {
    pub early_morning: Option<u16>,
    pub morning_rush: Option<u16>,
    pub midday: Option<u16>,
    pub evening_rush: Option<u16>,
    pub late_night: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StandardIntervals {
    pub base_minutes: u16,
    pub overrides: NamedBandOverrides,
    /// Caller-supplied origin-service-day offsets, with explicit first through
    /// last boundary. Last may be >1440. Following time uses the base cadence.
    pub boundaries_minutes: [u16; 6],
    /// Finite activation horizon, relative world clock, exact half-open.
    pub start_ms: u64,
    pub end_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StandardDailyV1 {
    pub(crate) service_id: String,
    pub(crate) cutover_ms: u64,
    pub(crate) authored: StandardIntervals,
}

impl StandardDailyV1 {
    pub(super) fn binding(&self) -> PeriodicScheduleV1 {
        PeriodicScheduleV1 {
            service_id: self.service_id.clone(),
            cutover_ms: self.cutover_ms,
            start_ms: self.authored.start_ms,
            end_ms: self.authored.end_ms,
            minimum_spacing_ms: MIN,
            bands: Vec::new(),
        }
    }
}

pub(super) fn compile(
    runtime: &OperationRuntime,
    schedule: &StandardDailyV1,
) -> Result<PeriodicDeparturePlan, OperationError> {
    use OperationError::InvalidTimetable;
    let a = &schedule.authored;
    let active = a.start_ms..a.end_ms;
    let bounds = a.boundaries_minutes.map(|m| u64::from(m) * MIN);
    if active.is_empty()
        || active.end > MAX_SAFE_MS
        || active.end - active.start > MAX_PERIODIC_WINDOW_MS
        || bounds[0] >= WORLD_DAY_MS
        || bounds[5] > bounds[0] + WORLD_DAY_MS
        || bounds.windows(2).any(|v| v[0] >= v[1])
        || !(1..=MAX_INTERVAL_MINUTES).contains(&a.base_minutes)
    {
        return Err(InvalidTimetable);
    }
    let o = &a.overrides;
    let rates = [
        o.early_morning,
        o.morning_rush,
        o.midday,
        o.evening_rush,
        o.late_night,
    ]
    .map(|rate| rate.unwrap_or(a.base_minutes));
    if rates
        .iter()
        .any(|r| !(1..=MAX_INTERVAL_MINUTES).contains(r))
    {
        return Err(InvalidTimetable);
    }
    // Only operate a proven reusable origin round trip, as with the existing
    // one-service calendar path. Does not calculate throughput or buy stock.
    runtime
        .preview_nominal_fleet(
            &schedule.service_id,
            FleetPlanningRequest::TimedHeadway {
                headway_ms: u64::from(a.base_minutes) * MIN,
            },
        )
        .map_err(|_| InvalidTimetable)?;
    let mut profile: Vec<(u64, u64, u64)> = Vec::with_capacity(6);
    for (low, high, rate) in
        (0..5)
            .map(|i| (bounds[i], bounds[i + 1], rates[i]))
            .chain(std::iter::once((
                bounds[5],
                bounds[0] + WORLD_DAY_MS,
                a.base_minutes,
            )))
    {
        if low == high {
            continue;
        }
        let step = u64::from(rate) * MIN;
        if let Some(previous) = profile.last_mut().filter(|p| p.1 == low && p.2 == step) {
            previous.1 = high;
        } else {
            profile.push((low, high, step));
        }
    }

    let mut chunks = Vec::new();
    let mut count = 0u64;
    let mut project = |anchor: u64, end: u64, step: u64| -> Result<(), OperationError> {
        let low = anchor.max(active.start);
        let high = end.min(active.end);
        if low >= high {
            return Ok(());
        }
        let first = (low - anchor).div_ceil(step);
        let last = (high - 1 - anchor) / step;
        if first > last {
            return Ok(());
        }
        count += 1 + last - first;
        if count > MAX_PERIODIC_DEPARTURES {
            return Err(InvalidTimetable);
        }
        chunks.push(Timetable {
            first_departure_ms: anchor + first * step,
            last_departure_ms: anchor + last * step,
            headway_ms: step,
        });
        Ok(())
    };
    if active.start < bounds[0] {
        // Before the first named period of world day zero, use the base rate,
        // anchored at the actual world start. No artificial negative day.
        project(0, bounds[0], u64::from(a.base_minutes) * MIN)?;
    }
    // One complete service day begins every 24h. Hold its authored late-night
    // cadence even after the civil clock has rolled to the next date.
    for day in (active.start / WORLD_DAY_MS).saturating_sub(1)..=(active.end - 1) / WORLD_DAY_MS {
        let origin = day * WORLD_DAY_MS;
        for &(low, high, step) in &profile {
            project(origin + low, origin + high, step)?;
        }
    }
    Ok(PeriodicDeparturePlan::from_sorted_chunks(
        &schedule.service_id,
        active,
        MIN,
        chunks,
    ))
}

impl OperationRuntime {
    pub(crate) fn set_standard_timetable(
        &mut self,
        service_id: String,
        authored: StandardIntervals,
    ) -> Result<(), OperationError> {
        if self
            .standard_timetable
            .as_ref()
            .is_some_and(|active| active.service_id != service_id)
        {
            return Err(OperationError::InvalidTimetable);
        }
        let source = ScheduleSource::StandardDailyV1(StandardDailyV1 {
            service_id,
            cutover_ms: self.now_ms,
            authored,
        });
        if self.started {
            return self.bind_periodic_schedule(&source, false);
        }
        // Cold Apply replaces this one provisional rail timetable before
        // dispatch. Process only its empty time-zero boundary: no unrelated
        // cursor may be discarded and the new grid still begins in the
        // future. Preserve the existing started-format reader contract.
        if self.now_ms != 0
            || self.services.len() != 1
            || !self.paused
            || !self.trains.is_empty()
            || !self.occupied.is_empty()
        {
            return Err(OperationError::InvalidClock);
        }
        let mut cold = self.clone();
        cold.started = true;
        cold.bind_periodic_schedule(&source, false)?;
        *self = cold;
        Ok(())
    }

    pub(crate) fn saved_standard_timetable(&self) -> Option<&StandardDailyV1> {
        self.standard_timetable.as_ref()
    }

    pub fn has_standard_timetable(&self) -> bool {
        self.standard_timetable.is_some()
    }

    pub fn standard_intervals(&self) -> Option<(&str, &StandardIntervals)> {
        let source = self.standard_timetable.as_ref()?;
        Some((&source.service_id, &source.authored))
    }

    /// Prevent legacy scheduled-demand callers from mistaking the retired
    /// scalar headway for the actual varying timetable.
    pub fn checked_scalar_passenger_offers(
        &self,
    ) -> Result<Vec<crate::operation::PassengerOffer>, OperationError> {
        if self.has_standard_timetable() {
            return Err(OperationError::InvalidTimetable);
        }
        Ok(self.passenger_offers())
    }

    /// Authoritative origin-time grid for actual schedule and timed OD.
    pub fn standard_departure_grid(&self, service_id: &str) -> Option<&PeriodicDeparturePlan> {
        if self.standard_timetable.as_ref()?.service_id != service_id {
            return None;
        }
        match &self.services.get(service_id)?.departure_source {
            DepartureSource::Periodic(plan) => Some(plan),
            DepartureSource::Absolute => None,
        }
    }
}
