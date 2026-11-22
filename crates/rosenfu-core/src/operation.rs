//! Deterministic, headless operation. Infrastructure adapters supply physical
//! distances, caps and shared resource IDs; the renderer never owns this clock.
use serde::{Deserialize, Serialize};
mod passenger_replay;
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
}

impl RailAdmissionModel {
    fn progressive(self) -> bool {
        matches!(
            self,
            Self::Progressive | Self::InlineTerminalDwell | Self::InlinePlatformHandoff
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OperationRuntime {
    now_ms: u64,
    paused: bool,
    speed: u32,
    started: bool,
    services: BTreeMap<String, CompiledService>,
    trains: BTreeMap<TrainKey, Train>,
    occupied: BTreeMap<String, TrainKey>,
    rail_admission: RailAdmissionModel,
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

    fn compile(
        specs: Vec<ServiceRunSpec>,
        rail_admission: RailAdmissionModel,
    ) -> Result<Self, OperationError> {
        let mut services = BTreeMap::new();
        for spec in specs {
            let t = &spec.timetable;
            if spec.service_id.trim().is_empty()
                || services.contains_key(&spec.service_id)
                || t.headway_ms == 0
                || t.first_departure_ms > t.last_departure_ms
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
            let mut legs = Vec::new();
            let mut journey = 0u64;
            let mut nominal_entry_ms = 0u64;
            let platform = rail_admission == RailAdmissionModel::InlineTerminalDwell;
            let handoff = rail_admission == RailAdmissionModel::InlinePlatformHandoff;
            if handoff
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
                    || (rail_admission == RailAdmissionModel::TurnbackHold && index == 0)
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
                legs.push(CompiledLeg {
                    duration_ms,
                    clearance_ms,
                    resources: l
                        .sections
                        .iter()
                        .flat_map(|s| s.resources.iter().cloned())
                        .collect(),
                    early_releases,
                    release_events,
                    nominal_entry_ms,
                    through_lane: (handoff && index + 1 < spec.legs.len()).then(|| {
                        l.sections.last().expect("proved nonempty leg").resources[0].clone()
                    }),
                });
                if handoff {
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
                    next_departure,
                },
            );
        }
        Ok(Self {
            now_ms: 0,
            paused: true,
            speed: 1,
            started: false,
            services,
            trains: BTreeMap::new(),
            occupied: BTreeMap::new(),
            rail_admission,
        })
    }
    pub(crate) fn progressive_rail(&self) -> bool {
        self.rail_admission.progressive()
    }

    pub(crate) fn terminal_hold(&self) -> bool {
        self.rail_admission == RailAdmissionModel::TurnbackHold
    }

    pub(crate) fn inline_terminal_dwell(&self) -> bool {
        self.rail_admission == RailAdmissionModel::InlineTerminalDwell
    }

    pub(crate) fn inline_platform_handoff(&self) -> bool {
        self.rail_admission == RailAdmissionModel::InlinePlatformHandoff
    }

    fn own_lease_allows_admission(&self) -> bool {
        self.terminal_hold() || self.inline_platform_handoff()
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
            service.next_departure = service
                .spec
                .timetable
                .next_unprocessed_departure(self.started, self.now_ms);
        }
        Ok(())
    }

    pub(crate) fn has_started(&self) -> bool {
        self.started
    }

    pub(crate) fn admission_model(&self) -> RailAdmissionModel {
        self.rail_admission
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
    pub fn now_ms(&self) -> u64 {
        self.now_ms
    }
    pub fn trains(&self) -> impl Iterator<Item = &Train> {
        self.trains.values()
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
        const MAX_SPAN_MS: u64 = 3_600_000;
        const MAX_EVENTS: usize = 256;
        const MAX_SERVICES: usize = 64;
        const MAX_TRAINS: u64 = 64;
        const MAX_STATIC_WEIGHT: usize = 262_144;
        if until_ms
            .checked_sub(self.now_ms)
            .is_none_or(|span| span > MAX_SPAN_MS)
            || events == 0
            || events > MAX_EVENTS
            || self.services.len() > MAX_SERVICES
        {
            return Err(PassengerReplayError::OutsideBudget);
        }
        // Count scheduled births in the whole requested window, even if
        // some trains would have finished before other trains depart.
        let mut trains = self.trains.len() as u64;
        let mut weight = 0usize;
        let mut spend = |units: usize| -> Result<(), PassengerReplayError> {
            weight = weight
                .checked_add(units)
                .filter(|&total| total <= MAX_STATIC_WEIGHT)
                .ok_or(PassengerReplayError::OutsideBudget)?;
            Ok(())
        };
        for service in self.services.values() {
            let t = &service.spec.timetable;
            let end = t.last_departure_ms.min(until_ms);
            if let Some(next) = service.next_departure.filter(|&next| next <= end) {
                trains = trains
                    .saturating_add((end - next) / t.headway_ms)
                    .saturating_add(1);
            }
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
        let mut shadow = self.clone();
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
        self.advance_to_inner(target, event_budget, None)
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
        let margin = self
            .services
            .values()
            .flat_map(|s| s.legs.iter().zip(&s.spec.legs))
            .map(|(c, l)| {
                c.duration_ms
                    .saturating_add(c.clearance_ms)
                    .saturating_add(l.dwell_ms)
            })
            .max()
            .unwrap_or(0);
        if target.checked_add(margin).is_none() {
            return Err(OperationError::Overflow);
        }
        let mut arrivals = Vec::new();
        let mut events = 0;
        loop {
            let next = self.next_event();
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
            // Arrivals release resources only after the rear has cleared them.
            let keys: Vec<_> = self.trains.keys().cloned().collect();
            for key in keys {
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
                    if self.progressive_rail() {
                        let leg = &self.services[&key.service_id].legs[train.leg];
                        if leg.release_events.contains(&(at - entered_ms)) {
                            for (resource, &offset) in &leg.early_releases {
                                if offset == at - entered_ms
                                    && self.occupied.get(resource) == Some(&key)
                                {
                                    self.occupied.remove(resource);
                                }
                            }
                        }
                    }
                    if release_ms == at {
                        let mut train = self.trains.remove(&key).expect("collected existing train");
                        let service = &self.services[&key.service_id];
                        for resource in &service.legs[train.leg].resources {
                            // This train may have cleared the lane earlier and
                            // another train may now own it.
                            let retain = (self.terminal_hold() && train.leg == 0)
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
                    s.next_departure = at
                        .checked_add(s.spec.timetable.headway_ms)
                        .filter(|n| *n <= s.spec.timetable.last_departure_ms);
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
            for (ready, key) in waiting {
                let t = &self.trains[&key];
                let c = &self.services[&key.service_id].legs[t.leg];
                if c.resources.iter().any(|r| {
                    self.occupied
                        .get(r)
                        .is_some_and(|owner| !self.own_lease_allows_admission() || *owner != key)
                }) {
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
    fn next_event(&self) -> Option<u64> {
        let mut next = self
            .services
            .values()
            .filter_map(|s| s.next_departure)
            .min();
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
        }
    }
    pub fn restore(saved: OperationSave) -> Result<Self, OperationError> {
        Self::restore_checked(saved, None)
    }

    pub(crate) fn restore_with_terminal_hold(saved: OperationSave) -> Result<Self, OperationError> {
        Self::restore_checked(saved, Some(RailAdmissionModel::TurnbackHold))
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

    fn restore_checked(
        saved: OperationSave,
        admitted: Option<RailAdmissionModel>,
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
            _ => return Err(OperationError::InvalidClock),
        };
        if !saved.started && (saved.now_ms != 0 || !saved.trains.is_empty()) {
            return Err(OperationError::InvalidClock);
        }
        let mut runtime = Self::compile(saved.services, rail_admission)?;
        runtime.set_speed(saved.speed)?;
        runtime.now_ms = saved.now_ms;
        runtime.started = saved.started;
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
            let table = &service.spec.timetable;
            if let Some(t) = next {
                if t < table.first_departure_ms
                    || t > table.last_departure_ms
                    || (t - table.first_departure_ms) % table.headway_ms != 0
                    || (saved.started && t <= saved.now_ms)
                {
                    return Err(OperationError::InvalidTimetable);
                }
            }
            let expected = table.next_unprocessed_departure(saved.started, saved.now_ms);
            if next != expected {
                return Err(OperationError::InvalidTimetable);
            }
            service.next_departure = next;
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
            let table = &service.spec.timetable;
            if train.key.scheduled_ms > saved.now_ms
                || train.key.scheduled_ms < table.first_departure_ms
                || train.key.scheduled_ms > table.last_departure_ms
                || (train.key.scheduled_ms - table.first_departure_ms) % table.headway_ms != 0
            {
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
            if runtime.inline_platform_handoff() {
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
                if ready_ms < train.key.scheduled_ms || (runtime.terminal_hold() && train.leg != 0)
                {
                    return Err(OperationError::InvalidClock);
                }
                let resources = &runtime.services[&train.key.service_id].legs[train.leg].resources;
                if ready_ms <= saved.now_ms
                    && !resources.iter().any(|r| {
                        runtime.occupied.get(r).is_some_and(|owner| {
                            !runtime.inline_platform_handoff() || *owner != train.key
                        })
                    })
                {
                    return Err(OperationError::InvalidClock);
                }
            }
        }
        runtime.paused = saved.paused;
        Ok(runtime)
    }
}
#[cfg(test)]
mod tests;
