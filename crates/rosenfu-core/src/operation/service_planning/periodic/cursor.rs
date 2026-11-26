//! One-service arithmetic departure cursor. A finite, immutable rule window
//! can be replayed or queried; it is not yet part of OperationSave.
use super::{DailyServiceBand, PeriodicPlanningError, WORLD_DAY_MS};
use crate::operation::{OperationRuntime, Timetable};
use std::ops::Range;

pub const MIN_PERIODIC_SPACING_MS: u64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeriodicCursorError {
    InvalidService,
    InvalidSpacing,
    NotCold,
    TooSoon,
    Planning(PeriodicPlanningError),
}

/// A frozen, half-open activation horizon and compact clocks for one service.
/// It cannot be edited in place or deserialized as if already authoritative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeriodicDeparturePlan {
    service_id: String,
    active: Range<u64>,
    chunks: Vec<Timetable>,
}

#[derive(Debug)]
pub struct PeriodicDepartureCursor<'a> {
    plan: &'a PeriodicDeparturePlan,
    chunk: usize,
    next: Option<u64>,
}

impl OperationRuntime {
    /// Deterministically compile a finite one-service departure rule. No
    /// departures outside the activation horizon participate in its phase.
    /// Cadence never moves off the input grid. Within a band skip candidates
    /// shorter than the spacing floor; across bands skip candidates shorter
    /// than both the floor and the faster adjacent band's intended cadence
    /// (i.e. enforce max(floor, min(previous cadence, new cadence))).
    /// The floor is a schedule policy, not measured physical capacity.
    pub fn compile_periodic_departures(
        &self,
        service_id: &str,
        bands: &[DailyServiceBand],
        active: Range<u64>,
        minimum_spacing_ms: u64,
    ) -> Result<PeriodicDeparturePlan, PeriodicCursorError> {
        use PeriodicCursorError::*;
        if !service_id
            .parse::<u64>()
            .is_ok_and(|id| id.to_string() == service_id)
        {
            return Err(InvalidService);
        }
        if !(MIN_PERIODIC_SPACING_MS..=WORLD_DAY_MS).contains(&minimum_spacing_ms) {
            return Err(InvalidSpacing);
        }
        let planning = self
            .preview_periodic_timetable(service_id, bands, active.clone())
            .map_err(Planning)?;
        Ok(PeriodicDeparturePlan::from_sorted_chunks(
            service_id,
            active,
            minimum_spacing_ms,
            planning
                .chunks
                .into_iter()
                .map(|chunk| chunk.timetable)
                .collect(),
        ))
    }

    /// Staged cold edit: only prepare while the whole runtime is paused and
    /// has no vehicles or retained leases. After time has started, require a
    /// full spacing floor ahead of the committed clock. The old timetable
    /// and trains are untouched; this does not activate or save the new rule.
    pub fn prepare_idle_periodic_departures(
        &self,
        service_id: &str,
        bands: &[DailyServiceBand],
        active: Range<u64>,
        minimum_spacing_ms: u64,
    ) -> Result<PeriodicDeparturePlan, PeriodicCursorError> {
        use PeriodicCursorError::*;
        if !(MIN_PERIODIC_SPACING_MS..=WORLD_DAY_MS).contains(&minimum_spacing_ms) {
            return Err(InvalidSpacing);
        }
        if !self.paused || !self.trains.is_empty() || !self.occupied.is_empty() {
            return Err(NotCold);
        }
        if self.started
            && self
                .now_ms
                .checked_add(minimum_spacing_ms)
                .is_none_or(|safe| active.start < safe)
        {
            return Err(TooSoon);
        }
        self.compile_periodic_departures(service_id, bands, active, minimum_spacing_ms)
    }
}

fn at_or_after(table: &Timetable, threshold: u64) -> Option<u64> {
    if threshold > table.last_departure_ms {
        return None;
    }
    let skip = threshold
        .saturating_sub(table.first_departure_ms)
        .div_ceil(table.headway_ms);
    let candidate = table.first_departure_ms + skip * table.headway_ms;
    (candidate <= table.last_departure_ms).then_some(candidate)
}

impl PeriodicDeparturePlan {
    /// Callers bound and validate nonoverlapping, positive, exact time grids.
    pub(in crate::operation) fn from_sorted_chunks(
        service_id: &str,
        active: Range<u64>,
        minimum_spacing_ms: u64,
        source: Vec<Timetable>,
    ) -> Self {
        let mut chunks = Vec::with_capacity(source.len());
        let mut previous: Option<(u64, u64)> = None;
        for t in source {
            let threshold = previous.map_or(t.first_departure_ms, |(last, cadence)| {
                // All planning milliseconds obey MAX_SAFE_MS; adding two
                // operands within that range cannot overflow u64.
                last + minimum_spacing_ms.max(cadence.min(t.headway_ms))
            });
            let Some(first) = at_or_after(&t, threshold) else {
                continue;
            };
            let step = minimum_spacing_ms.div_ceil(t.headway_ms) * t.headway_ms;
            let last = first + ((t.last_departure_ms - first) / step) * step;
            chunks.push(Timetable {
                first_departure_ms: first,
                last_departure_ms: last,
                headway_ms: step,
            });
            previous = Some((last, t.headway_ms));
        }
        Self {
            service_id: service_id.into(),
            active,
            chunks,
        }
    }

