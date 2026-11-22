use super::*;
use rosenfu_core::operation::{Timetable, Train, Vehicle};
use rosenfu_core::world::FixedOdScenario;
use rosenfu_core::world::OperationServiceConfig;
#[path = "operation/bus_transfers.rs"]
mod bus_transfers;
#[derive(serde::Serialize)]
pub struct OperationLiveView {
    pub now_ms: u64,
    pub paused: bool,
    pub speed: u32,
    pub invalidated: bool,
    pub catching_up: bool,
    pub timetable_edit_token: String,
    pub timetable_edit_blocker: Option<&'static str>,
    pub configuration_token: String,
    pub trains: Vec<Train>,
}

#[derive(serde::Serialize)]
pub struct OperationView {
    #[serde(flatten)]
    pub live: OperationLiveView,
    pub services: Vec<OperationServiceConfig>,
    pub bus_services: Vec<rosenfu_core::world::BusOperationConfig>,
}

impl std::ops::Deref for OperationView {
    type Target = OperationLiveView;

    fn deref(&self) -> &Self::Target {
        &self.live
    }
}

#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum OperationTickView {
    Full(OperationView),
    Live(OperationLiveView),
}
pub(super) fn next_session_token() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, serde::Serialize)]
pub struct FixedOdScenarioView {
    token: String,
    scenario: Option<FixedOdScenario>,
}

/// A user-supplied walking assumption, not saved infrastructure or verified access.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixedOdRoadTransfer {
    pub station_id: String,
    pub bus_stop_id: String,
    pub walk_seconds: u32,
}

impl WorldEditor {
    fn operation_configuration_token(&self) -> String {
        // The native mount/operation revision changes on reopen, explicit
        // configure and reset; structural edits and Undo/Redo rotate history.
        // Elapsed time never changes timetable rows, and is not part of this key.
        format!(
            "{}:{}",
            self.operation_edit_revision,
            self.world_edit_history().revision
        )
    }

    fn operation_edit_token(&self) -> String {
        format!(
            "{}:{}",
            self.operation_configuration_token(),
            self.world.operation().map_or(0, |op| op.runtime.now_ms()),
        )
    }

