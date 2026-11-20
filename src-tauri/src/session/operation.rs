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
                trains: vec![],
                offers: vec![],
            };
        };
        let save = o.runtime.save();
        OperationView {
            now_ms: save.now_ms,
            paused: save.paused,
            speed: save.speed,
            invalidated: o.invalidated,
            catching_up: self.operation_target.is_some(),
            services: o.configs.clone(),
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
        self.operation_target = None;
        Ok(self.operation_snapshot())
    }
    /// Explicit destructive operation-clock reset, never invoked by edit Undo.
    pub fn reset_operation(&mut self) -> Result<OperationView, String> {
        self.world
            .configure_operation(vec![])
            .map_err(|e| format!("{e:?}"))?;
        self.operation_tick = None;
        self.operation_target = None;
        Ok(self.operation_snapshot())
    }
    pub fn set_operation_clock(
        &mut self,
        paused: bool,
        speed: u32,
    ) -> Result<OperationView, String> {
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
        self.operation_tick = Some(std::time::Instant::now());
        Ok(self.operation_snapshot())
    }
    /// Client only wakes the authority. Multiple viewers cannot multiply time.
    /// Bounded event processing retains a target rather than dropping departures.
    pub fn advance_operation(&mut self) -> Result<OperationView, String> {
        let instant = std::time::Instant::now();
        let elapsed = self
            .operation_tick
            .map(|t| instant.saturating_duration_since(t).as_millis())
            .unwrap_or(0);
        self.operation_tick = Some(instant);
        let Some(o) = self.world.operation_mut() else {
            return Ok(self.operation_snapshot());
        };
        if o.invalidated {
            self.operation_target = None;
            return Ok(self.operation_snapshot());
        }
        if o.runtime.paused() {
            return Ok(self.operation_snapshot());
        }
        let delta = u64::try_from(elapsed)
            .map_err(|_| "時計の範囲を超えました。")?
            .checked_mul(o.runtime.speed() as u64)
            .ok_or("時計の範囲を超えました。")?;
        let target = self
            .operation_target
            .unwrap_or(o.runtime.now_ms())
            .checked_add(delta)
            .ok_or("時計の範囲を超えました。")?;
        let result = o
            .runtime
            .advance_to(target, 2000)
            .map_err(|e| format!("運行の進行に失敗しました: {e:?}"))?;
        self.operation_target = (!result.reached_target).then_some(target);
        Ok(self.operation_snapshot())
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
