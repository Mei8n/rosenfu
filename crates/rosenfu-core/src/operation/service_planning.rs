//! Nominal circulation only; this does not change dispatch, buy trains or prove capacity.
use super::{rail_lane, OperationRuntime, RunLeg, Section};
pub mod periodic;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(
    tag = "mode",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum FleetPlanningRequest {
    TimedHeadway { headway_ms: u64 },
    TrainsetCount { trainsets: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NominalFleetPlanning {
    pub cycle_ms: u64,
    pub running_ms: u64,
    /// Interstation waits and both terminal stands; dwell and clearance overlap.
    pub dwell_and_clearance_ms: u64,
    pub headway_ms: u64,
    pub trainsets: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleetPlanningError {
    UnsupportedRun,
    InvalidInput,
    TimeOutsideBudget,
}

// Exact milliseconds on the JSON side too. This is a calculator bound,
// not a new restriction on persisted timetable intervals or the simulation.
pub(super) const MAX_SAFE_MS: u64 = 9_007_199_254_740_991;
const MAX_LEGS: usize = 128;

fn endpoint_lane(section: Option<&Section>) -> Option<&str> {
    let mut lanes = section?.resources.iter().filter(|id| rail_lane(id));
    let lane = lanes.next()?;
    lanes.next().is_none().then_some(lane.as_str())
}

fn same_lane(incoming: &RunLeg, leaving: &RunLeg) -> bool {
    let a = endpoint_lane(incoming.sections.last());
    a.is_some() && a == endpoint_lane(leaving.sections.first())
}

impl OperationRuntime {
    /// Offer a capacity-free planning estimate for an already configured,
    /// physically reusable rail circuit: an exact mirrored return on the
    /// same platform lane at each end. Legacy one-way runs, arbitrary
    /// loops, and unspecified transfers are not guessed as train reuse.
    pub fn preview_nominal_fleet(
        &self,
        service_id: &str,
        request: FleetPlanningRequest,
    ) -> Result<NominalFleetPlanning, FleetPlanningError> {
        use FleetPlanningError::{InvalidInput, TimeOutsideBudget, UnsupportedRun};
        if service_id.parse::<u64>().is_err() {
            return Err(UnsupportedRun);
        }
        let service = self.services.get(service_id).ok_or(UnsupportedRun)?;
        let legs = &service.spec.legs;
        if !(2..=MAX_LEGS).contains(&legs.len()) || !legs.len().is_multiple_of(2) {
            return Err(UnsupportedRun);
        }
        let middle = legs.len() / 2;
        let (out, back) = legs.split_at(middle);
        let start = &out[0].from;
        let turnaround = &out[middle - 1].to;
        if start == turnaround
            || out
                .iter()
                .zip(back.iter().rev())
                .any(|(x, y)| x.from != y.to || x.to != y.from)
            || out.iter().enumerate().any(|(i, leg)| {
                leg.from == *turnaround || out[..i].iter().any(|previous| previous.from == leg.from)
            })
            || !same_lane(&out[middle - 1], &back[0])
            || !same_lane(&back[middle - 1], &out[0])
        {
            return Err(UnsupportedRun);
        }
        // The runtime admits the next leg at max(arrival + dwell, rear clear).
        // Use the exact compiled times and include the same origin stand for
        // prospective reuse after the last arrival (the live run retires there).
        let (running_ms, dwell_and_clearance_ms) = legs
            .iter()
            .zip(&service.legs)
            .try_fold((0u64, 0u64), |(run, stand), (planned, compiled)| {
                Some((
                    run.checked_add(compiled.duration_ms)?,
                    stand.checked_add(planned.dwell_ms.max(compiled.clearance_ms))?,
                ))
            })
            .ok_or(TimeOutsideBudget)?;
        let cycle_ms = running_ms
            .checked_add(dwell_and_clearance_ms)
            .filter(|millis| (1..=MAX_SAFE_MS).contains(millis))
            .ok_or(TimeOutsideBudget)?;
        let (headway_ms, trainsets) = match request {
            FleetPlanningRequest::TimedHeadway { headway_ms }
                if (1..=MAX_SAFE_MS).contains(&headway_ms) =>
            {
                (headway_ms, cycle_ms.div_ceil(headway_ms))
            }
            FleetPlanningRequest::TrainsetCount { trainsets } if trainsets > 0 => (
                cycle_ms.div_ceil(u64::from(trainsets)),
                u64::from(trainsets),
            ),
            _ => return Err(InvalidInput),
        };
        Ok(NominalFleetPlanning {
            cycle_ms,
            running_ms,
            dwell_and_clearance_ms,
            headway_ms,
            trainsets,
        })
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::operation::{ServiceRunSpec, Timetable, Vehicle};

    pub(crate) fn run(dwell_ms: u64) -> ServiceRunSpec {
        let leg = |from: &str, to: &str| RunLeg {
            from: from.into(),
            to: to.into(),
            sections: vec![Section {
                length_m: 100.,
                speed_limit_kmh: 36.,
                resources: vec!["rail:1:section:0:lane:0".into()],
            }],
            dwell_ms,
        };
        ServiceRunSpec {
            service_id: "1".into(),
            vehicle: Vehicle {
                max_speed_kmh: 36.,
                acceleration_mps2: 1.,
                braking_mps2: 1.,
                length_m: 10.,
            },
            timetable: Timetable {
                first_departure_ms: 0,
                last_departure_ms: 0,
                headway_ms: 600_000,
            },
            legs: vec![leg("11", "12"), leg("12", "11")],
        }
    }

    #[test]
    fn minimum_trainsets_and_nominal_headway_keep_the_exact_schedule_unchanged() {
        let mut runtime = OperationRuntime::new(vec![run(5_000)]).unwrap();
        let actual = runtime.advance_to(100_000, 256).unwrap();
        assert_eq!(
            actual
                .arrivals
                .iter()
                .map(|arrival| arrival.at_ms)
                .collect::<Vec<_>>(),
            [20_000, 45_000]
        );
        let unchanged = runtime.save();
        let by_interval = runtime
            .preview_nominal_fleet(
                "1",
                FleetPlanningRequest::TimedHeadway { headway_ms: 24_999 },
            )
            .unwrap();
        assert_eq!(
            by_interval,
            NominalFleetPlanning {
                cycle_ms: 50_000,
                running_ms: 40_000,
                dwell_and_clearance_ms: 10_000,
                headway_ms: 24_999,
                trainsets: 3
            }
        );
        assert_eq!(
            runtime
                .preview_nominal_fleet(
                    "1",
                    FleetPlanningRequest::TimedHeadway { headway_ms: 25_000 }
                )
                .unwrap()
                .trainsets,
            2
        );
        let by_fleet = runtime
            .preview_nominal_fleet("1", FleetPlanningRequest::TrainsetCount { trainsets: 3 })
            .unwrap();
        assert_eq!(by_fleet.headway_ms, 16_667);
        assert_eq!(runtime.save(), unchanged);
    }

    #[test]
    fn planning_never_adds_dwell_to_clearance_or_guesses_one_way_loops_or_other_lanes() {
        let short = run(500);
        let runtime = OperationRuntime::new(vec![short.clone()]).unwrap();
        let by_fleet = runtime
            .preview_nominal_fleet("1", FleetPlanningRequest::TrainsetCount { trainsets: 1 })
            .unwrap();
        assert_eq!(
            (by_fleet.cycle_ms, by_fleet.dwell_and_clearance_ms),
            (42_000, 2_000)
        );
        let request = FleetPlanningRequest::TrainsetCount { trainsets: 1 };
        let mut oneway = short.clone();
        oneway.legs.pop();
        let mut invalid_lane = short.clone();
        invalid_lane.legs[1].sections[0].resources[0] = "rail:1:section:0:lane:1".into();
        let mut loop_like = short;
        loop_like.legs[1].to = "13".into();
        for spec in [oneway, invalid_lane, loop_like] {
            let op = OperationRuntime::new(vec![spec]).unwrap();
            assert_eq!(
                op.preview_nominal_fleet("1", request),
                Err(FleetPlanningError::UnsupportedRun)
            );
        }
        assert_eq!(
            runtime
                .preview_nominal_fleet("1", FleetPlanningRequest::TrainsetCount { trainsets: 0 }),
            Err(FleetPlanningError::InvalidInput)
        );
    }

    #[test]
    fn request_wire_is_explicit_integer_and_rejects_extra_options() {
        let request = |text| serde_json::from_str::<FleetPlanningRequest>(text);
        assert_eq!(
            request(r#"{"mode":"timedHeadway","headwayMs":120000}"#).unwrap(),
            FleetPlanningRequest::TimedHeadway {
                headway_ms: 120_000
            }
        );
        assert_eq!(
            request(r#"{"mode":"trainsetCount","trainsets":3}"#).unwrap(),
            FleetPlanningRequest::TrainsetCount { trainsets: 3 }
        );
        assert!(request(r#"{"mode":"timedHeadway","headwayMs":0.5}"#).is_err());
        assert!(request(r#"{"mode":"trainsetCount","trainsets":3,"apply":true}"#).is_err());
    }
}
