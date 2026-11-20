use super::*;
use rosenfu_core::operation::{PassengerOffer, Timetable, Train, Vehicle};
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
            .map_err(|e| format!("運行設定を適用できません: {e:?}"))?;
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
            .map_err(|e| format!("バス運行設定を適用できません: {e:?}"))?;
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
    pub fn preview_fixed_od(
        &self,
        origin_station_id: String,
        destination_station_id: String,
        trips: Option<f64>,
        alternative_seconds: Option<f64>,
    ) -> Result<rosenfu_core::demand::OdAssignment, String> {
        use rosenfu_core::demand::*;
        let exists = |id: &str| {
            if let Some(bus) = id.strip_prefix("bus:") {
                self.world
                    .snapshot()
                    .road_transport
                    .bus_stops
                    .iter()
                    .any(|s| s.id == bus)
            } else {
                self.world
                    .snapshot()
                    .stations
                    .iter()
                    .any(|s| s.id.0.to_string() == id)
            }
        };
        if origin_station_id == destination_station_id
            || !exists(&origin_station_id)
            || !exists(&destination_station_id)
        {
            return Err("異なる既存の駅・停留所を選択してください。".into());
        }
        if trips.is_some_and(|n| !n.is_finite() || !(0.0..=1e9).contains(&n)) {
            return Err("人数は0〜10億の範囲で入力してください。".into());
        }
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
        assign_fixed_od(&[od], &legs, &[], &ChoiceWeights::default(), 10000)?
            .into_iter()
            .next()
            .ok_or("需要結果がありません。".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let a = GeoPoint {
            latitude: 35.,
            longitude: 139.,
        };
        let b = GeoPoint {
            latitude: 35.,
            longitude: 139.01,
        };
        d.road_transport = RoadTransport {
            evidence: vec![],
            nodes: vec![
                RoadNode {
                    id: "a".into(),
                    point: a,
                },
                RoadNode {
                    id: "b".into(),
                    point: b,
                },
            ],
            edges: vec![RoadEdge {
                id: "ab".into(),
                from: "a".into(),
                to: "b".into(),
                source: "synthetic-qa".into(),
                revision: "1".into(),
                geometry: vec![a, b],
                bus_access: Access::Allowed,
                tram_access: Access::Unknown,
                level: None,
            }],
            bus_stops: vec![
                BusStop {
                    id: "A".into(),
                    owner: e.company,
                    name: "A".into(),
                    edge_id: "ab".into(),
                    position: 0.2,
                },
                BusStop {
                    id: "B".into(),
                    owner: e.company,
                    name: "B".into(),
                    edge_id: "ab".into(),
                    position: 0.8,
                },
            ],
            bus_routes: vec![BusRoute {
                id: "R".into(),
                owner: e.company,
                name: "QA bus".into(),
                edges: vec!["ab".into()],
                stops: vec![
                    BusStopOccurrence {
                        stop_id: "A".into(),
                        edge_index: 0,
                    },
                    BusStopOccurrence {
                        stop_id: "B".into(),
                        edge_index: 0,
                    },
                ],
            }],
            tram_sections: vec![],
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
}
