//! Explicit engine-only opt-in. The product World/storage still accepts its
//! ordinary absolute OperationSave, not this separate experiment envelope.
use super::service_planning::periodic::{
    cursor::{PeriodicDeparturePlan, MIN_PERIODIC_SPACING_MS},
    DailyServiceBand, WORLD_DAY_MS,
};
use super::{
    AdvanceResult, CompiledService, OperationError, OperationRuntime, OperationSave,
    PassengerReplay, PassengerReplayError, RailAdmissionModel, Train,
};
use serde::{Deserialize, Serialize};
use std::ops::Range;
#[cfg(feature = "periodic-world-qa")]
pub mod calendar;
#[cfg(feature = "standard-periodic-rail")]
pub mod standard;
#[cfg(feature = "periodic-world-qa")]
use calendar::{JapanScheduleV1, JapanServiceProfiles, PinnedJapanCalendar};
#[cfg(feature = "standard-periodic-rail")]
use standard::StandardDailyV1;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum DepartureSource {
    Absolute,
    Periodic(PeriodicDeparturePlan),
}

impl DepartureSource {
    pub(super) fn is_periodic(&self) -> bool {
        matches!(self, Self::Periodic(_))
    }
}

impl CompiledService {
    pub(super) fn next_unprocessed_departure(&self, started: bool, now: u64) -> Option<u64> {
        match &self.departure_source {
            DepartureSource::Absolute => {
                self.spec.timetable.next_unprocessed_departure(started, now)
            }
            DepartureSource::Periodic(plan) => {
                plan.cursor_after(started.then_some(now)).next_departure()
            }
        }
    }

    pub(super) fn scheduled(&self, departure: u64) -> bool {
        match &self.departure_source {
            DepartureSource::Absolute => self.spec.timetable.contains(departure),
            DepartureSource::Periodic(plan) => plan.is_scheduled(departure),
        }
    }

    pub(super) fn future_births_through(&self, until: u64) -> u64 {
        let Some(next) = self.next_departure.filter(|&n| n <= until) else {
            return 0;
        };
        match &self.departure_source {
            DepartureSource::Absolute => {
                let t = &self.spec.timetable;
                ((until.min(t.last_departure_ms) - next) / t.headway_ms).saturating_add(1)
            }
            DepartureSource::Periodic(plan) => plan.departure_count(next..until.saturating_add(1)),
        }
    }
}

