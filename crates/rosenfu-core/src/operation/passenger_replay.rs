//! One earliest-arrival railway itinerary in an observed simulation window.
//! Not a fixed-OD input, a traveler count or a real-world travel forecast.
use super::{PassengerReplay, ReplayedPassengerLeg, TrainKey};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayItineraryStatus {
    Reached,
    NoArrivalWithinWindow,
    Incomplete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReplayRide {
    pub service_id: String,
    pub from: String,
    pub to: String,
    pub departed_ms: u64,
    pub arrived_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReplayItinerary {
    pub version: u8,
    pub ready_ms: u64,
    pub observed_until_ms: u64,
    pub requested_until_ms: u64,
    pub status: ReplayItineraryStatus,
    pub arrival_ms: Option<u64>,
    pub platform_wait_ms: u64,
    pub onboard_ms: u64,
    pub rides: Vec<ReplayRide>,
}

/// Same-station rail transfers only. Count a fixed allowance before boarding;
/// do not add it again to elapsed platform time. No station-name matching.
pub const REPLAY_TRANSFER_ALLOWANCE_MS: u64 = 60_000;

#[derive(Clone, Copy)]
struct Label<'a> {
    previous: Option<usize>,
    leg: &'a ReplayedPassengerLeg,
    continuing: bool,
}

impl PassengerReplay {
    /// Find the earliest arrival at the selected station by timestamp, not by the
    /// configured headway. Only the supplied simulated observations are consulted.
    pub fn earliest_rail_itinerary(&self, origin: &str, destination: &str) -> ReplayItinerary {
        let ready_ms = self.observed_from_ms;
        let mut out = ReplayItinerary {
            version: 1,
            ready_ms,
            observed_until_ms: self.observed_until_ms,
            requested_until_ms: self.requested_until_ms,
            status: if self.complete {
                ReplayItineraryStatus::NoArrivalWithinWindow
            } else {
                ReplayItineraryStatus::Incomplete
            },
            arrival_ms: None,
            platform_wait_ms: 0,
            onboard_ms: 0,
            rides: Vec::new(),
        };
        // A partial replay is conservative: don't present a selected route as
        // authoritative when omitted arrivals could change it.
        if !self.complete || origin == destination {
            return out;
        }
        let mut order: Vec<_> = self
            .completed_legs
            .iter()
            .filter(|leg| {
                leg.entered_ms >= ready_ms
                    && leg.entered_ms < leg.arrived_ms
                    && leg.arrived_ms <= self.observed_until_ms
                    && leg.train.service_id.parse::<u64>().is_ok()
            })
            .collect();
        order.sort_unstable_by_key(|leg| (leg.entered_ms, &leg.train, leg.sequence));
        let mut labels: Vec<Label<'_>> = Vec::new();
        let mut best_at: BTreeMap<&str, usize> = BTreeMap::new();
        let mut aboard: BTreeMap<&TrainKey, usize> = BTreeMap::new();
        for leg in order {
            let continuation = aboard.get(&leg.train).copied().filter(|&index| {
                let prior = labels[index].leg;
                prior.sequence.checked_add(1) == Some(leg.sequence)
                    && prior.to == leg.from
                    && prior.arrived_ms <= leg.entered_ms
            });
            let transfer = best_at.get(leg.from.as_str()).copied().filter(|&index| {
                labels[index]
                    .leg
                    .arrived_ms
                    .checked_add(REPLAY_TRANSFER_ALLOWANCE_MS)
                    .is_some_and(|boarding_ready| boarding_ready <= leg.entered_ms)
            });
            let (previous, continuing) = match (leg.from == origin, continuation, transfer) {
                (true, _, _) => (None, false),
                (false, Some(index), _) => (Some(index), true),
                (false, None, Some(index)) => (Some(index), false),
                (false, None, None) => continue,
            };
            let index = labels.len();
            labels.push(Label {
                previous,
                leg,
                continuing,
            });
            aboard.insert(&leg.train, index);
            if best_at
                .get(leg.to.as_str())
                .is_none_or(|&prior| leg.arrived_ms < labels[prior].leg.arrived_ms)
            {
                best_at.insert(leg.to.as_str(), index);
            }
        }
        let Some(&last) = best_at.get(destination) else {
            return out;
        };
        let mut itinerary = Vec::new();
        let mut index = Some(last);
        while let Some(i) = index {
            itinerary.push(labels[i]);
            index = labels[i].previous;
        }
        itinerary.reverse();
        let mut previous_arrival = ready_ms;
        for label in itinerary {
            let leg = label.leg;
            if label.continuing {
                // Own-train hold and dwell are onboard time, not another
                // boarding or half-headway wait.
                out.onboard_ms += leg.arrived_ms - previous_arrival;
                let ride = out
                    .rides
                    .last_mut()
                    .expect("a continuation has a prior ride");
                ride.to = leg.to.clone();
                ride.arrived_ms = leg.arrived_ms;
            } else {
                out.platform_wait_ms += leg.entered_ms - previous_arrival;
                out.onboard_ms += leg.arrived_ms - leg.entered_ms;
                out.rides.push(ReplayRide {
                    service_id: leg.train.service_id.clone(),
                    from: leg.from.clone(),
                    to: leg.to.clone(),
                    departed_ms: leg.entered_ms,
                    arrived_ms: leg.arrived_ms,
                });
            }
            previous_arrival = leg.arrived_ms;
        }
        out.status = ReplayItineraryStatus::Reached;
        out.arrival_ms = Some(previous_arrival);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const MIN: u64 = 60_000;
    fn leg(
        service: &str,
        seq: usize,
        from: &str,
        to: &str,
        start: u64,
        end: u64,
    ) -> ReplayedPassengerLeg {
        ReplayedPassengerLeg {
            train: TrainKey {
                service_id: service.into(),
                scheduled_ms: if service == "1" { MIN } else { start * MIN },
            },
            sequence: seq,
            from: from.into(),
            to: to.into(),
            entered_ms: start * MIN,
            arrived_ms: end * MIN,
            cumulative_reservation_wait_ms: 0,
        }
    }
    fn replay(legs: Vec<ReplayedPassengerLeg>, end: u64) -> PassengerReplay {
        PassengerReplay {
            observed_from_ms: 0,
            requested_until_ms: end * MIN,
            observed_until_ms: end * MIN,
            complete: true,
            completed_legs: legs,
        }
    }
    #[test]
    fn delayed_first_ride_misses_connection_without_double_counting() {
        let scenario = |at_b, wait| {
            let mut a = leg("1", 0, "A", "B", 1, at_b);
            a.cumulative_reservation_wait_ms = wait;
            replay(
                vec![
                    leg("2", 0, "B", "C", 21, 29),
                    a,
                    leg("2", 0, "B", "C", 11, 19),
                ],
                30,
            )
            .earliest_rail_itinerary("A", "C")
        };
        let normal = scenario(9, 0);
        let delayed = scenario(12, 3 * MIN);
        assert_eq!(normal.arrival_ms, Some(19 * MIN));
        assert_eq!(delayed.arrival_ms, Some(29 * MIN));
        assert_eq!(normal.platform_wait_ms, 3 * MIN);
        assert_eq!(delayed.platform_wait_ms, 10 * MIN);
        assert_eq!(normal.onboard_ms, 16 * MIN);
        assert_eq!(delayed.onboard_ms, 19 * MIN);
        assert_eq!(delayed.platform_wait_ms + delayed.onboard_ms, 29 * MIN);
        assert_eq!(delayed.rides[1].departed_ms, 21 * MIN);
    }
    #[test]
    fn staying_aboard_an_intermediate_hold_is_not_a_transfer() {
        let db = replay(
            vec![leg("1", 0, "A", "B", 1, 9), leg("1", 1, "B", "C", 12, 20)],
            30,
        );
        let through = db.earliest_rail_itinerary("A", "C");
        let board_late = db.earliest_rail_itinerary("B", "C");
        assert_eq!(
            (
                through.platform_wait_ms,
                through.onboard_ms,
                through.rides.len()
            ),
            (MIN, 19 * MIN, 1)
        );
        assert_eq!(
            (board_late.platform_wait_ms, board_late.onboard_ms),
            (12 * MIN, 8 * MIN)
        );
    }
    #[test]
    fn truncation_and_left_censored_departure_do_not_prove_unserved() {
        let mut db = replay(
            vec![leg("1", 0, "A", "B", 1, 9), leg("2", 0, "B", "C", 11, 19)],
            18,
        );
        let status = |db: &PassengerReplay| db.earliest_rail_itinerary("A", "C").status;
        assert_eq!(status(&db), ReplayItineraryStatus::NoArrivalWithinWindow);
        db.observed_until_ms = 17 * MIN;
        db.complete = false;
        assert_eq!(status(&db), ReplayItineraryStatus::Incomplete);
        db.complete = true;
        db.requested_until_ms = 30 * MIN;
        db.observed_until_ms = 30 * MIN;
        db.observed_from_ms = 2 * MIN;
        assert_eq!(status(&db), ReplayItineraryStatus::NoArrivalWithinWindow);
    }
    #[test]
    fn earlier_arrival_can_board_when_allowance_exact_and_bus_is_not_used() {
        let db = replay(
            vec![
                leg("bus:x", 0, "A", "C", 1, 8),
                leg("1", 0, "A", "B", 1, 9),
                leg("2", 0, "B", "C", 10, 11),
                leg("3", 0, "A", "C", 2, 20),
            ],
            30,
        );
        assert_eq!(
            db.earliest_rail_itinerary("A", "C").arrival_ms,
            Some(11 * MIN)
        );
    }
}
