//! Explicitly bounded, single-direction scheduled OD scenario. This is neither
//! observed congestion replay nor a timetable source or a nationwide assignment.
use super::{
    assign_fixed_od_budgeted, ChoiceWeights, FixedOd, OdAssignment, PassengerLeg,
    MAX_BATCH_OUTPUT_ENTRIES, MAX_BATCH_WORK, MAX_PATH_STATIONS,
};
use serde::Serialize;

const MAX_SAFE_CLOCK_MS: u64 = (1 << 53) - 1;
const MAX_QUERY_SPAN_MS: u64 = 24 * 60 * 60 * 1000;

pub(crate) fn scheduled_od_span(ready_ms: u64, end_ms: u64) -> Option<u64> {
    end_ms
        .checked_sub(ready_ms)
        .filter(|&span| span > 0 && span <= MAX_QUERY_SPAN_MS && end_ms <= MAX_SAFE_CLOCK_MS)
}

/// Trusted schedule producer supplies its exact first train and count *at the
/// boarding stop* covering [ready_ms, end_ms). No verified coverage is unknown,
/// not a zero-service claim, and must be passed as Unverified instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledBoarding {
    Unverified,
    Verified {
        ready_ms: u64,
        end_ms: u64,
        first_departure_ms: Option<u64>,
        scheduled_departures: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduledCorridorStatus {
    Scheduled,
    NoDepartureInWindow,
    UnverifiedSchedule,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScheduledCorridorAssignment {
    pub status: ScheduledCorridorStatus,
    pub first_departure_ms: Option<u64>,
    pub scheduled_departures: Option<u64>,
    pub assignment: OdAssignment,
}

/// Compare one selected through train corridor and the OD's existing outside
/// alternative. `ready_ms` is a representative platform-ready clock instant,
/// not a distribution by trip count; access/egress still contribute walk cost.
/// No departure before the selected cutoff does not mean there is no later
/// service. Walk links, alternative rail paths and train-to-train transfers are
/// deliberately excluded. Board/stop continuity and the weighted allocator
/// are shared with the regular fixed-OD calculation.
pub fn assign_scheduled_corridor(
    od: &FixedOd,
    corridor: &[PassengerLeg],
    schedule: ScheduledBoarding,
    weights: &ChoiceWeights,
) -> Result<ScheduledCorridorAssignment, String> {
    let first = corridor.first().ok_or("empty scheduled corridor")?;
    let last = corridor.last().ok_or("empty scheduled corridor")?;
    let (origin, destination) = match (od.access.as_slice(), od.egress.as_slice()) {
        ([origin], [destination]) => (origin, destination),
        _ => return Err("scheduled corridor requires one boarding and alighting station".into()),
    };
    if corridor.len() >= MAX_PATH_STATIONS
        || origin.station != first.from
        || destination.station != last.to
        || !first.board_allowed
        || !last.alight_allowed
        || corridor
            .iter()
            .any(|leg| leg.service_id != first.service_id)
        || corridor.windows(2).any(|pair| {
            pair[0].to != pair[1].from || pair[0].sequence.checked_add(1) != Some(pair[1].sequence)
        })
    {
        return Err("unsupported scheduled corridor".into());
    }
    let mut visited = std::collections::BTreeSet::from([first.from.as_str()]);
    if corridor.iter().any(|leg| !visited.insert(leg.to.as_str())) {
        return Err("scheduled corridor revisits a station".into());
    }
    let (status, first_departure_ms, scheduled_departures, wait_seconds) = match schedule {
        ScheduledBoarding::Unverified => {
            (ScheduledCorridorStatus::UnverifiedSchedule, None, None, 0.0)
        }
        ScheduledBoarding::Verified {
            ready_ms,
            end_ms,
            first_departure_ms,
            scheduled_departures,
        } => {
            let span = scheduled_od_span(ready_ms, end_ms).ok_or("invalid scheduled OD window")?;
            match (first_departure_ms, scheduled_departures) {
                (None, 0) => (
                    ScheduledCorridorStatus::NoDepartureInWindow,
                    None,
                    Some(0),
                    0.0,
                ),
                (Some(at), n)
                    if (ready_ms..end_ms).contains(&at)
                        && n > 0
                        && n <= span
                        && n <= end_ms - at =>
                {
                    (
                        ScheduledCorridorStatus::Scheduled,
                        Some(at),
                        Some(n),
                        (at - ready_ms) as f64 / 1000.0,
                    )
                }
                _ => return Err("scheduled OD first departure/count mismatch".into()),
            }
        }
    };
    // Run the same bounded solver even for no service in order to certify the
    // supplied corridor and inputs, then discard the non-operating candidate.
    let mut assignment = assign_fixed_od_budgeted(
        std::slice::from_ref(od),
        corridor,
        &[],
        weights,
        MAX_PATH_STATIONS + 1,
        (MAX_BATCH_WORK, MAX_BATCH_OUTPUT_ENTRIES),
        Some(wait_seconds),
    )?
    .pop()
    .expect("one fixed OD in scope");
    if status != ScheduledCorridorStatus::Scheduled {
        assignment.rail.clear();
        assignment.partial_candidates.clear();
        assignment.alternative_trips = 0.0;
        assignment.unserved_trips = 0.0;
        assignment.unresolved_trips = 0.0;
        let trips = od.trips.unwrap_or(0.0);
        if status == ScheduledCorridorStatus::UnverifiedSchedule {
            assignment.search_complete = false;
            assignment.unresolved_trips = trips;
        } else if od.alternative_seconds.is_some() {
            assignment.alternative_trips = trips;
        } else {
            assignment.unserved_trips = trips;
        }
    }
    Ok(ScheduledCorridorAssignment {
        status,
        first_departure_ms,
        scheduled_departures,
        assignment,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demand::{assign_fixed_od, MarketKind, StationAccess};

    fn fixture() -> (FixedOd, [PassengerLeg; 2]) {
        let od = FixedOd {
            id: "metro-home-to-core".into(),
            origin_region: "metro".into(),
            destination_region: "metro".into(),
            owner_region: "metro".into(),
            origin_location: "residential".into(),
            destination_location: "centre".into(),
            market: MarketKind::Metro,
            trips: Some(100.0),
            access: vec![StationAccess {
                station: "11".into(),
                seconds: 60.0,
            }],
            egress: vec![StationAccess {
                station: "13".into(),
                seconds: 40.0,
            }],
            alternative_seconds: Some(1140.0),
        };
        let leg = |sequence, from: &str, to: &str, ride_seconds, through_seconds| PassengerLeg {
            leg_id: format!("train:{sequence}"),
            sequence,
            board_allowed: sequence == 0,
            alight_allowed: sequence == 1,
            service_id: "train".into(),
            from: from.into(),
            to: to.into(),
            ride_seconds,
            through_seconds,
            headway_seconds: 600.0,
        };
        (
            od,
            [
                leg(0, "11", "12", 200.0, 0.0),
                leg(1, "12", "13", 245.0, 35.0),
            ],
        )
    }

    fn allocation_sum(a: &OdAssignment) -> f64 {
        a.rail.iter().map(|p| p.trips).sum::<f64>()
            + a.alternative_trips
            + a.unserved_trips
            + a.unresolved_trips
    }

    fn inspect(
        od: &FixedOd,
        legs: &[PassengerLeg],
        schedule: ScheduledBoarding,
    ) -> ScheduledCorridorAssignment {
        assign_scheduled_corridor(od, legs, schedule, &ChoiceWeights::default()).unwrap()
    }

    #[test]
    fn same_scheduled_corridor_peak_wait_and_frequency_shift_share_without_extra_dwell() {
        let (od, legs) = fixture();
        let ready_ms = 7 * 3_600_000 + 60_000;
        let end_ms = ready_ms + 1_800_000;
        let band = |wait_ms, scheduled_departures| ScheduledBoarding::Verified {
            ready_ms,
            end_ms,
            scheduled_departures,
            first_departure_ms: Some(ready_ms + wait_ms),
        };
        let ordinary = inspect(&od, &legs, band(540_000, 3));
        let peak = inspect(&od, &legs, band(240_000, 6));
        let at_departure = inspect(&od, &legs, band(0, 6));
        let default = assign_fixed_od(&[od], &legs, &[], &ChoiceWeights::default(), 100).unwrap();
        assert_eq!(
            default[0].rail[0].time.wait_seconds, 300.0,
            "ordinary half-headway stays unchanged"
        );
        assert_eq!(ordinary.scheduled_departures, Some(3));
        assert_eq!(peak.scheduled_departures, Some(6));
        for (scenario, wait_seconds) in [(&ordinary, 540.0), (&peak, 240.0), (&at_departure, 0.0)] {
            let result = &scenario.assignment;
            assert_eq!(scenario.status, ScheduledCorridorStatus::Scheduled);
            assert_eq!(result.rail[0].time.wait_seconds, wait_seconds);
            assert_eq!(result.rail[0].time.ride_seconds, 480.0);
            assert_eq!(result.rail[0].time.transfers, 0);
            assert_eq!(result.rail[0].generalized_seconds, 580.0 + wait_seconds);
            assert!((allocation_sum(result) - 100.0).abs() < 1e-8);
        }
        assert!(peak.assignment.rail[0].trips > ordinary.assignment.rail[0].trips);
        assert!(peak.assignment.alternative_trips < ordinary.assignment.alternative_trips);
    }

    #[test]
    fn known_empty_horizon_and_unverified_schedule_are_distinct_intercity_results() {
        let (mut od, legs) = fixture();
        od.market = MarketKind::Intercity;
        od.destination_region = "intercity destination".into();
        let off = ScheduledBoarding::Verified {
            ready_ms: 1_000,
            end_ms: 61_000,
            scheduled_departures: 0,
            first_departure_ms: None,
        };
        let none = inspect(&od, &legs, off);
        assert_eq!(none.status, ScheduledCorridorStatus::NoDepartureInWindow);
        assert!(none.assignment.search_complete);
        assert!(none.assignment.rail.is_empty());
        assert_eq!(none.assignment.alternative_trips, 100.0);
        assert_eq!(none.assignment.unserved_trips, 0.0);
        let unknown = inspect(&od, &legs, ScheduledBoarding::Unverified);
        assert_eq!(unknown.status, ScheduledCorridorStatus::UnverifiedSchedule);
        assert!(!unknown.assignment.search_complete);
        assert_eq!(unknown.scheduled_departures, None);
        assert_eq!(unknown.assignment.alternative_trips, 0.0);
        assert_eq!(unknown.assignment.unresolved_trips, 100.0);
        od.alternative_seconds = None;
        assert_eq!(inspect(&od, &legs, off).assignment.unserved_trips, 100.0);
        od.trips = None;
        let unknown_trips = inspect(&od, &legs, off);
        assert_eq!(unknown_trips.assignment.potential_trips, None);
        assert_eq!(allocation_sum(&unknown_trips.assignment), 0.0);
        od.owner_region = "incorrect".into();
        assert!(assign_scheduled_corridor(&od, &legs, off, &ChoiceWeights::default()).is_err());
    }

    #[test]
    fn wrong_pattern_pass_stop_and_cutoff_never_authorize_boarding() {
        let (od, mut legs) = fixture();
        let end = ScheduledBoarding::Verified {
            ready_ms: 5_000,
            end_ms: 10_000,
            scheduled_departures: 1,
            first_departure_ms: Some(10_000),
        };
        let verify = |legs: &[PassengerLeg], schedule| {
            assign_scheduled_corridor(&od, legs, schedule, &ChoiceWeights::default())
        };
        assert!(verify(&legs, end).is_err());
        let offered = ScheduledBoarding::Verified {
            ready_ms: 5_000,
            end_ms: 10_000,
            scheduled_departures: 1,
            first_departure_ms: Some(9_999),
        };
        assert!(verify(&legs, offered).is_ok());
        legs[0].board_allowed = false;
        assert!(verify(&legs, offered).is_err());
        legs[0].board_allowed = true;
        legs[1].service_id = "other".into();
        assert!(verify(&legs, offered).is_err());
    }
}
