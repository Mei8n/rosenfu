//! Opt-in preview of cyclic bands on the relative world clock. No calendar or
//! live dispatch state is manufactured by this compiler.
use super::{FleetPlanningError, FleetPlanningRequest, MAX_SAFE_MS};
use crate::operation::{OperationRuntime, Timetable};
use serde::{Deserialize, Serialize};
use std::ops::Range;
pub mod cursor;

pub const WORLD_DAY_MS: u64 = 86_400_000;
pub const MAX_PERIODIC_WINDOW_MS: u64 = 31 * WORLD_DAY_MS;
pub const MAX_PERIODIC_BANDS: usize = 32;
pub const MAX_PERIODIC_DEPARTURES: u64 = 43_200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DailyServiceBand {
    pub start_ms: u64,
    pub end_ms: u64,
    pub service: DailyBandService,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum DailyBandService {
    Off,
    Planned { fleet: FleetPlanningRequest },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodicDepartureChunk {
    /// Inclusive first and last departures, both on the same cadence grid.
    pub timetable: Timetable,
    pub nominal_trainsets: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodicTimetablePreview {
    pub chunks: Vec<PeriodicDepartureChunk>,
    pub planned_departures: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeriodicPlanningError {
    InvalidWindow,
    InvalidBands,
    Fleet(FleetPlanningError),
    DepartureBudget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedBand {
    time: Range<u64>,
    headway_ms: u64,
    trainsets: u64,
}

impl OperationRuntime {
    /// Plan departures whose origin departure time is in `[start,end)`.
    ///
    /// Each (merged) band's cadence begins at its start on each 24-hour
    /// relative world day; an uncovered interval is off. Adjacent bands that
    /// resolve identically share a cadence within the same day. There is no
    /// lead-time shift, civil calendar, or minimum guarantee between different
    /// bands or across midnight. A shorter query never rephases a departure.
    /// Chunks describe one service, not separate service IDs or a runnable
    /// multi-window spec. This never mutates the live absolute timetable.
    pub fn preview_periodic_timetable(
        &self,
        service_id: &str,
        bands: &[DailyServiceBand],
        window: Range<u64>,
    ) -> Result<PeriodicTimetablePreview, PeriodicPlanningError> {
        use PeriodicPlanningError::*;
        if window.start >= window.end
            || window.end > MAX_SAFE_MS
            || window.end - window.start > MAX_PERIODIC_WINDOW_MS
        {
            return Err(InvalidWindow);
        }
        if bands.len() > MAX_PERIODIC_BANDS {
            return Err(InvalidBands);
        }
        let mut previous_end = 0;
        for band in bands {
            if band.start_ms < previous_end
                || band.start_ms >= band.end_ms
                || band.end_ms > WORLD_DAY_MS
            {
                return Err(InvalidBands);
            }
            previous_end = band.end_ms;
        }
        if !self.services.contains_key(service_id) {
            return Err(Fleet(FleetPlanningError::UnsupportedRun));
        }

        let mut resolved: Vec<ResolvedBand> = Vec::new();
        for band in bands {
            let DailyBandService::Planned { fleet } = band.service else {
                continue;
            };
            let plan = self
                .preview_nominal_fleet(service_id, fleet)
                .map_err(Fleet)?;
            if let Some(previous) = resolved.last_mut() {
                if previous.time.end == band.start_ms
                    && previous.headway_ms == plan.headway_ms
                    && previous.trainsets == plan.trainsets
                {
                    previous.time.end = band.end_ms;
                    continue;
                }
            }
            resolved.push(ResolvedBand {
                time: band.start_ms..band.end_ms,
                headway_ms: plan.headway_ms,
                trainsets: plan.trainsets,
            });
        }

        let mut preview = PeriodicTimetablePreview {
            chunks: Vec::new(),
            planned_departures: 0,
        };
        // Limiting endpoints to an exact binary64 integer (MAX_SAFE_MS)
        // keeps the day+offset arithmetic safely below u64::MAX even for
        // the last represented day, and clamps all emitted times to window.
        for day in window.start / WORLD_DAY_MS..=(window.end - 1) / WORLD_DAY_MS {
            let day_start = day * WORLD_DAY_MS;
            for band in &resolved {
                let anchor = day_start + band.time.start;
                let start = anchor.max(window.start);
                let end = (day_start + band.time.end).min(window.end);
                if start >= end {
                    continue;
                }
                let first = (start - anchor).div_ceil(band.headway_ms);
                let last = (end - 1 - anchor) / band.headway_ms;
                if first > last {
                    continue;
                }
                preview.planned_departures += last - first + 1;
                if preview.planned_departures > MAX_PERIODIC_DEPARTURES {
                    return Err(DepartureBudget);
                }
                // first <= last, so the products are bounded by end - anchor.
                preview.chunks.push(PeriodicDepartureChunk {
                    timetable: Timetable {
                        first_departure_ms: anchor + first * band.headway_ms,
                        last_departure_ms: anchor + last * band.headway_ms,
                        headway_ms: band.headway_ms,
                    },
                    nominal_trainsets: band.trainsets,
                });
            }
        }
        Ok(preview)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::service_planning::tests::run;

    fn band(start_ms: u64, end_ms: u64, service: DailyBandService) -> DailyServiceBand {
        DailyServiceBand {
            start_ms,
            end_ms,
            service,
        }
    }

    fn headway(headway_ms: u64) -> DailyBandService {
        DailyBandService::Planned {
            fleet: FleetPlanningRequest::TimedHeadway { headway_ms },
        }
    }

    fn departures(preview: &PeriodicTimetablePreview) -> Vec<u64> {
        preview
            .chunks
            .iter()
            .flat_map(|chunk| {
                let t = &chunk.timetable;
                (0..=(t.last_departure_ms - t.first_departure_ms) / t.headway_ms)
                    .map(move |i| t.first_departure_ms + i * t.headway_ms)
            })
            .collect()
    }

    #[test]
    fn bands_gaps_and_late_queries_preserve_phase_without_touching_runtime() {
        let mut op = OperationRuntime::new(vec![run(5_000)]).unwrap();
        let _ = op.advance_to(100_000, 256).unwrap();
        let saved = op.save();
        let bands = [
            band(0, 31_000, headway(20_000)),
            band(31_000, 71_000, headway(20_000)),
            band(71_000, 80_000, DailyBandService::Off),
            band(
                82_000,
                120_000,
                DailyBandService::Planned {
                    fleet: FleetPlanningRequest::TrainsetCount { trainsets: 3 },
                },
            ),
        ];
        let whole = op
            .preview_periodic_timetable("1", &bands, 0..120_000)
            .unwrap();
        assert_eq!(
            departures(&whole),
            [0, 20_000, 40_000, 60_000, 82_000, 98_667, 115_334]
        );
        assert_eq!(whole.planned_departures, 7);
        assert_eq!(
            whole
                .chunks
                .iter()
                .map(|chunk| chunk.nominal_trainsets)
                .collect::<Vec<_>>(),
            [3, 3]
        );
        let mut split = departures(
            &op.preview_periodic_timetable("1", &bands, 0..40_000)
                .unwrap(),
        );
        split.extend(departures(
            &op.preview_periodic_timetable("1", &bands, 40_000..120_000)
                .unwrap(),
        ));
        assert_eq!(split, departures(&whole));
        assert_eq!(op.save(), saved);
    }

    #[test]
    fn midnight_clips_both_days_without_duplicate_departures() {
        let op = OperationRuntime::new(vec![run(5_000)]).unwrap();
        let bands = [
            band(0, 80_000, headway(30_000)),
            band(WORLD_DAY_MS - 70_000, WORLD_DAY_MS, headway(25_000)),
        ];
        let preview = op
            .preview_periodic_timetable("1", &bands, WORLD_DAY_MS - 21_000..WORLD_DAY_MS + 30_000)
            .unwrap();
        assert_eq!(departures(&preview), [WORLD_DAY_MS - 20_000, WORLD_DAY_MS]);
        let edge = op
            .preview_periodic_timetable(
                "1",
                &[band(0, WORLD_DAY_MS, headway(1_000))],
                MAX_SAFE_MS - 4_000..MAX_SAFE_MS,
            )
            .unwrap();
        assert_eq!(edge.planned_departures, 4);
        assert!(departures(&edge)
            .iter()
            .all(|t| *t >= MAX_SAFE_MS - 4_000 && *t < MAX_SAFE_MS));
    }

    #[test]
    fn invalid_patterns_and_excess_departures_fail_without_partial_chunks() {
        let op = OperationRuntime::new(vec![run(5_000)]).unwrap();
        let compile =
            |bands: &[DailyServiceBand], window| op.preview_periodic_timetable("1", bands, window);
        assert_eq!(
            compile(&[], 0..0),
            Err(PeriodicPlanningError::InvalidWindow)
        );
        assert_eq!(
            compile(&[], 0..MAX_PERIODIC_WINDOW_MS + 1),
            Err(PeriodicPlanningError::InvalidWindow)
        );
        assert_eq!(
            compile(&[band(0, 11, headway(1)), band(10, 20, headway(1))], 0..10),
            Err(PeriodicPlanningError::InvalidBands)
        );
        assert_eq!(
            compile(&[band(1, WORLD_DAY_MS + 1, headway(1))], 0..10),
            Err(PeriodicPlanningError::InvalidBands)
        );
        assert_eq!(
            compile(&[band(0, 1, headway(1)); MAX_PERIODIC_BANDS + 1], 0..10),
            Err(PeriodicPlanningError::InvalidBands)
        );
        assert_eq!(
            compile(&[band(0, 10, headway(0))], 100..200),
            Err(PeriodicPlanningError::Fleet(
                FleetPlanningError::InvalidInput
            ))
        );
        let dense = [band(0, WORLD_DAY_MS, headway(1))];
        assert_eq!(
            compile(&dense, 0..MAX_PERIODIC_DEPARTURES + 1),
            Err(PeriodicPlanningError::DepartureBudget)
        );
        assert_eq!(
            compile(&dense, 0..MAX_PERIODIC_DEPARTURES)
                .unwrap()
                .planned_departures,
            MAX_PERIODIC_DEPARTURES
        );
        // Individual chunks fit, but the cap is for their union.
        let separate = [
            band(0, 30_000, headway(1)),
            band(40_000, 60_000, headway(1)),
        ];
        assert_eq!(
            compile(&separate, 0..60_000),
            Err(PeriodicPlanningError::DepartureBudget)
        );
        assert!(compile(
            &[band(0, WORLD_DAY_MS, DailyBandService::Off)],
            0..MAX_PERIODIC_WINDOW_MS
        )
        .unwrap()
        .chunks
        .is_empty());
    }

    #[test]
    fn band_wire_never_infers_calendar_or_off_mode() {
        let decode = |json| serde_json::from_str::<DailyServiceBand>(json);
        assert_eq!(
            decode(r#"{"startMs":0,"endMs":1000,"service":{"state":"planned","fleet":{"mode":"trainsetCount","trainsets":3}}}"#).unwrap().service,
            DailyBandService::Planned { fleet: FleetPlanningRequest::TrainsetCount { trainsets: 3 } }
        );
        assert!(decode(r#"{"startMs":0,"endMs":1000}"#).is_err());
        assert!(
            decode(r#"{"startMs":0,"endMs":1000,"weekday":true,"service":{"state":"off"}}"#)
                .is_err()
        );
    }
}
