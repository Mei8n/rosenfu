//! Alpha 0.2 fixed-OD reference assignment. Synthetic inputs, not population data.
//! No mutation, charging or capacity simulation occurs here. Recompute after network changes.
mod observed_fixed_od;
mod scheduled_corridor;
pub use observed_fixed_od::{ObservedFixedOd, ObservedFixedOdStatus};
#[cfg(feature = "standard-periodic-rail")]
pub(crate) use scheduled_corridor::scheduled_od_span;
pub use scheduled_corridor::{
    assign_scheduled_corridor, ScheduledBoarding, ScheduledCorridorAssignment,
    ScheduledCorridorStatus,
};

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
    /// Scheduled stationary time counted only if already on this same run.
    #[serde(default)]
    pub through_seconds: f64,
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

/// Optional player-supplied costs. These affect a preview only, never the ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChoicePrices {
    pub public_fare_yen: u32,
    pub alternative_cost_yen: u32,
    pub value_of_time_yen_per_hour: u32,
    /// None preserves the shared public fare.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bus_fare_yen: Option<u32>,
    /// At most one selected boarding service; never inferred from shared track.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_adjustment: Option<ServiceFareAdjustment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceFareAdjustment {
    pub service_id: String,
    pub delta_yen: i32,
}

impl ChoicePrices {
    fn seconds(&self, yen: u32) -> f64 {
        f64::from(yen) * 3600.0 / f64::from(self.value_of_time_yen_per_hour)
    }

    fn validate(&self) -> Result<(), String> {
        if self.public_fare_yen > 1_000_000
            || self.bus_fare_yen.is_some_and(|fare| fare > 1_000_000)
            || self.alternative_cost_yen > 1_000_000
            || !(1..=1_000_000).contains(&self.value_of_time_yen_per_hour)
        {
            return Err(
                "運賃・代替費用は0〜100万円、時間価値は1〜100万円/時の整数を入力してください。"
                    .into(),
            );
        }
        if let Some(adjustment) = &self.service_adjustment {
            let fare =
                i64::from(self.base_fare(&adjustment.service_id)) + i64::from(adjustment.delta_yen);
            if !identifier(&adjustment.service_id)
                || !(-1_000_000..=1_000_000).contains(&adjustment.delta_yen)
                || !(0..=1_000_000).contains(&fare)
            {
                return Err("系統別の加算・割引後の運賃を0〜100万円にしてください。".into());
            }
        }
        Ok(())
    }

    fn base_fare(&self, service: &str) -> u32 {
        if service.starts_with("bus:") {
            self.bus_fare_yen.unwrap_or(self.public_fare_yen)
        } else {
            self.public_fare_yen
        }
    }

    fn fare(&self, service: &str) -> u32 {
        let delta = self
            .service_adjustment
            .as_ref()
            .filter(|a| a.service_id == service)
            .map_or(0, |a| a.delta_yen);
        // validate() has already bounded the effective fare.
        self.base_fare(service).saturating_add_signed(delta)
    }

