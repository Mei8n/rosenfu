use super::*;
use rosenfu_core::demand::{
    ChoiceWeights, FixedOd, MarketKind, ScheduledCorridorAssignment, StationAccess,
};
use rosenfu_core::operation::{
    periodic_dispatch::standard::{NamedBandOverrides, StandardIntervals},
    service_planning::FleetPlanningRequest,
};
use rosenfu_core::world::PlayerId;

const MINUTE: u64 = 60_000;
const DAY: u64 = 24 * 60 * MINUTE;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StandardScheduleStatus {
    configuration_token: String,
    service_id: String,
    can_apply: bool,
    active_elsewhere: bool,
    has_bus_configs: bool,
    authored: Option<StandardIntervals>,
    departure_count: Option<u64>,
    origin: Option<NamedStop>,
    destinations: Vec<NamedStop>,
}

#[derive(serde::Serialize)]
struct NamedStop {
    id: String,
    name: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StandardOdRequest {
    pub origin: String,
    pub destination: String,
    pub trips: f64,
    pub alternative_seconds: Option<f64>,
    pub start_ms: u64,
    pub end_ms: u64,
}

impl WorldEditor {
    fn standard_actor(&self, service_id: ServiceId) -> Result<PlayerId, String> {
        let data = self.world.snapshot();
        if self.company_setup_required
            || !data
                .services
                .iter()
                .any(|s| s.id == service_id && s.owner == self.company)
        {
            return Err("自社の鉄道系統を選択してください。".into());
        }
        data.companies
            .iter()
            .find(|c| c.id == self.company)
            .map(|c| c.owner)
            .ok_or_else(|| "自社の鉄道会社を確認してください。".into())
    }

    pub fn standard_schedule_status(
        &self,
        service: ServiceId,
        revision: &str,
    ) -> Result<StandardScheduleStatus, String> {
        self.standard_actor(service)?;
        if self.operation_configuration_token() != revision {
            return Err("最新のワールドとダイヤを確認してください。".into());
        }
        let key = service.0.to_string();
        let operation = self.world.operation();
        let active = operation.and_then(|o| o.runtime.standard_intervals());
        let authored = active
            .filter(|(id, _)| *id == key)
            .map(|(_, setup)| setup.clone());
        let active_elsewhere = active.is_some() && authored.is_none();
        let departure_count = operation
            .and_then(|o| o.runtime.standard_departure_grid(&key))
            .map(|grid| grid.departure_count(grid.active_window()));
        // The panel caches this structural eligibility by configuration token.
        // Backlog/vehicle/pause readiness comes from live ticks and is checked
        // again during Apply; don't cache a transient backlog as unsupported.
        let can_apply = !active_elsewhere
            && operation.is_some_and(|o| {
                !o.invalidated
                    && o.runtime
                        .preview_nominal_fleet(
                            &key,
                            FleetPlanningRequest::TimedHeadway { headway_ms: MINUTE },
                        )
                        .is_ok()
            })
            && self
                .world
                .operation_save()
                .is_some_and(|saved| saved.supports_standard_scope());
        let named = |id: &str| {
            let id_num = id.parse::<u64>().ok()?;
            self.world
                .snapshot()
                .stations
                .iter()
                .find(|s| s.id.0 == id_num)
                .map(|s| NamedStop {
                    id: id.to_owned(),
                    name: s.name.clone(),
                })
        };
        let spec = operation.and_then(|o| o.runtime.service_spec(&key));
        let origin = spec
            .and_then(|s| s.legs.first())
            .and_then(|l| named(&l.from));
        let destinations = spec.map_or_else(Vec::new, |s| {
            s.legs
                .iter()
                .take(s.legs.len() / 2)
                .filter_map(|l| named(&l.to))
                .collect()
        });
        Ok(StandardScheduleStatus {
            configuration_token: revision.to_owned(),
            service_id: key,
            can_apply,
            active_elsewhere,
            has_bus_configs: operation.is_some_and(|o| !o.bus_configs.is_empty()),
            authored,
            departure_count,
            origin,
            destinations,
        })
    }

    pub fn apply_standard_schedule(
        &mut self,
        service: ServiceId,
        expected: &str,
        base_minutes: u16,
        overrides: NamedBandOverrides,
        boundaries_minutes: [u16; 6],
    ) -> Result<OperationView, String> {
        let actor = self.standard_actor(service)?;
        self.require_current_timetable_token(expected)?;
        let saved = self
            .world
            .operation_save()
            .ok_or("先に運行する系統を設定してください。")?;
        let start_ms = saved
            .runtime
            .now_ms
            .checked_add(MINUTE)
            .and_then(|first| first.div_ceil(MINUTE).checked_mul(MINUTE))
            .ok_or("時刻が上限を超えます。")?;
        let end_ms = start_ms.checked_add(DAY).ok_or("時刻が上限を超えます。")?;
        let authored = StandardIntervals {
            base_minutes,
            overrides,
            boundaries_minutes,
            start_ms,
            end_ms,
        };
        let mut candidate = self.world.clone();
        candidate.apply_standard_timetable(&saved, actor, self.company, service, authored).map_err(|err| match err {
            rosenfu_core::operation::OperationError::InvalidClock => "時間帯ダイヤは一時停止中で車両がいないときに適用できます。運行開始前は、この鉄道系統だけにダイヤを設定してください。".to_owned(),
            rosenfu_core::operation::OperationError::InvalidTimetable => "間隔と時間帯の境界を確認してください。この系統または設定では適用できません。".to_owned(),
            _ => "この系統・線路構成は時間帯ダイヤに未対応です。元のダイヤは変更していません。".to_owned(),
        })?;
        rosenfu_storage::ensure_world_saveable(&candidate)
            .map_err(|_| "ダイヤを保存できません。変更していません。".to_owned())?;
        self.world = candidate;
        self.operation_tick = None;
        self.update_operation_revision();
        Ok(self.operation_snapshot())
    }

    pub fn preview_standard_schedule_od(
        &self,
        service: ServiceId,
        revision: &str,
        query: StandardOdRequest,
    ) -> Result<ScheduledCorridorAssignment, String> {
        let player = self.standard_actor(service)?;
        if self.operation_configuration_token() != revision {
            return Err("最新のワールドとダイヤを確認してください。".into());
        }
        let scenario = FixedOdScenario {
            origin_station_id: query.origin.clone(),
            destination_station_id: query.destination.clone(),
            trips: Some(query.trips),
            alternative_seconds: query.alternative_seconds,
        };
        scenario.validate()?;
        let od = FixedOd {
            id: "standard-scheduled-od".into(),
            origin_region: "synthetic".into(),
            destination_region: "synthetic".into(),
            owner_region: "synthetic".into(),
            origin_location: query.origin.clone(),
            destination_location: query.destination.clone(),
            market: MarketKind::Metro,
            trips: scenario.trips,
            alternative_seconds: scenario.alternative_seconds,
            access: vec![StationAccess {
                station: query.origin,
                seconds: 0.,
            }],
            egress: vec![StationAccess {
                station: query.destination,
                seconds: 0.,
            }],
        };
        self.world
            .preview_standard_scheduled_corridor(
                player,
                self.company,
                service,
                &od,
                query.start_ms..query.end_ms,
                &ChoiceWeights::default(),
            )
            .map_err(|_| "往路の乗車駅・到着駅と、最大24時間の試算範囲を確認してください。".into())
    }
}