// Explicit version strings and a distinct envelope deliberately keep
// OperationSave's existing representation and rail model tags unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeriodicOperationSave {
    runtime: OperationSave,
    source: ScheduleSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "version", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ScheduleSource {
    PeriodicV1(PeriodicScheduleV1),
    #[cfg(feature = "periodic-world-qa")]
    JapanCalendarV1(JapanScheduleV1),
    #[cfg(feature = "standard-periodic-rail")]
    StandardDailyV1(StandardDailyV1),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PeriodicScheduleV1 {
    service_id: String,
    cutover_ms: u64,
    start_ms: u64,
    end_ms: u64,
    minimum_spacing_ms: u64,
    bands: Vec<DailyServiceBand>,
}

#[cfg(feature = "periodic-world-qa")]
impl PeriodicOperationSave {
    /// Match the untouched absolute clock at the headless QA cutover. The
    /// caller separately proves ownership and exact compiled configuration.
    pub(crate) fn matches_frozen(&self, service_id: &str, frozen: &OperationSave) -> bool {
        // Calendar experiments use a separate engine receipt. The existing
        // frozen-world QA codec only admitted the earlier relative variant.
        let ScheduleSource::PeriodicV1(source) = &self.source else {
            return false;
        };
        source.service_id == service_id
            && source.cutover_ms == frozen.now_ms
            && self.runtime.services == frozen.services
            && self.runtime.format == frozen.format
            && self.runtime.now_ms >= frozen.now_ms
    }
}

/// No Deref to ordinary Runtime: raw OperationSave and scalar-headway
/// passenger offers cannot be mistaken for periodic world state.
pub struct PeriodicOperation {
    runtime: OperationRuntime,
    source: ScheduleSource,
}

impl PeriodicOperation {
    /// Switch exactly one physically compiled, reusable rail service to one
    /// finite same-day origin dispatch window. The existing absolute source
    /// must already be exhausted. Requires fixed admission, no ongoing
    /// handoff, paused started clock and no active vehicles/leases. The
    /// caller must not have any pending external catch-up to commit.
    pub fn start_from_idle(
        mut runtime: OperationRuntime,
        service_id: String,
        active_window: Range<u64>,
        minimum_spacing_ms: u64,
        bands: Vec<DailyServiceBand>,
    ) -> Result<Self, OperationError> {
        let schedule = PeriodicScheduleV1 {
            service_id,
            cutover_ms: runtime.now_ms,
            start_ms: active_window.start,
            end_ms: active_window.end,
            minimum_spacing_ms,
            bands,
        };
        let source = ScheduleSource::PeriodicV1(schedule);
        runtime.bind_periodic_schedule(&source, false)?;
        Ok(Self { runtime, source })
    }

    /// Explicit Japan day classes and whole origin-day offsets (>24h can
    /// continue the previous service day). No calendar source is chosen or
    /// fetched by the engine. Same cold fixed-rail restrictions as v1.
    #[cfg(feature = "periodic-world-qa")]
    pub fn start_japan_calendar_from_idle(
        mut runtime: OperationRuntime,
        service_id: String,
        active_window: Range<u64>,
        minimum_spacing_ms: u64,
        calendar: PinnedJapanCalendar,
        profiles: JapanServiceProfiles,
    ) -> Result<Self, OperationError> {
        let source = ScheduleSource::JapanCalendarV1(JapanScheduleV1 {
            base: PeriodicScheduleV1 {
                service_id,
                cutover_ms: runtime.now_ms,
                start_ms: active_window.start,
                end_ms: active_window.end,
                minimum_spacing_ms,
                bands: Vec::new(),
            },
            calendar,
            profiles,
        });
        runtime.bind_periodic_schedule(&source, false)?;
        Ok(Self { runtime, source })
    }

    pub fn advance_to(
        &mut self,
        until: u64,
        event_budget: usize,
    ) -> Result<AdvanceResult, OperationError> {
        self.runtime.advance_to(until, event_budget)
    }

    pub fn now_ms(&self) -> u64 {
        self.runtime.now_ms()
    }

    pub fn trains(&self) -> impl Iterator<Item = &Train> {
        self.runtime.trains()
    }

    /// Actual observed replay with physical waiting, not scalar scheduled OD.
    pub fn preview_passenger_replay(
        &self,
        until: u64,
        event_budget: usize,
    ) -> Result<PassengerReplay, PassengerReplayError> {
        self.runtime.preview_passenger_replay(until, event_budget)
    }

    pub fn save(&self) -> PeriodicOperationSave {
        PeriodicOperationSave {
            runtime: self.runtime.save(),
            source: self.source.clone(),
        }
    }

    pub fn restore(saved: PeriodicOperationSave) -> Result<Self, OperationError> {
        #[cfg(feature = "standard-periodic-rail")]
        if matches!(&saved.source, ScheduleSource::StandardDailyV1(_)) {
            return Err(OperationError::InvalidTimetable);
        }
        let runtime = OperationRuntime::restore_checked_with_source(
            saved.runtime,
            None,
            Some(&saved.source),
        )?;
        Ok(Self {
            runtime,
            source: saved.source,
        })
    }
}

impl OperationRuntime {
    pub(super) fn bind_periodic_schedule(
        &mut self,
        source: &ScheduleSource,
        restoring: bool,
    ) -> Result<(), OperationError> {
        #[cfg(feature = "standard-periodic-rail")]
        let standard = matches!(source, ScheduleSource::StandardDailyV1(_));
        #[cfg(not(feature = "standard-periodic-rail"))]
        let standard = false;
        #[cfg(feature = "standard-periodic-rail")]
        let standard_binding;
        let schedule = match source {
            ScheduleSource::PeriodicV1(schedule) => schedule,
            #[cfg(feature = "periodic-world-qa")]
            ScheduleSource::JapanCalendarV1(schedule) => &schedule.base,
            #[cfg(feature = "standard-periodic-rail")]
            ScheduleSource::StandardDailyV1(schedule) => {
                standard_binding = schedule.binding();
                &standard_binding
            }
        };
        if !(self.rail_admission == RailAdmissionModel::Fixed
            || (standard && self.rail_admission == RailAdmissionModel::TurnbackHold))
            || !self.started
            || schedule.cutover_ms > self.now_ms
            || !self.timetable_handoffs.is_empty()
            || (!restoring
                && (!self.paused || !self.trains.is_empty() || !self.occupied.is_empty()))
        {
            return Err(OperationError::InvalidClock);
        }
        // The existing relative v1 format remains same-day. Explicit
        // calendar v1 instead authorizes a bounded overnight series.
        if schedule.start_ms >= schedule.end_ms
            || (matches!(source, ScheduleSource::PeriodicV1(_))
                && schedule.start_ms / WORLD_DAY_MS != (schedule.end_ms - 1) / WORLD_DAY_MS)
            || schedule.minimum_spacing_ms < MIN_PERIODIC_SPACING_MS
            || schedule
                .cutover_ms
                .checked_add(schedule.minimum_spacing_ms)
                .is_none_or(|earliest| schedule.start_ms < earliest)
        {
            return Err(OperationError::InvalidTimetable);
        }
        if !schedule
            .service_id
            .parse::<u64>()
            .is_ok_and(|id| id.to_string() == schedule.service_id)
        {
            return Err(OperationError::InvalidTimetable);
        }
        let service = self
            .services
            .get(&schedule.service_id)
            .ok_or(OperationError::InvalidPath)?;
        // Only an idle, exact same-service Standard source can supersede
        // its own grid. Compile first and publish neither grid nor cursor
        // until all outstanding-runtime and new-plan guards have succeeded.
        #[cfg(feature = "standard-periodic-rail")]
        let replace_standard = standard
            && !restoring
            && schedule.cutover_ms == self.now_ms
            && self
                .standard_timetable
                .as_ref()
                .is_some_and(|active| active.service_id == schedule.service_id);
        #[cfg(not(feature = "standard-periodic-rail"))]
        let replace_standard = false;
        if (service.departure_source.is_periodic() && !replace_standard)
            || (!standard
                && (service
                    .spec
                    .timetable
                    .next_unprocessed_departure(true, schedule.cutover_ms)
                    .is_some()
                    || (!restoring && service.next_departure.is_some())))
            || schedule.end_ms.checked_add(service.journey_ms).is_none()
        {
            return Err(OperationError::InvalidTimetable);
        }
        let plan = match source {
            ScheduleSource::PeriodicV1(_) => self
                .compile_periodic_departures(
                    &schedule.service_id,
                    &schedule.bands,
                    schedule.start_ms..schedule.end_ms,
                    schedule.minimum_spacing_ms,
                )
                .map_err(|_| OperationError::InvalidTimetable)?,
            #[cfg(feature = "periodic-world-qa")]
            ScheduleSource::JapanCalendarV1(calendar) => calendar::compile(self, calendar)?,
            #[cfg(feature = "standard-periodic-rail")]
            ScheduleSource::StandardDailyV1(schedule) => standard::compile(self, schedule)?,
        };
        if plan.cursor_after(None).next_departure().is_none() {
            return Err(OperationError::InvalidTimetable);
        }
        let service = self
            .services
            .get_mut(&schedule.service_id)
            .ok_or(OperationError::InvalidPath)?;
        service.departure_source = DepartureSource::Periodic(plan);
        service.next_departure = service.next_unprocessed_departure(self.started, self.now_ms);
        #[cfg(feature = "standard-periodic-rail")]
        if let ScheduleSource::StandardDailyV1(authored) = source {
            self.standard_timetable = Some(authored.clone());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::service_planning::{
        periodic::DailyBandService, tests::run, FleetPlanningRequest,
    };
    use crate::operation::TrainState;

    #[cfg(feature = "standard-periodic-rail")]
    #[test]
    fn repeating_standard_bands_dispatch_at_later_days_and_stop_at_the_bound() {
        use standard::{NamedBandOverrides, StandardIntervals};

        let mut runtime = OperationRuntime::new(vec![run(5_000)]).unwrap();
        let start = 60_000;
        let end = start + 2 * WORLD_DAY_MS;
        runtime
            .set_standard_timetable(
                "1".into(),
                StandardIntervals {
                    base_minutes: 240,
                    overrides: NamedBandOverrides {
                        early_morning: None,
                        morning_rush: Some(6),
                        midday: None,
                        evening_rush: None,
                        late_night: None,
                    },
                    boundaries_minutes: [0, 360, 600, 960, 1200, 1440],
                    start_ms: start,
                    end_ms: end,
                },
            )
            .unwrap();
        let grid = runtime.standard_departure_grid("1").unwrap();
        let morning = 360 * 60_000;
        let window = |offset| offset + morning..offset + morning + 3_600_000;
        assert_eq!(grid.active_window(), start..end);
        assert_eq!(grid.departure_count(window(0)), 10);
        assert_eq!(grid.departure_count(window(WORLD_DAY_MS)), 10);
        assert_eq!(grid.departure_count(window(2 * WORLD_DAY_MS)), 0);
        let before = runtime.advance_to(WORLD_DAY_MS + morning, 10_000).unwrap();
        assert!(
            before.reached_target
                && before
                    .arrivals
                    .iter()
                    .all(|a| a.at_ms < WORLD_DAY_MS + morning)
        );
        let later = runtime
            .advance_to(WORLD_DAY_MS + morning + 45_000, 100)
            .unwrap();
        assert!(later.reached_target);
        assert_eq!(later.arrivals.len(), 2);
        assert_eq!(later.arrivals[0].train.scheduled_ms, WORLD_DAY_MS + morning);
        let after = runtime.advance_to(end + 3_600_000, 10_000).unwrap();
        assert!(after.reached_target);
        assert!(after.arrivals.iter().all(|a| a.train.scheduled_ms < end));
        assert!(runtime.trains.is_empty());
    }

    fn timetable_request() -> Vec<DailyServiceBand> {
        vec![
            DailyServiceBand {
                start_ms: 120_000,
                end_ms: 130_000,
                service: DailyBandService::Planned {
                    fleet: FleetPlanningRequest::TimedHeadway { headway_ms: 60_000 },
                },
            },
            DailyServiceBand {
                start_ms: 240_000,
                end_ms: 301_000,
                service: DailyBandService::Planned {
                    fleet: FleetPlanningRequest::TimedHeadway { headway_ms: 60_000 },
                },
            },
        ]
    }

    fn idle_with_competitor() -> OperationRuntime {
        let mut competitor = run(5_000);
        competitor.service_id = "2".into();
        competitor.legs.truncate(1);
        competitor.legs[0].from = "13".into();
        competitor.legs[0].to = "14".into();
        competitor.timetable.first_departure_ms = 119_000;
        competitor.timetable.last_departure_ms = 119_000;
        let mut op = OperationRuntime::new(vec![run(5_000), competitor]).unwrap();
        op.advance_to(60_000, 32).unwrap();
        assert!(op.trains().next().is_none());
        op
    }

    #[test]
    fn periodic_birth_uses_real_existing_rail_admission_and_same_train_return() {
        let mut periodic = PeriodicOperation::start_from_idle(
            idle_with_competitor(),
            "1".into(),
            120_000..300_000,
            60_000,
            timetable_request(),
        )
        .unwrap();
        assert_eq!(periodic.runtime.services["1"].next_departure, Some(120_000));
        assert_eq!(periodic.runtime.services["2"].next_departure, Some(119_000));
        assert_eq!(periodic.runtime.services.len(), 2);
        // The absolute source already finished. At 120 seconds the periodic
        // train is born but cannot enter the competing single-track lease.
        periodic.advance_to(120_000, 32).unwrap();
        let waiting = periodic.trains().find(|t| t.key.service_id == "1").unwrap();
        assert_eq!(waiting.key.scheduled_ms, 120_000);
        assert_eq!(waiting.state, TrainState::Waiting { ready_ms: 120_000 });
        assert_eq!(
            periodic
                .runtime
                .occupied
                .values()
                .next()
                .unwrap()
                .service_id,
            "2"
        );
        let before = periodic.save();
        let wire = serde_json::to_string(&before).unwrap();
        assert!(wire.contains("\"version\":\"periodic_v1\""));
        assert!(serde_json::from_str::<OperationSave>(&wire).is_err());
        let mut reopened =
            PeriodicOperation::restore(serde_json::from_str(&wire).unwrap()).unwrap();
        let replay = periodic.preview_passenger_replay(290_000, 64).unwrap();
        let rides: Vec<_> = replay
            .completed_legs
            .iter()
            .filter(|l| l.train.service_id == "1")
            .map(|l| {
                (
                    l.train.scheduled_ms,
                    l.entered_ms,
                    l.from.as_str(),
                    l.to.as_str(),
                )
            })
            .collect();
        assert_eq!(
            rides,
            [
                (120_000, 140_000, "11", "12"),
                (120_000, 165_000, "12", "11"),
                (240_000, 240_000, "11", "12"),
                (240_000, 265_000, "12", "11"),
            ]
        );
        assert_eq!(periodic.save(), before, "replay must not advance live time");
        let expected = periodic.advance_to(330_000, 64).unwrap();
        let mut chunked = Vec::new();
        loop {
            let frame = reopened.advance_to(330_000, 1).unwrap();
            chunked.extend(frame.arrivals);
            if frame.reached_target {
                break;
            }
            let saved = reopened.save();
            reopened = PeriodicOperation::restore(saved).unwrap();
        }
        assert_eq!(chunked, expected.arrivals);
        assert_eq!(periodic.save(), reopened.save());
        assert!(reopened.trains().next().is_none());
        assert_eq!(reopened.runtime.services["1"].next_departure, None);
    }

    #[test]
    fn excludes_unsupported_cold_paths_and_tampered_cursors_and_births() {
        let start = |op, window| {
            PeriodicOperation::start_from_idle(op, "1".into(), window, 60_000, timetable_request())
        };
        let never_started = OperationRuntime::new(vec![run(5_000)]).unwrap();
        assert!(matches!(
            start(never_started, 120_000..300_000),
            Err(OperationError::InvalidClock)
        ));
        let pending = {
            let mut spec = run(5_000);
            spec.timetable.last_departure_ms = 600_000;
            let mut op = OperationRuntime::new(vec![spec]).unwrap();
            op.advance_to(60_000, 32).unwrap();
            op
        };
        assert!(matches!(
            start(pending, 120_000..300_000),
            Err(OperationError::InvalidTimetable)
        ));
        assert!(matches!(
            start(idle_with_competitor(), 120_000..WORLD_DAY_MS + 1),
            Err(OperationError::InvalidTimetable)
        ));
        let mut good = start(idle_with_competitor(), 120_000..300_000).unwrap();
        let mut bad = good.save();
        bad.runtime.next_departures[0].1 = Some(120_001);
        assert!(matches!(
            PeriodicOperation::restore(bad),
            Err(OperationError::InvalidTimetable)
        ));
        good.advance_to(120_000, 32).unwrap();
        let mut bad = good.save();
        bad.runtime
            .trains
            .iter_mut()
            .find(|t| t.key.service_id == "1")
            .unwrap()
            .key
            .scheduled_ms = 119_999;
        assert!(matches!(
            PeriodicOperation::restore(bad),
            Err(OperationError::InvalidTimetable)
        ));
        let wire = serde_json::to_string(&good.save()).unwrap();
        assert!(serde_json::from_str::<PeriodicOperationSave>(
            &wire.replace("periodic_v1", "periodic_v2")
        )
        .is_err());
    }

    #[test]
    fn replay_budget_counts_only_periodic_births_in_the_half_open_plan() {
        let mut periodic = PeriodicOperation::start_from_idle(
            idle_with_competitor(),
            "1".into(),
            120_000..300_000,
            60_000,
            timetable_request(),
        )
        .unwrap();
        let s = &periodic.runtime.services["1"];
        assert_eq!(s.future_births_through(119_999), 0);
        assert_eq!(s.future_births_through(120_000), 1);
        assert_eq!(s.future_births_through(239_999), 1);
        assert_eq!(s.future_births_through(300_000), 2);
        periodic.advance_to(210_000, 32).unwrap();
        assert_eq!(
            periodic.runtime.services["1"].future_births_through(239_999),
            0
        );
        assert!(periodic
            .preview_passenger_replay(239_999, 16)
            .unwrap()
            .completed_legs
            .is_empty());
        let replay = periodic.preview_passenger_replay(280_000, 16).unwrap();
        assert_eq!(replay.completed_legs.len(), 1);
        assert_eq!(replay.completed_legs[0].train.scheduled_ms, 240_000);
    }
}