    fn uses_custom_fares(&self) -> bool {
        self.bus_fare_yen.is_some() || self.service_adjustment.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PassengerStep {
    Rail { leg_id: String },
    Walk { from: String, to: String },
}

/// Physical estimates, separate from weighted choice cost and transfer penalty.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct PathTimeBreakdown {
    pub walk_seconds: f64,
    pub wait_seconds: f64,
    pub ride_seconds: f64,
    pub transfers: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathAllocation {
    pub stations: Vec<String>,
    pub leg_ids: Vec<String>,
    pub steps: Vec<PassengerStep>,
    /// Boarding sequence, not one entry per edge. Transfers increase boardings, not OD trips.
    pub services: Vec<String>,
    /// Mode-specific or adjusted fares in boarding order; absent for legacy shared pricing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boarding_fares_yen: Option<Vec<u32>>,
    pub generalized_seconds: f64,
    pub time: PathTimeBreakdown,
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
    /// Whole OD quarantined after incomplete exploration; not proven unserved.
    pub unresolved_trips: f64,
    pub search_complete: bool,
}

// Small reference solver, not a nationwide production search. Bound the bytes
// represented by each charged entry as well as the number of cloned entries.
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_PATH_STATIONS: usize = 128;
const MAX_OD_ROWS: usize = 10_000;
const MAX_GRAPH_EDGES: usize = 100_000;
const MAX_ACCESS_ROWS: usize = 100_000;
const MAX_BATCH_WORK: usize = 1_000_000;
const MAX_BATCH_OUTPUT_ENTRIES: usize = 50_000;

fn identifier(value: &str) -> bool {
    value.len() <= MAX_IDENTIFIER_BYTES && !value.trim().is_empty()
}

fn spend(left: &mut usize, units: usize) -> bool {
    if let Some(remaining) = left.checked_sub(units) {
        *left = remaining;
        true
    } else {
        *left = 0;
        false
    }
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
    boarding_wait: f64,
    through: f64,
}
#[derive(Clone, Copy)]
enum SearchStep<'a> {
    Rail { leg_id: &'a str },
    Walk { from: &'a str, to: &'a str },
}

impl SearchStep<'_> {
    fn into_passenger_step(self) -> PassengerStep {
        match self {
            Self::Rail { leg_id } => PassengerStep::Rail {
                leg_id: leg_id.to_owned(),
            },
            Self::Walk { from, to } => PassengerStep::Walk {
                from: from.to_owned(),
                to: to.to_owned(),
            },
        }
    }
}

// Network and access identifiers outlive the OD search. Own strings only in
// returned paths; branching must not copy every ID in the path so far.
#[derive(Clone)]
struct State<'a> {
    station: &'a str,
    visited: Vec<&'a str>,
    services: Vec<&'a str>,
    current_service: Option<&'a str>,
    next_sequence: usize,
    can_alight: bool,
    leg_ids: Vec<&'a str>,
    steps: Vec<SearchStep<'a>>,
    cost: f64,
    time: PathTimeBreakdown,
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
    assign_fixed_od_with_prices(ods, legs, walks, weights, max_states_per_od, None)
}

pub fn assign_fixed_od_with_prices(
    ods: &[FixedOd],
    legs: &[PassengerLeg],
    walks: &[WalkLink],
    weights: &ChoiceWeights,
    max_states_per_od: usize,
    prices: Option<ChoicePrices>,
) -> Result<Vec<OdAssignment>, String> {
    assign_fixed_od_budgeted(
        ods,
        legs,
        walks,
        weights,
        max_states_per_od,
        (MAX_BATCH_WORK, MAX_BATCH_OUTPUT_ENTRIES),
        (None, prices),
    )
}

