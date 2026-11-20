//! Alpha 0.2 fixed-OD reference assignment. Synthetic inputs, not population data.
//! No mutation, charging or capacity simulation occurs here. Recompute after network changes.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PassengerLeg {
    pub service_id: String,
    pub from: String,
    pub to: String,
    /// Scheduled running time, including applicable intermediate dwell, in seconds.
    pub ride_seconds: f64,
    /// Expected wait is half the headway; this is not exact departure-time routing.
    pub headway_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WalkLink {
    pub from: String,
    pub to: String,
    pub seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StationAccess {
    pub station: String,
    pub seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixedOd {
    /// Unique record ID across mesh and zone inputs: never add both representations.
    pub id: String,
    pub origin_region: String,
    pub destination_region: String,
    /// None is unknown, Some(0) is known zero. This is trips per input period, not population.
    pub trips: Option<f64>,
    pub access: Vec<StationAccess>,
    pub egress: Vec<StationAccess>,
    /// Aggregated non-rail generalized time; None means no known alternative.
    pub alternative_seconds: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChoiceWeights {
    pub walk: f64,
    pub wait: f64,
    pub ride: f64,
    pub transfer_penalty_seconds: f64,
    /// Multinomial-logit scale in generalized seconds; tunable game coefficient.
    pub choice_scale_seconds: f64,
}
impl Default for ChoiceWeights {
    fn default() -> Self {
        Self {
            walk: 1.0,
            wait: 1.0,
            ride: 1.0,
            transfer_penalty_seconds: 300.0,
            choice_scale_seconds: 600.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathAllocation {
    pub stations: Vec<String>,
    /// Boarding sequence, not one entry per edge. Transfers increase boardings, not OD trips.
    pub services: Vec<String>,
    pub generalized_seconds: f64,
    pub trips: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OdAssignment {
    pub id: String,
    pub potential_trips: Option<f64>,
    pub rail: Vec<PathAllocation>,
    pub alternative_trips: f64,
    pub unserved_trips: f64,
    /// No available candidate yet and exploration was cut off; not proven unserved.
    pub unresolved_trips: f64,
    pub search_complete: bool,
}

fn nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

#[derive(Clone)]
struct Edge {
    to: String,
    service: Option<String>,
    seconds: f64,
    headway: f64,
}
#[derive(Clone)]
struct State {
    station: String,
    visited: Vec<String>,
    services: Vec<String>,
    current_service: Option<String>,
    cost: f64,
}

/// Reference full recalculation. Regions label OD ownership, never constrain route search.
/// `max_states_per_od` bounds work, not geography; incomplete output must be labelled approximate.
/// Candidate paths are simple station paths, with a logit share per distinct station/service path.
/// No capacity, real statistics, fare charging, four-purpose demand or multiplayer is implied.
pub fn assign_fixed_od(
    ods: &[FixedOd],
    legs: &[PassengerLeg],
    walks: &[WalkLink],
    weights: &ChoiceWeights,
    max_states_per_od: usize,
) -> Result<Vec<OdAssignment>, String> {
    if ![
        weights.walk,
        weights.wait,
        weights.ride,
        weights.transfer_penalty_seconds,
    ]
    .into_iter()
    .all(nonnegative)
        || !weights.choice_scale_seconds.is_finite()
        || weights.choice_scale_seconds <= 0.0
        || max_states_per_od == 0
    {
        return Err("invalid demand weights or search budget".into());
    }
    let mut graph: BTreeMap<String, Vec<Edge>> = BTreeMap::new();
    let mut keys = BTreeSet::new();
    for leg in legs {
        if leg.from.is_empty()
            || leg.to.is_empty()
            || leg.service_id.is_empty()
            || leg.from == leg.to
            || !nonnegative(leg.ride_seconds)
            || !leg.headway_seconds.is_finite()
            || leg.headway_seconds <= 0.0
        {
            return Err("invalid passenger leg".into());
        }
        if !keys.insert((
            leg.from.clone(),
            leg.to.clone(),
            Some(leg.service_id.clone()),
        )) {
            return Err("duplicate passenger leg".into());
        }
        graph.entry(leg.from.clone()).or_default().push(Edge {
            to: leg.to.clone(),
            service: Some(leg.service_id.clone()),
            seconds: leg.ride_seconds,
            headway: leg.headway_seconds,
        });
    }
    for link in walks {
        if link.from.is_empty()
            || link.to.is_empty()
            || link.from == link.to
            || !nonnegative(link.seconds)
        {
            return Err("invalid walking link".into());
        }
        if !keys.insert((link.from.clone(), link.to.clone(), None)) {
            return Err("duplicate walking link".into());
        }
        graph.entry(link.from.clone()).or_default().push(Edge {
            to: link.to.clone(),
            service: None,
            seconds: link.seconds,
            headway: 0.0,
        });
    }
    for edges in graph.values_mut() {
        edges.sort_by(|a, b| (&a.to, &a.service).cmp(&(&b.to, &b.service)));
    }
    let mut ids = BTreeSet::new();
    let mut results = Vec::new();
    for od in ods {
        if od.id.trim().is_empty()
            || !ids.insert(&od.id)
            || od.origin_region.trim().is_empty()
            || od.destination_region.trim().is_empty()
            || od.trips.is_some_and(|n| !nonnegative(n))
            || od.alternative_seconds.is_some_and(|n| !nonnegative(n))
        {
            return Err("invalid or duplicate fixed OD".into());
        }
        let access = validate_access(&od.access)?;
        let egress = validate_access(&od.egress)?;
        let mut result = OdAssignment {
            id: od.id.clone(),
            potential_trips: od.trips,
            rail: Vec::new(),
            alternative_trips: 0.0,
            unserved_trips: 0.0,
            unresolved_trips: 0.0,
            search_complete: true,
        };
        // Unknown is preserved; a known zero does not require expensive exploration.
        let Some(trips) = od.trips.filter(|n| *n > 0.0) else {
            results.push(result);
            continue;
        };
        let mut stack: Vec<_> = access
            .iter()
            .rev()
            .map(|(station, &seconds)| State {
                station: station.clone(),
                visited: vec![station.clone()],
                services: vec![],
                current_service: None,
                cost: seconds * weights.walk,
            })
            .collect();
        let mut states = 0;
        while let Some(state) = stack.pop() {
            if states >= max_states_per_od {
                result.search_complete = false;
                break;
            }
            states += 1;
            if let Some(seconds) = egress.get(&state.station) {
                if !state.services.is_empty() {
                    let cost = state.cost + seconds * weights.walk;
                    if !cost.is_finite() {
                        return Err("demand cost overflow".into());
                    }
                    result.rail.push(PathAllocation {
                        stations: state.visited.clone(),
                        services: state.services.clone(),
                        generalized_seconds: cost,
                        trips: 0.0,
                    });
                }
            }
            if let Some(edges) = graph.get(&state.station) {
                for edge in edges.iter().rev() {
                    if state.visited.contains(&edge.to) {
                        continue;
                    }
                    let mut next = state.clone();
                    next.station = edge.to.clone();
                    next.visited.push(edge.to.clone());
                    if let Some(service) = &edge.service {
                        next.cost += edge.seconds * weights.ride;
                        if state.current_service.as_ref() != Some(service) {
                            next.cost += edge.headway * 0.5 * weights.wait;
                            if !state.services.is_empty() {
                                next.cost += weights.transfer_penalty_seconds;
                            }
                            next.services.push(service.clone());
                        }
                    } else {
                        next.cost += edge.seconds * weights.walk;
                    }
                    next.current_service = edge.service.clone();
                    if !next.cost.is_finite() {
                        return Err("demand cost overflow".into());
                    }
                    stack.push(next);
                }
            }
        }
        // Stable ordering makes the calculation independent of incoming graph ordering.
        result.rail.sort_by(|a, b| {
            a.generalized_seconds
                .total_cmp(&b.generalized_seconds)
                .then_with(|| a.stations.cmp(&b.stations))
                .then_with(|| a.services.cmp(&b.services))
        });
        let best = result
            .rail
            .iter()
            .map(|p| p.generalized_seconds)
            .chain(od.alternative_seconds)
            .min_by(f64::total_cmp);
        if let Some(best) = best {
            let score = |cost: f64| (-(cost - best) / weights.choice_scale_seconds).exp();
            let other = od.alternative_seconds.map(score).unwrap_or(0.0);
            let total = result
                .rail
                .iter()
                .map(|p| score(p.generalized_seconds))
                .sum::<f64>()
                + other;
            for path in &mut result.rail {
                path.trips = trips * (score(path.generalized_seconds) / total);
            }
            result.alternative_trips = trips * (other / total);
        } else if result.search_complete {
            result.unserved_trips = trips;
        } else {
            result.unresolved_trips = trips;
        }
        results.push(result);
    }
    Ok(results)
}

fn validate_access(items: &[StationAccess]) -> Result<BTreeMap<String, f64>, String> {
    let mut result = BTreeMap::new();
    for access in items {
        if access.station.is_empty()
            || !nonnegative(access.seconds)
            || result
                .insert(access.station.clone(), access.seconds)
                .is_some()
        {
            return Err("invalid or duplicate station access".into());
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn access(station: u64) -> StationAccess {
        StationAccess {
            station: station.to_string(),
            seconds: 0.0,
        }
    }
    fn od() -> FixedOd {
        FixedOd {
            id: "synthetic-city-a-to-city-b".into(),
            origin_region: "region-a".into(),
            destination_region: "region-b".into(),
            trips: Some(100.0),
            access: vec![access(1)],
            egress: vec![access(3)],
            alternative_seconds: Some(1800.0),
        }
    }
    fn leg(from: u64, to: u64, service: u64, seconds: f64) -> PassengerLeg {
        PassengerLeg {
            from: from.to_string(),
            to: to.to_string(),
            service_id: service.to_string(),
            ride_seconds: seconds,
            headway_seconds: 600.0,
        }
    }
    fn evaluate(ods: &[FixedOd], legs: &[PassengerLeg]) -> Vec<OdAssignment> {
        assign_fixed_od(ods, legs, &[], &ChoiceWeights::default(), 1000).unwrap()
    }
    fn conserved(result: &OdAssignment) {
        let sum = result.rail.iter().map(|p| p.trips).sum::<f64>()
            + result.alternative_trips
            + result.unserved_trips
            + result.unresolved_trips;
        assert!((sum - result.potential_trips.unwrap()).abs() < 1e-9);
    }
    #[test]
    fn no_rail_preserves_potential_and_distinguishes_unknown_zero_and_unserved() {
        let mut unknown = od();
        unknown.id = "unknown".into();
        unknown.trips = None;
        let mut zero = od();
        zero.id = "zero".into();
        zero.trips = Some(0.0);
        let mut isolated = od();
        isolated.id = "isolated".into();
        isolated.alternative_seconds = None;
        let result = evaluate(&[od(), unknown, zero, isolated], &[]);
        assert_eq!(result[0].alternative_trips, 100.0);
        assert_eq!(result[1].potential_trips, None);
        assert_eq!(result[2].potential_trips, Some(0.0));
        assert_eq!(result[3].unserved_trips, 100.0);
        conserved(&result[0]);
        conserved(&result[2]);
        conserved(&result[3]);
    }
    #[test]
    fn direct_continuation_waits_once_and_is_directional() {
        let legs = [leg(1, 2, 1, 300.0), leg(2, 3, 1, 300.0)];
        let result = evaluate(&[od()], &legs);
        assert_eq!(result[0].rail[0].generalized_seconds, 900.0);
        conserved(&result[0]);
        let mut reverse = od();
        reverse.access = vec![access(3)];
        reverse.egress = vec![access(1)];
        assert!(evaluate(&[reverse], &legs)[0].rail.is_empty());
    }
    #[test]
    fn transfer_and_explicit_walk_do_not_invent_physical_connections() {
        let legs = [leg(1, 2, 1, 300.0), leg(4, 3, 2, 300.0)];
        assert!(evaluate(&[od()], &legs)[0].rail.is_empty());
        let r = assign_fixed_od(
            &[od()],
            &legs,
            &[WalkLink {
                from: "2".into(),
                to: "4".into(),
                seconds: 120.0,
            }],
            &ChoiceWeights::default(),
            100,
        )
        .unwrap();
        assert_eq!(r[0].rail[0].services.len(), 2);
        assert_eq!(r[0].rail[0].generalized_seconds, 1620.0);
        conserved(&r[0]);
    }
    #[test]
    fn frequency_new_line_and_stop_changes_react_monotonically() {
        let slow = leg(1, 3, 1, 1600.0);
        let before = evaluate(&[od()], &[slow.clone()])[0].alternative_trips;
        let mut frequent = slow.clone();
        frequent.headway_seconds = 60.0;
        assert!(evaluate(&[od()], &[frequent])[0].alternative_trips < before);
        assert!(evaluate(&[od()], &[slow, leg(1, 3, 2, 500.0)])[0].alternative_trips < before);
        // Removing a served destination is not silently treated as a stop.
        assert_eq!(
            evaluate(&[od()], &[leg(1, 2, 1, 500.0)])[0].alternative_trips,
            100.0
        );
    }
    #[test]
    fn unrelated_frequency_is_not_merged_and_ring_terminates() {
        let legs = [
            leg(1, 3, 1, 600.0),
            leg(1, 2, 2, 100.0),
            leg(2, 1, 2, 100.0),
        ];
        assert_eq!(evaluate(&[od()], &legs), evaluate(&[od()], &legs[..1]));
    }
    #[test]
    fn parallel_choices_multiple_access_and_cross_region_survive() {
        let mut demand = od();
        demand.access.push(StationAccess {
            station: "2".into(),
            seconds: 180.0,
        });
        let legs = [leg(1, 3, 1, 900.0), leg(2, 3, 2, 600.0)];
        let r = evaluate(&[demand], &legs);
        assert_eq!(r[0].rail.len(), 2);
        conserved(&r[0]);
        assert!(r[0].rail.iter().all(|p| p.trips > 0.0));
    }
    #[test]
    fn illustrative_long_distance_is_not_population_data() {
        let mut demand = od();
        demand.id = "illustrative-long-distance-not-observed".into();
        demand.trips = Some(10000.0);
        demand.alternative_seconds = Some(18000.0);
        let slow = evaluate(&[demand.clone()], &[leg(1, 3, 1, 24000.0)]);
        let fast = evaluate(&[demand], &[leg(1, 3, 1, 9000.0)]);
        assert!(fast[0].rail[0].trips > slow[0].rail[0].trips);
        conserved(&fast[0]);
    }
    #[test]
    fn bounded_search_reports_unknown_reachability_not_false_no_path() {
        let mut demand = od();
        demand.alternative_seconds = None;
        let r = assign_fixed_od(
            &[demand],
            &[leg(1, 2, 1, 10.0), leg(2, 3, 1, 10.0)],
            &[],
            &ChoiceWeights::default(),
            1,
        )
        .unwrap();
        assert!(!r[0].search_complete);
        assert_eq!(r[0].unresolved_trips, 100.0);
        assert_eq!(r[0].unserved_trips, 0.0);
        conserved(&r[0]);
    }
    #[test]
    fn weights_are_adjustable_and_recalculation_is_pure_and_order_independent() {
        let legs = [leg(1, 3, 2, 900.0), leg(1, 3, 1, 600.0)];
        let a = evaluate(&[od()], &legs);
        let b = evaluate(&[od()], &[legs[1].clone(), legs[0].clone()]);
        assert_eq!(a, b);
        let weights = ChoiceWeights {
            ride: 2.0,
            ..ChoiceWeights::default()
        };
        let weighted = assign_fixed_od(&[od()], &legs, &[], &weights, 1000).unwrap();
        assert!(weighted[0].alternative_trips > a[0].alternative_trips);
        assert_eq!(a, evaluate(&[od()], &legs));
        conserved(&weighted[0]);
    }
    #[test]
    fn invalid_inputs_duplicate_od_and_overflow_are_rejected() {
        assert!(assign_fixed_od(&[od(), od()], &[], &[], &ChoiceWeights::default(), 100).is_err());
        let mut bad = od();
        bad.trips = Some(f64::NAN);
        assert!(assign_fixed_od(&[bad], &[], &[], &ChoiceWeights::default(), 100).is_err());
        let duplicate = leg(1, 3, 1, 10.0);
        assert!(assign_fixed_od(
            &[od()],
            &[duplicate.clone(), duplicate],
            &[],
            &ChoiceWeights::default(),
            100
        )
        .is_err());
        let weights = ChoiceWeights {
            ride: f64::MAX,
            ..ChoiceWeights::default()
        };
        assert!(assign_fixed_od(&[od()], &[leg(1, 3, 1, 2.0)], &[], &weights, 100).is_err());
    }
}
