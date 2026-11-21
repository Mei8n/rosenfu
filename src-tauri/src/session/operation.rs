use super::*;
use rosenfu_core::operation::{PassengerOffer, Timetable, Train, Vehicle};
use rosenfu_core::world::FixedOdScenario;
use rosenfu_core::world::OperationServiceConfig;
#[derive(serde::Serialize)]
pub struct OperationView {
    pub now_ms: u64,
    pub paused: bool,
    pub speed: u32,
    pub invalidated: bool,
    pub catching_up: bool,
    pub services: Vec<OperationServiceConfig>,
    pub bus_services: Vec<rosenfu_core::world::BusOperationConfig>,
    pub trains: Vec<Train>,
    pub offers: Vec<PassengerOffer>,
}
pub(super) fn next_fixed_od_token() -> u64 {
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
    pub fn operation_snapshot(&self) -> OperationView {
        let Some(o) = self.world.operation() else {
            return OperationView {
                now_ms: 0,
                paused: true,
                speed: 1,
                invalidated: false,
                catching_up: false,
                services: vec![],
                bus_services: vec![],
                trains: vec![],
                offers: vec![],
            };
        };
        OperationView {
            now_ms: o.runtime.now_ms(),
            paused: o.runtime.paused(),
            speed: o.runtime.speed(),
            invalidated: o.invalidated,
            catching_up: o.pending_target.is_some(),
            services: o.configs.clone(),
            bus_services: o.bus_configs.clone(),
            trains: o.runtime.trains().cloned().collect(),
            offers: if o.invalidated {
                vec![]
            } else {
                o.runtime.passenger_offers()
            },
        }
    }
    pub fn configure_operation_service(
        &mut self,
        service_id: ServiceId,
        timetable: Timetable,
        vehicle: Vehicle,
        dwell_ms: u64,
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
        if self
            .world
            .operation()
            .is_some_and(|o| o.runtime.now_ms() != 0 || o.runtime.trains().next().is_some())
        {
            return Err("運行開始後のダイヤ変更は、運行をリセットしてから行ってください。".into());
        }
        let mut configs = self
            .world
            .operation()
            .map(|o| o.configs.clone())
            .unwrap_or_default();
        configs.retain(|c| c.service_id != service_id);
        configs.push(OperationServiceConfig {
            service_id,
            timetable,
            vehicle,
            dwell_ms,
        });
        let mut candidate = self.world.clone();
        candidate
            .configure_operation(configs)
            .map_err(|e| match e {
                rosenfu_core::operation::OperationError::CrossingBudget =>
                    "平面交差の確認上限を超えました。運行設定は変更していません。以前のダイヤを維持します。".to_owned(),
                rosenfu_core::operation::OperationError::CrossingPlatform =>
                    "停車するホームが平面交差に近すぎます。この組み合わせの運行設定は変更していません。".to_owned(),
                _ => format!("運行設定を適用できません: {e:?}"),
            })?;
        rosenfu_storage::ensure_world_saveable(&candidate).map_err(|e| e.to_string())?;
        self.world = candidate;
        self.operation_tick = None;
        Ok(self.operation_snapshot())
    }
    pub fn configure_bus_operation(
        &mut self,
        route_id: String,
        timetable: Timetable,
        vehicle: Vehicle,
        assumed_speed_kmh: f64,
        dwell_ms: u64,
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
        if self
            .world
            .operation()
            .is_some_and(|o| o.runtime.now_ms() != 0 || o.runtime.trains().next().is_some())
        {
            return Err("運行をリセットしてからダイヤを変更してください。".into());
        }
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
        bus.retain(|c| c.route_id != route_id);
        bus.push(rosenfu_core::world::BusOperationConfig {
            route_id,
            timetable,
            vehicle,
            assumed_speed_kmh,
            dwell_ms,
        });
        let mut candidate = self.world.clone();
        candidate
            .configure_all_operation(configs, bus)
            .map_err(|e| match e {
                rosenfu_core::operation::OperationError::CrossingBudget =>
                    "平面交差の確認上限を超えました。運行設定は変更していません。以前のダイヤを維持します。".to_owned(),
                rosenfu_core::operation::OperationError::CrossingPlatform =>
                    "停車するホームが平面交差に近すぎます。この組み合わせの運行設定は変更していません。".to_owned(),
                _ => format!("バス運行設定を適用できません: {e:?}"),
            })?;
        rosenfu_storage::ensure_world_saveable(&candidate).map_err(|e| e.to_string())?;
        self.world = candidate;
        self.operation_tick = None;
        Ok(self.operation_snapshot())
    }
    /// Explicit destructive operation-clock reset, never invoked by edit Undo.
    pub fn reset_operation(&mut self) -> Result<OperationView, String> {
        self.world
            .configure_all_operation(vec![], vec![])
            .map_err(|e| format!("{e:?}"))?;
        self.operation_tick = None;
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
        self.advance_operation_at(instant)?;
        let o = self
            .world
            .operation_mut()
            .ok_or("先に運行する系統とダイヤを設定してください。")?;
        if o.invalidated && !paused {
            return Err(
                "線路または系統が変更されたため運行を停止しています。運行をリセットしてください。"
                    .into(),
            );
        }
        o.runtime
            .set_speed(speed)
            .map_err(|_| "速度は1・2・4・8・16倍から選択してください。")?;
        o.runtime.set_paused(paused);
        self.operation_tick = Some(instant);
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
        let elapsed = self
            .operation_tick
            .map(|t| instant.saturating_duration_since(t).as_millis())
            .unwrap_or(0);
        self.operation_tick = Some(instant);
        let Some(o) = self.world.operation_mut() else {
            return Ok(self.operation_snapshot());
        };
        if o.invalidated {
            return Ok(self.operation_snapshot());
        }
        if o.runtime.paused() {
            return Ok(self.operation_snapshot());
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
        Ok(self.operation_snapshot())
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
        self.fixed_od_token = next_fixed_od_token();
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
        let walks = if let Some(transfer) = transfer {
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
        )
        .unwrap();
        e
    }
    #[test]
    fn bus_operation_uses_partial_edges_namespaced_stops_and_shared_od() {
        use rosenfu_core::road_transport::*;
        let mut e = editor();
        let mut d = e.snapshot();
        d.road_transport = RoadTransport {
            bus_stops: vec![
                BusStop {
                    id: "A".into(),
                    owner: e.company,
                    name: "A".into(),
                    edge_id: "qa:0-1".into(),
                    position: 0.2,
                },
                BusStop {
                    id: "B".into(),
                    owner: e.company,
                    name: "B".into(),
                    edge_id: "qa:1-2".into(),
                    position: 0.8,
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
            Timetable {
                first_departure_ms: 0,
                last_departure_ms: 60000,
                headway_ms: 60000,
            },
            Vehicle {
                max_speed_kmh: 60.,
                acceleration_mps2: 1.,
                braking_mps2: 1.2,
                length_m: 12.,
            },
            30.,
            10000,
        )
        .unwrap();
        let offers = e.operation_snapshot().offers;
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
            e.configure_operation_service(service, timetable.clone(), vehicle.clone(), 10000)
                .unwrap();
            e.configure_bus_operation("bus-1".into(), timetable, vehicle, 30., 10000)
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
