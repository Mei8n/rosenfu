use super::*;
use serde::{Deserialize, Serialize};

/// One explicitly saved assumption set, not measured traffic or a cached result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixedOdScenario {
    pub origin_station_id: String,
    pub destination_station_id: String,
    pub trips: Option<f64>,
    pub alternative_seconds: Option<f64>,
}

fn valid_id(id: &str) -> bool {
    if let Some(bus) = id.strip_prefix("bus:") {
        !bus.trim().is_empty() && bus.len() <= 124 && !bus.chars().any(char::is_control)
    } else {
        id.len() <= 20
            && id
                .parse::<u64>()
                .is_ok_and(|n| n != 0 && n.to_string() == id)
    }
}

impl FixedOdScenario {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !valid_id(&self.origin_station_id)
            || !valid_id(&self.destination_station_id)
            || self.origin_station_id == self.destination_station_id
        {
            return Err("異なる既存の駅・停留所を選択してください。");
        }
        if self
            .trips
            .is_some_and(|n| !n.is_finite() || !(0.0..=1e9).contains(&n))
        {
            return Err("人数は0〜10億の範囲で入力してください。");
        }
        if self
            .alternative_seconds
            .is_some_and(|n| !n.is_finite() || n <= 0.0 || n > 31_536_000.)
        {
            return Err("代替手段の所要時間は正の値かつ1年以下にしてください。");
        }
        Ok(())
    }

    pub fn endpoints_exist(&self, world: &WorldDraft) -> bool {
        let exists = |id: &str| {
            if let Some(bus) = id.strip_prefix("bus:") {
                world
                    .road_transport
                    .bus_stops
                    .iter()
                    .any(|stop| stop.id == bus)
            } else if let Ok(id) = id.parse::<u64>() {
                world.stations.iter().any(|station| station.id.0 == id)
            } else {
                false
            }
        };
        exists(&self.origin_station_id) && exists(&self.destination_station_id)
    }
}

impl World {
    pub fn fixed_od_scenario(&self) -> Option<&FixedOdScenario> {
        self.fixed_od_scenario.as_ref()
    }

    /// Storage accepts well-formed orphan input so edited worlds still load.
    /// It is never executable until both endpoints exist again.
    pub fn restore_fixed_od_scenario(
        &mut self,
        scenario: Option<FixedOdScenario>,
    ) -> Result<(), &'static str> {
        if let Some(ref scenario) = scenario {
            scenario.validate()?;
        }
        self.fixed_od_scenario = scenario;
        Ok(())
    }

    pub fn retain_fixed_od_scenario_from(&mut self, previous: &Self) {
        self.fixed_od_scenario
            .clone_from(&previous.fixed_od_scenario);
    }
}