    pub fn service_id(&self) -> &str {
        &self.service_id
    }

    pub fn active_window(&self) -> Range<u64> {
        self.active.clone()
    }

    /// Check a scheduled TrainKey time against this exact immutable plan.
    pub fn is_scheduled(&self, at: u64) -> bool {
        let table = self
            .chunks
            .get(self.chunks.partition_point(|t| t.last_departure_ms < at));
        table.and_then(|t| at_or_after(t, at)) == Some(at)
    }

    /// Count births for a replay budget directly from the compact clock.
    /// A query's edges never change phase or respace the frozen plan.
    pub fn departure_count(&self, window: Range<u64>) -> u64 {
        if window.is_empty() {
            return 0;
        }
        self.chunks
            .iter()
            .skip(
                self.chunks
                    .partition_point(|t| t.last_departure_ms < window.start),
            )
            .take_while(|t| t.first_departure_ms < window.end)
            .filter_map(|t| {
                let first = at_or_after(t, window.start)?;
                let last = t.last_departure_ms.min(window.end - 1);
                (last >= first).then(|| 1 + (last - first) / t.headway_ms)
            })
            .sum()
    }

    /// Derive the next unprocessed birth from the last inclusive, committed
    /// clock tick (or None if the runtime has never processed any tick).
    pub fn cursor_after(&self, processed_through: Option<u64>) -> PeriodicDepartureCursor<'_> {
        let found = match processed_through {
            None => self.first_at_or_after(self.active.start),
            Some(through) => through
                .checked_add(1)
                .and_then(|t| self.first_at_or_after(t)),
        };
        let (chunk, next) = match found {
            Some((i, t)) => (i, Some(t)),
            None => (self.chunks.len(), None),
        };
        PeriodicDepartureCursor {
            plan: self,
            chunk,
            next,
        }
    }

    fn first_at_or_after(&self, threshold: u64) -> Option<(usize, u64)> {
        let idx = self
            .chunks
            .partition_point(|t| t.last_departure_ms < threshold);
        let table = self.chunks.get(idx)?;
        // last_departure_ms is itself on the grid: for threshold <= last
        // there must be a candidate in this chunk.
        Some((idx, at_or_after(table, threshold)?))
    }
}

