//! Pinned civil-day classification and origin service-day grids. Experimental
//! only: no wall clock, network, default anchor, or on-screen real-year label.
use super::*;
use crate::operation::service_planning::{
    periodic::{
        DailyBandService, MAX_PERIODIC_BANDS, MAX_PERIODIC_DEPARTURES, MAX_PERIODIC_WINDOW_MS,
    },
    MAX_SAFE_MS,
};
use crate::operation::Timetable;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JapanCivilDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}
impl JapanCivilDate {
    fn ordinal(self) -> Option<u64> {
        let y = u64::from(self.year);
        let m = usize::from(self.month);
        let d = u64::from(self.day);
        let leap = y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
        let months = [
            31,
            if leap { 29 } else { 28 },
            31,
            30,
            31,
            30,
            31,
            31,
            30,
            31,
            30,
            31,
        ];
        if !(1900..=9999).contains(&y) || !(1..=12).contains(&m) || d == 0 || d > months[m - 1] {
            return None;
        }
        let prior = y - 1;
        Some(
            365 * prior + prior / 4 - prior / 100
                + prior / 400
                + months[..m - 1].iter().sum::<u64>()
                + d
                - 1,
        )
    }
}

/// Caller-supplied normalized holiday edition. Integrity checks do not prove
/// the authority or completeness of its publishing source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JapanHolidayEdition {
    pub id: String,
    pub first: JapanCivilDate,
    pub last: JapanCivilDate,
    pub holidays: Vec<JapanCivilDate>,
}

