//! Deterministic, headless operation. Infrastructure adapters supply physical
//! distances, caps and shared resource IDs; the renderer never owns this clock.
use serde::{Deserialize, Serialize};
mod passenger_replay;
pub mod periodic_dispatch;
use periodic_dispatch::{DepartureSource, ScheduleSource};
pub mod service_planning;
pub use passenger_replay::{ReplayItinerary, ReplayItineraryStatus, ReplayRide};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationError {
    InvalidTimetable,
    InvalidPath,
    InvalidVehicle,
    InvalidClock,
    CrossingBudget,
    CrossingPlatform,
    ReservationScopeChanged,
    TimetableHandoffPending,
    OpposingMeetInProgress,
    Overflow,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vehicle {
    pub max_speed_kmh: f64,
    pub acceleration_mps2: f64,
    pub braking_mps2: f64,
    pub length_m: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Section {
    pub length_m: f64,
    pub speed_limit_kmh: f64,
    pub resources: Vec<String>,
}
/// A stop-to-stop leg may pass any number of non-stopping stations. Resources
/// are reserved together, avoiding trains deadlocking head-to-head in a corridor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunLeg {
    pub from: String,
    pub to: String,
    pub sections: Vec<Section>,
    pub dwell_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timetable {
    pub first_departure_ms: u64,
    pub last_departure_ms: u64,
    pub headway_ms: u64,
}
impl Timetable {
    fn is_valid(&self) -> bool {
        self.headway_ms != 0 && self.first_departure_ms <= self.last_departure_ms
    }

    fn contains(&self, departure_ms: u64) -> bool {
        departure_ms >= self.first_departure_ms
            && departure_ms <= self.last_departure_ms
            && (departure_ms - self.first_departure_ms).checked_rem(self.headway_ms) == Some(0)
    }

    // A started runtime has already accounted for every departure up to now.
    fn next_unprocessed_departure(&self, started: bool, now_ms: u64) -> Option<u64> {
        if !started || now_ms < self.first_departure_ms {
            return Some(self.first_departure_ms);
        }
        (now_ms - self.first_departure_ms)
            .checked_div(self.headway_ms)
            .and_then(|n| n.checked_add(1))
            .and_then(|n| n.checked_mul(self.headway_ms))
            .and_then(|d| self.first_departure_ms.checked_add(d))
            .filter(|n| *n <= self.last_departure_ms)
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceRunSpec {
    pub service_id: String,
    pub vehicle: Vehicle,
    pub timetable: Timetable,
    pub legs: Vec<RunLeg>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PassengerOffer {
    pub leg_id: String,
    pub sequence: usize,
    pub board_allowed: bool,
    pub alight_allowed: bool,
    pub service_id: String,
    pub from: String,
    pub to: String,
    pub ride_seconds: f64,
    /// Scheduled time at the origin already spent aboard a continuing run.
    pub through_seconds: f64,
    pub headway_seconds: f64,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TrainKey {
    pub service_id: String,
    pub scheduled_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TrainState {
    Waiting {
        ready_ms: u64,
    },
    Moving {
        entered_ms: u64,
        arrival_ms: u64,
        release_ms: u64,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Train {
    pub key: TrainKey,
    pub leg: usize,
    pub delay_ms: u64,
    pub state: TrainState,
}
/// A currently contested resource, not a timetable estimate or a track capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RailWaitReason {
    Crossing,
    Junction,
    Platform,
    Track,
}

impl RailWaitReason {
    fn of_resource(resource: &str) -> Option<Self> {
        if resource.starts_with("rail:crossing:") {
            return Some(Self::Crossing);
        }
        if resource.starts_with("rail:join:") || resource.starts_with("rail:lane-join:") {
            return Some(Self::Junction);
        }
        let mut parts = resource.split(':');
        if matches!(
            (parts.next(), parts.next(), parts.next(), parts.next(), parts.next()),
            (Some("rail"), Some(alignment), Some("point"), Some(anchor), None)
                if alignment.parse::<u64>().is_ok() && anchor.parse::<usize>().is_ok()
        ) {
            return Some(Self::Junction);
        }
        rail_lane(resource).then_some(Self::Track)
    }
}

/// Read-only context for a current reservation conflict. These labels are
/// certified by the active model and never predict the exact release time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RailWaitScope {
    HeldAcrossConnections,
    NeedsAcrossConnections,
    ParkedOnSharedBranch,
    SharedExitForBypass,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arrival {
    pub train: TrainKey,
    pub station_id: String,
    pub at_ms: u64,
    pub delay_ms: u64,
}
#[derive(Debug, Clone, PartialEq)]
struct CompiledLeg {
    duration_ms: u64,
    clearance_ms: u64,
    resources: BTreeSet<String>,
    spans_connections: bool,
    // Milliseconds from entry; absence means reserved through the ordinary rear-clear event.
    early_releases: BTreeMap<String, u64>,
    release_events: BTreeSet<u64>,
    nominal_entry_ms: u64,
    through_lane: Option<String>,
}
#[derive(Debug, Clone, PartialEq)]
struct CompiledService {
    spec: ServiceRunSpec,
    legs: Vec<CompiledLeg>,
    journey_ms: u64,
    terminal_hold: bool,
    departure_source: DepartureSource,
    next_departure: Option<u64>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub(crate) enum RailAdmissionModel {
    Fixed = 2,
    Progressive = 3,
    TurnbackHold = 4,
    InlineTerminalDwell = 5,
    InlinePlatformHandoff = 6,
    FixedRefuge = 7,
    ConnectedTerminalNeck = 8,
}

impl RailAdmissionModel {
    fn progressive(self) -> bool {
        matches!(
            self,
            Self::Progressive
                | Self::InlineTerminalDwell
                | Self::InlinePlatformHandoff
                | Self::FixedRefuge
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OperationRuntime {
    now_ms: u64,
    paused: bool,
    speed: u32,
    started: bool,
    time_margin_ms: u64,
    services: BTreeMap<String, CompiledService>,
    trains: BTreeMap<TrainKey, Train>,
    occupied: BTreeMap<String, TrainKey>,
    timetable_handoffs: BTreeMap<String, TimetableHandoff>,
    #[cfg(feature = "standard-periodic-rail")]
    standard_timetable: Option<periodic_dispatch::standard::StandardDailyV1>,
    rail_admission: RailAdmissionModel,
    // Re-derived from checked World geometry on configuration/restore. The
    // public runtime alone has no authority to choose this exact-meet tie.
    preferred_opposing_births: Vec<OpposingBirthPriority>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpposingBirthPriority {
    pub(crate) siding: String,
    pub(crate) through: String,
    pub(crate) deferred: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TimetableHandoff {
    cutover_ms: u64,
    previous: Timetable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationSave {
    pub format: u32,
    pub started: bool,
    pub now_ms: u64,
    pub paused: bool,
    pub speed: u32,
    pub services: Vec<ServiceRunSpec>,
    pub trains: Vec<Train>,
    pub next_departures: Vec<(String, Option<u64>)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    timetable_handoffs: Vec<(String, TimetableHandoff)>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdvanceResult {
    pub now_ms: u64,
    pub reached_target: bool,
    pub arrivals: Vec<Arrival>,
}
/// Observations from a detached replay of the simulated clock, never a
/// timetable offer or a measurement of real passengers. A leg beginning
/// before the replay window may finish in it. Positions are occurrence
/// indexes: repeated visits to the same station are not interchangeable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReplayedPassengerLeg {
    pub train: TrainKey,
    pub sequence: usize,
    pub from: String,
    pub to: String,
    pub entered_ms: u64,
    pub arrived_ms: u64,
    /// Occupancy waiting accumulated by this train through the arrival,
    /// including prior legs. Not an additional per-leg or passenger wait.
    pub cumulative_reservation_wait_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PassengerReplay {
    pub observed_from_ms: u64,
    pub requested_until_ms: u64,
    pub observed_until_ms: u64,
    /// False means an event cap truncated observation; omitted rides are
    /// not evidence of an unavailable service.
    pub complete: bool,
    pub completed_legs: Vec<ReplayedPassengerLeg>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PassengerReplayError {
    OutsideBudget,
    Runtime(OperationError),
}

fn positive(x: f64) -> bool {
    x.is_finite() && x > 0.0
}
fn millis(s: f64) -> Result<u64, OperationError> {
    let n = (s * 1000.0).ceil();
    if n.is_finite() && n > 0.0 && n < (u64::MAX / 4) as f64 {
        Ok(n as u64)
    } else {
        Err(OperationError::Overflow)
    }
}
/// Forward/backward speed envelope, stopping at both ends. Short runs never
/// assume that the maximum speed is reached. Gradients do not change speed.
pub fn running_time_ms(sections: &[Section], vehicle: &Vehicle) -> Result<u64, OperationError> {
    running_time_profile(sections, vehicle, None)
}

/// Nominal position time on one physical-speed section. Splitting the speed
/// envelope at an internal marker does not stop the train at that marker.
pub(crate) fn uniform_section_passage_ms(
    section: &Section,
    vehicle: &Vehicle,
    fraction: f64,
) -> Result<u64, OperationError> {
    if !(fraction > 0.0 && fraction < 1.0) {
        return Err(OperationError::InvalidPath);
    }
    let before = section.length_m * fraction;
    let pieces = [before, section.length_m - before].map(|length_m| Section {
        length_m,
        speed_limit_kmh: section.speed_limit_kmh,
        resources: Vec::new(),
    });
    let mut boundaries = Vec::with_capacity(2);
    running_time_profile(&pieces, vehicle, Some(&mut boundaries))?;
    boundaries
        .first()
        .copied()
        .ok_or(OperationError::InvalidPath)
}

fn running_time_profile(
    sections: &[Section],
    vehicle: &Vehicle,
    mut boundaries: Option<&mut Vec<u64>>,
) -> Result<u64, OperationError> {
    if !positive(vehicle.max_speed_kmh)
        || !positive(vehicle.acceleration_mps2)
        || !positive(vehicle.braking_mps2)
        || !positive(vehicle.length_m)
    {
        return Err(OperationError::InvalidVehicle);
    }
    if sections.is_empty()
        || sections.iter().any(|s| {
            !positive(s.length_m)
                || !positive(s.speed_limit_kmh)
                || s.resources.iter().any(|r| r.trim().is_empty())
        })
    {
        return Err(OperationError::InvalidPath);
    }
    let caps: Vec<f64> = sections
        .iter()
        .map(|s| s.speed_limit_kmh.min(vehicle.max_speed_kmh) / 3.6)
        .collect();
    let mut v = vec![0.0; sections.len() + 1];
    for i in 1..sections.len() {
        v[i] = caps[i - 1].min(caps[i]);
    }
    for i in 0..sections.len() {
        v[i + 1] = v[i + 1]
            .min((v[i] * v[i] + 2.0 * vehicle.acceleration_mps2 * sections[i].length_m).sqrt());
    }
    for i in (0..sections.len()).rev() {
        v[i] = v[i]
            .min((v[i + 1] * v[i + 1] + 2.0 * vehicle.braking_mps2 * sections[i].length_m).sqrt());
    }
    let mut seconds = 0.0;
    for (i, s) in sections.iter().enumerate() {
        let a = vehicle.acceleration_mps2;
        let b = vehicle.braking_mps2;
        let peak = ((2.0 * s.length_m + v[i] * v[i] / a + v[i + 1] * v[i + 1] / b)
            / (1.0 / a + 1.0 / b))
            .sqrt()
            .min(caps[i]);
        let accelerate = (peak * peak - v[i] * v[i]) / (2.0 * a);
        let brake = (peak * peak - v[i + 1] * v[i + 1]) / (2.0 * b);
        seconds += (peak - v[i]) / a
            + (peak - v[i + 1]) / b
            + (s.length_m - accelerate - brake).max(0.0) / peak;
        if let Some(times) = boundaries.as_deref_mut() {
            times.push(millis(seconds)?);
        }
    }
    millis(seconds)
}
/// Only adapter lane keys may release progressively. Joins, point locks,
/// roads and any unknown resource stay reserved for the whole leg.
fn rail_lane(resource: &str) -> bool {
    let mut parts = resource.split(':');
    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next(), parts.next(), parts.next(), parts.next()),
        (Some("rail"), Some(alignment), Some("section"), Some(section), Some("lane"), Some(lane), None)
            if alignment.parse::<u64>().is_ok() && section.parse::<usize>().is_ok()
                && matches!(lane, "0" | "1" | "2" | "3")
    )
}

/// Canonical direct-lane key used both at the world geometry boundary and
/// when deriving the fixed, acyclic order of a serialized run's blocks.
pub(crate) fn inline_lane_key(resource: &str) -> Option<(u64, usize, u8)> {
    let mut parts = resource.split(':');
    match (
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
    ) {
        (
            Some("rail"),
            Some(alignment),
            Some("section"),
            Some(section),
            Some("lane"),
            Some(lane @ ("0" | "1")),
            None,
        ) => {
            let parsed = (
                alignment.parse().ok()?,
                section.parse().ok()?,
                lane.parse().ok()?,
            );
            (resource == format!("rail:{}:section:{}:lane:{}", parsed.0, parsed.1, parsed.2))
                .then_some(parsed)
        }
        _ => None,
    }
}

/// The ordinary Bus adapter uses only these disjoint road resources. Recheck
/// the namespace on load too; IDs in a serialized run are not authority.
pub(crate) fn independent_neck_bus(spec: &ServiceRunSpec) -> bool {
    let bus_id = |s: &str| s.strip_prefix("bus:").is_some_and(|id| !id.is_empty());
    bus_id(&spec.service_id)
        && !spec.legs.is_empty()
        && spec.legs.iter().all(|leg| {
            bus_id(&leg.from)
                && bus_id(&leg.to)
                && !leg.sections.is_empty()
                && leg.sections.iter().all(|section| {
                    !section.resources.is_empty()
                        && section.resources.iter().all(|key| {
                            key.strip_prefix("road:edge:")
                                .is_some_and(|id| !id.is_empty())
                        })
                })
        })
}

/// The terminal is on a common single lead; departure and arrival on the
/// double approach use distinct lanes. These are resource-shape checks only:
/// the World must independently prove the actual points and berth.
pub(crate) fn terminal_neck(spec: &ServiceRunSpec) -> Option<(u64, u64)> {
    let [out, back] = spec.legs.as_slice() else {
        return None;
    };
    if out.from == out.to || out.from != back.to || out.to != back.from {
        return None;
    }
    let layout = |leg: &RunLeg| {
        let mut lanes = Vec::new();
        let mut joins = BTreeSet::new();
        for s in &leg.sections {
            let mut lane = None;
            for r in &s.resources {
                if let Some(key) = inline_lane_key(r) {
                    if lane.replace(key).is_some() {
                        return None;
                    }
                    continue;
                }
                let tail = r.strip_prefix("rail:join:")?;
                let (id, side) = tail.split_once(':')?;
                let id: u64 = id.parse().ok()?;
                let is_end = match side {
                    "Start" => false,
                    "End" => true,
                    _ => return None,
                };
                if r != &format!("rail:join:{id}:{side}") {
                    return None;
                }
                joins.insert((id, is_end));
                if joins.len() > 2 {
                    return None;
                }
            }
            let lane = lane?;
            if lanes.last() != Some(&lane) {
                lanes.push(lane);
                if lanes.len() > 2 {
                    return None;
                }
            }
        }
        let [first, last]: [_; 2] = lanes.try_into().ok()?;
        let two_ports: [_; 2] = joins.into_iter().collect::<Vec<_>>().try_into().ok()?;
        Some((first, last, two_ports))
    };
    let ((main, 0, arriving), (lead, 0, 0), out_ports) = layout(out)? else {
        return None;
    };
    let ((back_lead, 0, 0), (back_main, 0, leaving), back_ports) = layout(back)? else {
        return None;
    };
    (main != lead
        && lead == back_lead
        && main == back_main
        && arriving <= 1
        && leaving == 1 - arriving
        && out_ports == back_ports
        && out_ports.iter().any(|&(id, _)| id == main)
        && out_ports.iter().any(|&(id, _)| id == lead))
    .then_some((main, lead))
}

/// Only one alignment and one direction per run. Inside each leg blocks
/// are contiguous and cannot retreat. Consecutive legs share exactly their
/// boundary block (regardless of speed-profile subdivisions inside it).
/// A parked car can therefore wait only on a strictly later physical block:
/// a lower-index held lock cannot be awaited by a higher-index one on lane 0,
/// and the reversed order applies independently to lane 1.
pub(crate) fn inline_platform_handoff(spec: &ServiceRunSpec) -> bool {
    if !(1..=2).contains(&spec.legs.len()) {
        return false;
    }
    let mut previous: Option<(u64, usize, u8)> = None;
    for leg in &spec.legs {
        if leg.from == leg.to || leg.sections.is_empty() {
            return false;
        }
        for (index, section) in leg.sections.iter().enumerate() {
            let [resource] = section.resources.as_slice() else {
                return false;
            };
            let Some(current) = inline_lane_key(resource) else {
                return false;
            };
            if let Some(prior) = previous {
                if prior.0 != current.0
                    || prior.2 != current.2
                    || if index == 0 {
                        prior != current
                    } else if current.2 == 0 {
                        current.1 < prior.1 || current.1 - prior.1 > 1
                    } else {
                        prior.1 < current.1 || prior.1 - current.1 > 1
                    }
                {
                    return false;
                }
            }
            previous = Some(current);
        }
    }
    true
}

/// Validate the persisted resource skeleton even when construction changed:
/// entry approach -> explicit middle branch -> exit approach. A local keeps
/// only its middle block; its only cross-service waits can be on the exit
/// throat or stem, neither of which can be held by a parked train.
pub(crate) fn fixed_refuge_resources(spec: &ServiceRunSpec) -> Option<([u64; 3], bool)> {
    fn groups(leg: &RunLeg) -> Option<(Vec<(u64, u8)>, BTreeSet<&str>)> {
        let mut lanes = Vec::new();
        let mut joins = BTreeSet::new();
        for section in &leg.sections {
            let mut lane = None;
            for r in &section.resources {
                if let Some((id, 0, side @ 0..=1)) = inline_lane_key(r) {
                    if lane.replace((id, side)).is_some() {
                        return None;
                    }
                } else {
                    joins.insert(r.as_str());
                }
            }
            let lane = lane?;
            if lanes.last() != Some(&lane) {
                lanes.push(lane);
            }
            if lanes.len() > 3 {
                return None;
            }
        }
        Some((lanes, joins))
    }
    let expect = |joins: &BTreeSet<&str>, pairs: &[(u64, &str)]| {
        joins.len() == pairs.len()
            && pairs
                .iter()
                .all(|(id, end)| joins.contains(format!("rail:join:{id}:{end}").as_str()))
    };
    let ports = |side: u8| {
        if side == 0 {
            ("End", "Start")
        } else {
            ("Start", "End")
        }
    };
    let (entry, middle, exit, side) = match spec.legs.as_slice() {
        [through] => {
            let (route, joins) = groups(through)?;
            let [(entry, side), (mid, 0), (exit, far)] = route.as_slice() else {
                return None;
            };
            let (leave, enter) = ports(*side);
            if side != far
                || !expect(
                    &joins,
                    &[
                        (*entry, leave),
                        (*mid, enter),
                        (*mid, leave),
                        (*exit, enter),
                    ],
                )
            {
                return None;
            }
            (*entry, *mid, *exit, *side)
        }
        [incoming, outgoing] => {
            let (into, joins_in) = groups(incoming)?;
            let (out, joins_out) = groups(outgoing)?;
            let ([(entry, side), (mid, 0)], [(same, 0), (exit, far)]) =
                (into.as_slice(), out.as_slice())
            else {
                return None;
            };
            let (leave, enter) = ports(*side);
            if mid != same
                || side != far
                || !expect(
                    &joins_in,
                    &[
                        (*entry, leave),
                        (*mid, enter),
                        (*mid, leave),
                        (*exit, enter),
                    ],
                )
                || !expect(&joins_out, &[(*mid, leave), (*exit, enter)])
            {
                return None;
            }
            (*entry, *mid, *exit, *side)
        }
        _ => return None,
    };
    (entry != middle && entry != exit && middle != exit)
        .then_some(([entry, middle, exit], side == 0))
}

fn progressive_release_offsets(
    sections: &[Section],
    times: &[u64],
    train_length: f64,
    release_at_arrival: bool,
) -> BTreeMap<String, u64> {
    let mut last = BTreeMap::new();
    let mut distance = Vec::with_capacity(sections.len() + 1);
    distance.push(0.0);
    for (index, section) in sections.iter().enumerate() {
        distance.push(distance[index] + section.length_m);
        for resource in section.resources.iter().filter(|r| rail_lane(r)) {
            last.insert(resource.clone(), index);
        }
    }
    // The head must reach a later section boundary strictly beyond the tail-
    // clearing position, so use the cumulative true motion profile there, not
    // a nominal section speed. The one-way terminal model also uses arrival
    // as evidence for a proven-clear approach (never for the last block).
    // The margin covers accumulation/cancellation on finite values.
    let margin = f64::EPSILON * (sections.len() as f64 + 2.0) * distance[sections.len()] * 4.0;
    last.into_iter()
        .filter_map(|(resource, i)| {
            let threshold = distance[i + 1] + train_length + margin;
            let boundary = distance.partition_point(|p| *p <= threshold);
            (boundary > 0
                && (boundary < sections.len()
                    || (release_at_arrival && boundary == sections.len())))
            .then(|| (resource, times[boundary - 1]))
        })
        .collect()
}

impl OperationRuntime {
    pub fn new(specs: Vec<ServiceRunSpec>) -> Result<Self, OperationError> {
        Self::compile(specs, RailAdmissionModel::Fixed)
    }

    /// Qualified mode for trusted, directional rail-section resource IDs.
    /// Admission still reserves all resources to the next stop atomically; only
    /// a lane proven to have cleared the tail is released early. No station
    /// capacity or rolling block acquisition is implied.
    pub(crate) fn new_with_progressive_rail(
        specs: Vec<ServiceRunSpec>,
    ) -> Result<Self, OperationError> {
        Self::compile(specs, RailAdmissionModel::Progressive)
    }

    /// Only the world adapter may activate the qualified straight-corridor model.
    pub(crate) fn new_with_terminal_hold(
        specs: Vec<ServiceRunSpec>,
    ) -> Result<Self, OperationError> {
        Self::compile(specs, RailAdmissionModel::TurnbackHold)
    }

    pub(crate) fn new_with_connected_terminal_neck(
        specs: Vec<ServiceRunSpec>,
    ) -> Result<Self, OperationError> {
        Self::compile(specs, RailAdmissionModel::ConnectedTerminalNeck)
    }

    /// Qualified one-way inline terminal. The final physical lane block
    /// remains occupied while passengers detrain, then the train retires.
    pub(crate) fn new_with_inline_terminal_dwell(
        specs: Vec<ServiceRunSpec>,
    ) -> Result<Self, OperationError> {
        Self::compile(specs, RailAdmissionModel::InlineTerminalDwell)
    }

    pub(crate) fn new_with_inline_platform_handoff(
        specs: Vec<ServiceRunSpec>,
    ) -> Result<Self, OperationError> {
        Self::compile(specs, RailAdmissionModel::InlinePlatformHandoff)
    }

    pub(crate) fn new_with_fixed_refuge(
        specs: Vec<ServiceRunSpec>,
    ) -> Result<Self, OperationError> {
        Self::compile(specs, RailAdmissionModel::FixedRefuge)
    }

    fn compile(
        specs: Vec<ServiceRunSpec>,
        rail_admission: RailAdmissionModel,
    ) -> Result<Self, OperationError> {
        let mut services = BTreeMap::new();
        let mut time_margin_ms = 0;
        let neck_mode = rail_admission == RailAdmissionModel::ConnectedTerminalNeck;
        if neck_mode {
            let mut rail = specs.iter().filter(|spec| !independent_neck_bus(spec));
            if specs.len() > 2
                || !rail.next().is_some_and(|spec| {
                    spec.service_id.parse::<u64>().is_ok() && terminal_neck(spec).is_some()
                })
                || rail.next().is_some()
            {
                return Err(OperationError::InvalidPath);
            }
        }
        for spec in specs {
            let t = &spec.timetable;
            if spec.service_id.trim().is_empty()
                || services.contains_key(&spec.service_id)
                || !t.is_valid()
            {
                return Err(OperationError::InvalidTimetable);
            }
            if spec.legs.is_empty()
                || spec
                    .legs
                    .iter()
                    .any(|l| l.from.trim().is_empty() || l.to.trim().is_empty())
                || spec.legs.windows(2).any(|p| p[0].to != p[1].from)
            {
                return Err(OperationError::InvalidPath);
            }
            let neck_hold = neck_mode && spec.service_id.parse::<u64>().is_ok();
            let terminal_hold = rail_admission == RailAdmissionModel::TurnbackHold || neck_hold;
            let mut legs = Vec::new();
            let mut journey = 0u64;
            let mut nominal_entry_ms = 0u64;
            let platform = rail_admission == RailAdmissionModel::InlineTerminalDwell;
            let inline = rail_admission == RailAdmissionModel::InlinePlatformHandoff;
            let refuge = rail_admission == RailAdmissionModel::FixedRefuge;
            let handoff = inline || refuge;
            if refuge
                && (spec.service_id.parse::<u64>().is_err()
                    || fixed_refuge_resources(&spec).is_none())
            {
                return Err(OperationError::InvalidPath);
            }
            if inline
                && (spec.service_id.parse::<u64>().is_err() || !inline_platform_handoff(&spec))
            {
                return Err(OperationError::InvalidPath);
            }
            if platform
                && (spec.legs.len() != 1
                    || spec.service_id.parse::<u64>().is_err()
                    || spec.legs[0]
                        .sections
                        .iter()
                        .any(|s| s.resources.len() != 1 || !rail_lane(&s.resources[0])))
            {
                return Err(OperationError::InvalidPath);
            }
            for (index, l) in spec.legs.iter().enumerate() {
                let progressive =
                    rail_admission.progressive() && spec.service_id.parse::<u64>().is_ok();
                let mut boundaries = Vec::new();
                let duration_ms = running_time_profile(
                    &l.sections,
                    &spec.vehicle,
                    progressive.then_some(&mut boundaries),
                )?;
                let early_releases = if progressive {
                    progressive_release_offsets(
                        &l.sections,
                        &boundaries,
                        spec.vehicle.length_m,
                        platform || handoff,
                    )
                } else {
                    BTreeMap::new()
                };
                let release_events = early_releases.values().copied().collect();
                let lowest = l
                    .sections
                    .iter()
                    .map(|s| s.speed_limit_kmh.min(spec.vehicle.max_speed_kmh))
                    .fold(f64::INFINITY, f64::min)
                    / 3.6;
                let physical_clearance_ms = millis(spec.vehicle.length_m / lowest)?;
                let clearance_ms = if platform
                    || (handoff && index + 1 == spec.legs.len())
                    || (index == 0 && terminal_hold)
                {
                    physical_clearance_ms.max(l.dwell_ms)
                } else {
                    physical_clearance_ms
                };
                journey = journey
                    .checked_add(duration_ms)
                    .and_then(|n| n.checked_add(physical_clearance_ms))
                    .and_then(|n| n.checked_add(l.dwell_ms))
                    .ok_or(OperationError::Overflow)?;
                time_margin_ms = time_margin_ms.max(
                    duration_ms
                        .saturating_add(clearance_ms)
                        .saturating_add(l.dwell_ms),
                );
                let resources: BTreeSet<_> = l
                    .sections
                    .iter()
                    .flat_map(|s| s.resources.iter().cloned())
                    .collect();
                // Inspect the authoritative lockset once, never on each UI tick.
                // One transition names two different alignment IDs; detecting
                // both ends of the SAME alignment requires multiple transitions.
                let spans_connections = rail_admission == RailAdmissionModel::Fixed
                    && resources.iter().any(|key| {
                        key.strip_prefix("rail:join:")
                            .and_then(|s| s.strip_suffix(":End"))
                            .filter(|id| id.parse::<u64>().is_ok())
                            .is_some_and(|id| resources.contains(&format!("rail:join:{id}:Start")))
                    });
                legs.push(CompiledLeg {
                    duration_ms,
                    clearance_ms,
                    resources,
                    spans_connections,
                    early_releases,
                    release_events,
                    nominal_entry_ms,
                    through_lane: if handoff && index + 1 < spec.legs.len() {
                        Some(
                            l.sections
                                .last()
                                .and_then(|section| {
                                    section
                                        .resources
                                        .iter()
                                        .find(|r| inline_lane_key(r).is_some())
                                })
                                .ok_or(OperationError::InvalidPath)?
                                .clone(),
                        )
                    } else {
                        None
                    },
                });
                if handoff || neck_hold {
                    nominal_entry_ms = nominal_entry_ms
                        .checked_add(duration_ms)
                        .and_then(|t| t.checked_add(clearance_ms.max(l.dwell_ms)))
                        .ok_or(OperationError::Overflow)?;
                }
            }
            if rail_admission == RailAdmissionModel::TurnbackHold
                && (legs.len() != 2
                    || spec.legs[0].from != spec.legs[1].to
                    || spec.legs[0].from == spec.legs[0].to
                    || legs[0].resources.is_empty()
                    || legs[0].resources != legs[1].resources
                    || !legs[0]
                        .resources
                        .iter()
                        .all(|r| rail_lane(r) && r.ends_with(":0")))
            {
                return Err(OperationError::InvalidPath);
            }
            if t.last_departure_ms.checked_add(journey).is_none() {
                return Err(OperationError::Overflow);
            }
            let next_departure = Some(t.first_departure_ms);
            services.insert(
                spec.service_id.clone(),
                CompiledService {
                    spec,
                    legs,
                    journey_ms: journey,
                    terminal_hold,
                    departure_source: DepartureSource::Absolute,
                    next_departure,
                },
            );
        }
        Ok(Self {
            now_ms: 0,
            paused: true,
            speed: 1,
            started: false,
            time_margin_ms,
            services,
            trains: BTreeMap::new(),
            occupied: BTreeMap::new(),
            timetable_handoffs: BTreeMap::new(),
            #[cfg(feature = "standard-periodic-rail")]
            standard_timetable: None,
            rail_admission,
            preferred_opposing_births: Vec::new(),
        })
    }
    pub(crate) fn progressive_rail(&self) -> bool {
        self.rail_admission.progressive()
    }

    pub(crate) fn terminal_hold(&self) -> bool {
        matches!(
            self.rail_admission,
            RailAdmissionModel::TurnbackHold | RailAdmissionModel::ConnectedTerminalNeck
        )
    }

    pub(crate) fn inline_terminal_dwell(&self) -> bool {
        self.rail_admission == RailAdmissionModel::InlineTerminalDwell
    }

    pub(crate) fn inline_platform_handoff(&self) -> bool {
        self.rail_admission == RailAdmissionModel::InlinePlatformHandoff
    }

    pub(crate) fn fixed_refuge(&self) -> bool {
        self.rail_admission == RailAdmissionModel::FixedRefuge
    }

    fn parked_platform_handoff(&self) -> bool {
        self.inline_platform_handoff() || self.fixed_refuge()
    }

    fn own_lease_allows_admission(&self, service: &str) -> bool {
        self.services[service].terminal_hold || self.parked_platform_handoff()
    }

    fn reservation_blocks(&self, resource: &str, key: &TrainKey) -> bool {
        self.occupied
            .get(resource)
            .is_some_and(|owner| !self.own_lease_allows_admission(&key.service_id) || owner != key)
    }

    /// First current rail reservation conflict in deterministic resource order.
    /// May report no category for an unknown resource. Scheduled dwell, moving
    /// vehicles and any bus are never reported as rail reservation waits.
    pub fn rail_wait_reason(&self, key: &TrainKey) -> Option<RailWaitReason> {
        self.rail_wait_advisory(key).map(|(reason, _)| reason)
    }

    pub fn rail_wait_advisory(
        &self,
        key: &TrainKey,
    ) -> Option<(RailWaitReason, Option<RailWaitScope>)> {
        key.service_id.parse::<u64>().ok()?;
        let train = self.trains.get(key)?;
        if !matches!(train.state, TrainState::Waiting { ready_ms } if ready_ms <= self.now_ms) {
            return None;
        }
        let leg = self.services.get(&key.service_id)?.legs.get(train.leg)?;
        let resource = leg
            .resources
            .iter()
            .find(|key_resource| self.reservation_blocks(key_resource, key))?;
        let reason = RailWaitReason::of_resource(resource)?;
        let owner = || {
            self.occupied
                .get(resource)
                .and_then(|owner| self.trains.get(owner))
        };
        if self.fixed_refuge() {
            if let Some(owner) = owner() {
                if reason == RailWaitReason::Track
                    && matches!(owner.state, TrainState::Waiting { .. })
                    && owner
                        .leg
                        .checked_sub(1)
                        .and_then(|index| self.services[&owner.key.service_id].legs.get(index))
                        .and_then(|before| before.through_lane.as_ref())
                        == Some(resource)
                {
                    return Some((
                        RailWaitReason::Platform,
                        Some(RailWaitScope::ParkedOnSharedBranch),
                    ));
                }
                // The certified two-branch model only allows this departure
                // from the stop to share the outgoing stem/throat with a
                // through service whose lockset does not include our branch.
                if train.leg == 1
                    && matches!(owner.state, TrainState::Moving { .. })
                    && self.services[&owner.key.service_id].legs.len() == 1
                    && self.services[&key.service_id].legs[0]
                        .through_lane
                        .as_ref()
                        .is_some_and(|lane| {
                            !self.services[&owner.key.service_id].legs[0]
                                .resources
                                .contains(lane)
                        })
                {
                    return Some((reason, Some(RailWaitScope::SharedExitForBypass)));
                }
            }
        }
        if reason == RailWaitReason::Track
            && self.inline_terminal_dwell()
            && owner().is_some_and(|owner| self.is_reached_terminal_lane(owner, resource))
        {
            return Some((RailWaitReason::Platform, None));
        }
        let scope = if self.rail_admission == RailAdmissionModel::Fixed
            && reason == RailWaitReason::Junction
        {
            let owner_spans = owner()
                .filter(|owner| matches!(owner.state, TrainState::Moving { .. }))
                .and_then(|owner| {
                    self.services
                        .get(&owner.key.service_id)?
                        .legs
                        .get(owner.leg)
                })
                .is_some_and(|leg| leg.spans_connections);
            if owner_spans {
                Some(RailWaitScope::HeldAcrossConnections)
            } else if leg.spans_connections {
                Some(RailWaitScope::NeedsAcrossConnections)
            } else {
                None
            }
        } else {
            None
        };
        Some((reason, scope))
    }

    /// In the certified terminal-dwell model, identify the exact lane of
    /// the blocking train's reached berth. Don't infer it from an approach,
    /// a station name, or another mode's less specific lock scope.
    fn is_reached_terminal_lane(&self, owner: &Train, resource: &str) -> bool {
        if inline_lane_key(resource).is_none()
            || !matches!(owner.state, TrainState::Moving { arrival_ms, .. } if arrival_ms <= self.now_ms)
        {
            return false;
        }
        self.services
            .get(&owner.key.service_id)
            .and_then(|service| service.spec.legs.get(owner.leg))
            .and_then(|leg| leg.sections.last())
            .is_some_and(|section| section.resources.iter().any(|key| key == resource))
    }

    /// Transfer only an idle clock to a freshly compiled timetable. Departures
    /// are on the new absolute timetable grid, never replayed through the past.
    pub(crate) fn retain_idle_clock(&mut self, previous: &Self) -> Result<(), OperationError> {
        if !previous.paused || !previous.trains.is_empty() {
            return Err(OperationError::InvalidClock);
        }
        debug_assert!(!self.started && self.trains.is_empty());
        self.now_ms = previous.now_ms;
        self.speed = previous.speed;
        self.started = previous.started;
        for service in self.services.values_mut() {
            service.next_departure = service.next_unprocessed_departure(self.started, self.now_ms);
        }
        Ok(())
    }

    fn has_live_birth_at_or_before(
        trains: &BTreeMap<TrainKey, Train>,
        id: &str,
        time: u64,
    ) -> bool {
        let first = TrainKey {
            service_id: id.to_owned(),
            scheduled_ms: 0,
        };
        trains
            .range(first..)
            .next()
            .is_some_and(|(key, _)| key.service_id == id && key.scheduled_ms <= time)
    }

    /// Future dispatch only: all current trains and compiled leases stay in place.
    /// One previous grid per affected service is sufficient; refuse overlapping
    /// handoffs rather than accepting a birth not proven by its original grid.
    pub(crate) fn handoff_rail_timetable(
        &mut self,
        id: &str,
        timetable: Timetable,
    ) -> Result<(), OperationError> {
        if !self.paused || !self.started || self.trains.is_empty() {
            return Err(OperationError::InvalidClock);
        }
        if id.parse::<u64>().is_err() {
            return Err(OperationError::InvalidPath);
        }
        let service = self.services.get(id).ok_or(OperationError::InvalidPath)?;
        if service.departure_source.is_periodic() {
            return Err(OperationError::InvalidTimetable);
        }
        if service.spec.timetable == timetable {
            return Ok(());
        }
        if !timetable.is_valid() {
            return Err(OperationError::InvalidTimetable);
        }
        let next = timetable
            .next_unprocessed_departure(true, self.now_ms)
            .ok_or(OperationError::InvalidTimetable)?;
        timetable
            .last_departure_ms
            .checked_add(service.journey_ms)
            .ok_or(OperationError::Overflow)?;
        if self.timetable_handoffs.contains_key(id) {
            return Err(OperationError::TimetableHandoffPending);
        }
        if self.changes_active_deferred_meet(id, &timetable) {
            return Err(OperationError::OpposingMeetInProgress);
        }
        let service = self.services.get_mut(id).expect("validated service");
        if Self::has_live_birth_at_or_before(&self.trains, id, self.now_ms) {
            self.timetable_handoffs.insert(
                id.to_owned(),
                TimetableHandoff {
                    cutover_ms: self.now_ms,
                    previous: service.spec.timetable.clone(),
                },
            );
        }
        service.spec.timetable = timetable;
        service.next_departure = Some(next);
        Ok(())
    }

    pub(crate) fn has_started(&self) -> bool {
        self.started
    }

    pub(crate) fn admission_model(&self) -> RailAdmissionModel {
        self.rail_admission
    }

    /// Identical absolute schedules, or two aligned repeating grids with different
    /// starts. Serializing the complete pair
    /// cannot take longer than the sum of their compiled run, dwell and
    /// rear-clear times: the proved two-throat layout has no third holder,
    /// and the initial ready through train is older than the siding exit.
    /// A strict gap beyond that bound keeps successive pairs independent.
    fn opposing_birth_schedule_qualifies(&self, siding: &str, through: &str) -> bool {
        // Expired handoff records are pruned after a whole advance, not
        // each event. Compare the current grids: the caller requires two
        // fresh births and no older train or occupied resource.
        if self.rail_admission != RailAdmissionModel::Fixed || siding == through {
            return false;
        }
        let Some(a) = self.services.get(siding) else {
            return false;
        };
        let Some(b) = self.services.get(through) else {
            return false;
        };
        let (ta, tb) = (&a.spec.timetable, &b.spec.timetable);
        if a.departure_source.is_periodic() || b.departure_source.is_periodic() {
            return false;
        }
        if ta.first_departure_ms == tb.first_departure_ms
            && ta.last_departure_ms == tb.last_departure_ms
            && ta.first_departure_ms == ta.last_departure_ms
        {
            return true;
        }
        let gap = ta.headway_ms;
        if gap == 0
            || gap != tb.headway_ms
            || !a
                .journey_ms
                .checked_add(b.journey_ms)
                .is_some_and(|bound| gap > bound)
        {
            return false;
        }
        if ta.first_departure_ms == tb.first_departure_ms {
            return ta.last_departure_ms == tb.last_departure_ms;
        }
        // The event loop already proved both births occur now and no older
        // authority remains. An earlier service can begin several intervals
        // ahead, provided both have real repeat grids in the same phase. The
        // equal safe gap also bounds an unmatched tail after one grid ends.
        let starts_apart = ta.first_departure_ms.abs_diff(tb.first_departure_ms);
        starts_apart % gap == 0
            && ta.last_departure_ms - ta.first_departure_ms >= gap
            && tb.last_departure_ms - tb.first_departure_ms >= gap
    }

    /// Partition by all compiled authorities, including station and throat
    /// locks. Also consider components of exactly three, whose third user
    /// must pass an independent time proof at dispatch. Never enumerate
    /// pairs for a crowded authority.
    pub(crate) fn opposing_authority_candidates(&self) -> Vec<(&str, &str, Option<&str>)> {
        let mut users = BTreeMap::<&str, BTreeSet<&str>>::new();
        for (id, service) in &self.services {
            for key in service.legs.iter().flat_map(|leg| &leg.resources) {
                users.entry(key).or_default().insert(id);
            }
        }
        let mut peers = BTreeMap::<&str, BTreeSet<&str>>::new();
        let mut crowded = BTreeSet::new();
        for ids in users.into_values() {
            if ids.len() > 3 {
                crowded.extend(ids);
            } else if ids.len() > 1 {
                for &id in &ids {
                    peers
                        .entry(id)
                        .or_default()
                        .extend(ids.iter().copied().filter(|other| *other != id));
                }
            }
        }
        let mut seen = BTreeSet::new();
        let mut result = Vec::new();
        for &root in peers.keys() {
            if !seen.insert(root) {
                continue;
            }
            let mut group = vec![root];
            let mut i = 0;
            while i < group.len() {
                for &neighbor in &peers[group[i]] {
                    if seen.insert(neighbor) {
                        group.push(neighbor);
                    }
                }
                i += 1;
            }
            if !(2..=3).contains(&group.len()) || group.iter().any(|id| crowded.contains(id)) {
                continue;
            }
            for &a in &group {
                for &b in &peers[a] {
                    if a < b {
                        let deferred = group.iter().copied().find(|id| *id != a && *id != b);
                        result.push((a, b, deferred));
                    }
                }
            }
        }
        result
    }

    pub(crate) fn set_preferred_opposing_births(&mut self, pairs: Vec<OpposingBirthPriority>) {
        self.preferred_opposing_births = pairs;
    }

    /// This first extension is intentionally one-shot only. The third
    /// service cannot be born until both proved runs could fully serialize,
    /// including all dwells and rear clearance.
    fn deferred_meet_window(&self, pair: &OpposingBirthPriority) -> Option<(u64, u64)> {
        let a = self.services.get(&pair.siding)?;
        let b = self.services.get(&pair.through)?;
        let c = self.services.get(pair.deferred.as_ref()?)?;
        let birth = a.spec.timetable.first_departure_ms;
        let deferred = c.spec.timetable.first_departure_ms;
        let end = birth.checked_add(a.journey_ms)?.checked_add(b.journey_ms)?;
        (!a.departure_source.is_periodic()
            && !b.departure_source.is_periodic()
            && !c.departure_source.is_periodic()
            && a.spec.timetable.last_departure_ms == birth
            && b.spec.timetable.first_departure_ms == birth
            && b.spec.timetable.last_departure_ms == birth
            && c.spec.timetable.last_departure_ms == deferred
            && deferred > end)
            .then_some((birth, end))
    }

    /// Do not let a handoff insert a third contender after a conditional
    /// meet has already been admitted. The existing save format can
    /// reconstruct this guard from the unchanged schedules and live births.
    fn changes_active_deferred_meet(&self, edited: &str, proposed: &Timetable) -> bool {
        self.preferred_opposing_births.iter().any(|pair| {
            if pair.siding != edited
                && pair.through != edited
                && pair.deferred.as_deref() != Some(edited)
            {
                return false;
            }
            let Some((birth, end)) = self.deferred_meet_window(pair) else {
                return false;
            };
            // An equally isolated one-shot later birth does not change
            // either active participant or its protection on reload.
            if pair.deferred.as_deref() == Some(edited)
                && proposed.first_departure_ms == proposed.last_departure_ms
                && proposed.first_departure_ms > end
            {
                return false;
            }
            [&pair.siding, &pair.through].into_iter().any(|id| {
                self.trains.contains_key(&TrainKey {
                    service_id: id.clone(),
                    scheduled_ms: birth,
                })
            })
        })
    }

    /// Guard the exact proof during edits while its siding train can still
    /// depend on it, including after the partner has completed.
    pub(crate) fn loses_active_opposing_birth_proof(
        &self,
        pairs: &[OpposingBirthPriority],
    ) -> bool {
        self.preferred_opposing_births.iter().any(|pair| {
            !pairs.contains(pair)
                && (pair.deferred.is_none() || self.deferred_meet_window(pair).is_some())
                && self.trains.values().any(|train| {
                    train.key.service_id == pair.siding
                        && (train.leg > 0 || matches!(train.state, TrainState::Moving { .. }))
                })
        })
    }

    fn prefer_proved_birth_ties(&self, at: u64, waiting: &mut [(u64, TrainKey)]) {
        if waiting.len() < 2 || self.preferred_opposing_births.is_empty() {
            return;
        }
        let mut fresh = BTreeMap::new();
        let mut busy = BTreeSet::new();
        for (index, (ready, key)) in waiting.iter().enumerate() {
            if *ready == at
                && key.scheduled_ms == at
                && self
                    .trains
                    .get(key)
                    .is_some_and(|t| t.leg == 0 && t.delay_ms == 0)
            {
                fresh.insert(key.service_id.as_str(), index);
            }
        }
        for (key, train) in &self.trains {
            if !fresh.contains_key(key.service_id.as_str())
                || key.scheduled_ms != at
                || !matches!(train.state, TrainState::Waiting { .. })
            {
                busy.insert(key.service_id.as_str());
            }
        }
        busy.extend(self.occupied.values().map(|key| key.service_id.as_str()));
        // Components are disjoint. In a three-service component only
        // one pair can qualify at a given time; its third user must be
        // absent now and unable to start until the whole meet can clear.
        let swaps: Vec<_> = self
            .preferred_opposing_births
            .iter()
            .filter_map(|pair| {
                let siding = pair.siding.as_str();
                let through = pair.through.as_str();
                let a = *fresh.get(siding)?;
                let b = *fresh.get(through)?;
                if let Some(other) = pair.deferred.as_deref() {
                    let (birth, end) = self.deferred_meet_window(pair)?;
                    if birth != at
                        || busy.contains(other)
                        || fresh.contains_key(other)
                        || !self.services[other]
                            .next_departure
                            .is_some_and(|next| next > end)
                    {
                        return None;
                    }
                }
                (a > b
                    && !busy.contains(siding)
                    && !busy.contains(through)
                    && self.opposing_birth_schedule_qualifies(siding, through))
                .then_some((a, b))
            })
            .collect();
        drop(fresh);
        for (a, b) in swaps {
            waiting.swap(a, b);
        }
    }

    /// Verify neither the admission model nor another service's run changes.
    /// A legacy outbound-only target also stays outbound unless its physical
    /// route already invalidated the old run and must be recompiled.
    pub(crate) fn retains_admission(
        &self,
        previous: &Self,
        edited_service: &str,
        previous_invalidated: bool,
    ) -> bool {
        self.rail_admission == previous.rail_admission
            && previous.services.iter().all(|(id, before)| {
                let Some(after) = self.services.get(id) else {
                    return false;
                };
                if id != edited_service {
                    return after.spec == before.spec;
                }
                previous_invalidated
                    || (before.spec.legs.len() == after.spec.legs.len()
                        && before.spec.legs.iter().zip(&after.spec.legs).all(|(a, b)| {
                            a.from == b.from && a.to == b.to && a.sections == b.sections
                        }))
            })
    }

    pub fn paused(&self) -> bool {
        self.paused
    }
    pub fn speed(&self) -> u32 {
        self.speed
    }
    pub fn service_spec(&self, id: &str) -> Option<&ServiceRunSpec> {
        self.services.get(id).map(|s| &s.spec)
    }
    pub(crate) fn unblocked_leg_timing(&self, service_id: &str, leg: usize) -> Option<(u64, u64)> {
        let service = self.services.get(service_id)?;
        let compiled = service.legs.get(leg)?;
        let stationary_ms = compiled
            .clearance_ms
            .max(service.spec.legs.get(leg)?.dwell_ms);
        Some((compiled.duration_ms, stationary_ms))
    }

    pub fn now_ms(&self) -> u64 {
        self.now_ms
    }
    /// True while a save still needs a prior rail timetable to validate live births.
    pub fn rail_timetable_handoff_in_progress(&self) -> bool {
        !self.timetable_handoffs.is_empty()
    }

    pub fn trains(&self) -> impl Iterator<Item = &Train> {
        self.trains.values()
    }
    pub(crate) fn trains_with_service_prefix<'a>(
        &'a self,
        prefix: &'a str,
    ) -> impl Iterator<Item = &'a Train> + 'a {
        self.trains
            .range(
                TrainKey {
                    service_id: prefix.to_owned(),
                    scheduled_ms: 0,
                }..,
            )
            .take_while(move |(key, _)| key.service_id.starts_with(prefix))
            .map(|(_, train)| train)
    }
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }
    pub fn set_speed(&mut self, speed: u32) -> Result<(), OperationError> {
        if ![1, 2, 4, 8, 16].contains(&speed) {
            return Err(OperationError::InvalidClock);
        }
        self.speed = speed;
        Ok(())
    }
    pub fn passenger_offers(&self) -> Vec<PassengerOffer> {
        self.services
            .values()
            .flat_map(|s| {
                s.spec
                    .legs
                    .iter()
                    .zip(&s.legs)
                    .enumerate()
                    .map(move |(i, (l, c))| PassengerOffer {
                        service_id: s.spec.service_id.clone(),
                        leg_id: format!("{}:{i}", s.spec.service_id),
                        sequence: i,
                        board_allowed: true,
                        alight_allowed: true,
                        from: l.from.clone(),
                        to: l.to.clone(),
                        ride_seconds: c.duration_ms as f64 / 1000.0,
                        through_seconds: i.checked_sub(1).map_or(0.0, |previous| {
                            s.spec.legs[previous]
                                .dwell_ms
                                .max(s.legs[previous].clearance_ms)
                                as f64
                                / 1000.0
                        }),
                        headway_seconds: s.spec.timetable.headway_ms as f64 / 1000.0,
                    })
            })
            .collect()
    }
    /// Preview a bounded number of exact operation events from the current
    /// runtime snapshot. Does not advance the live simulation, persist
    /// extra state, invent arrivals beyond the requested/reached window,
    /// or modify planned OD. Clock speed/pause do not affect replay.
    pub fn preview_passenger_replay(
        &self,
        until_ms: u64,
        events: usize,
    ) -> Result<PassengerReplay, PassengerReplayError> {
        self.preview_passenger_replay_in_scope(until_ms, events, false)
    }

    /// Use the narrower observation budget for a rail-only cohort only if
    /// every road run and its current leases are independent of every rail
    /// run. Unknown resources and future mixed modes retain the full replay.
    pub fn preview_rail_cohort_replay(
        &self,
        until_ms: u64,
        events: usize,
    ) -> Result<PassengerReplay, PassengerReplayError> {
        self.preview_passenger_replay_in_scope(until_ms, events, self.can_omit_road_replay())
    }

    fn can_omit_road_replay(&self) -> bool {
        // This proof must remain cheap even if the complete replay would
        // refuse a large world. Current graphs express traffic interactions
        // exclusively through the compiled locks. A future rail/road conflict
        // must use a shared resource or explicitly disable this projection.
        let mut work = 262_144usize;
        let mut charge = |amount: usize| -> bool {
            let Some(remaining) = work.checked_sub(amount) else {
                return false;
            };
            work = remaining;
            true
        };
        let road_tag = |value: &str| value.strip_prefix("bus:").is_some_and(|id| !id.is_empty());
        let lock_in_family = |key: &str, road: bool| {
            let namespace = if road { "road:edge:" } else { "rail:" };
            key.strip_prefix(namespace)
                .is_some_and(|suffix| !suffix.is_empty())
        };
        let mut have_bus = false;
        let mut have_rail = false;
        // Only attempt this for at most the existing service cap, and
        // perform no heap allocation while proving the separation.
        if self.services.len() > 64
            || self.trains.len() > 64
            || !self.services.keys().any(|id| road_tag(id))
        {
            return false;
        }
        for (id, service) in &self.services {
            let road = road_tag(id);
            if id != &service.spec.service_id
                || id.len() > 128
                || !charge(64 + id.len())
                || service.legs.len() != service.spec.legs.len()
                || (!road && id.parse::<u64>().is_err())
            {
                return false;
            }
            have_bus |= road;
            have_rail |= !road;
            let station_in_family = |key: &str| {
                if road {
                    road_tag(key)
                } else {
                    key.parse::<u64>().is_ok()
                }
            };
            for (spec, leg) in service.spec.legs.iter().zip(&service.legs) {
                if !charge(
                    128usize
                        .saturating_add(spec.from.len())
                        .saturating_add(spec.to.len()),
                ) || !station_in_family(&spec.from)
                    || !station_in_family(&spec.to)
                    || leg.resources.is_empty()
                {
                    return false;
                }
                // Crossing, lane, point, platform and terminal ownership all
                // come from these compiled locks, including retained leases.
                for key in leg
                    .resources
                    .iter()
                    .chain(leg.early_releases.keys())
                    .chain(leg.through_lane.iter())
                {
                    if !charge(64usize.saturating_add(key.len())) || !lock_in_family(key, road) {
                        return false;
                    }
                }
            }
        }
        for (resource, owner) in &self.occupied {
            if !charge(64usize.saturating_add(resource.len()))
                || !self.trains.contains_key(owner)
                || !self.services.contains_key(&owner.service_id)
                || !lock_in_family(resource, road_tag(&owner.service_id))
            {
                return false;
            }
        }
        for (key, train) in &self.trains {
            let Some(leg) = self
                .services
                .get(&key.service_id)
                .and_then(|s| s.legs.get(train.leg))
            else {
                return false;
            };
            if train.key != *key {
                return false;
            }
            if !road_tag(&key.service_id)
                && matches!(train.state, TrainState::Waiting {ready_ms} if ready_ms <= self.now_ms)
            {
                // A previously eligible but unblocked waiter would use the
                // next event as a wake-up even if that event were unrelated.
                // Valid live frames normally admit it immediately; otherwise
                // leave the full timeline in place, rather than guess.
                let mut blocked = false;
                for resource in &leg.resources {
                    if !charge(64usize.saturating_add(resource.len())) {
                        return false;
                    }
                    if self.reservation_blocks(resource, key) {
                        blocked = true;
                        break;
                    }
                }
                if !blocked {
                    return false;
                }
            }
        }
        have_bus && have_rail
    }

    fn preview_passenger_replay_in_scope(
        &self,
        until_ms: u64,
        events: usize,
        omit_proven_road: bool,
    ) -> Result<PassengerReplay, PassengerReplayError> {
        let keep = |id: &str| !omit_proven_road || !id.starts_with("bus:");
        const MAX_SPAN_MS: u64 = 3_600_000;
        const MAX_EVENTS: usize = 256;
        const MAX_SERVICES: usize = 64;
        const MAX_TRAINS: u64 = 64;
        const MAX_STATIC_WEIGHT: usize = 262_144;
        let count_services = if omit_proven_road {
            self.services.keys().filter(|id| keep(id)).count()
        } else {
            self.services.len()
        };
        if until_ms
            .checked_sub(self.now_ms)
            .is_none_or(|span| span > MAX_SPAN_MS)
            || events == 0
            || events > MAX_EVENTS
            || count_services > MAX_SERVICES
        {
            return Err(PassengerReplayError::OutsideBudget);
        }
        // Count scheduled births in the whole requested window, even if
        // some trains would have finished before other trains depart.
        let mut trains = if omit_proven_road {
            self.trains
                .keys()
                .filter(|key| keep(&key.service_id))
                .count()
        } else {
            self.trains.len()
        } as u64;
        let mut weight = 0usize;
        let mut spend = |units: usize| -> Result<(), PassengerReplayError> {
            weight = weight
                .checked_add(units)
                .filter(|&total| total <= MAX_STATIC_WEIGHT)
                .ok_or(PassengerReplayError::OutsideBudget)?;
            Ok(())
        };
        for service in self
            .services
            .values()
            .filter(|service| keep(&service.spec.service_id))
        {
            trains = trains.saturating_add(service.future_births_through(until_ms));
            if trains > MAX_TRAINS || service.spec.service_id.len() > 128 {
                return Err(PassengerReplayError::OutsideBudget);
            }
            spend(service.spec.service_id.len())?;
            for leg in &service.spec.legs {
                if leg.from.len() > 128 || leg.to.len() > 128 {
                    return Err(PassengerReplayError::OutsideBudget);
                }
                spend(128 + leg.from.len() + leg.to.len())?;
                for section in &leg.sections {
                    spend(64)?;
                    for resource in &section.resources {
                        spend(resource.len().saturating_add(128))?;
                    }
                }
            }
        }
        let mut shadow = if omit_proven_road {
            Self {
                now_ms: self.now_ms,
                paused: self.paused,
                speed: self.speed,
                started: self.started,
                time_margin_ms: self.time_margin_ms,
                rail_admission: self.rail_admission,
                preferred_opposing_births: self.preferred_opposing_births.clone(),
                // Replay does not serialize or validate historical births.
                timetable_handoffs: BTreeMap::new(),
                #[cfg(feature = "standard-periodic-rail")]
                standard_timetable: None,
                services: self
                    .services
                    .iter()
                    .filter(|(id, _)| keep(id))
                    .map(|(id, service)| (id.clone(), service.clone()))
                    .collect(),
                trains: self
                    .trains
                    .iter()
                    .filter(|(key, _)| keep(&key.service_id))
                    .map(|(key, train)| (key.clone(), train.clone()))
                    .collect(),
                occupied: self
                    .occupied
                    .iter()
                    .filter(|(_, owner)| keep(&owner.service_id))
                    .map(|(resource, owner)| (resource.clone(), owner.clone()))
                    .collect(),
            }
        } else {
            self.clone()
        };
        let mut completed_legs = Vec::new();
        let result = shadow
            .advance_to_inner(until_ms, events, Some(&mut completed_legs))
            .map_err(PassengerReplayError::Runtime)?;
        Ok(PassengerReplay {
            observed_from_ms: self.now_ms,
            requested_until_ms: until_ms,
            observed_until_ms: result.now_ms,
            complete: result.reached_target,
            completed_legs,
        })
    }

    /// Call on the authority once per elapsed interval, never per viewer.
    /// A zero event budget makes no progress. Continue toward the SAME target
    /// after backpressure; do not add the wall delta again.
    pub fn wall_target(&self, elapsed_ms: u64) -> Result<u64, OperationError> {
        if self.paused {
            return Ok(self.now_ms);
        }
        self.now_ms
            .checked_add(
                elapsed_ms
                    .checked_mul(u64::from(self.speed))
                    .ok_or(OperationError::Overflow)?,
            )
            .ok_or(OperationError::Overflow)
    }
    pub fn advance_to(
        &mut self,
        target: u64,
        event_budget: usize,
    ) -> Result<AdvanceResult, OperationError> {
        let result = self.advance_to_inner(target, event_budget, None)?;
        if !self.timetable_handoffs.is_empty() {
            self.timetable_handoffs.retain(|id, handoff| {
                Self::has_live_birth_at_or_before(&self.trains, id, handoff.cutover_ms)
            });
        }
        Ok(result)
    }

    fn advance_to_inner(
        &mut self,
        target: u64,
        event_budget: usize,
        mut replay: Option<&mut Vec<ReplayedPassengerLeg>>,
    ) -> Result<AdvanceResult, OperationError> {
        if target < self.now_ms {
            return Err(OperationError::InvalidClock);
        }
        if target.checked_add(self.time_margin_ms).is_none() {
            return Err(OperationError::Overflow);
        }
        let mut departure = self
            .services
            .values()
            .filter_map(|service| service.next_departure)
            .min();
        let mut arrivals = Vec::new();
        let mut due_movements = Vec::new();
        let progressive_rail = self.progressive_rail();
        let mut events = 0;
        loop {
            let next = self.next_event(departure);
            let Some(at) = next.filter(|at| *at <= target) else {
                self.started = true;
                self.now_ms = target;
                return Ok(AdvanceResult {
                    now_ms: target,
                    reached_target: true,
                    arrivals,
                });
            };
            if events >= event_budget {
                return Ok(AdvanceResult {
                    now_ms: self.now_ms,
                    reached_target: false,
                    arrivals,
                });
            }
            events += 1;
            self.started = true;
            self.now_ms = at;
            // Retain train-key order for simultaneous events without cloning idle
            // trains or movements whose arrival/clearance is still in the future.
            // Arrivals release resources only after the rear has cleared them.
            due_movements.extend(self.trains.values().filter_map(|train| {
                let TrainState::Moving {
                    entered_ms,
                    arrival_ms,
                    release_ms,
                } = train.state
                else {
                    return None;
                };
                let early_release = progressive_rail
                    && self.services[&train.key.service_id].legs[train.leg]
                        .release_events
                        .contains(&(at - entered_ms));
                (arrival_ms == at || release_ms == at || early_release)
                    .then(|| (train.key.clone(), early_release))
            }));
            for (key, early_release) in due_movements.drain(..) {
                let train = &self.trains[&key];
                if let TrainState::Moving {
                    entered_ms,
                    arrival_ms,
                    release_ms,
                } = train.state
                {
                    if arrival_ms == at {
                        let leg = &self.services[&key.service_id].spec.legs[train.leg];
                        if let Some(observations) = replay.as_mut() {
                            observations.push(ReplayedPassengerLeg {
                                train: key.clone(),
                                sequence: train.leg,
                                from: leg.from.clone(),
                                to: leg.to.clone(),
                                entered_ms,
                                arrived_ms: at,
                                cumulative_reservation_wait_ms: train.delay_ms,
                            });
                        }
                        arrivals.push(Arrival {
                            train: key.clone(),
                            station_id: leg.to.clone(),
                            at_ms: at,
                            delay_ms: train.delay_ms,
                        });
                    }
                    if early_release {
                        for (resource, &offset) in
                            &self.services[&key.service_id].legs[train.leg].early_releases
                        {
                            if offset == at - entered_ms
                                && self.occupied.get(resource) == Some(&key)
                            {
                                self.occupied.remove(resource);
                            }
                        }
                    }
                    if release_ms == at {
                        let mut train = self.trains.remove(&key).expect("collected existing train");
                        let service = &self.services[&key.service_id];
                        for resource in &service.legs[train.leg].resources {
                            // This train may have cleared the lane earlier and
                            // another train may now own it.
                            let retain = (service.terminal_hold
                                && train.leg == 0
                                && service.legs[1].resources.contains(resource))
                                || service.legs[train.leg].through_lane.as_ref() == Some(resource);
                            if !retain && self.occupied.get(resource) == Some(&key) {
                                self.occupied.remove(resource);
                            }
                        }
                        let ready_ms = arrival_ms
                            .checked_add(service.spec.legs[train.leg].dwell_ms)
                            .ok_or(OperationError::Overflow)?
                            .max(release_ms);
                        train.leg += 1;
                        if train.leg < service.legs.len() {
                            train.state = TrainState::Waiting { ready_ms };
                            self.trains.insert(key, train);
                        }
                    }
                }
            }
            // Train events do not change timetable cursors; preserving the
            // minimum in this advance avoids rescanning every service twice
            // at arrival, rear-clear, or dwell-only events.
            let has_new_departure = departure == Some(at);
            if has_new_departure {
                departure = None;
                for (id, s) in &mut self.services {
                    if s.next_departure == Some(at) {
                        let key = TrainKey {
                            service_id: id.clone(),
                            scheduled_ms: at,
                        };
                        self.trains.insert(
                            key.clone(),
                            Train {
                                key,
                                leg: 0,
                                delay_ms: 0,
                                state: TrainState::Waiting { ready_ms: at },
                            },
                        );
                        s.next_departure = s.next_unprocessed_departure(true, at);
                    }
                    if let Some(next) = s.next_departure {
                        departure = Some(departure.map_or(next, |previous| previous.min(next)));
                    }
                }
            }
            let mut waiting: Vec<_> = self
                .trains
                .values()
                .filter_map(|t| match t.state {
                    TrainState::Waiting { ready_ms } if ready_ms <= at => {
                        Some((ready_ms, t.key.clone()))
                    }
                    _ => None,
                })
                .collect();
            waiting.sort();
            if has_new_departure {
                self.prefer_proved_birth_ties(at, &mut waiting);
            }
            for (ready, key) in waiting {
                let t = &self.trains[&key];
                let c = &self.services[&key.service_id].legs[t.leg];
                if c.resources.iter().any(|r| self.reservation_blocks(r, &key)) {
                    continue;
                }
                let arrival_ms = at
                    .checked_add(c.duration_ms)
                    .ok_or(OperationError::Overflow)?;
                let release_ms = arrival_ms
                    .checked_add(c.clearance_ms)
                    .ok_or(OperationError::Overflow)?;
                for r in &c.resources {
                    self.occupied.insert(r.clone(), key.clone());
                }
                let t = self.trains.get_mut(&key).expect("selected waiting train");
                t.delay_ms = t
                    .delay_ms
                    .checked_add(at - ready)
                    .ok_or(OperationError::Overflow)?;
                t.state = TrainState::Moving {
                    entered_ms: at,
                    arrival_ms,
                    release_ms,
                };
            }
        }
    }
    fn next_event(&self, mut next: Option<u64>) -> Option<u64> {
        for t in self.trains.values() {
            let at = match t.state {
                TrainState::Moving {
                    entered_ms,
                    arrival_ms,
                    release_ms,
                } => {
                    let next = if arrival_ms > self.now_ms {
                        arrival_ms
                    } else {
                        release_ms
                    };
                    let event = self.services[&t.key.service_id].legs[t.leg]
                        .release_events
                        .range((
                            std::ops::Bound::Excluded(self.now_ms - entered_ms),
                            std::ops::Bound::Unbounded,
                        ))
                        .next()
                        .map(|offset| entered_ms + offset);
                    Some(event.map_or(next, |at| next.min(at)))
                }
                TrainState::Waiting { ready_ms } => {
                    if ready_ms > self.now_ms {
                        Some(ready_ms)
                    } else {
                        None
                    }
                }
            };
            if let Some(at) = at {
                next = Some(next.map_or(at, |n| n.min(at)));
            }
        }
        next
    }
    /// Persist active state directly, without replaying all elapsed service days.
    /// Restore validates schedule times and exclusive occupancy before acceptance.
    pub fn save(&self) -> OperationSave {
        OperationSave {
            format: self.rail_admission as u32,
            trains: self.trains.values().cloned().collect(),
            next_departures: self
                .services
                .iter()
                .map(|(id, s)| (id.clone(), s.next_departure))
                .collect(),
            started: self.started,
            now_ms: self.now_ms,
            paused: self.paused,
            speed: self.speed,
            services: self.services.values().map(|s| s.spec.clone()).collect(),
            timetable_handoffs: self
                .timetable_handoffs
                .iter()
                .map(|(id, handoff)| (id.clone(), handoff.clone()))
                .collect(),
        }
    }
    pub fn restore(saved: OperationSave) -> Result<Self, OperationError> {
        Self::restore_checked(saved, None)
    }

    pub(crate) fn restore_with_terminal_hold(saved: OperationSave) -> Result<Self, OperationError> {
        Self::restore_checked(saved, Some(RailAdmissionModel::TurnbackHold))
    }

    pub(crate) fn restore_with_connected_terminal_neck(
        saved: OperationSave,
    ) -> Result<Self, OperationError> {
        Self::restore_checked(saved, Some(RailAdmissionModel::ConnectedTerminalNeck))
    }

    pub(crate) fn restore_with_progressive_rail(
        saved: OperationSave,
    ) -> Result<Self, OperationError> {
        Self::restore_checked(saved, Some(RailAdmissionModel::Progressive))
    }

    pub(crate) fn restore_with_inline_terminal_dwell(
        saved: OperationSave,
    ) -> Result<Self, OperationError> {
        Self::restore_checked(saved, Some(RailAdmissionModel::InlineTerminalDwell))
    }

    pub(crate) fn restore_with_inline_platform_handoff(
        saved: OperationSave,
    ) -> Result<Self, OperationError> {
        Self::restore_checked(saved, Some(RailAdmissionModel::InlinePlatformHandoff))
    }

    pub(crate) fn restore_with_fixed_refuge(saved: OperationSave) -> Result<Self, OperationError> {
        Self::restore_checked(saved, Some(RailAdmissionModel::FixedRefuge))
    }

    fn restore_checked(
        saved: OperationSave,
        admitted: Option<RailAdmissionModel>,
    ) -> Result<Self, OperationError> {
        Self::restore_checked_with_source(saved, admitted, None)
    }

    pub(crate) fn restore_checked_with_source(
        saved: OperationSave,
        admitted: Option<RailAdmissionModel>,
        periodic: Option<&ScheduleSource>,
    ) -> Result<Self, OperationError> {
        let rail_admission = match saved.format {
            2 => RailAdmissionModel::Fixed,
            3 if cfg!(test) || admitted == Some(RailAdmissionModel::Progressive) => {
                RailAdmissionModel::Progressive
            }
            4 if admitted == Some(RailAdmissionModel::TurnbackHold) => {
                RailAdmissionModel::TurnbackHold
            }
            5 if admitted == Some(RailAdmissionModel::InlineTerminalDwell) => {
                RailAdmissionModel::InlineTerminalDwell
            }
            6 if admitted == Some(RailAdmissionModel::InlinePlatformHandoff) => {
                RailAdmissionModel::InlinePlatformHandoff
            }
            7 if admitted == Some(RailAdmissionModel::FixedRefuge) => {
                RailAdmissionModel::FixedRefuge
            }
            8 if admitted == Some(RailAdmissionModel::ConnectedTerminalNeck) => {
                RailAdmissionModel::ConnectedTerminalNeck
            }
            _ => return Err(OperationError::InvalidClock),
        };
        if !saved.started && (saved.now_ms != 0 || !saved.trains.is_empty()) {
            return Err(OperationError::InvalidClock);
        }
        let mut runtime = Self::compile(saved.services, rail_admission)?;
        runtime.set_speed(saved.speed)?;
        runtime.now_ms = saved.now_ms;
        runtime.started = saved.started;
        if let Some(schedule) = periodic {
            if !saved.timetable_handoffs.is_empty() {
                return Err(OperationError::InvalidTimetable);
            }
            runtime.bind_periodic_schedule(schedule, true)?;
        }
        if saved.next_departures.len() != runtime.services.len() {
            return Err(OperationError::InvalidTimetable);
        }
        let mut seen = BTreeSet::new();
        for (id, next) in saved.next_departures {
            if !seen.insert(id.clone()) {
                return Err(OperationError::InvalidTimetable);
            }
            let service = runtime
                .services
                .get_mut(&id)
                .ok_or(OperationError::InvalidTimetable)?;
            let expected = service.next_unprocessed_departure(saved.started, saved.now_ms);
            if next != expected {
                return Err(OperationError::InvalidTimetable);
            }
            service.next_departure = next;
        }
        if saved.timetable_handoffs.len() > runtime.services.len() {
            return Err(OperationError::InvalidTimetable);
        }
        for (id, handoff) in saved.timetable_handoffs {
            let service = runtime
                .services
                .get(&id)
                .ok_or(OperationError::InvalidTimetable)?;
            if !saved.started
                || id.parse::<u64>().is_err()
                || handoff.cutover_ms > saved.now_ms
                || !handoff.previous.is_valid()
                || handoff.previous == service.spec.timetable
                || service
                    .spec
                    .timetable
                    .next_unprocessed_departure(true, handoff.cutover_ms)
                    .is_none()
                || handoff
                    .previous
                    .last_departure_ms
                    .checked_add(service.journey_ms)
                    .is_none()
                || runtime.timetable_handoffs.insert(id, handoff).is_some()
            {
                return Err(OperationError::InvalidTimetable);
            }
        }
        for train in saved.trains {
            let service = runtime
                .services
                .get(&train.key.service_id)
                .ok_or(OperationError::InvalidPath)?;
            let leg = service
                .legs
                .get(train.leg)
                .ok_or(OperationError::InvalidPath)?;
            let birth = train.key.scheduled_ms;
            let authorized = runtime
                .timetable_handoffs
                .get(&train.key.service_id)
                .filter(|handoff| birth <= handoff.cutover_ms)
                .map_or_else(
                    || service.scheduled(birth),
                    |handoff| handoff.previous.contains(birth),
                );
            if birth > saved.now_ms || !authorized {
                return Err(OperationError::InvalidTimetable);
            }
            // Inline-terminal services have only their first leg. Their initial
            // wait is anchored to the timetable and incurs delay only when
            // admitted. A self-consistent profile at the wrong epoch would
            // otherwise restore a shortened lease and an invented replay ride.
            if runtime.inline_terminal_dwell() {
                let scheduled_ms = train.key.scheduled_ms;
                let valid = match train.state {
                    TrainState::Waiting { ready_ms } => {
                        ready_ms == scheduled_ms && train.delay_ms == 0
                    }
                    TrainState::Moving { entered_ms, .. } => {
                        entered_ms.checked_sub(scheduled_ms) == Some(train.delay_ms)
                    }
                };
                if !valid {
                    return Err(OperationError::InvalidClock);
                }
            }
            if rail_admission == RailAdmissionModel::ConnectedTerminalNeck && service.terminal_hold
            {
                let valid = match train.state {
                    TrainState::Waiting { ready_ms } => {
                        train.leg == 0 && ready_ms == birth && train.delay_ms == 0
                    }
                    TrainState::Moving { entered_ms, .. } => {
                        birth
                            .checked_add(leg.nominal_entry_ms)
                            .and_then(|at| at.checked_add(train.delay_ms))
                            == Some(entered_ms)
                    }
                };
                if !valid {
                    return Err(OperationError::InvalidClock);
                }
            }
            if runtime.parked_platform_handoff() {
                let scheduled = train
                    .key
                    .scheduled_ms
                    .checked_add(leg.nominal_entry_ms)
                    .ok_or(OperationError::InvalidClock)?;
                let delayed = scheduled
                    .checked_add(train.delay_ms)
                    .ok_or(OperationError::InvalidClock)?;
                let valid = match train.state {
                    TrainState::Waiting { ready_ms } => {
                        if let Some(i) = train.leg.checked_sub(1) {
                            let before = &service.legs[i];
                            let remaining_dwell = service.spec.legs[i]
                                .dwell_ms
                                .saturating_sub(before.clearance_ms);
                            ready_ms == delayed
                                && ready_ms
                                    .checked_sub(remaining_dwell)
                                    .is_some_and(|rear_clear| rear_clear <= saved.now_ms)
                        } else {
                            ready_ms == scheduled && train.delay_ms == 0
                        }
                    }
                    TrainState::Moving { entered_ms, .. } => entered_ms == delayed,
                };
                if !valid {
                    return Err(OperationError::InvalidClock);
                }
                if let (TrainState::Waiting { .. }, Some(i)) =
                    (&train.state, train.leg.checked_sub(1))
                {
                    let held = service.legs[i]
                        .through_lane
                        .as_ref()
                        .ok_or(OperationError::InvalidPath)?;
                    if runtime
                        .occupied
                        .insert(held.clone(), train.key.clone())
                        .is_some()
                    {
                        return Err(OperationError::InvalidPath);
                    }
                }
            }
            if let TrainState::Moving {
                entered_ms,
                arrival_ms,
                release_ms,
            } = train.state
            {
                if entered_ms > saved.now_ms
                    || release_ms <= saved.now_ms
                    || entered_ms.checked_add(leg.duration_ms) != Some(arrival_ms)
                    || arrival_ms.checked_add(leg.clearance_ms) != Some(release_ms)
                {
                    return Err(OperationError::InvalidClock);
                }
                for resource in &leg.resources {
                    if leg
                        .early_releases
                        .get(resource)
                        .is_some_and(|offset| *offset <= saved.now_ms - entered_ms)
                    {
                        continue;
                    }
                    if runtime
                        .occupied
                        .insert(resource.clone(), train.key.clone())
                        .is_some()
                    {
                        return Err(OperationError::InvalidPath);
                    }
                }
            }
            if runtime.trains.insert(train.key.clone(), train).is_some() {
                return Err(OperationError::InvalidPath);
            }
        }
        for train in runtime.trains.values() {
            if train.delay_ms > saved.now_ms.saturating_sub(train.key.scheduled_ms) {
                return Err(OperationError::InvalidClock);
            }
            if let TrainState::Waiting { ready_ms } = train.state {
                if ready_ms < train.key.scheduled_ms
                    || (runtime.services[&train.key.service_id].terminal_hold && train.leg != 0)
                {
                    return Err(OperationError::InvalidClock);
                }
                let resources = &runtime.services[&train.key.service_id].legs[train.leg].resources;
                if ready_ms <= saved.now_ms
                    && !resources.iter().any(|r| {
                        runtime.occupied.get(r).is_some_and(|owner| {
                            !runtime.own_lease_allows_admission(&train.key.service_id)
                                || *owner != train.key
                        })
                    })
                {
                    return Err(OperationError::InvalidClock);
                }
            }
        }
        if runtime.timetable_handoffs.iter().any(|(id, handoff)| {
            !Self::has_live_birth_at_or_before(&runtime.trains, id, handoff.cutover_ms)
        }) {
            return Err(OperationError::InvalidTimetable);
        }
        runtime.paused = saved.paused;
        Ok(runtime)
    }
}
#[cfg(test)]
mod tests;