impl PeriodicDepartureCursor<'_> {
    pub fn next_departure(&self) -> Option<u64> {
        self.next
    }

    /// Advance only when the dispatcher commits this exact birth. A stale,
    /// duplicate or future event can neither advance nor fabricate a train.
    pub fn take_at(&mut self, at: u64) -> bool {
        if self.next != Some(at) {
            return false;
        }
        let table = &self.plan.chunks[self.chunk];
        if at < table.last_departure_ms {
            self.next = Some(at + table.headway_ms);
        } else {
            self.chunk += 1;
            self.next = self
                .plan
                .chunks
                .get(self.chunk)
                .map(|t| t.first_departure_ms);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::service_planning::periodic::{DailyBandService, MAX_SAFE_MS};
    use crate::operation::service_planning::{tests::run, FleetPlanningRequest};

    const M: u64 = 60_000;

    fn band(start: u64, end: u64, headway_ms: u64) -> DailyServiceBand {
        DailyServiceBand {
            start_ms: start,
            end_ms: end,
            service: DailyBandService::Planned {
                fleet: FleetPlanningRequest::TimedHeadway { headway_ms },
            },
        }
    }

    fn times(plan: &PeriodicDeparturePlan, through: Option<u64>) -> Vec<u64> {
        let mut out = Vec::new();
        let mut cursor = plan.cursor_after(through);
        while let Some(at) = cursor.next_departure() {
            out.push(at);
            assert!(cursor.take_at(at));
        }
        out
    }

    #[test]
    fn skips_only_grid_points_across_band_edges_and_midnight() {
        let op = OperationRuntime::new(vec![run(5_000)]).unwrap();
        let bands = [
            band(0, 4 * M, M),
            band(4 * M + 5_000, 8 * M, 2 * M),
            band(WORLD_DAY_MS - 5 * M, WORLD_DAY_MS, 2 * M),
        ];
        let plan = op
            .compile_periodic_departures("1", &bands, WORLD_DAY_MS - 6 * M..WORLD_DAY_MS + 8 * M, M)
            .unwrap();
        let day = WORLD_DAY_MS;
        let expected = [
            day - 5 * M,
            day - 3 * M,
            day - M,
            day,
            day + M,
            day + 2 * M,
            day + 3 * M,
            day + 4 * M + 5_000,
            day + 6 * M + 5_000,
        ];
        assert_eq!(times(&plan, None), expected);
        let same_overnight_cadence = [band(0, 6 * M, 2 * M), band(day - 5 * M, day, 2 * M)];
        let overnight = op
            .compile_periodic_departures("1", &same_overnight_cadence, day - 6 * M..day + 7 * M, M)
            .unwrap();
        // A 1-minute midnight gap would violate the 2-minute faster band.
        // Drop 00:00 rather than moving it off its original grid.
        assert_eq!(
            times(&overnight, None),
            [day - 5 * M, day - 3 * M, day - M, day + 2 * M, day + 4 * M]
        );
        assert!(!overnight.is_scheduled(day));
        assert!(overnight.is_scheduled(day + 2 * M));
        assert_eq!(overnight.departure_count(day..day + 4 * M), 1);
    }

    #[test]
    fn committed_clock_restart_and_budget_use_identical_half_open_times() {
        let op = OperationRuntime::new(vec![run(5_000)]).unwrap();
        let plan = op
            .compile_periodic_departures("1", &[band(0, 12 * M, 40_000)], 0..10 * M, M)
            .unwrap();
        assert_eq!(plan.service_id(), "1");
        assert_eq!(plan.active_window(), 0..10 * M);
        let expected = (0..10 * M).step_by(80_000).collect::<Vec<_>>();
        assert_eq!(times(&plan, None), expected);
        for cutoff in [0, 1, 80_000, 80_001, 7 * M, 10 * M, u64::MAX] {
            let later: Vec<_> = expected.iter().copied().filter(|t| *t > cutoff).collect();
            assert_eq!(times(&plan, Some(cutoff)), later);
            if cutoff < 10 * M {
                assert_eq!(
                    plan.departure_count(0..cutoff + 1) + later.len() as u64,
                    expected.len() as u64
                );
            }
        }
        let mut cursor = plan.cursor_after(Some(80_000));
        assert!(!cursor.take_at(80_000));
        assert!(!cursor.take_at(240_000));
        assert!(cursor.take_at(160_000));
        assert!(!cursor.take_at(160_000));
        assert_eq!(cursor.next_departure(), Some(240_000));
    }

    #[test]
    fn cold_prep_does_not_change_clock_or_save_and_rejects_active_and_retroactive() {
        let mut op = OperationRuntime::new(vec![run(5_000)]).unwrap();
        let bands = [band(0, 20 * M, M)];
        let before = op.save();
        assert!(op
            .prepare_idle_periodic_departures("1", &bands, 0..3 * M, M)
            .is_ok());
        assert_eq!(op.save(), before);
        op.set_paused(false);
        assert_eq!(
            op.prepare_idle_periodic_departures("1", &bands, 0..3 * M, M),
            Err(PeriodicCursorError::NotCold)
        );
        op.set_paused(true);
        op.advance_to(1, 256).unwrap();
        assert_eq!(
            op.prepare_idle_periodic_departures("1", &bands, 2 * M..3 * M, M),
            Err(PeriodicCursorError::NotCold)
        );
        op.advance_to(M, 256).unwrap();
        assert_eq!(
            op.prepare_idle_periodic_departures("1", &bands, 2 * M - 1..3 * M, M),
            Err(PeriodicCursorError::TooSoon)
        );
        let saved = op.save();
        assert_eq!(
            times(
                &op.prepare_idle_periodic_departures("1", &bands, 2 * M..3 * M, M)
                    .unwrap(),
                Some(M)
            ),
            [2 * M]
        );
        assert_eq!(op.save(), saved);
    }

    #[test]
    fn no_service_and_extreme_clock_never_create_phantom_departure() {
        let op = OperationRuntime::new(vec![run(5_000)]).unwrap();
        assert_eq!(
            op.compile_periodic_departures("01", &[], 0..M, M),
            Err(PeriodicCursorError::InvalidService)
        );
        assert_eq!(
            op.compile_periodic_departures("1", &[], 0..M, 1),
            Err(PeriodicCursorError::InvalidSpacing)
        );
        let empty = op.compile_periodic_departures("1", &[], 0..M, M).unwrap();
        assert!(times(&empty, None).is_empty());
        let near = op
            .compile_periodic_departures(
                "1",
                &[band(0, WORLD_DAY_MS, M)],
                MAX_SAFE_MS - 3 * M..MAX_SAFE_MS,
                M,
            )
            .unwrap();
        let times = times(&near, None);
        assert_eq!(times.len(), 3);
        assert!(times
            .iter()
            .all(|t| near.active_window().contains(t) && near.is_scheduled(*t)));
        assert!(!near.is_scheduled(MAX_SAFE_MS));
        assert_eq!(near.departure_count(u64::MAX - 1..u64::MAX), 0);
    }
}