impl JapanHolidayEdition {
    pub fn normalized_digest(&self) -> Option<[u8; 32]> {
        if self.id.is_empty()
            || self.id.len() > 80
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            || self.holidays.len() > 4096
        {
            return None;
        }
        let first = self.first.ordinal()?;
        let last = self.last.ordinal()?;
        if first > last || self.holidays.windows(2).any(|p| p[0] >= p[1]) {
            return None;
        }
        // Canonical version, bounded ASCII ID and Gregorian ordinals,
        // independent of JSON key ordering and original date formatting.
        let mut digest = Sha256::new();
        digest.update(b"rosenfu-jp-holiday-edition-v1\0");
        digest.update(self.id.as_bytes());
        digest.update([0]);
        digest.update(first.to_be_bytes());
        digest.update(last.to_be_bytes());
        for date in &self.holidays {
            let day = date.ordinal().filter(|day| (first..=last).contains(day))?;
            digest.update(day.to_be_bytes());
        }
        Some(digest.finalize().into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedJapanCalendar {
    /// World ms 0 is midnight at this Japan civil date (UTC+09:00).
    pub anchor: JapanCivilDate,
    pub edition: JapanHolidayEdition,
    pub edition_sha256: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JapanServiceProfiles {
    pub weekdays: Vec<DailyServiceBand>,
    pub weekends_and_holidays: Vec<DailyServiceBand>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JapanScheduleV1 {
    pub(super) base: PeriodicScheduleV1,
    pub(super) calendar: PinnedJapanCalendar,
    pub(super) profiles: JapanServiceProfiles,
}

/// Starts belong to their origin service day (<24h). An end can exceed
/// midnight without resetting the existing cadence. Active windows from
/// adjacent days that overlap are refused, not given artificial priority.
pub(super) fn compile(
    runtime: &OperationRuntime,
    schedule: &JapanScheduleV1,
) -> Result<PeriodicDeparturePlan, OperationError> {
    use OperationError::InvalidTimetable;
    let base = &schedule.base;
    let active = base.start_ms..base.end_ms;
    if active.is_empty()
        || active.end > MAX_SAFE_MS
        || active.end - active.start > MAX_PERIODIC_WINDOW_MS
        || !(MIN_PERIODIC_SPACING_MS..=WORLD_DAY_MS).contains(&base.minimum_spacing_ms)
        || !base.bands.is_empty()
    {
        return Err(InvalidTimetable);
    }

    let pin = &schedule.calendar;
    if pin.edition.normalized_digest() != Some(pin.edition_sha256) {
        return Err(InvalidTimetable);
    }
    let anchor_day = pin.anchor.ordinal().ok_or(InvalidTimetable)?;
    let coverage = pin.edition.first.ordinal().ok_or(InvalidTimetable)?
        ..=pin.edition.last.ordinal().ok_or(InvalidTimetable)?;
    let holidays = pin
        .edition
        .holidays
        .iter()
        .map(|d| d.ordinal().ok_or(InvalidTimetable))
        .collect::<Result<Vec<_>, _>>()?;

    let mut profiles = [Vec::new(), Vec::new()];
    for (bands, resolved) in [
        &schedule.profiles.weekdays,
        &schedule.profiles.weekends_and_holidays,
    ]
    .into_iter()
    .zip(&mut profiles)
    {
        if bands.len() > MAX_PERIODIC_BANDS {
            return Err(InvalidTimetable);
        }
        let mut previous_end = 0;
        for band in bands {
            if band.start_ms < previous_end
                || band.start_ms >= WORLD_DAY_MS
                || band.start_ms >= band.end_ms
                || band.end_ms > 2 * WORLD_DAY_MS
            {
                return Err(InvalidTimetable);
            }
            previous_end = band.end_ms;
            if let DailyBandService::Planned { fleet } = band.service {
                let nominal = runtime
                    .preview_nominal_fleet(&base.service_id, fleet)
                    .map_err(|_| InvalidTimetable)?;
                resolved.push((band.start_ms, band.end_ms, nominal.headway_ms));
            }
        }
    }

    let mut chunks = Vec::new();
    let mut previous_end = active.start;
    let mut count = 0;
    // Account for the previous service-day tail, but never invent a service
    // before world day zero. Absolute intersections never move the grid anchor.
    for day in (active.start / WORLD_DAY_MS).saturating_sub(1)..=(active.end - 1) / WORLD_DAY_MS {
        let origin = day * WORLD_DAY_MS;
        let intersects = |start, end| origin + start < active.end && origin + end > active.start;
        if !profiles
            .iter()
            .flatten()
            .any(|&(start, end, _)| intersects(start, end))
        {
            continue; // Uncovered day with no possible event does not matter.
        }
        let date = anchor_day + day;
        if !coverage.contains(&date) {
            return Err(InvalidTimetable);
        }
        let alternate = date % 7 >= 5 || holidays.binary_search(&date).is_ok();
        for &(start, end, step) in &profiles[usize::from(alternate)] {
            if !intersects(start, end) {
                continue;
            }
            let anchor = origin + start;
            let low = anchor.max(active.start);
            let high = (origin + end).min(active.end);
            if low < previous_end {
                return Err(InvalidTimetable);
            }
            previous_end = high;
            let first = (low - anchor).div_ceil(step);
            let last = (high - 1 - anchor) / step;
            if first > last {
                continue;
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
        }
    }
    Ok(PeriodicDeparturePlan::from_sorted_chunks(
        &base.service_id,
        active,
        base.minimum_spacing_ms,
        chunks,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::service_planning::{tests::run, FleetPlanningRequest};

    const H: u64 = 3_600_000;
    const M: u64 = 60_000;
    const D: u64 = WORLD_DAY_MS;

    fn dt(year: u16, month: u8, day: u8) -> JapanCivilDate {
        JapanCivilDate { year, month, day }
    }

    fn profile(start_ms: u64, end_ms: u64, headway_ms: u64) -> DailyServiceBand {
        DailyServiceBand {
            start_ms,
            end_ms,
            service: DailyBandService::Planned {
                fleet: FleetPlanningRequest::TimedHeadway { headway_ms },
            },
        }
    }

    fn pinned(
        anchor: JapanCivilDate,
        last: JapanCivilDate,
        holidays: Vec<JapanCivilDate>,
    ) -> PinnedJapanCalendar {
        // Fixture excerpts from the Cabinet Office holiday table; they
        // declare only their tested date interval, not a national data pack.
        // https://www8.cao.go.jp/chosei/shukujitsu/gaiyou.html
        let edition = JapanHolidayEdition {
            id: "official-excerpt-test".into(),
            first: anchor,
            last,
            holidays,
        };
        PinnedJapanCalendar {
            anchor,
            edition_sha256: edition.normalized_digest().unwrap(),
            edition,
        }
    }

    fn sep_2026() -> PinnedJapanCalendar {
        pinned(
            dt(2026, 9, 18),
            dt(2026, 9, 23),
            vec![dt(2026, 9, 21), dt(2026, 9, 22), dt(2026, 9, 23)],
        )
    }

    fn idle() -> OperationRuntime {
        let mut op = OperationRuntime::new(vec![run(5_000)]).unwrap();
        op.advance_to(M, 64).unwrap();
        op
    }

    fn activate(
        pin: PinnedJapanCalendar,
        window: Range<u64>,
        profiles: JapanServiceProfiles,
    ) -> Result<PeriodicOperation, OperationError> {
        PeriodicOperation::start_japan_calendar_from_idle(
            idle(),
            "1".into(),
            window,
            M,
            pin,
            profiles,
        )
    }

    fn night_profiles() -> JapanServiceProfiles {
        JapanServiceProfiles {
            weekdays: vec![profile(22 * H + 30 * M, 25 * H + 10 * M, 2 * H)],
            weekends_and_holidays: vec![profile(8 * H, 9 * H, H)],
        }
    }

    #[test]
    fn friday_2430_continues_same_cadence_after_saturday_midnight_and_restore() {
        let mut game = activate(sep_2026(), 21 * H..D + 10 * H, night_profiles()).unwrap();
        game.advance_to(D, 64).unwrap();
        assert_eq!(game.runtime.services["1"].next_departure, Some(D + 30 * M));
        let original = game.save();
        let wire = serde_json::to_vec(&original).unwrap();
        let mut resumed =
            PeriodicOperation::restore(serde_json::from_slice(&wire).unwrap()).unwrap();
        assert_eq!(resumed.save(), original);
        let replay = resumed.preview_passenger_replay(D + 45 * M, 64).unwrap();
        assert_eq!(replay.completed_legs.len(), 2);
        assert!(replay
            .completed_legs
            .iter()
            .all(|leg| leg.train.scheduled_ms == D + 30 * M));
        let full = game.advance_to(D + 10 * H, 64).unwrap();
        assert_eq!(full, resumed.advance_to(D + 10 * H, 64).unwrap());
        let starts = full
            .arrivals
            .iter()
            .map(|a| a.train.scheduled_ms)
            .collect::<Vec<_>>();
        assert_eq!(starts, [D + 30 * M, D + 30 * M, D + 8 * H, D + 8 * H]);
        assert_eq!(game.save(), resumed.save());
    }

    #[test]
    fn intervening_and_substitute_weekday_holidays_use_the_alternate_profile() {
        let expected = |pin, window: Range<u64>, at| {
            let game = activate(
                pin,
                window.clone(),
                JapanServiceProfiles {
                    weekdays: vec![profile(9 * H, 10 * H, H)],
                    weekends_and_holidays: vec![profile(8 * H, 9 * H, H)],
                },
            )
            .unwrap();
            assert_eq!(game.runtime.services["1"].next_departure, Some(at));
        };
        // Tue Sep 22 is Article 3(3) between two holidays.
        expected(sep_2026(), 4 * D + 7 * H..4 * D + 11 * H, 4 * D + 8 * H);
        // Ordinary Friday Sep 18 uses weekday instructions.
        expected(sep_2026(), 7 * H..11 * H, 9 * H);
        // Mon Mar 22 is Article 3(2) substitute day for the Sunday equinox.
        let spring = pinned(
            dt(2027, 3, 20),
            dt(2027, 3, 22),
            vec![dt(2027, 3, 21), dt(2027, 3, 22)],
        );
        expected(spring, 2 * D + 7 * H..2 * D + 11 * H, 2 * D + 8 * H);
        assert_eq!(
            dt(2028, 3, 1).ordinal().unwrap() - dt(2028, 2, 28).ordinal().unwrap(),
            2
        );
        assert!(dt(2100, 2, 29).ordinal().is_none());
    }

    #[test]
    fn overlap_uncovered_days_and_altered_pins_fail_closed_without_guessing() {
        let friday = pinned(dt(2026, 9, 18), dt(2026, 9, 18), vec![]);
        // Known Friday tail can run into uncovered Saturday; there is no
        // possible independently configured Saturday departure before 01:00.
        assert!(activate(friday.clone(), 21 * H..D + H, night_profiles()).is_ok());
        assert!(matches!(
            activate(friday, 21 * H..D + 10 * H, night_profiles()),
            Err(OperationError::InvalidTimetable)
        ));
        let mut overlap = night_profiles();
        overlap
            .weekends_and_holidays
            .insert(0, profile(15 * M, 45 * M, 2 * M));
        assert!(matches!(
            activate(sep_2026(), 21 * H..D + 10 * H, overlap),
            Err(OperationError::InvalidTimetable)
        ));
        let game = activate(sep_2026(), 21 * H..D + 10 * H, night_profiles()).unwrap();
        let mut tampered = game.save();
        let ScheduleSource::JapanCalendarV1(source) = &mut tampered.source else {
            panic!()
        };
        source.calendar.edition.holidays.pop();
        assert!(matches!(
            PeriodicOperation::restore(tampered),
            Err(OperationError::InvalidTimetable)
        ));
    }
}
