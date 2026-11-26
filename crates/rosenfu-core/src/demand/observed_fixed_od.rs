//! Explicit single-OD cohort from one short simulated rail replay.
//! Not an extrapolation of observed delays, capacity, or a timetable average.
use super::{ChoiceScores, ChoiceWeights};
use crate::operation::{PassengerReplay, ReplayItinerary, ReplayItineraryStatus};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservedFixedOdStatus {
    Reached,
    NoArrivalWithinWindow,
    Incomplete,
    OutsideBudget,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ObservedFixedOd {
    pub status: ObservedFixedOdStatus,
    pub potential_trips: Option<f64>,
    pub rail_trips: f64,
    pub alternative_trips: f64,
    pub unresolved_trips: f64,
    /// Includes the regular fixed-OD transfer penalty, not another delay.
    pub generalized_rail_seconds: Option<f64>,
    /// Never invented from configuration or a truncated replay.
    pub itinerary: Option<ReplayItinerary>,
}

impl ObservedFixedOd {
    pub fn outside_budget(trips: Option<f64>) -> Self {
        Self::unresolved(ObservedFixedOdStatus::OutsideBudget, trips, None)
    }

    fn unresolved(
        status: ObservedFixedOdStatus,
        trips: Option<f64>,
        itinerary: Option<ReplayItinerary>,
    ) -> Self {
        Self {
            status,
            potential_trips: trips,
            rail_trips: 0.,
            alternative_trips: 0.,
            unresolved_trips: trips.unwrap_or(0.),
            generalized_rail_seconds: None,
            itinerary,
        }
    }

    pub fn from_replay(
        replay: &PassengerReplay,
        origin: &str,
        destination: &str,
        trips: Option<f64>,
        alternative_seconds: Option<f64>,
    ) -> Self {
        let journey = replay.earliest_rail_itinerary(origin, destination);
        match journey.status {
            ReplayItineraryStatus::Incomplete => {
                return Self::unresolved(ObservedFixedOdStatus::Incomplete, trips, Some(journey));
            }
            ReplayItineraryStatus::NoArrivalWithinWindow => {
                // An omitted arrival may occur after the window; never assign
                // all trips to a substitute or claim no rail service.
                return Self::unresolved(
                    ObservedFixedOdStatus::NoArrivalWithinWindow,
                    trips,
                    Some(journey),
                );
            }
            ReplayItineraryStatus::Reached => {}
        }
        let w = ChoiceWeights::default();
        let cost = journey.platform_wait_ms as f64 / 1000. * w.wait
            + journey.onboard_ms as f64 / 1000. * w.ride
            + journey.rides.len().saturating_sub(1) as f64 * w.transfer_penalty_seconds;
        let shares = ChoiceScores::new(
            std::iter::once(cost),
            alternative_seconds,
            w.choice_scale_seconds,
        )
        .expect("one supported rail itinerary");
        let number = trips.unwrap_or(0.);
        Self {
            status: ObservedFixedOdStatus::Reached,
            potential_trips: trips,
            rail_trips: number * shares.share(cost),
            alternative_trips: number * shares.alternative_share(),
            unresolved_trips: 0.,
            generalized_rail_seconds: Some(cost),
            itinerary: Some(journey),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::{ReplayedPassengerLeg, TrainKey};
    const MIN: u64 = 60_000;

    fn leg(
        service: &str,
        run: u64,
        sequence: usize,
        from: &str,
        to: &str,
        entry: u64,
        arrival: u64,
    ) -> ReplayedPassengerLeg {
        ReplayedPassengerLeg {
            train: TrainKey {
                service_id: service.into(),
                scheduled_ms: run * MIN,
            },
            sequence,
            from: from.into(),
            to: to.into(),
            entered_ms: entry * MIN,
            arrived_ms: arrival * MIN,
            cumulative_reservation_wait_ms: 0,
        }
    }
    fn scenario(a_arrival: u64) -> PassengerReplay {
        let mut inbound = leg("1", 1, 0, "A", "B", 1, a_arrival);
        inbound.cumulative_reservation_wait_ms = (a_arrival - 9) * MIN;
        PassengerReplay {
            observed_from_ms: 0,
            requested_until_ms: 30 * MIN,
            observed_until_ms: 30 * MIN,
            complete: true,
            completed_legs: vec![
                inbound,
                leg("2", 11, 0, "B", "C", 11, 19),
                leg("2", 21, 0, "B", "C", 21, 29),
            ],
        }
    }
    #[test]
    fn sole_delay_misses_transfer_shifts_demand_and_is_not_added_again() {
        let assign =
            |at| ObservedFixedOd::from_replay(&scenario(at), "A", "C", Some(100.), Some(26. * 60.));
        let clear = assign(9);
        let delayed = assign(12);
        assert!(clear.rail_trips > 50. && delayed.rail_trips < 50.);
        for (r, arrival, wait, ride, weighted) in
            [(clear, 19, 3, 16, 24), (delayed, 29, 10, 19, 34)]
        {
            assert_eq!(r.status, ObservedFixedOdStatus::Reached);
            assert!((r.rail_trips + r.alternative_trips + r.unresolved_trips - 100.).abs() < 1e-12);
            let journey = r.itinerary.unwrap();
            assert_eq!(journey.arrival_ms, Some(arrival * MIN));
            assert_eq!(journey.platform_wait_ms, wait * MIN);
            assert_eq!(journey.onboard_ms, ride * MIN);
            assert_eq!(r.generalized_rail_seconds, Some(weighted as f64 * 60.));
        }
    }
    #[test]
    fn same_train_dwell_is_onboard_and_partial_or_finite_scope_does_not_allocate() {
        let mut replay = scenario(9);
        replay.completed_legs = vec![
            leg("1", 1, 0, "A", "B", 1, 9),
            leg("1", 1, 1, "B", "C", 12, 20),
        ];
        let assign =
            |r: &PassengerReplay| ObservedFixedOd::from_replay(r, "A", "C", Some(50.), Some(1200.));
        let exact = assign(&replay);
        let trip = exact.itinerary.unwrap();
        assert_eq!(trip.rides.len(), 1);
        assert_eq!(trip.platform_wait_ms, MIN);
        assert_eq!(trip.onboard_ms, 19 * MIN);
        assert_eq!(exact.generalized_rail_seconds, Some(1200.));
        assert_eq!(exact.rail_trips, 25.);
        replay.complete = false;
        replay.observed_until_ms = 25 * MIN; // even includes a valid path; don't allocate
        let not_complete = assign(&replay);
        assert_eq!(not_complete.status, ObservedFixedOdStatus::Incomplete);
        replay.complete = true;
        replay.observed_from_ms = 2 * MIN; // cannot board a train already out of A
        let no_arrival = assign(&replay);
        assert_eq!(
            no_arrival.status,
            ObservedFixedOdStatus::NoArrivalWithinWindow
        );
        for r in [
            not_complete,
            no_arrival,
            ObservedFixedOd::outside_budget(Some(50.)),
        ] {
            assert_eq!(
                (r.rail_trips, r.alternative_trips, r.unresolved_trips),
                (0., 0., 50.)
            );
        }
        let unknown = ObservedFixedOd::from_replay(&scenario(9), "A", "C", None, Some(26. * 60.));
        assert_eq!(unknown.potential_trips, None);
        assert_eq!(
            (
                unknown.rail_trips,
                unknown.alternative_trips,
                unknown.unresolved_trips
            ),
            (0., 0., 0.)
        );
    }
}
