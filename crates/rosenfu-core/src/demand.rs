//! Alpha 0.2 fixed-OD reference assignment. Synthetic inputs, not population data.
//! No mutation, charging or capacity simulation occurs here. Recompute after network changes.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PassengerLeg {
    pub leg_id: String,
    pub sequence: usize,
    pub board_allowed: bool,
    pub alight_allowed: bool,
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
pub enum MarketKind {
    Metro,
    Intercity,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixedOd {
    /// Unique record ID across mesh and zone inputs: never add both representations.
    pub id: String,
    pub origin_region: String,
    pub destination_region: String,
    pub origin_location: String,
    pub destination_location: String,
    pub owner_region: String,
    pub market: MarketKind,
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
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PassengerStep {
    Rail { leg_id: String },
    Walk { from: String, to: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathAllocation {
    pub stations: Vec<String>,
    pub leg_ids: Vec<String>,
    pub steps: Vec<PassengerStep>,
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
    /// Diagnostic paths only, zero allocated trips. Not authoritative when search is incomplete.
    pub partial_candidates: Vec<PathAllocation>,
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
    leg_id: Option<String>,
    sequence: usize,
    board_allowed: bool,
    alight_allowed: bool,
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
    next_sequence: usize,
    can_alight: bool,
    leg_ids: Vec<String>,
    steps: Vec<PassengerStep>,
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
        || max_states_per_od > 100_000
    {
        return Err("invalid demand weights or search budget".into());
    }
    let mut graph: BTreeMap<String, Vec<Edge>> = BTreeMap::new();
    let mut keys = BTreeSet::new();
    let mut leg_ids = BTreeSet::new();
    let mut patterns: BTreeMap<&str, BTreeMap<usize, &PassengerLeg>> = BTreeMap::new();
    for leg in legs {
        if leg.leg_id.is_empty()
            || !leg_ids.insert(&leg.leg_id)
            || leg.sequence == usize::MAX
            || leg.from.is_empty()
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
        if patterns
            .entry(&leg.service_id)
            .or_default()
            .insert(leg.sequence, leg)
            .is_some()
        {
            return Err("duplicate service sequence".into());
        }
        checked_cost(leg.ride_seconds * weights.ride)?;
        checked_cost(leg.headway_seconds * 0.5 * weights.wait)?;
        graph.entry(leg.from.clone()).or_default().push(Edge {
            leg_id: Some(leg.leg_id.clone()),
            sequence: leg.sequence,
            board_allowed: leg.board_allowed,
            alight_allowed: leg.alight_allowed,
            to: leg.to.clone(),
            service: Some(leg.service_id.clone()),
            seconds: leg.ride_seconds,
            headway: leg.headway_seconds,
        });
    }
    for pattern in patterns.values() {
        let ordered: Vec<_> = pattern.values().collect();
        for (position, leg) in ordered.iter().enumerate() {
            if leg.sequence != position || (position > 0 && ordered[position - 1].to != leg.from) {
                return Err("non-contiguous directed service pattern".into());
            }
        }
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
        checked_cost(link.seconds * weights.walk)?;
        graph.entry(link.from.clone()).or_default().push(Edge {
            leg_id: None,
            sequence: 0,
            board_allowed: true,
            alight_allowed: true,
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
    let mut markets = BTreeSet::new();
    let mut results = Vec::new();
    for od in ods {
        if od.id.trim().is_empty()
            || !ids.insert(&od.id)
            || od.origin_region.trim().is_empty()
            || od.destination_region.trim().is_empty()
            || od.origin_location.is_empty()
            || od.destination_location.is_empty()
            || od.owner_region != od.origin_region
            || !markets.insert((&od.origin_location, &od.destination_location))
            || ((od.market == MarketKind::Metro) != (od.origin_region == od.destination_region))
            || od.trips.is_some_and(|n| !nonnegative(n) || n > 1e12)
            || od.alternative_seconds.is_some_and(|n| !nonnegative(n))
        {
            return Err("invalid or duplicate fixed OD".into());
        }
        let access = validate_access(&od.access)?;
        let egress = validate_access(&od.egress)?;
        for seconds in access.values().chain(egress.values()) {
            checked_cost(seconds * weights.walk)?;
        }
        let mut result = OdAssignment {
            id: od.id.clone(),
            potential_trips: od.trips,
            rail: Vec::new(),
            partial_candidates: Vec::new(),
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
            .take(max_states_per_od)
            .map(|(station, &seconds)| State {
                station: station.clone(),
                visited: vec![station.clone()],
                services: vec![],
                current_service: None,
                next_sequence: 0,
                can_alight: true,
                leg_ids: vec![],
                steps: vec![],
                cost: seconds * weights.walk,
            })
            .collect();
        if access.len() > max_states_per_od {
            stack.truncate(max_states_per_od);
            result.search_complete = false;
        }
        let mut states = 0;
        let mut edges_examined = 0;
        'search: while let Some(state) = stack.pop() {
            if states >= max_states_per_od {
                result.search_complete = false;
                break;
            }
            states += 1;
            if let Some(seconds) = egress.get(&state.station) {
                if !state.services.is_empty() && state.can_alight {
                    let cost = state.cost + seconds * weights.walk;
                    if !cost.is_finite() {
                        return Err("demand cost overflow".into());
                    }
                    result.rail.push(PathAllocation {
                        stations: state.visited.clone(),
                        leg_ids: state.leg_ids.clone(),
                        steps: state.steps.clone(),
                        services: state.services.clone(),
                        generalized_seconds: cost,
                        trips: 0.0,
                    });
                }
            }
            if let Some(edges) = graph.get(&state.station) {
                for edge in edges.iter().rev() {
                    if edges_examined >= max_states_per_od {
                        result.search_complete = false;
                        break 'search;
                    }
                    edges_examined += 1;
                    let same_service =
                        edge.service.is_some() && edge.service == state.current_service;
                    // At a closed pattern's terminal, the next departure is a new boarding,
                    // not a free same-train continuation across the timetable boundary.
                    let terminal_reboard = same_service
                        && edge.sequence == 0
                        && edge
                            .service
                            .as_deref()
                            .and_then(|id| patterns.get(id))
                            .is_some_and(|pattern| state.next_sequence == pattern.len());
                    let continuation = same_service && !terminal_reboard;
                    if continuation {
                        if edge.sequence != state.next_sequence {
                            continue;
                        }
                    } else if !state.can_alight || (edge.service.is_some() && !edge.board_allowed) {
                        continue;
                    }
                    if state.visited.contains(&edge.to) {
                        continue;
                    }
                    let mut next = state.clone();
                    next.station = edge.to.clone();
                    next.visited.push(edge.to.clone());
                    if let Some(service) = &edge.service {
                        next.cost += edge.seconds * weights.ride;
                        if !continuation {
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
                    next.next_sequence = edge.sequence + 1;
                    next.can_alight = edge.alight_allowed;
                    if let Some(id) = &edge.leg_id {
                        next.leg_ids.push(id.clone());
                        next.steps.push(PassengerStep::Rail { leg_id: id.clone() });
                    }
                    if edge.leg_id.is_none() {
                        next.steps.push(PassengerStep::Walk {
                            from: state.station.clone(),
                            to: edge.to.clone(),
                        });
                    }
                    if !next.cost.is_finite() {
                        return Err("demand cost overflow".into());
                    }
                    if stack.len() >= max_states_per_od {
                        result.search_complete = false;
                        break 'search;
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
        if !result.search_complete {
            result.partial_candidates = std::mem::take(&mut result.rail);
            result.unresolved_trips = trips;
            results.push(result);
            continue;
        }
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

fn checked_cost(value: f64) -> Result<(), String> {
    if !nonnegative(value) {
        Err("demand cost overflow".into())
    } else {
        Ok(())
    }
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
            origin_location: "cell-a".into(),
            destination_location: "cell-b".into(),
            owner_region: "region-a".into(),
            market: MarketKind::Intercity,
            trips: Some(100.0),
            access: vec![access(1)],
            egress: vec![access(3)],
            alternative_seconds: Some(1800.0),
        }
    }
    fn leg(from: u64, to: u64, service: u64, seconds: f64) -> PassengerLeg {
        PassengerLeg {
            leg_id: format!("{service}:{from}:{to}"),
            sequence: 0,
            board_allowed: true,
            alight_allowed: true,
            from: from.to_string(),
            to: to.to_string(),
            service_id: service.to_string(),
            ride_seconds: seconds,
            headway_seconds: 600.0,
        }
    }
    fn evaluate(ods: &[FixedOd], legs: &[PassengerLeg]) -> Vec<OdAssignment> {
        let mut legs = legs.to_vec();
        let mut sequence = BTreeMap::new();
        for leg in &mut legs {
            let value = sequence.entry(leg.service_id.clone()).or_insert(0);
            leg.sequence = *value;
            *value += 1;
        }
        assign_fixed_od(ods, &legs, &[], &ChoiceWeights::default(), 1000).unwrap()
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
        unknown.origin_location = "unknown-cell".into();
        unknown.trips = None;
        let mut zero = od();
        zero.id = "zero".into();
        zero.origin_location = "zero-cell".into();
        zero.trips = Some(0.0);
        let mut isolated = od();
        isolated.id = "isolated".into();
        isolated.origin_location = "isolated-cell".into();
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
        let before = evaluate(&[od()], std::slice::from_ref(&slow))[0].alternative_trips;
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
            &[
                leg(1, 2, 1, 10.0),
                PassengerLeg {
                    sequence: 1,
                    ..leg(2, 3, 1, 10.0)
                },
            ],
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
    #[test]
    fn rejects_wrong_order_same_service_instead_of_free_continuation() {
        let legs = [
            leg(1, 2, 1, 10.0),
            PassengerLeg {
                sequence: 1,
                ..leg(1, 3, 1, 10.0)
            },
        ];
        assert!(assign_fixed_od(&[od()], &legs, &[], &ChoiceWeights::default(), 100).is_err());
    }
    #[test]
    fn pass_through_cannot_board_alight_or_transfer_but_can_continue() {
        let legs = [
            PassengerLeg {
                alight_allowed: false,
                ..leg(1, 2, 1, 10.0)
            },
            PassengerLeg {
                sequence: 1,
                board_allowed: false,
                ..leg(2, 3, 1, 10.0)
            },
            leg(2, 3, 2, 1.0),
        ];
        let through = assign_fixed_od(&[od()], &legs, &[], &ChoiceWeights::default(), 100).unwrap();
        assert_eq!(through[0].rail.len(), 1);
        assert_eq!(through[0].rail[0].leg_ids, vec!["1:1:2", "1:2:3"]);
        let mut stop = od();
        stop.egress = vec![access(2)];
        assert!(
            assign_fixed_od(&[stop], &legs, &[], &ChoiceWeights::default(), 100).unwrap()[0]
                .rail
                .is_empty()
        );
        let mut start = od();
        start.access = vec![access(2)];
        let r = assign_fixed_od(&[start], &legs, &[], &ChoiceWeights::default(), 100).unwrap();
        assert!(r[0].rail.iter().all(|p| p.services == vec!["2"]));
    }
    #[test]
    fn cutoff_after_a_valid_path_quarantines_all_allocation() {
        let legs = [leg(1, 3, 1, 1.0), leg(1, 4, 2, 1.0), leg(4, 5, 3, 1.0)];
        let r = assign_fixed_od(&[od()], &legs, &[], &ChoiceWeights::default(), 2).unwrap();
        assert!(!r[0].search_complete);
        assert!(r[0].rail.is_empty());
        assert!(!r[0].partial_candidates.is_empty());
        assert!(r[0].partial_candidates.iter().all(|p| p.trips == 0.0));
        assert_eq!(r[0].alternative_trips, 0.0);
        assert_eq!(r[0].unresolved_trips, 100.0);
        conserved(&r[0]);
    }
    #[test]
    fn fanout_and_initial_frontier_are_hard_bounded() {
        let legs: Vec<_> = (2..100).map(|n| leg(1, n, n, 1.0)).collect();
        let r = assign_fixed_od(&[od()], &legs, &[], &ChoiceWeights::default(), 4).unwrap();
        assert!(!r[0].search_complete);
        assert!(r[0].partial_candidates.len() <= 4);
        conserved(&r[0]);
        let mut many = od();
        many.access = (1..100).map(access).collect();
        let r = assign_fixed_od(&[many], &[], &[], &ChoiceWeights::default(), 4).unwrap();
        assert!(!r[0].search_complete);
        conserved(&r[0]);
    }
    #[test]
    fn duplicate_market_with_distinct_record_ids_is_rejected() {
        let mut duplicate = od();
        duplicate.id = "different-source-id".into();
        assert!(
            assign_fixed_od(&[od(), duplicate], &[], &[], &ChoiceWeights::default(), 100).is_err()
        );
        let mut wrong_owner = od();
        wrong_owner.owner_region = "region-b".into();
        assert!(assign_fixed_od(&[wrong_owner], &[], &[], &ChoiceWeights::default(), 100).is_err());
    }
    #[test]
    fn disjoint_metro_intercity_ownership_preserves_partition_results() {
        let intercity = od();
        let mut local = od();
        local.id = "local".into();
        local.destination_region = "region-a".into();
        local.destination_location = "local-b".into();
        local.market = MarketKind::Metro;
        let mut remote = od();
        remote.id = "remote-local".into();
        remote.origin_location = "remote-a".into();
        remote.destination_location = "remote-b".into();
        remote.origin_region = "region-b".into();
        remote.destination_region = "region-b".into();
        remote.owner_region = "region-b".into();
        remote.market = MarketKind::Metro;
        let ods = [intercity, local, remote];
        let legs = [leg(1, 3, 1, 20.0)];
        let full = evaluate(&ods, &legs);
        let partition: Vec<_> = ods
            .iter()
            .filter(|od| od.owner_region == "region-a")
            .cloned()
            .collect();
        assert_eq!(evaluate(&partition, &legs), full[..2]);
        for r in full {
            conserved(&r);
        }
    }
    #[test]
    fn inaccessible_overflow_and_large_counts_are_checked() {
        let mut unreachable = od();
        unreachable.access[0].seconds = f64::MAX;
        let weights = ChoiceWeights {
            walk: 2.0,
            ..ChoiceWeights::default()
        };
        assert!(assign_fixed_od(&[unreachable], &[], &[], &weights, 10).is_err());
        let mut large = od();
        large.trips = Some(1e12);
        let r = evaluate(&[large], &[leg(1, 3, 1, 100.0), leg(1, 3, 2, 200.0)]);
        let sum = r[0].rail.iter().map(|p| p.trips).sum::<f64>() + r[0].alternative_trips;
        assert!((sum - 1e12).abs() < 0.001);
    }
    #[test]
    fn closed_pattern_terminal_reboarding_is_not_free_continuation() {
        let legs = [
            leg(1, 2, 1, 10.0),
            PassengerLeg {
                sequence: 1,
                ..leg(2, 3, 1, 10.0)
            },
            PassengerLeg {
                sequence: 2,
                ..leg(3, 1, 1, 10.0)
            },
        ];
        let mut demand = od();
        demand.access = vec![access(3)];
        demand.egress = vec![access(2)];
        let r = assign_fixed_od(&[demand], &legs, &[], &ChoiceWeights::default(), 100).unwrap();
        assert_eq!(r[0].rail[0].services, vec!["1", "1"]);
        assert_eq!(r[0].rail[0].generalized_seconds, 920.0);
        conserved(&r[0]);
    }
}