fn assign_fixed_od_budgeted(
    ods: &[FixedOd],
    legs: &[PassengerLeg],
    walks: &[WalkLink],
    weights: &ChoiceWeights,
    max_states_per_od: usize,
    (mut batch_work, mut batch_output): (usize, usize),
    (corridor_wait, prices): (Option<f64>, Option<ChoicePrices>),
) -> Result<Vec<OdAssignment>, String> {
    if let Some(prices) = &prices {
        prices.validate()?;
    }
    if ods.len() > MAX_OD_ROWS
        || legs.len().saturating_add(walks.len()) > MAX_GRAPH_EDGES
        || ods.iter().fold(0usize, |total, od| {
            total
                .saturating_add(od.access.len())
                .saturating_add(od.egress.len())
        }) > MAX_ACCESS_ROWS
    {
        return Err("demand input exceeds reference solver limits".into());
    }
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
        if !identifier(&leg.leg_id)
            || !leg_ids.insert(&leg.leg_id)
            || leg.sequence == usize::MAX
            || !identifier(&leg.from)
            || !identifier(&leg.to)
            || !identifier(&leg.service_id)
            || leg.from == leg.to
            || !nonnegative(leg.ride_seconds)
            || !nonnegative(leg.through_seconds)
            || (corridor_wait.is_none()
                && (!leg.headway_seconds.is_finite() || leg.headway_seconds <= 0.0))
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
        checked_cost((leg.ride_seconds + leg.through_seconds) * weights.ride)?;
        let boarding_wait = corridor_wait.unwrap_or(leg.headway_seconds * 0.5);
        checked_cost(boarding_wait * weights.wait)?;
        graph.entry(leg.from.clone()).or_default().push(Edge {
            leg_id: Some(leg.leg_id.clone()),
            sequence: leg.sequence,
            board_allowed: leg.board_allowed,
            alight_allowed: leg.alight_allowed,
            to: leg.to.clone(),
            service: Some(leg.service_id.clone()),
            seconds: leg.ride_seconds,
            boarding_wait,
            through: leg.through_seconds,
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
        if !identifier(&link.from)
            || !identifier(&link.to)
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
            boarding_wait: 0.0,
            through: 0.0,
        });
    }
    for edges in graph.values_mut() {
        edges.sort_by(|a, b| (&a.to, &a.service).cmp(&(&b.to, &b.service)));
    }
    let mut ids = BTreeSet::new();
    let mut markets = BTreeSet::new();
    let mut results = Vec::new();
    for od in ods {
        if !identifier(&od.id)
            || !ids.insert(&od.id)
            || !identifier(&od.origin_region)
            || !identifier(&od.destination_region)
            || !identifier(&od.origin_location)
            || !identifier(&od.destination_location)
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
        let initial_states = access.len().min(max_states_per_od);
        if initial_states > 0 && !spend(&mut batch_work, 2 * initial_states) {
            result.search_complete = false;
            result.unresolved_trips = trips;
            results.push(result);
            continue;
        }
        if batch_work == 0 {
            result.search_complete = false;
            result.unresolved_trips = trips;
            results.push(result);
            continue;
        }
        let mut stack: Vec<_> = access
            .iter()
            .rev()
            .take(max_states_per_od)
            .map(|(station, &seconds)| State {
                station: station.as_str(),
                visited: vec![station.as_str()],
                services: vec![],
                current_service: None,
                next_sequence: 0,
                can_alight: true,
                leg_ids: vec![],
                steps: vec![],
                cost: seconds * weights.walk,
                time: PathTimeBreakdown {
                    walk_seconds: seconds,
                    ..Default::default()
                },
            })
            .collect();
        if access.len() > max_states_per_od {
            result.search_complete = false;
        }
        let mut states = 0;
        let mut edges_examined = 0;
        'search: while let Some(state) = stack.pop() {
            if states >= max_states_per_od || !spend(&mut batch_work, 1) {
                result.search_complete = false;
                break;
            }
            states += 1;
            if let Some(seconds) = egress.get(state.station) {
                if !state.services.is_empty() && state.can_alight {
                    let custom_prices = prices.as_ref().filter(|p| p.uses_custom_fares());
                    // Include vectors, typed step IDs (up to two each), and record.
                    let units = 1
                        + state.visited.len()
                        + state.leg_ids.len()
                        + 3 * state.steps.len()
                        + state.services.len()
                        + custom_prices.map_or(0, |_| state.services.len());
                    if !spend(&mut batch_work, units) || !spend(&mut batch_output, units) {
                        result.search_complete = false;
                        break 'search;
                    }
                    let boarding_fares_yen: Option<Vec<_>> = custom_prices.map(|p| {
                        state
                            .services
                            .iter()
                            .map(|service| p.fare(service))
                            .collect()
                    });
                    let mut cost = state.cost + seconds * weights.walk;
                    if let Some(prices) = &prices {
                        cost += if let Some(fares) = &boarding_fares_yen {
                            fares.iter().map(|&yen| prices.seconds(yen)).sum::<f64>()
                        } else {
                            state.services.len() as f64 * prices.seconds(prices.public_fare_yen)
                        };
                    }
                    let mut time = state.time;
                    time.walk_seconds += seconds;
                    if !cost.is_finite() || !time.walk_seconds.is_finite() {
                        return Err("demand cost overflow".into());
                    }
                    result.rail.push(PathAllocation {
                        stations: state.visited.iter().map(|&id| id.to_owned()).collect(),
                        leg_ids: state.leg_ids.iter().map(|&id| id.to_owned()).collect(),
                        steps: state
                            .steps
                            .iter()
                            .map(|step| step.into_passenger_step())
                            .collect(),
                        services: state.services.iter().map(|&id| id.to_owned()).collect(),
                        boarding_fares_yen,
                        generalized_seconds: cost,
                        time,
                        trips: 0.0,
                    });
                }
            }
            if let Some(edges) = graph.get(state.station) {
                for edge in edges.iter().rev() {
                    if edges_examined >= max_states_per_od
                        || !spend(&mut batch_work, state.visited.len() + 1)
                    {
                        result.search_complete = false;
                        break 'search;
                    }
                    edges_examined += 1;
                    let same_service =
                        edge.service.is_some() && edge.service.as_deref() == state.current_service;
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
                    if state.visited.contains(&edge.to.as_str()) {
                        continue;
                    }
                    // Check before allocating any clone. An eligible unseen
                    // successor beyond the depth limit makes the OD incomplete.
                    if state.visited.len() >= MAX_PATH_STATIONS
                        || stack.len() >= max_states_per_od
                        || !spend(&mut batch_work, 8 * (state.visited.len() + 1))
                    {
                        result.search_complete = false;
                        break 'search;
                    }
                    let mut next = state.clone();
                    next.station = edge.to.as_str();
                    next.visited.push(edge.to.as_str());
                    if let Some(service) = edge.service.as_deref() {
                        let ride = edge.seconds + if continuation { edge.through } else { 0.0 };
                        next.cost += ride * weights.ride;
                        next.time.ride_seconds += ride;
                        if !continuation {
                            next.cost += edge.boarding_wait * weights.wait;
                            next.time.wait_seconds += edge.boarding_wait;
                            if !state.services.is_empty() {
                                next.cost += weights.transfer_penalty_seconds;
                                next.time.transfers += 1;
                            }
                            next.services.push(service);
                        }
                    } else {
                        next.cost += edge.seconds * weights.walk;
                        next.time.walk_seconds += edge.seconds;
                    }
                    next.current_service = edge.service.as_deref();
                    next.next_sequence = edge.sequence + 1;
                    next.can_alight = edge.alight_allowed;
                    if let Some(id) = edge.leg_id.as_deref() {
                        next.leg_ids.push(id);
                        next.steps.push(SearchStep::Rail { leg_id: id });
                    } else {
                        next.steps.push(SearchStep::Walk {
                            from: state.station,
                            to: edge.to.as_str(),
                        });
                    }
                    if !next.cost.is_finite()
                        || !next.time.ride_seconds.is_finite()
                        || !next.time.wait_seconds.is_finite()
                        || !next.time.walk_seconds.is_finite()
                    {
                        return Err("demand cost overflow".into());
                    }
                    stack.push(next);
                }
            }
        }
        // Stable ordering makes the calculation independent of incoming graph ordering.
        result
            .rail
            .sort_by(|a, b| a.generalized_seconds.total_cmp(&b.generalized_seconds));
        if !result.search_complete {
            result.partial_candidates = std::mem::take(&mut result.rail);
            result.unresolved_trips = trips;
            results.push(result);
            continue;
        }
        let alternative_cost = od.alternative_seconds.map(|seconds| {
            if let Some(prices) = &prices {
                seconds + prices.seconds(prices.alternative_cost_yen)
            } else {
                seconds
            }
        });
        if alternative_cost.is_some_and(|seconds| !seconds.is_finite()) {
            return Err("demand cost overflow".into());
        }
        if let Some(scores) = ChoiceScores::new(
            result.rail.iter().map(|p| p.generalized_seconds),
            alternative_cost,
            weights.choice_scale_seconds,
        ) {
            for path in &mut result.rail {
                path.trips = trips * scores.share(path.generalized_seconds);
            }
            result.alternative_trips = trips * scores.alternative_share();
        } else if result.search_complete {
            result.unserved_trips = trips;
        } else {
            result.unresolved_trips = trips;
        }
        results.push(result);
    }
    Ok(results)
}