    fn timetable_edit_blocker(&self) -> Option<&'static str> {
        let op = self.world.operation()?;
        if !op.runtime.paused() {
            return Some("ダイヤを変更するには先に運行を一時停止してください。");
        }
        if op.pending_target.is_some() {
            return Some(if op.invalidated {
                "停止した運行に未処理の時刻が残っているため、安全にダイヤを変更できません。"
            } else {
                "未処理の運行が残っています。追いつくまで運行を進めてください。"
            });
        }
        if op.runtime.trains().next().is_some() {
            return Some(if op.invalidated {
                "停止した運行に列車・バスが残っているため、安全にダイヤを変更できません。"
            } else {
                "走行中・停車中・進路待ちの列車やバスがいます。全車両の運行が終わった時点で一時停止してください。"
            });
        }
        None
    }

    fn require_timetable_edit(&self, expected: &str) -> Result<(), String> {
        if expected != self.operation_edit_token() {
            return Err(
                "ワールドまたは運行状態が変更されました。最新の運行状態を確認してください。".into(),
            );
        }
        self.timetable_edit_blocker()
            .map_or(Ok(()), |reason| Err(reason.into()))
    }

    fn update_operation_revision(&mut self) {
        self.operation_edit_revision = next_session_token();
    }

    /// Reconcile a selected service path on a disposable candidate. Used by
    /// Apply and only the history entries authored by that operation.
    pub(super) fn reconcile_service_path_edit(
        &self,
        candidate: &mut World,
        service_id: ServiceId,
    ) -> Result<bool, String> {
        candidate.retain_operation_from(&self.world);
        candidate.retain_fixed_od_scenario_from(&self.world);
        let invalidates_operation = self.world.operation().is_some_and(|op| !op.invalidated)
            && candidate.operation().is_some_and(|op| op.invalidated);
        if !invalidates_operation {
            return Ok(false);
        }
        if let Some(reason) = self.timetable_edit_blocker() {
            return Err(format!(
                "運行に影響する経路・停車駅の変更は適用していません。{reason}"
            ));
        }
        candidate
            .reconcile_idle_service_path_operation(service_id)
            .map_err(|_| {
                "現在のダイヤと運行ルールを維持できないため、経路・停車駅の変更は適用していません。別の経路や停車駅を選んでください。運行ルールも変える場合は、別名で保存したコピーで運行を再設定してください。"
                    .to_owned()
            })?;
        Ok(true)
    }

    pub(super) fn apply_reconciled_service_path_edit(
        &mut self,
        mut candidate: Self,
        service_id: ServiceId,
    ) -> Result<WorldDraft, String> {
        let before = self
            .world
            .snapshot()
            .service_paths
            .iter()
            .find(|p| p.service_id == service_id);
        let after = candidate
            .world
            .snapshot()
            .service_paths
            .iter()
            .find(|p| p.service_id == service_id);
        let history_service = (before != after).then_some(service_id);
        if self.reconcile_service_path_edit(&mut candidate.world, service_id)? {
            candidate.operation_tick = None;
        }
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        self.publish_reconciled_edit_with_service_path(candidate, false, history_service);
        Ok(self.snapshot())
    }

    pub fn operation_snapshot(&self) -> OperationView {
        self.full_operation_view(self.live_operation_view())
    }

    fn full_operation_view(&self, live: OperationLiveView) -> OperationView {
        let (services, bus_services) = self
            .world
            .operation()
            .map(|o| (o.configs.clone(), o.bus_configs.clone()))
            .unwrap_or_default();
        OperationView {
            live,
            services,
            bus_services,
        }
    }

    fn live_operation_view(&self) -> OperationLiveView {
        let o = self.world.operation();
        let now_ms = o.map_or(0, |o| o.runtime.now_ms());
        let configuration_token = self.operation_configuration_token();
        OperationLiveView {
            now_ms,
            paused: o.is_none_or(|o| o.runtime.paused()),
            speed: o.map_or(1, |o| o.runtime.speed()),
            invalidated: o.is_some_and(|o| o.invalidated),
            catching_up: o.is_some_and(|o| o.pending_target.is_some()),
            timetable_edit_token: format!("{configuration_token}:{now_ms}"),
            timetable_edit_blocker: self.timetable_edit_blocker(),
            configuration_token,
            trains: o
                .map(|o| o.runtime.trains().cloned().collect())
                .unwrap_or_default(),
        }
    }
    pub fn configure_operation_service(
        &mut self,
        service_id: ServiceId,
        timetable: Timetable,
        vehicle: Vehicle,
        dwell_ms: u64,
        expected_operation_token: &str,
    ) -> Result<OperationView, String> {
        if self.company_setup_required {
            return Err("鉄道会社を作成してください。".into());
        }
        if !self
            .world
            .snapshot()
            .services
            .iter()
            .any(|s| s.id == service_id && s.owner == self.company)
        {
            return Err("自社の系統を選択してください。".into());
        }
        self.require_timetable_edit(expected_operation_token)?;
        let mut configs = self
            .world
            .operation()
            .map(|o| o.configs.clone())
            .unwrap_or_default();
        let request = OperationServiceConfig {
            service_id,
            timetable,
            vehicle,
            dwell_ms,
        };
        if let Some(prior) = configs.iter_mut().find(|c| c.service_id == service_id) {
            *prior = request;
        } else {
            configs.push(request);
        }
        let buses = self
            .world
            .operation()
            .map(|o| o.bus_configs.clone())
            .unwrap_or_default();
        let mut candidate = self.world.clone();
        candidate
            .reconfigure_idle_operation(configs, buses, &service_id.0.to_string())
            .map_err(|e| match e {
                rosenfu_core::operation::OperationError::CrossingBudget =>
                    "平面交差の確認上限を超えました。運行設定は変更していません。以前のダイヤを維持します。".to_owned(),
                rosenfu_core::operation::OperationError::CrossingPlatform =>
                    "停車するホームが平面交差に近すぎます。この組み合わせの運行設定は変更していません。".to_owned(),
                rosenfu_core::operation::OperationError::ReservationScopeChanged =>
                    "他の系統の進路や運行ルールにも影響するため、個別には変更できません。現在のダイヤを維持しました。".to_owned(),
                _ => format!("運行設定を適用できません: {e:?}"),
            })?;
        rosenfu_storage::ensure_world_saveable(&candidate).map_err(|e| e.to_string())?;
        self.world = candidate;
        self.operation_tick = None;
        self.update_operation_revision();
        Ok(self.operation_snapshot())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn configure_bus_operation(
        &mut self,
        route_id: String,
        timetable: Timetable,
        vehicle: Vehicle,
        assumed_speed_kmh: f64,
        dwell_ms: u64,
        turnaround_ms: Option<u64>,
        expected_operation_token: &str,
    ) -> Result<OperationView, String> {
        if self.company_setup_required
            || !self
                .world
                .snapshot()
                .road_transport
                .bus_routes
                .iter()
                .any(|r| r.id == route_id && r.owner == self.company)
        {
            return Err("自社のバス路線を選択してください。".into());
        }
        self.require_timetable_edit(expected_operation_token)?;
        let configs = self
            .world
            .operation()
            .map(|o| o.configs.clone())
            .unwrap_or_default();
        let mut bus = self
            .world
            .operation()
            .map(|o| o.bus_configs.clone())
            .unwrap_or_default();
        // New ordinary open lines pass an explicit default; omitted inputs preserve history.
        let turnaround_ms = turnaround_ms.or_else(|| {
            bus.iter()
                .find(|c| c.route_id == route_id)
                .and_then(|c| c.turnaround_ms)
        });
        if turnaround_ms.is_some_and(|ms| ms > 3_600_000) {
            return Err("折り返し時間は0〜3600秒で設定してください。".into());
        }
        let request = rosenfu_core::world::BusOperationConfig {
            route_id: route_id.clone(),
            timetable,
            vehicle,
            assumed_speed_kmh,
            dwell_ms,
            turnaround_ms,
        };
        if let Some(prior) = bus.iter_mut().find(|c| c.route_id == request.route_id) {
            *prior = request;
        } else {
            bus.push(request);
        }
        let mut candidate = self.world.clone();
        candidate
            .reconfigure_idle_operation(configs, bus, &format!("bus:{route_id}"))
            .map_err(|e| match e {
                rosenfu_core::operation::OperationError::CrossingBudget =>
                    "平面交差の確認上限を超えました。運行設定は変更していません。以前のダイヤを維持します。".to_owned(),
                rosenfu_core::operation::OperationError::CrossingPlatform =>
                    "停車するホームが平面交差に近すぎます。この組み合わせの運行設定は変更していません。".to_owned(),
                rosenfu_core::operation::OperationError::ReservationScopeChanged =>
                    "他の系統の進路や運行ルールにも影響するため、個別には変更できません。現在のダイヤを維持しました。".to_owned(),
                _ => format!("バス運行設定を適用できません: {e:?}"),
            })?;
        rosenfu_storage::ensure_world_saveable(&candidate).map_err(|e| e.to_string())?;
        self.world = candidate;
        self.operation_tick = None;
        self.update_operation_revision();
        Ok(self.operation_snapshot())
    }
    /// Explicit destructive operation-clock reset, never invoked by edit Undo.
    pub fn reset_operation(&mut self) -> Result<OperationView, String> {
        self.world
            .configure_all_operation(vec![], vec![])
            .map_err(|e| format!("{e:?}"))?;
        self.operation_tick = None;
        self.update_operation_revision();
        Ok(self.operation_snapshot())
    }
    pub fn set_operation_clock(
        &mut self,
        paused: bool,
        speed: u32,
    ) -> Result<OperationView, String> {
        self.set_operation_clock_at(paused, speed, std::time::Instant::now())
    }
    fn set_operation_clock_at(
        &mut self,
        paused: bool,
        speed: u32,
        instant: std::time::Instant,
    ) -> Result<OperationView, String> {
        if ![1, 2, 4, 8, 16].contains(&speed) {
            return Err("速度は1・2・4・8・16倍から選択してください。".into());
        }
        self.advance_operation_state_at(instant)?;
        let o = self
            .world
            .operation_mut()
            .ok_or("先に運行する系統とダイヤを設定してください。")?;
        if o.invalidated && !paused {
            return Err(
                "線路または系統の変更で運行を停止しました。運行設定を確認してください。".into(),
            );
        }
        o.runtime
            .set_speed(speed)
            .map_err(|_| "速度は1・2・4・8・16倍から選択してください。")?;
        o.runtime.set_paused(paused);
        self.operation_tick = Some(instant);
        self.update_operation_revision();
        Ok(self.operation_snapshot())
    }
    /// Client only wakes the authority. Multiple viewers cannot multiply time.
    /// Bounded event processing retains a target rather than dropping departures.
    pub fn advance_operation(&mut self) -> Result<OperationView, String> {
        self.advance_operation_at(std::time::Instant::now())
    }
    fn advance_operation_at(
        &mut self,
        instant: std::time::Instant,
    ) -> Result<OperationView, String> {
        self.advance_operation_state_at(instant)?;
        Ok(self.operation_snapshot())
    }

    /// A client only receives a lean tick if it has exactly the authoritative
    /// rail and bus configurations for this world/revision.
    pub fn advance_operation_with_configuration(
        &mut self,
        known_configuration_token: Option<&str>,
    ) -> Result<OperationTickView, String> {
        self.advance_operation_state_at(std::time::Instant::now())?;
        Ok(self.operation_tick_view(known_configuration_token))
    }

    fn operation_tick_view(&self, known_configuration_token: Option<&str>) -> OperationTickView {
        let live = self.live_operation_view();
        if known_configuration_token == Some(live.configuration_token.as_str()) {
            OperationTickView::Live(live)
        } else {
            OperationTickView::Full(self.full_operation_view(live))
        }
    }

    fn advance_operation_state_at(&mut self, instant: std::time::Instant) -> Result<(), String> {
        let elapsed = self
            .operation_tick
            .map(|t| instant.saturating_duration_since(t).as_millis())
            .unwrap_or(0);
        self.operation_tick = Some(instant);
        let Some(o) = self.world.operation_mut() else {
            return Ok(());
        };
        if o.invalidated {
            return Ok(());
        }
        if o.runtime.paused() {
            return Ok(());
        }
        let delta = u64::try_from(elapsed)
            .map_err(|_| "時計の範囲を超えました。")?
            .checked_mul(o.runtime.speed() as u64)
            .ok_or("時計の範囲を超えました。")?;
        let target = o
            .pending_target
            .unwrap_or(o.runtime.now_ms())
            .checked_add(delta)
            .ok_or("時計の範囲を超えました。")?;
        let result = o
            .runtime
            .advance_to(target, 128)
            .map_err(|e| format!("運行の進行に失敗しました: {e:?}"))?;
        o.pending_target = (!result.reached_target).then_some(target);
        Ok(())
    }
    pub fn fixed_od_scenario(&self) -> FixedOdScenarioView {
        FixedOdScenarioView {
            token: self.fixed_od_token.to_string(),
            scenario: self.world.fixed_od_scenario().cloned(),
        }
    }

    pub fn set_fixed_od_scenario(
        &mut self,
        expected_token: &str,
        scenario: Option<FixedOdScenario>,
    ) -> Result<FixedOdScenarioView, String> {
        if self.company_setup_required || expected_token != self.fixed_od_token.to_string() {
            return Err("ワールドまたは保存済み需要条件が変更されています。".into());
        }
        if let Some(scenario) = scenario.as_ref() {
            scenario.validate()?;
            if !scenario.endpoints_exist(self.world.snapshot()) {
                return Err("異なる既存の駅・停留所を選択してください。".into());
            }
        }
        let mut candidate = self.world.clone();
        candidate.restore_fixed_od_scenario(scenario)?;
        rosenfu_storage::ensure_world_saveable(&candidate).map_err(|err| err.to_string())?;
        self.world = candidate;
        self.fixed_od_token = next_session_token();
        Ok(self.fixed_od_scenario())
    }

    pub fn preview_rail_journey_replay(
        &self,
        origin: &str,
        destination: &str,
    ) -> Result<rosenfu_core::operation::ReplayItinerary, String> {
        use rosenfu_core::operation::PassengerReplayError;
        if origin == destination
            || [origin, destination].iter().any(|id| {
                !self
                    .world
                    .snapshot()
                    .stations
                    .iter()
                    .any(|station| station.id.0.to_string() == *id)
            })
        {
            return Err("異なる既存の鉄道駅を選択してください。".into());
        }
        let operation = self
            .world
            .operation()
            .ok_or("先に運行する系統を設定してください。")?;
        if operation.invalidated {
            return Err("運行経路の更新が必要です。".into());
        }
        let runtime = &operation.runtime;
        let until = runtime
            .now_ms()
            .checked_add(30 * 60_000)
            .ok_or("時計の範囲を超えました。")?;
        let replay = runtime
            .preview_passenger_replay(until, 256)
            .map_err(|err| -> String {
                match err {
                    PassengerReplayError::OutsideBudget => {
                        "この規模の運行は試算上限を超えています。".into()
                    }
                    PassengerReplayError::Runtime(_) => "運行の再現に失敗しました。".into(),
                }
            })?;
        Ok(replay.earliest_rail_itinerary(origin, destination))
    }

    #[cfg(test)]
    pub fn preview_fixed_od(
        &self,
        origin_station_id: String,
        destination_station_id: String,
        trips: Option<f64>,
        alternative_seconds: Option<f64>,
    ) -> Result<rosenfu_core::demand::OdAssignment, String> {
        self.preview_fixed_od_with_transfer(
            origin_station_id,
            destination_station_id,
            trips,
            alternative_seconds,
            None,
        )
    }

    pub fn preview_fixed_od_with_transfer(
        &self,
        origin_station_id: String,
        destination_station_id: String,
        trips: Option<f64>,
        alternative_seconds: Option<f64>,
        transfer: Option<FixedOdRoadTransfer>,
    ) -> Result<rosenfu_core::demand::OdAssignment, String> {
        use rosenfu_core::demand::*;
        let scenario = FixedOdScenario {
            origin_station_id: origin_station_id.clone(),
            destination_station_id: destination_station_id.clone(),
            trips,
            alternative_seconds,
        };
        scenario.validate()?;
        if !scenario.endpoints_exist(self.world.snapshot()) {
            return Err("異なる既存の駅・停留所を選択してください。".into());
        }
        let mut walks = if let Some(transfer) = transfer {
            let rail = transfer.station_id;
            let bus = transfer.bus_stop_id;
            let seconds = f64::from(transfer.walk_seconds);
            let endpoints = FixedOdScenario {
                origin_station_id: rail.clone(),
                destination_station_id: bus.clone(),
                trips: None,
                alternative_seconds: None,
            };
            if rail.starts_with("bus:")
                || !bus.starts_with("bus:")
                || !(60..=3600).contains(&transfer.walk_seconds)
                || endpoints.validate().is_err()
                || !endpoints.endpoints_exist(self.world.snapshot())
            {
                return Err(
                    "徒歩乗り換えには既存の駅・バス停と1〜60分の所要時間を指定してください。"
                        .into(),
                );
            }
            vec![
                WalkLink {
                    from: rail.clone(),
                    to: bus.clone(),
                    seconds,
                },
                WalkLink {
                    from: bus,
                    to: rail,
                    seconds,
                },
            ]
        } else {
            Vec::new()
        };
        let operation = self
            .world
            .operation()
            .ok_or("先に運行する系統を設定してください。")?;
        if operation.invalidated {
            return Err("運行経路の更新が必要です。".into());
        }
        let legs = operation
            .runtime
            .passenger_offers()
            .into_iter()
            .map(|l| PassengerLeg {
                leg_id: l.leg_id,
                sequence: l.sequence,
                board_allowed: l.board_allowed,
                alight_allowed: l.alight_allowed,
                service_id: l.service_id,
                from: l.from,
                to: l.to,
                ride_seconds: l.ride_seconds,
                through_seconds: l.through_seconds,
                headway_seconds: l.headway_seconds,
            })
            .collect::<Vec<_>>();
        walks.extend(bus_transfers::coincident_ordinary_bus_walks(
            self.world.snapshot(),
            &legs,
        )?);
        let od = FixedOd {
            id: "synthetic-station-od".into(),
            origin_region: "synthetic".into(),
            destination_region: "synthetic".into(),
            origin_location: origin_station_id.clone(),
            destination_location: destination_station_id.clone(),
            owner_region: "synthetic".into(),
            market: MarketKind::Metro,
            trips,
            access: vec![StationAccess {
                station: origin_station_id.clone(),
                seconds: 0.,
            }],
            egress: vec![StationAccess {
                station: destination_station_id.clone(),
                seconds: 0.,
            }],
            alternative_seconds,
        };
        assign_fixed_od(&[od], &legs, &walks, &ChoiceWeights::default(), 10000)?
            .into_iter()
            .next()
            .ok_or("需要結果がありません。".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn od_mount_token_survives_automatic_first_save_but_not_active_archive() {
        let mut editor = WorldEditor::new();
        editor.create_new_world("New world".into()).unwrap();
        let mounted = editor.fixed_od_scenario();
        let root = tempfile::tempdir().unwrap();
        assert!(
            editor
                .save_world_as_slot(root.path(), "New world".into(), None, None)
                .unwrap()
                .saved
        );
        assert_eq!(editor.fixed_od_scenario().token, mounted.token);
        let a = editor
            .world
            .add_station(editor.company, "A".into(), 35.0, 139.0)
            .unwrap();
        let b = editor
            .world
            .add_station(editor.company, "B".into(), 35.0, 139.02)
            .unwrap();
        let scenario = FixedOdScenario {
            origin_station_id: a.0.to_string(),
            destination_station_id: b.0.to_string(),
            trips: Some(12.0),
            alternative_seconds: None,
        };
        let recorded = editor
            .set_fixed_od_scenario(&mounted.token, Some(scenario.clone()))
            .unwrap();
        let id = editor.active_slot_id.clone().unwrap();
        editor.move_save_slot(root.path(), &id, false).unwrap();
        assert!(editor.active_slot_id.is_none());
        assert_eq!(editor.fixed_od_scenario().scenario, Some(scenario));
        assert!(editor.set_fixed_od_scenario(&recorded.token, None).is_err());
    }

    #[test]
    fn fixed_od_scenario_roundtrips_and_remains_separate_from_preview_and_reset() {
        let mut editor = editor();
        let snapshot = editor.snapshot();
        let scenario = FixedOdScenario {
            origin_station_id: snapshot.stations[0].id.0.to_string(),
            destination_station_id: snapshot.stations[1].id.0.to_string(),
            trips: Some(100.0),
            alternative_seconds: Some(800.0),
        };
        let token = editor.fixed_od_scenario().token;
        assert!(editor
            .set_fixed_od_scenario("stale", Some(scenario.clone()))
            .is_err());
        editor
            .set_fixed_od_scenario(&token, Some(scenario.clone()))
            .unwrap();
        assert!(editor.set_fixed_od_scenario(&token, None).is_err());
        let preview = editor
            .preview_fixed_od(
                scenario.origin_station_id.clone(),
                scenario.destination_station_id.clone(),
                scenario.trips,
                scenario.alternative_seconds,
            )
            .unwrap();
        assert_eq!(preview.rail[0].time.wait_seconds, 30.0);
        assert!(preview.rail[0].time.ride_seconds > 0.0);
        assert_eq!(editor.fixed_od_scenario().scenario, Some(scenario.clone()));
        editor
            .world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(1234, 100)
            .unwrap();
        let expected_operation = editor.world.operation_save().unwrap();
        assert!(!editor.operation_snapshot().trains.is_empty());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        editor.save_with_view(&path, None, None, false).unwrap();
        assert_eq!(
            rosenfu_storage::load_world(&path)
                .unwrap()
                .fixed_od_scenario(),
            Some(&scenario)
        );
        let mut other = WorldEditor::new();
        assert!(other.fixed_od_scenario().scenario.is_none());
        other.load(&path).unwrap();
        assert_eq!(other.fixed_od_scenario().scenario, Some(scenario.clone()));
        assert_eq!(other.world.operation_save(), Some(expected_operation));
        assert_eq!(other.operation_snapshot().now_ms, 1234);
        assert!(!other.operation_snapshot().trains.is_empty());
        editor.reset_operation().unwrap();
        assert_eq!(editor.fixed_od_scenario().scenario, Some(scenario.clone()));
        let old_epoch = other.fixed_od_scenario().token;
        other.load(&path).unwrap(); // identical byte-for-byte, same slot/destination
        let reopened = other.fixed_od_scenario();
        assert_ne!(reopened.token, old_epoch);
        assert_eq!(reopened.scenario.as_ref(), Some(&scenario));
        assert!(other.set_fixed_od_scenario(&old_epoch, None).is_err());
        let autosave = dir.path().join("auto.json");
        other.save_with_view(&autosave, None, None, true).unwrap();
        assert_eq!(
            rosenfu_storage::load_world(&autosave)
                .unwrap()
                .fixed_od_scenario(),
            Some(&scenario)
        );
        other.create_new_world("Other company".into()).unwrap();
        assert!(other.fixed_od_scenario().scenario.is_none());
        assert!(other
            .set_fixed_od_scenario(&reopened.token, Some(scenario))
            .is_err());
    }

    fn editor() -> WorldEditor {
        let mut e = WorldEditor::new();
        e.company_setup_required = false;
        let a = e
            .world
            .build_alignment(
                e.company,
                "line".into(),
                vec![
                    GeoPoint {
                        latitude: 35.,
                        longitude: 139.,
                    },
                    GeoPoint {
                        latitude: 35.,
                        longitude: 139.02,
                    },
                ],
            )
            .unwrap();
        let s = e
            .world
            .place_station_on_alignment(e.company, "A".into(), a, 0.1, 100., 10.)
            .unwrap();
        let t = e
            .world
            .place_station_on_alignment(e.company, "B".into(), a, 0.9, 100., 10.)
            .unwrap();
        let segments = vec![rosenfu_core::world::RouteSegment::Alignment {
            alignment_id: a,
            from: s,
            to: t,
        }];
        let preview = e.world.preview_service_path(s, segments.clone()).unwrap();
        let stops = preview
            .occurrences
            .iter()
            .map(|o| o.reference.clone())
            .collect();
        let id = e
            .world
            .add_service_path(e.company, "S".into(), s, segments, stops)
            .unwrap();
        e.configure_operation_service(
            id,
            Timetable {
                first_departure_ms: 0,
                last_departure_ms: 60000,
                headway_ms: 60000,
            },
            Vehicle {
                max_speed_kmh: 100.,
                acceleration_mps2: 1.,
                braking_mps2: 1.,
                length_m: 80.,
            },
            30000,
            &e.operation_edit_token(),
        )
        .unwrap();
        e
    }
    // Non-qualifying ordinary rail geometry keeps the fixed reservation
    // model when the previously skipped middle station is added as a stop.
    fn stop_edit_editor(
        curved: bool,
    ) -> (
        WorldEditor,
        ServiceId,
        Vec<rosenfu_core::world::StationOccurrenceRef>,
    ) {
        let mut e = WorldEditor::new();
        e.company_setup_required = false;
        let point = |latitude, longitude| GeoPoint {
            latitude,
            longitude,
        };
        let mut points = vec![point(35., 139.)];
        if curved {
            points.push(point(35.001, 139.012));
        }
        points.push(point(35., 139.024));
        let line = e
            .world
            .build_alignment(e.company, "Test line".into(), points)
            .unwrap();
        let a = e
            .world
            .place_station_on_alignment(e.company, "A".into(), line, 0.10, 70., 10.)
            .unwrap();
        e.world
            .place_station_on_alignment(e.company, "B".into(), line, 0.37, 70., 10.)
            .unwrap();
        let c = e
            .world
            .place_station_on_alignment(e.company, "C".into(), line, 0.90, 70., 10.)
            .unwrap();
        let segments = vec![rosenfu_core::world::RouteSegment::Alignment {
            alignment_id: line,
            from: a,
            to: c,
        }];
        let all: Vec<_> = e
            .world
            .preview_service_path(a, segments.clone())
            .unwrap()
            .occurrences
            .into_iter()
            .map(|occ| occ.reference)
            .collect();
        assert_eq!(all.len(), 3);
        let service = e
            .world
            .add_service_path(
                e.company,
                "Express".into(),
                a,
                segments,
                vec![all[0].clone(), all[2].clone()],
            )
            .unwrap();
        e.configure_operation_service(
            service,
            Timetable {
                first_departure_ms: 0,
                last_departure_ms: 600_000,
                headway_ms: 120_000,
            },
            Vehicle {
                max_speed_kmh: 90.,
                acceleration_mps2: 1.,
                braking_mps2: 1.,
                length_m: 45.,
            },
            30_000,
            &e.operation_edit_token(),
        )
        .unwrap();
        assert_eq!(
            e.world.operation_save().unwrap().runtime.format,
            if curved { 2 } else { 4 }
        );
        (e, service, all)
    }

    #[test]
    fn stop_apply_cannot_freeze_live_vehicles_or_publish_any_change() {
        let (mut e, service, all) = stop_edit_editor(true);
        e.world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(1_000, 100)
            .unwrap();
        assert!(!e.operation_snapshot().trains.is_empty());
        let dest = tempfile::tempdir().unwrap();
        let file = dest.path().join("baseline.json");
        e.save(&file).unwrap();
        let before = rosenfu_storage::encode(&e.world).unwrap();
        let history = e.world_edit_history().revision;
        let rejected = e.update_service_path(service, "Including B".into(), all);
        assert!(
            rejected.is_err(),
            "busy stop Apply published/froze live trains"
        );
        assert_eq!(e.world_edit_history().revision, history);
        assert_eq!(rosenfu_storage::encode(&e.world).unwrap(), before);
        assert_eq!(std::fs::read(file).unwrap(), before);
        assert!(!e.operation_snapshot().invalidated);
    }

    #[test]
    fn scoped_stop_apply_reopens_same_clock_and_demand_without_reconfiguration_command() {
        let (mut e, id, all) = stop_edit_editor(true);
        let saved_timetable = e.operation_snapshot().services;
        let from = e.world.snapshot().stations[0].id.0.to_string();
        let to = e.world.snapshot().stations[1].id.0.to_string();
        let scenario = FixedOdScenario {
            origin_station_id: from.clone(),
            destination_station_id: to.clone(),
            trips: Some(100.),
            alternative_seconds: None,
        };
        e.set_fixed_od_scenario(&e.fixed_od_scenario().token, Some(scenario.clone()))
            .unwrap();
        {
            let op = e.world.operation_mut().unwrap();
            op.runtime.advance_to(2_000_000, 100).unwrap();
            op.runtime.set_speed(4).unwrap();
            assert!(op.runtime.trains().next().is_none());
        }
        let test_od = |e: &WorldEditor| {
            e.preview_fixed_od(from.clone(), to.clone(), Some(100.), None)
                .unwrap()
        };
        assert_eq!(test_od(&e).unserved_trips, 100.);
        e.world.operation_mut().unwrap().pending_target = Some(2_001_000);
        let before_debt = e.world.clone();
        assert!(e
            .update_service_path(id, "All stops".into(), all.clone())
            .err()
            .unwrap()
            .contains("未処理"));
        assert_eq!(e.world, before_debt);
        e.world.operation_mut().unwrap().pending_target = None;
        let token = e.operation_edit_token();
        e.update_service_path(id, "All stops".into(), all).unwrap();
        assert_ne!(e.operation_edit_token(), token);
        let edited = e.operation_snapshot();
        assert_eq!(edited.now_ms, 2_000_000);
        assert_eq!(edited.speed, 4);
        assert!(edited.paused && !edited.invalidated && edited.trains.is_empty());
        assert_eq!(edited.services, saved_timetable);
        assert_eq!(e.world.fixed_od_scenario(), Some(&scenario));
        assert_eq!(
            test_od(&e).rail.iter().map(|row| row.trips).sum::<f64>(),
            100.
        );
        let dir = tempfile::tempdir().unwrap();
        let save = dir.path().join("guarded.json");
        e.save(&save).unwrap();
        let saved = e.world.clone();
        e.load(&save).unwrap();
        assert_eq!(e.world, saved);
        assert_eq!(
            test_od(&e).rail.iter().map(|row| row.trips).sum::<f64>(),
            100.
        );
    }

    #[test]
    fn path_replacement_protects_running_clock_but_allows_noop_rename_and_unrelated_edits() {
        let (mut e, id, all) = stop_edit_editor(true);
        let [a, b, _] = e
            .world
            .snapshot()
            .stations
            .iter()
            .map(|s| s.id)
            .collect::<Vec<_>>()[..]
        else {
            panic!("fixture")
        };
        let segments = vec![rosenfu_core::world::RouteSegment::Alignment {
            alignment_id: e.world.snapshot().alignments[0].id,
            from: a,
            to: b,
        }];
        let nearby = e
            .preview_service_path_replacement(id, a, segments.clone())
            .unwrap()
            .occurrences
            .into_iter()
            .map(|o| o.reference)
            .collect::<Vec<_>>();
        let old_path = e
            .world
            .snapshot()
            .service_paths
            .iter()
            .find(|path| path.service_id == id)
            .unwrap()
            .clone();
        let unrelated = e
            .world
            .add_service_path(
                e.company,
                "Unconfigured".into(),
                a,
                old_path
                    .legs
                    .iter()
                    .map(|leg| leg.segment.clone())
                    .collect(),
                vec![all[0].clone(), all[2].clone()],
            )
            .unwrap();
        e.set_operation_clock_at(false, 1, std::time::Instant::now())
            .unwrap();
        // Even a newly started clock with no dispatched vehicle isn't a safe boundary.
        assert!(e.operation_snapshot().trains.is_empty());
        let before = e.world.clone();
        let revision = e.world_edit_history().revision;
        let replace = |e: &mut WorldEditor| {
            e.replace_service_path_with_source_routes(
                id,
                old_path.clone(),
                "Shorter".into(),
                a,
                segments.clone(),
                nearby.clone(),
                None,
                None,
            )
        };
        let error = replace(&mut e).err().unwrap();
        assert!(error.contains("一時停止"), "{error}");
        assert_eq!(e.world, before);
        assert_eq!(e.world_edit_history().revision, revision);
        e.update_service_path(id, "Renamed".into(), vec![all[0].clone(), all[2].clone()])
            .unwrap();
        e.update_service_path(unrelated, "Unconfigured changed".into(), all)
            .unwrap();
        assert_eq!(e.world.operation_save(), before.operation_save());
        e.world.operation_mut().unwrap().runtime.set_paused(true);
        // The expected original path survives rename. This edit is valid before dispatch.
        replace(&mut e).unwrap();
        assert!(e.operation_snapshot().paused && !e.operation_snapshot().invalidated);
        assert!(e
            .world
            .operation()
            .unwrap()
            .runtime
            .passenger_offers()
            .iter()
            .any(|offer| offer.to == b.0.to_string()));
    }

    #[test]
    fn idle_stop_change_that_would_change_legacy_mode_is_rejected_without_reset() {
        for started in [false, true] {
            let (mut e, id, all) = stop_edit_editor(false);
            if started {
                let op = e.world.operation_mut().unwrap();
                op.runtime.advance_to(2_000_000, 100).unwrap();
                assert!(op.runtime.trains().next().is_none());
            }
            let before = e.world.clone();
            let history = e.world_edit_history().revision;
            let error = e
                .update_service_path(id, "All stops".into(), all)
                .err()
                .unwrap();
            assert!(error.contains("別の経路や停車駅"));
            assert_eq!(e.world, before);
            assert_eq!(e.world_edit_history().revision, history);
        }
    }

    #[test]
    fn guarded_service_undo_redo_preserves_trains_and_history_and_allows_idle_retry() {
        let (mut e, id, all) = stop_edit_editor(true);
        let advance = |e: &mut WorldEditor, time| {
            e.world
                .operation_mut()
                .unwrap()
                .runtime
                .advance_to(time, 100)
                .unwrap();
        };
        let active = |e: &WorldEditor| e.operation_snapshot().trains.len();
        let depths = |e: &WorldEditor| {
            let h = e.world_edit_history();
            (h.revision, h.undo_depth, h.redo_depth)
        };
        let has_b = |e: &WorldEditor| {
            e.world
                .snapshot()
                .service_paths
                .iter()
                .find(|path| path.service_id == id)
                .unwrap()
                .stops
                .len()
                == 3
        };
        let dispatch = |e: &mut WorldEditor, departure| {
            let config = e.operation_snapshot().services[0].clone();
            e.configure_operation_service(
                id,
                Timetable {
                    first_departure_ms: departure,
                    last_departure_ms: departure,
                    ..config.timetable
                },
                config.vehicle,
                config.dwell_ms,
                &e.operation_edit_token(),
            )
            .unwrap();
            advance(e, departure + 1);
            assert_eq!(active(e), 1);
        };
        advance(&mut e, 2_000_000);
        e.update_service_path(id, "Stop at B".into(), all).unwrap();
        assert!(has_b(&e));

        // A normal untagged entry is still undoable during running operation
        // and does not consume the preceding protected service-path entry.
        let mut ordinary = e.clone();
        ordinary
            .world
            .add_station(e.company, "Unrelated".into(), 35.05, 139.05)
            .unwrap();
        e.commit_edit(ordinary, false);
        dispatch(&mut e, 2_001_000);
        e.undo_world_edit(e.snapshot()).unwrap();
        assert!(has_b(&e));
        assert_eq!(active(&e), 1);
        let before = e.world.clone();
        let h = depths(&e);
        let token = e.operation_edit_token();
        let error = e
            .undo_world_edit(e.snapshot())
            .err()
            .expect("Undo must not publish over an active train");
        assert!(error.contains("適用していません"));
        assert_eq!(e.world, before);
        assert_eq!(depths(&e), h);
        assert_eq!(e.operation_edit_token(), token);

        advance(&mut e, 3_000_000);
        assert_eq!(active(&e), 0);
        e.undo_world_edit(e.snapshot()).unwrap();
        assert!(!has_b(&e));
        let state = e.operation_snapshot();
        assert!(state.paused && !state.invalidated);
        assert_eq!(state.now_ms, 3_000_000);
        assert_eq!(state.services, before.operation().unwrap().configs);
        assert_eq!(depths(&e).2, 2);

        dispatch(&mut e, 3_001_000);
        let before = e.world.clone();
        let h = depths(&e);
        let error = e.redo_world_edit(e.snapshot()).err().unwrap();
        assert!(error.contains("適用していません"));
        assert_eq!(e.world, before);
        assert_eq!(depths(&e), h);
        advance(&mut e, 4_000_000);
        assert_eq!(active(&e), 0);
        e.redo_world_edit(e.snapshot()).unwrap();
        assert!(has_b(&e));
        let state = e.operation_snapshot();
        assert!(state.paused && !state.invalidated);
        assert_eq!(state.now_ms, 4_000_000);
        assert_eq!(state.services, before.operation().unwrap().configs);
        e.redo_world_edit(e.snapshot()).unwrap();
        assert!(e
            .world
            .snapshot()
            .stations
            .iter()
            .any(|s| s.name == "Unrelated"));
        assert_eq!(e.operation_snapshot().now_ms, 4_000_000);
        assert_eq!(depths(&e).2, 0);
    }

    fn add_test_bus(e: &mut WorldEditor, timetable: Timetable) {
        use rosenfu_core::road_transport::*;
        let mut d = e.snapshot();
        d.road_transport = RoadTransport {
            bus_stops: vec![
                BusStop {
                    id: "A".into(),
                    owner: e.company,
                    name: "A".into(),
                    edge_id: "qa:0-1".into(),
                    position: 0.2,
                    location_kind: Default::default(),
                },
                BusStop {
                    id: "B".into(),
                    owner: e.company,
                    name: "B".into(),
                    edge_id: "qa:1-2".into(),
                    position: 0.8,
                    location_kind: Default::default(),
                },
            ],
            bus_routes: vec![BusRoute {
                id: "R".into(),
                owner: e.company,
                name: "QA bus".into(),
                edges: vec!["qa:0-1".into(), "qa:1-2".into()],
                stops: vec![
                    BusStopOccurrence {
                        stop_id: "A".into(),
                        edge_index: 0,
                    },
                    BusStopOccurrence {
                        stop_id: "B".into(),
                        edge_index: 1,
                    },
                ],
            }],
            ..RoadTransport::synthetic_qa_graph()
        };
        let mut candidate = e.clone();
        candidate.world.replace(d).unwrap();
        e.commit_edit(candidate, false);
        e.configure_bus_operation(
            "R".into(),
            timetable,
            Vehicle {
                max_speed_kmh: 60.,
                acceleration_mps2: 1.,
                braking_mps2: 1.2,
                length_m: 12.,
            },
            30.,
            10000,
            None,
            &e.operation_edit_token(),
        )
        .unwrap();
    }
    #[test]
    fn bus_operation_uses_partial_edges_namespaced_stops_and_shared_od() {
        let mut e = editor();
        add_test_bus(
            &mut e,
            Timetable {
                first_departure_ms: 0,
                last_departure_ms: 60000,
                headway_ms: 60000,
            },
        );
        let offers = e.world.operation().unwrap().runtime.passenger_offers();
        let bus = offers.iter().find(|o| o.service_id == "bus:R").unwrap();
        assert_eq!(bus.from, "bus:A");
        assert_eq!(bus.to, "bus:B");
        assert!(bus.ride_seconds > 60.);
        let assigned = e
            .preview_fixed_od("bus:A".into(), "bus:B".into(), Some(50.), None)
            .unwrap();
        assert_eq!(assigned.rail.iter().map(|p| p.trips).sum::<f64>(), 50.);
        e.world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(1000, 100)
            .unwrap();
        assert!(e
            .operation_snapshot()
            .trains
            .iter()
            .any(|t| t.key.service_id == "bus:R"));
        let restored =
            rosenfu_storage::decode(&rosenfu_storage::encode(&e.world).unwrap()).unwrap();
        assert_eq!(e.world.operation_save(), restored.operation_save());
    }
    #[test]
    fn compact_ticks_keep_the_current_clock_and_resync_after_mutation_history_and_reopen() {
        use std::time::{Duration, Instant};

        let mut e = editor();
        let rail = e.operation_snapshot().services[0].clone();
        let timetable = Timetable {
            first_departure_ms: 20_000,
            last_departure_ms: 200_000,
            headway_ms: 60_000,
        };
        let token = e.operation_edit_token();
        e.configure_operation_service(
            rail.service_id,
            timetable.clone(),
            rail.vehicle.clone(),
            rail.dwell_ms,
            &token,
        )
        .unwrap();
        add_test_bus(&mut e, timetable.clone());
        let start = Instant::now();
        let initial = e.set_operation_clock_at(false, 1, start).unwrap();
        let legacy = e.operation_tick_view(None);
        let mut full = serde_json::to_value(&legacy).unwrap();
        assert_eq!(
            full["services"][0]["service_id"],
            rail.service_id.0.to_string()
        );
        assert_eq!(full["bus_services"][0]["route_id"], "R");

        // With the same clock and settings, both forms differ only in static rows.
        let compact = |e: &WorldEditor, known: &str| {
            assert!(matches!(
                e.operation_tick_view(Some(known)),
                OperationTickView::Live(_)
            ));
            serde_json::to_value(e.operation_tick_view(Some(known))).unwrap()
        };
        let initial_live = compact(&e, &initial.configuration_token);
        let rows = full.as_object_mut().unwrap();
        rows.remove("services").unwrap();
        rows.remove("bus_services").unwrap();
        assert_eq!(full, initial_live);
        e.advance_operation_state_at(start + Duration::from_millis(500))
            .unwrap();
        let live = compact(&e, &initial.configuration_token);
        assert_eq!(live["now_ms"], 500);
        assert_eq!(live["timetable_edit_token"], e.operation_edit_token());
        assert_ne!(
            live["timetable_edit_token"],
            initial_live["timetable_edit_token"]
        );

        let refreshed = |e: &WorldEditor, stale: &str| match e.operation_tick_view(Some(stale)) {
            OperationTickView::Full(current) => current,
            OperationTickView::Live(_) => panic!("stale config must force full resync"),
        };
        let paused = e
            .set_operation_clock_at(true, 1, start + Duration::from_millis(500))
            .unwrap();
        assert!(matches!(
            e.advance_operation_with_configuration(Some(&paused.configuration_token)),
            Ok(OperationTickView::Live(_))
        ));
        assert!(matches!(
            e.advance_operation_with_configuration(None),
            Ok(OperationTickView::Full(_))
        ));
        let new_rail = Timetable {
            headway_ms: 80_000,
            ..timetable.clone()
        };
        let edited = e
            .configure_operation_service(
                rail.service_id,
                new_rail.clone(),
                rail.vehicle.clone(),
                rail.dwell_ms,
                &paused.timetable_edit_token,
            )
            .unwrap();
        assert_eq!(
            refreshed(&e, &paused.configuration_token).services[0].timetable,
            new_rail
        );
        let bus = edited.bus_services[0].clone();
        let bus_edit = Timetable {
            headway_ms: 90_000,
            ..timetable
        };
        let edited_bus = e
            .configure_bus_operation(
                bus.route_id,
                bus_edit.clone(),
                bus.vehicle,
                bus.assumed_speed_kmh,
                bus.dwell_ms,
                None,
                &edited.timetable_edit_token,
            )
            .unwrap();
        assert_eq!(
            refreshed(&e, &edited.configuration_token).bus_services[0].timetable,
            bus_edit
        );

        e.rename_company("renamed".into()).unwrap();
        let renamed = refreshed(&e, &edited_bus.configuration_token);
        e.undo_world_edit(e.snapshot()).unwrap();
        let undone = refreshed(&e, &renamed.configuration_token);
        e.redo_world_edit(e.snapshot()).unwrap();
        let redone = refreshed(&e, &undone.configuration_token);
        let path = tempfile::tempdir().unwrap();
        let save = path.path().join("world.json");
        e.save(&save).unwrap();
        e.load(&save).unwrap();
        let reopened = refreshed(&e, &redone.configuration_token);
        assert_eq!(reopened.services, redone.services);
        assert_eq!(reopened.bus_services, redone.bus_services);
        assert_eq!(reopened.now_ms, 500);
        e.load(&save).unwrap();
        let same_slot = refreshed(&e, &reopened.configuration_token);
        e.reset_operation().unwrap();
        let cleared = refreshed(&e, &same_slot.configuration_token);
        assert_eq!(cleared.now_ms, 0);
        assert!(cleared.services.is_empty() && cleared.bus_services.is_empty());
    }

    #[test]
    fn timetable_edits_keep_idle_clock_and_unrelated_configs_with_exact_reopen() {
        let mut e = editor();
        let initial = e.operation_snapshot().services[0].clone();
        let first = Timetable {
            first_departure_ms: 0,
            last_departure_ms: 900_000,
            headway_ms: 300_000,
        };
        let set_rail = |e: &mut WorldEditor, timetable: Timetable, token: &str| {
            e.configure_operation_service(
                initial.service_id,
                timetable,
                initial.vehicle.clone(),
                initial.dwell_ms,
                token,
            )
        };
        let token = e.operation_edit_token();
        set_rail(&mut e, first.clone(), &token).unwrap();
        let future_bus = Timetable {
            first_departure_ms: 280_000,
            last_departure_ms: 900_000,
            headway_ms: 180_000,
        };
        add_test_bus(&mut e, future_bus.clone());
        let bus = e.operation_snapshot().bus_services[0].clone();
        let stations = &e.world.snapshot().stations;
        let scenario = FixedOdScenario {
            origin_station_id: stations[0].id.0.to_string(),
            destination_station_id: stations[1].id.0.to_string(),
            trips: Some(100.),
            alternative_seconds: Some(420.),
        };
        let operation_token = e.operation_edit_token();
        e.set_fixed_od_scenario(&e.fixed_od_scenario().token, Some(scenario.clone()))
            .unwrap();
        assert_eq!(operation_token, e.operation_edit_token());
        let trial = |e: &WorldEditor| {
            e.preview_fixed_od(
                scenario.origin_station_id.clone(),
                scenario.destination_station_id.clone(),
                scenario.trips,
                scenario.alternative_seconds,
            )
            .unwrap()
        };

        let edit = Timetable {
            headway_ms: 120_000,
            ..first.clone()
        };
        e.world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(1_000, 100)
            .unwrap();
        let active = rosenfu_storage::encode(&e.world).unwrap();
        let active_token = e.operation_edit_token();
        assert!(set_rail(&mut e, edit.clone(), &active_token)
            .err()
            .unwrap()
            .contains("列車やバス"));
        assert_eq!(rosenfu_storage::encode(&e.world).unwrap(), active);
        assert_eq!(e.operation_edit_token(), active_token);
        {
            let o = e.world.operation_mut().unwrap();
            o.runtime.advance_to(250_000, 100).unwrap();
            o.runtime.set_speed(4).unwrap();
            assert!(o.runtime.trains().next().is_none());
            o.pending_target = Some(251_000);
        }
        let token = e.operation_edit_token();
        assert!(set_rail(&mut e, edit.clone(), &token)
            .err()
            .unwrap()
            .contains("未処理"));
        e.world.operation_mut().unwrap().pending_target = None;
        e.world.operation_mut().unwrap().runtime.set_paused(false);
        assert!(set_rail(&mut e, edit.clone(), &token)
            .err()
            .unwrap()
            .contains("一時停止"));
        e.world.operation_mut().unwrap().runtime.set_paused(true);

        let before = e.world.clone();
        let proposal = Timetable {
            headway_ms: 0,
            ..first
        };
        assert!(set_rail(&mut e, proposal, &token).is_err());
        assert_eq!(e.world, before);
        assert_eq!(e.operation_edit_token(), token);
        let previous_od = trial(&e);
        let view = set_rail(&mut e, edit.clone(), &token).unwrap();
        assert_eq!(view.now_ms, 250_000);
        assert_eq!(view.speed, 4);
        assert!(view.paused && view.trains.is_empty());
        assert_eq!(view.bus_services, vec![bus.clone()]);
        assert_eq!(e.world.fixed_od_scenario(), Some(&scenario));
        assert!(trial(&e).rail[0].trips > previous_od.rail[0].trips);
        let rail = view.services[0].clone();
        assert_eq!(rail.timetable, edit);
        assert_ne!(view.timetable_edit_token, token);

        let later = Timetable {
            headway_ms: 200_000,
            ..future_bus
        };
        assert!(e
            .configure_bus_operation(
                bus.route_id.clone(),
                later.clone(),
                bus.vehicle.clone(),
                bus.assumed_speed_kmh,
                bus.dwell_ms,
                None,
                &token
            )
            .err()
            .unwrap()
            .contains("変更されました"));
        assert_eq!(e.operation_edit_token(), view.timetable_edit_token);
        let view = e
            .configure_bus_operation(
                bus.route_id,
                later.clone(),
                bus.vehicle,
                bus.assumed_speed_kmh,
                bus.dwell_ms,
                None,
                &view.timetable_edit_token,
            )
            .unwrap();
        assert_eq!(view.services, vec![rail]);
        assert_eq!(view.bus_services[0].timetable, later);
        assert_eq!(view.now_ms, 250_000);
        let dest = tempfile::tempdir().unwrap();
        let path = dest.path().join("operations.json");
        e.save(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let before_reopen = e.operation_edit_token();
        e.load(&path).unwrap();
        assert_ne!(e.operation_edit_token(), before_reopen);
        assert!(set_rail(&mut e, edit, &before_reopen)
            .err()
            .unwrap()
            .contains("変更されました"));
        assert_eq!(rosenfu_storage::encode(&e.world).unwrap(), bytes);
        let runtime = &mut e.world.operation_mut().unwrap().runtime;
        assert!(runtime.trains().next().is_none());
        runtime.advance_to(279_999, 100).unwrap();
        assert!(runtime.trains().next().is_none());
        runtime.advance_to(280_000, 100).unwrap();
        assert_eq!(runtime.trains().count(), 1);
        assert_eq!(runtime.trains().next().unwrap().key.service_id, "bus:R");
    }

    #[test]
    fn a_new_rail_service_cannot_replay_departures_before_or_at_the_started_clock() {
        let mut e = editor();
        let built = &e.world.snapshot().stations;
        let a = built[0].id;
        let b = built[1].id;
        let original = e.operation_snapshot().services[0].clone();
        let segments = vec![rosenfu_core::world::RouteSegment::Alignment {
            alignment_id: e.world.snapshot().alignments[0].id,
            from: a,
            to: b,
        }];
        let stops = e
            .world
            .preview_service_path(a, segments.clone())
            .unwrap()
            .occurrences
            .into_iter()
            .map(|o| o.reference)
            .collect();
        let extra = e
            .world
            .add_service_path(e.company, "Additional service".into(), a, segments, stops)
            .unwrap();
        e.world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(600_000, 100)
            .unwrap();
        let before = e.operation_edit_token();
        let updated = e
            .configure_operation_service(
                extra,
                Timetable {
                    first_departure_ms: 0,
                    last_departure_ms: 900_000,
                    headway_ms: 300_000,
                },
                original.vehicle.clone(),
                original.dwell_ms,
                &before,
            )
            .unwrap();
        assert_eq!(updated.now_ms, 600_000);
        assert_eq!(updated.services[0], original);
        assert!(updated.trains.is_empty());
        let runtime = &mut e.world.operation_mut().unwrap().runtime;
        runtime.advance_to(899_999, 100).unwrap();
        assert!(runtime.trains().next().is_none());
        runtime.advance_to(900_000, 100).unwrap();
        let train = runtime.trains().next().unwrap();
        assert_eq!(train.key.service_id, extra.0.to_string());
        assert_eq!(train.key.scheduled_ms, 900_000);
    }

    #[test]
    fn budget_debt_survives_save_and_pause() {
        let mut e = editor();
        let config = e.world.operation().unwrap().configs[0].clone();
        e.configure_operation_service(
            config.service_id,
            Timetable {
                first_departure_ms: 0,
                last_departure_ms: 10000,
                headway_ms: 1,
            },
            config.vehicle,
            config.dwell_ms,
            &e.operation_edit_token(),
        )
        .unwrap();
        let start = std::time::Instant::now();
        e.set_operation_clock_at(false, 1, start).unwrap();
        e.advance_operation_at(start + std::time::Duration::from_secs(5))
            .unwrap();
        assert_eq!(e.world.operation().unwrap().pending_target, Some(5000));
        let bytes = rosenfu_storage::encode(&e.world).unwrap();
        let restored = rosenfu_storage::decode(&bytes).unwrap();
        assert_eq!(restored.operation_save(), e.world.operation_save());
        e.set_operation_clock_at(true, 1, start + std::time::Duration::from_secs(5))
            .unwrap();
        let frozen = e.operation_snapshot().now_ms;
        e.advance_operation_at(start + std::time::Duration::from_secs(100))
            .unwrap();
        assert_eq!(e.operation_snapshot().now_ms, frozen);
    }
    #[test]
    fn transitions_settle_old_speed_without_losing_interval() {
        let mut e = editor();
        let start = std::time::Instant::now();
        e.set_operation_clock_at(false, 2, start).unwrap();
        e.set_operation_clock_at(false, 4, start + std::time::Duration::from_secs(3))
            .unwrap();
        assert_eq!(e.operation_snapshot().now_ms, 6000);
        e.set_operation_clock_at(true, 4, start + std::time::Duration::from_secs(5))
            .unwrap();
        assert_eq!(e.operation_snapshot().now_ms, 14000);
        e.advance_operation_at(start + std::time::Duration::from_secs(100))
            .unwrap();
        assert_eq!(e.operation_snapshot().now_ms, 14000);
        let save = rosenfu_storage::encode(&e.world).unwrap();
        assert_eq!(
            rosenfu_storage::decode(&save)
                .unwrap()
                .operation()
                .unwrap()
                .runtime
                .now_ms(),
            14000
        );
    }
    #[test]
    fn manual_save_settles_clock_and_switch_clears_state() {
        let mut e = editor();
        let root = tempfile::tempdir().unwrap();
        e.set_operation_clock(false, 1).unwrap();
        e.operation_tick = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
        let path = root.path().join("world.json");
        e.save(&path).unwrap();
        let saved = rosenfu_storage::load_world(&path).unwrap();
        assert!(saved.operation().unwrap().runtime.now_ms() >= 1000);
        e.create_new_world("new".into()).unwrap();
        assert!(e.world.operation().is_none());
        e.load(&path).unwrap();
        assert_eq!(e.world.operation_save(), saved.operation_save());
        assert!(e.operation_tick.is_none());
    }
    #[test]
    fn operation_tick_wire_omits_passenger_offers_but_native_fixed_od_still_uses_them() {
        #[derive(serde::Serialize)]
        struct LegacyWire<'a> {
            #[serde(flatten)]
            current: &'a OperationView,
            offers: Vec<rosenfu_core::operation::PassengerOffer>,
        }

        let mut e = editor();
        let first = e.operation_snapshot().services[0].clone();
        let path = e.world.snapshot().service_paths[0].clone();
        let from = e.world.snapshot().services[0].start;
        let segments: Vec<_> = path.legs.iter().map(|leg| leg.segment.clone()).collect();
        let mut configs = vec![first.clone()];
        for n in 2..=64 {
            let service_id = e
                .world
                .add_service_path(
                    e.company,
                    format!("Timing fixture {n}"),
                    from,
                    segments.clone(),
                    path.stops.clone(),
                )
                .unwrap();
            configs.push(OperationServiceConfig {
                service_id,
                ..first.clone()
            });
        }
        e.world.configure_operation(configs).unwrap();
        let start = std::time::Instant::now();
        e.set_operation_clock_at(false, 1, start).unwrap();
        let before = e.world.operation_save().unwrap();
        let view = e
            .advance_operation_at(start + std::time::Duration::from_millis(500))
            .unwrap();
        assert_eq!(view.now_ms, 500);
        assert_eq!(view.services.len(), 64);
        assert!(!view.trains.is_empty());

        let offers = e.world.operation().unwrap().runtime.passenger_offers();
        assert_eq!(offers.len(), 128);
        let old = serde_json::to_vec(&LegacyWire {
            current: &view,
            offers,
        })
        .unwrap();
        let current = serde_json::to_vec(&view).unwrap();
        let mut legacy: serde_json::Value = serde_json::from_slice(&old).unwrap();
        assert_eq!(
            legacy
                .as_object_mut()
                .unwrap()
                .remove("offers")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            128
        );
        assert_eq!(
            legacy,
            serde_json::from_slice::<serde_json::Value>(&current).unwrap()
        );
        assert!(current.len() < old.len());
        let stations = &e.world.snapshot().stations;
        let od = e
            .preview_fixed_od(
                stations[0].id.0.to_string(),
                stations[1].id.0.to_string(),
                Some(10.),
                None,
            )
            .unwrap();
        assert_eq!(od.potential_trips, Some(10.));
        assert!(od.search_complete);
        assert!((od.rail.iter().map(|path| path.trips).sum::<f64>() - 10.).abs() < 1e-9);
        assert_ne!(e.world.operation_save().unwrap(), before);
        let native = e.world.operation().unwrap();
        let begin = std::time::Instant::now();
        for _ in 0..2_000 {
            std::hint::black_box(native.runtime.passenger_offers());
        }
        eprintln!("operation wire: {} -> {} bytes; {} offers not built per tick; 2000 former offers-only builds {:.3} ms",
            old.len(),current.len(),128,begin.elapsed().as_secs_f64()*1000.);
    }

    #[test]
    fn synthetic_od_uses_real_service_and_does_not_mutate() {
        let e = editor();
        let stations = &e.world.snapshot().stations;
        let before = e.world.clone();
        let result = e
            .preview_fixed_od(
                stations[0].id.0.to_string(),
                stations[1].id.0.to_string(),
                Some(100.),
                None,
            )
            .unwrap();
        assert!(result.search_complete);
        assert_eq!(result.rail.iter().map(|p| p.trips).sum::<f64>(), 100.);
        assert_eq!(e.world, before);
    }
    #[test]
    fn paused_clock_does_not_spawn_and_backend_clock_advances() {
        let mut e = editor();
        assert!(e.advance_operation().unwrap().trains.is_empty());
        e.set_operation_clock(false, 2).unwrap();
        e.operation_tick = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
        let state = e.advance_operation().unwrap();
        assert!(state.now_ms >= 2000 && state.now_ms < 4000);
        assert_eq!(state.trains.len(), 1);
        e.set_operation_clock(true, 2).unwrap();
        let now = e.operation_snapshot().now_ms;
        e.operation_tick = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
        assert_eq!(e.advance_operation().unwrap().now_ms, now);
    }
    #[test]
    fn edit_undo_keeps_operation_time_and_save_envelope_restores_it() {
        let mut e = editor();
        e.world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(1234, 100)
            .unwrap();
        let mut candidate = e.clone();
        candidate
            .world
            .add_station(e.company, "independent".into(), 35.1, 139.1)
            .unwrap();
        e.commit_edit(candidate, false);
        e.undo_world_edit(e.snapshot()).unwrap();
        assert_eq!(e.operation_snapshot().now_ms, 1234);
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), rosenfu_storage::encode(&e.world).unwrap()).unwrap();
        let loaded = rosenfu_storage::load_world(file.path()).unwrap();
        assert_eq!(loaded.operation_save(), e.world.operation_save());
        assert!(WorldEditor::new().world.operation().is_none());
        e.reset_operation().unwrap();
        assert_eq!(e.operation_snapshot().now_ms, 0);
        assert!(e.operation_snapshot().services.is_empty());
    }
    #[test]
    fn explicit_bus_walk_links_rail_and_both_tram_types_without_mutating_world() {
        use rosenfu_core::road_transport::{BusRoute, BusStop, BusStopOccurrence};
        use rosenfu_core::world::RouteSegment;

        for road_tram in [None, Some(false), Some(true)] {
            let mut e = WorldEditor::new();
            e.company_setup_required = false;
            let qa = e.enable_road_qa_fixture(true).unwrap();
            let mut roads = qa.road_transport.clone();
            roads.bus_stops = [("pickup", 0.2), ("dropoff", 0.8)]
                .into_iter()
                .map(|(id, position)| BusStop {
                    id: id.into(),
                    owner: e.company,
                    edge_id: "qa:1-2".into(),
                    name: id.into(),
                    position,
                    location_kind: Default::default(),
                })
                .collect();
            roads.bus_routes.push(BusRoute {
                id: "bus-1".into(),
                owner: e.company,
                name: "Synthetic bus".into(),
                edges: vec!["qa:1-2".into()],
                stops: ["pickup", "dropoff"]
                    .into_iter()
                    .map(|id| BusStopOccurrence {
                        edge_index: 0,
                        stop_id: id.into(),
                    })
                    .collect(),
            });
            e.replace_road_transport(qa.road_transport, roads).unwrap();
            let alignment = if let Some(shared) = road_tram {
                e.edit_road_tram(RoadTramEdit::Build {
                    from: "qa:0".into(),
                    to: "qa:1".into(),
                    name: "Synthetic tram".into(),
                    shared,
                })
                .unwrap()
                .alignments[0]
                    .id
            } else {
                e.world
                    .build_alignment(
                        e.company,
                        "Synthetic rail".into(),
                        vec![
                            GeoPoint {
                                latitude: 35.,
                                longitude: 135.,
                            },
                            GeoPoint {
                                latitude: 35.,
                                longitude: 135.005,
                            },
                        ],
                    )
                    .unwrap()
            };
            let origin = e
                .world
                .place_station_on_alignment(e.company, "Origin".into(), alignment, 0.1, 40., 8.)
                .unwrap();
            let station = e
                .world
                .place_station_on_alignment(e.company, "Transfer".into(), alignment, 0.85, 40., 8.)
                .unwrap();
            let segments = vec![RouteSegment::Alignment {
                alignment_id: alignment,
                from: origin,
                to: station,
            }];
            let stops = e
                .world
                .preview_service_path(origin, segments.clone())
                .unwrap()
                .occurrences
                .into_iter()
                .map(|o| o.reference)
                .collect();
            let service = e
                .world
                .add_service_path(e.company, "Connections".into(), origin, segments, stops)
                .unwrap();
            let timetable = Timetable {
                first_departure_ms: 0,
                last_departure_ms: 60000,
                headway_ms: 60000,
            };
            let vehicle = Vehicle {
                max_speed_kmh: 35.,
                acceleration_mps2: 1.,
                braking_mps2: 1.2,
                length_m: 12.,
            };
            e.configure_operation_service(
                service,
                timetable.clone(),
                vehicle.clone(),
                10000,
                &e.operation_edit_token(),
            )
            .unwrap();
            e.configure_bus_operation(
                "bus-1".into(),
                timetable,
                vehicle,
                30.,
                10000,
                None,
                &e.operation_edit_token(),
            )
            .unwrap();
            e.world
                .operation_mut()
                .unwrap()
                .runtime
                .advance_to(1000, 100)
                .unwrap();
            let view = e.operation_snapshot();
            assert_eq!(view.now_ms, 1000);
            assert!(view.trains.iter().any(|t| t.key.service_id == "bus:bus-1"));
            assert!(view
                .trains
                .iter()
                .any(|t| t.key.service_id == service.0.to_string()));
            let before = rosenfu_storage::encode(&e.world).unwrap();
            let origin = origin.0.to_string();
            let station = station.0.to_string();
            let preview = |transfer| {
                e.preview_fixed_od_with_transfer(
                    origin.clone(),
                    "bus:dropoff".into(),
                    Some(50.),
                    None,
                    transfer,
                )
            };
            assert_eq!(preview(None).unwrap().unserved_trips, 50.);
            let assumption = |bus_stop_id: &str, walk_seconds| FixedOdRoadTransfer {
                station_id: station.clone(),
                bus_stop_id: bus_stop_id.into(),
                walk_seconds,
            };
            assert!(preview(Some(assumption("bus:missing", 180))).is_err());
            assert!(preview(Some(assumption(&station, 180))).is_err());
            assert!(preview(Some(assumption("bus:pickup", 0))).is_err());
            assert!(preview(Some(FixedOdRoadTransfer {
                station_id: "bus:dropoff".into(),
                ..assumption("bus:pickup", 180)
            }))
            .is_err());
            let assigned = preview(Some(assumption("bus:pickup", 180))).unwrap();
            assert!(assigned.search_complete);
            let [path] = assigned.rail.as_slice() else {
                panic!("expected the single explicit public transport path");
            };
            assert_eq!(
                path.services,
                vec![service.0.to_string(), "bus:bus-1".into()]
            );
            assert_eq!(
                path.stations,
                vec![
                    origin.clone(),
                    station.clone(),
                    "bus:pickup".into(),
                    "bus:dropoff".into()
                ]
            );
            assert_eq!(path.time.walk_seconds, 180.);
            assert_eq!(path.time.transfers, 1);
            assert_eq!(path.trips, 50.);
            assert_eq!(
                preview(Some(assumption("bus:pickup", 300))).unwrap().rail[0]
                    .time
                    .walk_seconds,
                300.
            );
            let opposite = e
                .preview_fixed_od_with_transfer(
                    "bus:dropoff".into(),
                    origin,
                    Some(50.),
                    None,
                    Some(assumption("bus:pickup", 180)),
                )
                .unwrap();
            assert!(opposite.rail.is_empty());
            assert_eq!(opposite.unserved_trips, 50.);
            assert_eq!(before, rosenfu_storage::encode(&e.world).unwrap());
        }
    }
}
