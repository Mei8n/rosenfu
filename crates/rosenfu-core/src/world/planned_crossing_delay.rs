//! Optional timetable-load estimate for the already verified Normal flat crossings.
//! This never writes a train hold, a passenger result, or a saved field.
use super::{operation_adapter::crossing, TrackType, World};
use crate::operation::{inline_lane_key, uniform_section_passage_ms, Timetable};
use std::collections::{BTreeMap, BTreeSet};

const HOUR_MS: u64 = 3_600_000;
const EXPOSURE_SECONDS: f64 = 120.0;
const SCAN_BUDGET: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannedCrossingDelayError {
    WindowOverflow,
    WorkBudgetExceeded,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedCrossingDelay {
    pub alignment_ids: [u64; 2],
    pub approach_alignment_id: u64,
    pub towards_alignment_end: bool,
    /// Nominal passage events with no reservation delays, per half-open hour.
    /// [This direction on own track, both directions on the crossing track].
    pub scheduled_passages: [u64; 2],
    /// Requested load, not a capacity cap or a change to departures.
    pub requested_utilization: f64,
    pub estimated_delay_seconds_per_train: f64,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct PlannedCrossingDelayPreview {
    pub estimates: Vec<PlannedCrossingDelay>,
    /// A compiled crossing was found, but the required movement/position
    /// proof was unavailable. Its missing estimate is not zero delay.
    pub incomplete_crossings: Vec<[u64; 2]>,
    /// A contributing move has only one nominal passage in this window;
    /// not enough evidence for this recurring-flow approximation.
    pub sparse_crossings: Vec<[u64; 2]>,
}

#[derive(Default)]
struct Load {
    starts: u64,
    squared_starts: f64,
}

impl World {
    /// Inspect nominal crossing passage during [start, start + 1 h).
    /// No reservation delays are included; this is not measured throughput.
    /// Only confirmed compiled reservations traversed at most once per defined
    /// direction of a recurring service supply a reading. Empty does not mean
    /// no possible obstructions.
    /// Invalidated and older unprotected operation supply no readings.
    pub fn preview_planned_crossing_delay(
        &self,
        start_ms: u64,
    ) -> Result<PlannedCrossingDelayPreview, PlannedCrossingDelayError> {
        let end_ms = start_ms
            .checked_add(HOUR_MS)
            .ok_or(PlannedCrossingDelayError::WindowOverflow)?;
        let Some(operation) = self.operation().filter(|o| !o.invalidated) else {
            return Ok(PlannedCrossingDelayPreview::default());
        };
        let mut budget = SCAN_BUDGET;
        let mut take = |count| {
            budget = budget
                .checked_sub(count)
                .ok_or(PlannedCrossingDelayError::WorkBudgetExceeded)?;
            Ok(())
        };
        let mut inputs = None;
        let mut site_positions = BTreeMap::new();
        let mut loads = BTreeMap::<[u64; 2], [[Load; 2]; 2]>::new();
        let mut incomplete = BTreeSet::new();
        let mut sparse = BTreeSet::new();
        for config in &operation.configs {
            take(1)?;
            let Some(spec) = operation
                .runtime
                .service_spec(&config.service_id.0.to_string())
            else {
                continue;
            };
            // A ServiceId is one stream per confirmed direction. The direction
            // is not in its single-track resource key. Use the two real stops
            // on the eligible isolated alignment instead of assuming direction.
            let mut route_crossings = BTreeMap::new();
            let mut entry_ms = Some(0u64);
            for (leg_index, leg) in spec.legs.iter().enumerate() {
                take(1)?;
                let timing = operation
                    .runtime
                    .unblocked_leg_timing(&spec.service_id, leg_index);
                for section in &leg.sections {
                    take(1)?;
                    take(section.resources.len())?;
                    let mut lane_ids = section
                        .resources
                        .iter()
                        .filter_map(|key| inline_lane_key(key));
                    let alignment = match (lane_ids.next(), lane_ids.next()) {
                        (Some((id, 0, 0)), None) => Some(id),
                        _ => None,
                    };
                    for key in &section.resources {
                        let Some(pair) = crossing_alignments(key) else {
                            continue;
                        };
                        if inputs.is_none() {
                            let mut stations = BTreeMap::new();
                            for position in &self.data.station_placements {
                                take(1)?;
                                stations.insert(position.station_id.0, position);
                            }
                            let mut alignments = BTreeMap::new();
                            for alignment in &self.data.alignments {
                                take(1)?;
                                alignments.insert(alignment.id.0, alignment);
                            }
                            inputs = Some((stations, alignments));
                        }
                        let (stations, alignments) = inputs.as_ref().expect("inputs initialized");
                        let positions = *site_positions.entry(pair).or_insert_with(|| {
                            let a = alignments.get(&pair[0])?;
                            let b = alignments.get(&pair[1])?;
                            if [a, b]
                                .iter()
                                .any(|a| a.track_type != TrackType::Normal || a.points.len() != 2)
                            {
                                return None;
                            }
                            crossing(a, b).map(|(a, b)| [a, b])
                        });
                        let movement = alignment.and_then(|id| {
                            // This first implementation proves the marker on
                            // a single physical section, not arbitrary profiles.
                            if leg.sections.len() != 1 {
                                return None;
                            }
                            let side = pair.iter().position(|p| *p == id)?;
                            let from = stations.get(&leg.from.parse::<u64>().ok()?)?;
                            let to = stations.get(&leg.to.parse::<u64>().ok()?)?;
                            if from.alignment_id.0 != id
                                || to.alignment_id.0 != id
                                || from.position == to.position
                            {
                                return None;
                            }
                            let fraction =
                                (positions?[side] - from.position) / (to.position - from.position);
                            let time = uniform_section_passage_ms(section, &spec.vehicle, fraction)
                                .ok()?;
                            (time < timing?.0).then_some(())?;
                            Some((
                                side,
                                usize::from(to.position > from.position),
                                entry_ms?.checked_add(time)?,
                            ))
                        });
                        if let Some((side, direction, offset)) = movement {
                            let count = window_passages(&spec.timetable, start_ms, end_ms, offset);
                            if route_crossings
                                .insert((pair, side, direction), count)
                                .is_some()
                            {
                                incomplete.insert(pair);
                            }
                            if count == 1 {
                                sparse.insert(pair);
                            }
                        } else {
                            incomplete.insert(pair);
                        }
                    }
                }
                entry_ms = entry_ms.and_then(|entered| {
                    let (run, stationary) = timing?;
                    entered.checked_add(run)?.checked_add(stationary)
                });
            }
            for ((pair, side, direction), passages) in route_crossings {
                let load = &mut loads.entry(pair).or_default()[side][direction];
                load.starts += passages;
                load.squared_starts += (passages as f64).powi(2);
            }
        }
        let mut out = Vec::new();
        for (pair, sides) in loads {
            if incomplete.contains(&pair) || sparse.contains(&pair) {
                continue;
            }
            for (side, approaches) in sides.iter().enumerate() {
                let other = &sides[1 - side];
                let crossing_starts = other.iter().map(|d| d.starts).sum::<u64>();
                for (direction, own) in approaches.iter().enumerate() {
                    if own.starts == 0 || crossing_starts == 0 {
                        continue;
                    }
                    let trains = (own.starts + crossing_starts) as f64;
                    let squares =
                        own.squared_starts + other.iter().map(|d| d.squared_starts).sum::<f64>();
                    let utilization = trains * EXPOSURE_SECONDS / 3600.0;
                    let mix = 1.0 - squares / trains.powi(2);
                    out.push(PlannedCrossingDelay {
                        alignment_ids: pair,
                        approach_alignment_id: pair[side],
                        towards_alignment_end: direction == 1,
                        scheduled_passages: [own.starts, crossing_starts],
                        requested_utilization: utilization,
                        estimated_delay_seconds_per_train: delay_seconds(utilization, mix),
                    });
                }
            }
        }
        Ok(PlannedCrossingDelayPreview {
            estimates: out,
            sparse_crossings: sparse.difference(&incomplete).copied().collect(),
            incomplete_crossings: incomplete.into_iter().collect(),
        })
    }
}

fn crossing_alignments(key: &str) -> Option<[u64; 2]> {
    let (lo, hi) = key.strip_prefix("rail:crossing:")?.split_once(':')?;
    Some([lo.parse().ok()?, hi.parse().ok()?])
}

fn window_passages(table: &Timetable, start_ms: u64, end_ms: u64, offset_ms: u64) -> u64 {
    let Some(end_ms) = end_ms.checked_sub(offset_ms).filter(|end| *end > 0) else {
        return 0;
    };
    scheduled_starts(table, start_ms.saturating_sub(offset_ms), end_ms)
}

fn scheduled_starts(table: &Timetable, start_ms: u64, end_ms: u64) -> u64 {
    let lo = start_ms.max(table.first_departure_ms);
    let hi = (end_ms - 1).min(table.last_departure_ms);
    if lo > hi || table.headway_ms == 0 {
        return 0;
    }
    let first_index = (lo - table.first_departure_ms).div_ceil(table.headway_ms);
    let last_index = (hi - table.first_departure_ms) / table.headway_ms;
    last_index.checked_sub(first_index).map_or(0, |n| n + 1)
}

fn delay_seconds(utilization: f64, mix: f64) -> f64 {
    // A timetable-spaced queue: limited random arrival mix, linear continuation
    // near saturation and fluid backlog accumulated over the inspection hour.
    let knee: f64 = 0.9;
    let slack = 1.0 - knee;
    let congestion = if utilization <= knee {
        utilization / (1.0 - utilization)
    } else {
        knee / slack + (utilization - knee) / slack.powi(2)
    };
    let backlog = if utilization > 1.0 {
        1800.0 * (1.0 - utilization.recip())
    } else {
        0.0
    };
    EXPOSURE_SECONDS * 0.2 * mix * congestion + backlog
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_nominal_starts_in_a_half_open_window_and_keeps_inclusive_last() {
        let table = Timetable {
            first_departure_ms: 1_000,
            last_departure_ms: 3_601_000,
            headway_ms: 1_200_000,
        };
        assert_eq!(scheduled_starts(&table, 1_000, 3_601_000), 3);
        assert_eq!(scheduled_starts(&table, 1_001, 3_601_001), 3);
        assert_eq!(scheduled_starts(&table, 3_601_000, 7_201_000), 1);
        assert_eq!(scheduled_starts(&table, 3_601_001, 7_201_001), 0);
        let many = Timetable {
            first_departure_ms: 0,
            last_departure_ms: u64::MAX,
            headway_ms: 1,
        };
        assert_eq!(
            scheduled_starts(&many, u64::MAX - HOUR_MS, u64::MAX),
            HOUR_MS
        );
    }

    #[test]
    fn earlier_starts_can_cross_at_window_start_but_not_at_window_end() {
        let table = Timetable {
            first_departure_ms: HOUR_MS - 10_000,
            last_departure_ms: HOUR_MS + 10_000,
            headway_ms: 10_000,
        };
        assert_eq!(window_passages(&table, 0, HOUR_MS, 10_000), 0);
        assert_eq!(
            window_passages(&table, HOUR_MS, HOUR_MS + 10_000, 10_000),
            1
        );
        assert_eq!(window_passages(&table, 0, 2, 2), 0);
        assert_eq!(window_passages(&table, 0, 1, u64::MAX), 0);
    }

    #[test]
    fn position_marker_does_not_insert_a_station_stop() {
        use crate::operation::{Section, Vehicle};
        let section = Section {
            length_m: 1400.0,
            speed_limit_kmh: 72.0,
            resources: Vec::new(),
        };
        let vehicle = Vehicle {
            max_speed_kmh: 72.0,
            acceleration_mps2: 1.0,
            braking_mps2: 1.0,
            length_m: 40.0,
        };
        for (fraction, time_ms) in [(1.0 / 14.0, 14_143), (0.5, 45_000), (13.0 / 14.0, 75_858)] {
            assert_eq!(
                uniform_section_passage_ms(&section, &vehicle, fraction).unwrap(),
                time_ms
            );
        }
    }

    #[test]
    fn overload_continues_finitely_and_does_not_clamp_the_asked_rates() {
        assert!((delay_seconds(0.4, 0.5) - 8.0).abs() < 1e-9);
        assert!((delay_seconds(0.9, 0.5) - 108.0).abs() < 1e-9);
        assert!((delay_seconds(1.0, 0.5) - 228.0).abs() < 1e-9);
        assert!(delay_seconds(1.2, 0.5) > delay_seconds(1.0, 0.5));
    }
}