/// Stable normalized choice scores with an exact best-score reference. Reused
/// unchanged for scheduled and explicitly observed choices.
struct ChoiceScores {
    best: f64,
    scale: f64,
    other: f64,
    total: f64,
}

impl ChoiceScores {
    fn new(
        rail: impl Iterator<Item = f64> + Clone,
        alternative: Option<f64>,
        scale: f64,
    ) -> Option<Self> {
        let best = rail.clone().chain(alternative).min_by(f64::total_cmp)?;
        let score = |cost: f64| (-(cost - best) / scale).exp();
        let other = alternative.map(score).unwrap_or(0.0);
        Some(Self {
            best,
            scale,
            other,
            total: rail.map(score).sum::<f64>() + other,
        })
    }

    fn share(&self, cost: f64) -> f64 {
        (-(cost - self.best) / self.scale).exp() / self.total
    }

    fn alternative_share(&self) -> f64 {
        self.other / self.total
    }
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
        if !identifier(&access.station)
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
            through_seconds: 0.0,
            headway_seconds: 600.0,
        }
    }
    #[test]
    fn fare_preview_counts_boardings_and_changes_choice_without_changing_travel_time() {
        let demand = od();
        let mut second = leg(2, 3, 10, 300.0);
        second.sequence = 1;
        let legs = [
            leg(1, 2, 10, 300.0),
            second,
            leg(1, 2, 20, 300.0),
            leg(2, 3, 30, 300.0),
        ];
        let run = |prices| {
            assign_fixed_od_with_prices(
                std::slice::from_ref(&demand),
                &legs,
                &[],
                &ChoiceWeights::default(),
                1000,
                prices,
            )
            .unwrap()
            .remove(0)
        };
        let reference = run(None);
        let explicit_free = run(Some(ChoicePrices {
            public_fare_yen: 0,
            alternative_cost_yen: 0,
            value_of_time_yen_per_hour: 3600,
            bus_fare_yen: None,
            service_adjustment: None,
        }));
        assert_eq!(reference, explicit_free);
        let fares = ChoicePrices {
            public_fare_yen: 200,
            alternative_cost_yen: 0,
            value_of_time_yen_per_hour: 3600,
            bus_fare_yen: None,
            service_adjustment: None,
        };
        let priced = run(Some(fares.clone()));
        assert!(priced.alternative_trips > reference.alternative_trips);
        for priced_path in &priced.rail {
            let previous = reference
                .rail
                .iter()
                .find(|path| path.leg_ids == priced_path.leg_ids)
                .unwrap();
            assert_eq!(
                priced_path.generalized_seconds - previous.generalized_seconds,
                200.0 * priced_path.services.len() as f64
            );
            assert_eq!(priced_path.time, previous.time);
        }
        let costlier_other = run(Some(ChoicePrices {
            alternative_cost_yen: 600,
            ..fares.clone()
        }));
        assert!(costlier_other.alternative_trips < priced.alternative_trips);
        let total =
            priced.alternative_trips + priced.rail.iter().map(|path| path.trips).sum::<f64>();
        assert!((total - 100.0).abs() < 1e-9);
        assert!(assign_fixed_od_with_prices(
            &[demand],
            &legs,
            &[],
            &ChoiceWeights::default(),
            1000,
            Some(ChoicePrices {
                value_of_time_yen_per_hour: 0,
                ..fares
            }),
        )
        .is_err());
    }

    #[test]
    fn custom_fares_follow_boarding_service_not_physical_segments() {
        let demand = od();
        let mut through = leg(2, 3, 10, 300.0);
        through.sequence = 1;
        let mut bus = leg(2, 3, 30, 300.0);
        bus.service_id = "bus:30".into();
        let legs = [leg(1, 2, 10, 300.0), through, leg(1, 2, 20, 300.0), bus];
        let run = |prices| {
            assign_fixed_od_with_prices(
                std::slice::from_ref(&demand),
                &legs,
                &[],
                &ChoiceWeights::default(),
                1000,
                prices,
            )
        };
        let base = run(None).unwrap();
        let prices = ChoicePrices {
            public_fare_yen: 200,
            alternative_cost_yen: 0,
            value_of_time_yen_per_hour: 3600,
            bus_fare_yen: Some(100),
            service_adjustment: Some(ServiceFareAdjustment {
                service_id: "10".into(),
                delta_yen: -50,
            }),
        };
        let priced = run(Some(prices.clone())).unwrap();
        for (ids, expected) in [
            (vec!["10"], vec![150]),
            (vec!["10", "bus:30"], vec![150, 100]),
            (vec!["20", "bus:30"], vec![200, 100]),
        ] {
            let path = priced[0].rail.iter().find(|p| p.services == ids).unwrap();
            let unpriced = base[0].rail.iter().find(|p| p.services == ids).unwrap();
            assert_eq!(path.boarding_fares_yen.as_ref(), Some(&expected));
            assert_eq!(
                path.generalized_seconds - unpriced.generalized_seconds,
                f64::from(expected.iter().sum::<u32>())
            );
            assert_eq!(path.time, unpriced.time);
        }
        assert!(run(Some(ChoicePrices {
            service_adjustment: Some(ServiceFareAdjustment {
                service_id: "bus:30".into(),
                delta_yen: -101
            }),
            ..prices
        }))
        .is_err());
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
        assert_eq!(
            result[0].rail[0].time,
            PathTimeBreakdown {
                walk_seconds: 0.0,
                wait_seconds: 300.0,
                ride_seconds: 600.0,
                transfers: 0
            }
        );
        conserved(&result[0]);
        let mut reverse = od();
        reverse.access = vec![access(3)];
        reverse.egress = vec![access(1)];
        assert!(evaluate(&[reverse], &legs)[0].rail.is_empty());
    }
    #[test]
    fn departure_wait_does_not_count_prior_dwell_for_new_boarders() {
        let legs = [
            leg(1, 2, 1, 100.0),
            PassengerLeg {
                through_seconds: 25.0,
                ..leg(2, 3, 1, 80.0)
            },
        ];
        let through = evaluate(&[od()], &legs);
        let mut boarding_input = od();
        boarding_input.access = vec![access(2)];
        let boarded = evaluate(&[boarding_input], &legs);
        let through = &through[0].rail[0];
        let boarded = &boarded[0].rail[0];
        assert_eq!(
            (through.time.ride_seconds, through.time.wait_seconds),
            (205.0, 300.0)
        );
        assert_eq!(
            (boarded.time.ride_seconds, boarded.time.wait_seconds),
            (80.0, 300.0)
        );
        assert_eq!(
            (through.generalized_seconds, boarded.generalized_seconds),
            (505.0, 380.0)
        );
    }

    #[test]
    fn transfer_and_explicit_walk_do_not_invent_physical_connections() {
        let legs = [
            leg(1, 2, 1, 300.0),
            leg(5, 4, 2, 100.0),
            PassengerLeg {
                sequence: 1,
                through_seconds: 45.0,
                ..leg(4, 3, 2, 300.0)
            },
        ];
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
        let path = &r[0].rail[0];
        assert_eq!(path.stations, ["1", "2", "4", "3"]);
        assert_eq!(path.services, ["1", "2"]);
        assert_eq!(path.leg_ids, ["1:1:2", "2:4:3"]);
        assert_eq!(
            path.steps,
            [
                PassengerStep::Rail {
                    leg_id: "1:1:2".into()
                },
                PassengerStep::Walk {
                    from: "2".into(),
                    to: "4".into()
                },
                PassengerStep::Rail {
                    leg_id: "2:4:3".into()
                },
            ]
        );
        assert_eq!(path.generalized_seconds, 1620.0);
        assert_eq!(
            r[0].rail[0].time,
            PathTimeBreakdown {
                walk_seconds: 120.0,
                wait_seconds: 600.0,
                ride_seconds: 600.0,
                transfers: 1
            }
        );
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
        assert_eq!(r[0].partial_candidates.len(), 1);
        assert_eq!(r[0].partial_candidates[0].stations, ["1", "3"]);
        assert_eq!(r[0].partial_candidates[0].leg_ids, ["1:1:3"]);
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
    #[test]
    fn long_chain_with_many_egresses_quarantines_even_valid_prefixes() {
        let edges: Vec<_> = (1..=MAX_PATH_STATIONS + 1)
            .map(|station| PassengerLeg {
                sequence: station - 1,
                ..leg(station as u64, station as u64 + 1, 1, 1.0)
            })
            .collect();
        let mut input = od();
        input.egress = (2..=MAX_PATH_STATIONS + 2)
            .map(|station| access(station as u64))
            .collect();
        let result = evaluate(&[input], &edges);
        assert!(!result[0].search_complete);
        assert!(result[0].rail.is_empty());
        assert_eq!(result[0].partial_candidates.len(), MAX_PATH_STATIONS - 1);
        assert!(result[0]
            .partial_candidates
            .iter()
            .all(|p| p.stations.len() <= MAX_PATH_STATIONS && p.trips == 0.0));
        assert_eq!(result[0].unresolved_trips, 100.0);
        conserved(&result[0]);
    }

    #[test]
    fn batch_budget_covers_output_and_work_across_od_rows() {
        let mut second = od();
        second.id = "second-market".into();
        second.destination_location = "second-destination".into();
        let ods = [od(), second];
        let legs = [leg(1, 3, 1, 1.0)];
        let weights = ChoiceWeights::default();
        for (work, output) in [(1_000, 8), (32, 1_000)] {
            let results = assign_fixed_od_budgeted(
                &ods,
                &legs,
                &[],
                &weights,
                1_000,
                (work, output),
                (None, None),
            )
            .unwrap();
            assert!(results[0].search_complete);
            assert!(!results[1].search_complete);
            assert_eq!(results[1].unresolved_trips, 100.0);
            assert!(results[1].rail.is_empty());
            assert!(results[1].partial_candidates.is_empty());
            for result in results {
                conserved(&result);
            }
        }
    }

    #[test]
    fn identifier_byte_cap_applies_before_path_clones() {
        let mut oversized = od();
        oversized.access[0].station = "界".repeat(MAX_IDENTIFIER_BYTES / 3 + 1);
        assert!(assign_fixed_od(&[oversized], &[], &[], &ChoiceWeights::default(), 10).is_err());
    }
}
