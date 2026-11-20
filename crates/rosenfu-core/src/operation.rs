//! Deterministic, headless operation. Infrastructure adapters supply physical
//! distances, caps and shared resource IDs; the renderer never owns this clock.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationError {
    InvalidTimetable,
    InvalidPath,
    InvalidVehicle,
    InvalidClock,
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
}
#[derive(Debug, Clone, PartialEq)]
struct CompiledService {
    spec: ServiceRunSpec,
    legs: Vec<CompiledLeg>,
    next_departure: Option<u64>,
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
    }
    millis(seconds)
}
impl OperationRuntime {
    pub fn new(specs: Vec<ServiceRunSpec>) -> Result<Self, OperationError> {
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
            for l in &spec.legs {
                let duration_ms = running_time_ms(&l.sections, &spec.vehicle)?;
                let lowest = l
                    .sections
                    .iter()
                    .map(|s| s.speed_limit_kmh.min(spec.vehicle.max_speed_kmh))
                    .fold(f64::INFINITY, f64::min)
                    / 3.6;
                let clearance_ms = millis(spec.vehicle.length_m / lowest)?;
                journey = journey
                    .checked_add(duration_ms)
                    .and_then(|n| n.checked_add(clearance_ms))
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
                });
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
        })
    }
    pub fn paused(&self) -> bool {
        self.paused
    }
    pub fn speed(&self) -> u32 {
        self.speed
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
                        ride_seconds: (c.duration_ms
                            + if i == 0 {
                                0
                            } else {
                                s.spec.legs[i - 1].dwell_ms.max(s.legs[i - 1].clearance_ms)
                            }) as f64
                            / 1000.0,
                        headway_seconds: s.spec.timetable.headway_ms as f64 / 1000.0,
                    })
            })
            .collect()
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
                    arrival_ms,
                    release_ms,
                    ..
                } = train.state
                {
                    if arrival_ms == at {
                        arrivals.push(Arrival {
                            train: key.clone(),
                            station_id: self.services[&key.service_id].spec.legs[train.leg]
                                .to
                                .clone(),
                            at_ms: at,
                            delay_ms: train.delay_ms,
                        });
                    }
                    if release_ms == at {
                        let mut train = self.trains.remove(&key).expect("collected existing train");
                        let service = &self.services[&key.service_id];
                        for resource in &service.legs[train.leg].resources {
                            self.occupied.remove(resource);
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
                if c.resources.iter().any(|r| self.occupied.contains_key(r)) {
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
                    arrival_ms,
                    release_ms,
                    ..
                } => {
                    if arrival_ms > self.now_ms {
                        Some(arrival_ms)
                    } else {
                        Some(release_ms)
                    }
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
            format: 2,
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
        if saved.format != 2 || (!saved.started && (saved.now_ms != 0 || !saved.trains.is_empty()))
        {
            return Err(OperationError::InvalidClock);
        }
        let mut runtime = Self::new(saved.services)?;
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
                if ready_ms < train.key.scheduled_ms {
                    return Err(OperationError::InvalidClock);
                }
                let resources = &runtime.services[&train.key.service_id].legs[train.leg].resources;
                if ready_ms <= saved.now_ms
                    && !resources.iter().any(|r| runtime.occupied.contains_key(r))
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
