//! Local feature-only codec. No ordinary save path, overwrite, or promotion.
use super::{decode, encode, CheckedJson, StorageError};
use rosenfu_core::operation::service_planning::periodic::DailyServiceBand;
use rosenfu_core::operation::{AdvanceResult, OperationError, Train};
use rosenfu_core::world::periodic_qa::{BoundPeriodicQa, BoundPeriodicQaSave};
use rosenfu_core::world::{CompanyId, PlayerId, ServiceId, World};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ops::Range;

const MAX_QA_BYTES: usize = 2 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Format {
    RosenfuPeriodicQaV1,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    format: Format,
    frozen_world_json: String,
    frozen_sha256: [u8; 32],
    periodic: BoundPeriodicQaSave,
}

/// Keeps the paused original world private: no ordinary scheduled OD or
/// crossing-planning call can inadvertently use its old absolute departures.
pub struct PeriodicQaWorld {
    frozen_json: String,
    frozen_sha256: [u8; 32],
    engine: BoundPeriodicQa,
}

impl PeriodicQaWorld {
    pub fn start(
        world: &World,
        player: PlayerId,
        company: CompanyId,
        service: ServiceId,
        window: Range<u64>,
        spacing_ms: u64,
        bands: Vec<DailyServiceBand>,
    ) -> Result<Self, StorageError> {
        let engine = world
            .start_bound_periodic_qa(player, company, service, window, spacing_ms, bands)
            .map_err(|_| StorageError::InvalidHeader)?;
        let frozen = encode(world)?;
        if frozen.len() > MAX_QA_BYTES / 2 {
            return Err(StorageError::TooLarge);
        }
        let frozen_sha256 = Sha256::digest(&frozen).into();
        let frozen_json = String::from_utf8(frozen).map_err(|_| StorageError::InvalidHeader)?;
        Ok(Self {
            frozen_json,
            frozen_sha256,
            engine,
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>, StorageError> {
        let mut bytes = serde_json::to_vec(&File {
            format: Format::RosenfuPeriodicQaV1,
            frozen_world_json: self.frozen_json.clone(),
            frozen_sha256: self.frozen_sha256,
            periodic: self.engine.save(),
        })?;
        bytes.push(b'\n');
        if bytes.len() > MAX_QA_BYTES {
            return Err(StorageError::TooLarge);
        }
        Ok(bytes)
    }

    pub fn decode(
        bytes: &[u8],
        player: PlayerId,
        company: CompanyId,
    ) -> Result<Self, StorageError> {
        if bytes.len() > MAX_QA_BYTES {
            return Err(StorageError::TooLarge);
        }
        serde_json::from_slice::<CheckedJson>(bytes)?;
        let file = serde_json::from_slice::<File>(bytes)?;
        if file.frozen_world_json.len() > MAX_QA_BYTES / 2 {
            return Err(StorageError::TooLarge);
        }
        let frozen = file.frozen_world_json.as_bytes();
        let found: [u8; 32] = Sha256::digest(frozen).into();
        if file.frozen_sha256 != found {
            return Err(StorageError::InvalidHeader);
        }
        let world = decode(frozen)?;
        let engine = world
            .restore_bound_periodic_qa(player, company, file.periodic)
            .map_err(|_| StorageError::InvalidHeader)?;
        Ok(Self {
            frozen_json: file.frozen_world_json,
            frozen_sha256: found,
            engine,
        })
    }

    pub fn now_ms(&self) -> u64 {
        self.engine.now_ms()
    }

    pub fn advance_to(
        &mut self,
        until_ms: u64,
        event_budget: usize,
    ) -> Result<AdvanceResult, OperationError> {
        self.engine.advance_to(until_ms, event_budget)
    }

    pub fn trains(&self) -> impl Iterator<Item = &Train> {
        self.engine.trains()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rosenfu_core::operation::service_planning::{
        periodic::DailyBandService, FleetPlanningRequest,
    };
    use rosenfu_core::operation::{Timetable, Vehicle};
    use rosenfu_core::world::{GeoPoint, OperationServiceConfig, RouteSegment};

    fn fixture(last_lon: f64) -> (World, PlayerId, CompanyId, ServiceId) {
        let mut w = World::default();
        let player = w.add_player("QA player".into()).unwrap();
        let company = w.add_company(player, "QA railway".into()).unwrap();
        let alignment = w
            .build_alignment(
                company,
                "QA isolated curve".into(),
                vec![
                    GeoPoint {
                        longitude: 139.0,
                        latitude: 35.0,
                    },
                    GeoPoint {
                        longitude: 139.01,
                        latitude: 35.003,
                    },
                    GeoPoint {
                        longitude: last_lon,
                        latitude: 35.0,
                    },
                ],
            )
            .unwrap();
        let from = w
            .place_station_on_alignment(company, "A".into(), alignment, 0.1, 100., 10.)
            .unwrap();
        let to = w
            .place_station_on_alignment(company, "B".into(), alignment, 0.9, 100., 10.)
            .unwrap();
        let legs = vec![RouteSegment::Alignment {
            alignment_id: alignment,
            from,
            to,
        }];
        let stops = w
            .preview_service_path(from, legs.clone())
            .unwrap()
            .occurrences
            .into_iter()
            .map(|o| o.reference)
            .collect();
        let service = w
            .add_service_path(company, "QA service".into(), from, legs, stops)
            .unwrap();
        w.configure_operation(vec![OperationServiceConfig {
            service_id: service,
            vehicle: Vehicle {
                max_speed_kmh: 100.,
                acceleration_mps2: 1.,
                braking_mps2: 1.,
                length_m: 80.,
            },
            timetable: Timetable {
                first_departure_ms: 0,
                last_departure_ms: 0,
                headway_ms: 60_000,
            },
            dwell_ms: 10_000,
        }])
        .unwrap();
        assert_eq!(w.operation_save().unwrap().runtime.format, 2);
        let idle = w
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(1_000_000, 128)
            .unwrap();
        assert!(idle.reached_target);
        assert!(w.operation().unwrap().runtime.trains().next().is_none());
        (w, player, company, service)
    }

    fn bands() -> Vec<DailyServiceBand> {
        vec![DailyServiceBand {
            start_ms: 1_100_000,
            end_ms: 1_200_000,
            service: DailyBandService::Planned {
                fleet: FleetPlanningRequest::TimedHeadway { headway_ms: 60_000 },
            },
        }]
    }

    #[test]
    fn separate_format_roundtrip_of_real_owned_world_stays_out_of_ordinary_codec() {
        let (world, player, company, service) = fixture(139.02);
        let ordinary = encode(&world).unwrap();
        let mut qa = PeriodicQaWorld::start(
            &world,
            player,
            company,
            service,
            1_100_000..1_200_000,
            60_000,
            bands(),
        )
        .unwrap();
        assert!(qa.advance_to(1_100_000, 128).unwrap().reached_target);
        assert_eq!(qa.trains().next().unwrap().key.scheduled_ms, 1_100_000);
        let wire = qa.encode().unwrap();
        assert!(matches!(decode(&wire), Err(StorageError::InvalidFormat)));
        assert!(encode(&decode(&ordinary).unwrap()).is_ok_and(|bytes| bytes == ordinary));
        assert_eq!(encode(&world).unwrap(), ordinary);
        assert!(PeriodicQaWorld::decode(&wire, PlayerId(player.0 + 1), company).is_err());
        let mut reopened = PeriodicQaWorld::decode(&wire, player, company).unwrap();
        assert_eq!(reopened.now_ms(), qa.now_ms());
        assert_eq!(reopened.encode().unwrap(), wire);
        assert_eq!(
            reopened.advance_to(1_600_000, 128).unwrap(),
            qa.advance_to(1_600_000, 128).unwrap()
        );
        assert_eq!(reopened.encode().unwrap(), qa.encode().unwrap());
    }

    #[test]
    fn queued_clock_and_swapped_physical_world_are_refused() {
        let (mut world, player, company, service) = fixture(139.02);
        let start = |w: &World| {
            PeriodicQaWorld::start(
                w,
                player,
                company,
                service,
                1_100_000..1_200_000,
                60_000,
                bands(),
            )
        };
        world.operation_mut().unwrap().pending_target = Some(1_010_000);
        assert!(start(&world).is_err());
        world.operation_mut().unwrap().pending_target = None;
        let wire = start(&world).unwrap().encode().unwrap();
        let (changed, _, _, _) = fixture(139.03);
        let mut swapped: File = serde_json::from_slice(&wire).unwrap();
        let changed_bytes = encode(&changed).unwrap();
        swapped.frozen_sha256 = Sha256::digest(&changed_bytes).into();
        swapped.frozen_world_json = String::from_utf8(changed_bytes).unwrap();
        let rebuilt = serde_json::to_vec(&swapped).unwrap();
        assert!(matches!(
            PeriodicQaWorld::decode(&rebuilt, player, company),
            Err(StorageError::InvalidHeader)
        ));
    }

    #[cfg(feature = "standard-periodic-rail")]
    #[test]
    fn direct_scheduled_demand_uses_standard_origin_grid_after_reopen() {
        use rosenfu_core::demand::{
            ChoiceWeights, FixedOd, MarketKind, ScheduledCorridorStatus, StationAccess,
        };
        use rosenfu_core::operation::periodic_dispatch::standard::{
            NamedBandOverrides, StandardIntervals,
        };
        const H: u64 = 3_600_000;
        const M: u64 = 60_000;
        let (mut world, player, company, service) = fixture(139.02);
        let old = world.operation_save().unwrap();
        let outbound = &old.runtime.services[0].legs[0];
        let od = FixedOd {
            id: "synthetic-owned-intercity-od".into(),
            origin_region: "home".into(),
            destination_region: "center".into(),
            origin_location: "home-zone".into(),
            destination_location: "center-zone".into(),
            owner_region: "home".into(),
            market: MarketKind::Intercity,
            trips: Some(100.0),
            access: vec![StationAccess {
                station: outbound.from.clone(),
                seconds: 60.0,
            }],
            egress: vec![StationAccess {
                station: outbound.to.clone(),
                seconds: 60.0,
            }],
            alternative_seconds: Some(1200.0),
        };
        world
            .apply_standard_timetable(
                &old,
                player,
                company,
                service,
                StandardIntervals {
                    base_minutes: 60,
                    boundaries_minutes: [300, 420, 600, 960, 1200, 1500],
                    start_ms: 6 * H,
                    end_ms: 27 * H,
                    overrides: NamedBandOverrides {
                        early_morning: None,
                        morning_rush: Some(20),
                        midday: None,
                        evening_rush: Some(30),
                        late_night: None,
                    },
                },
            )
            .unwrap();
        let before = encode(&world).unwrap();
        let reopened = decode(&before).unwrap();
        let direct = |w: &World, ready, end| {
            w.preview_standard_scheduled_corridor(
                player,
                company,
                service,
                &od,
                ready..end,
                &ChoiceWeights::default(),
            )
            .unwrap()
        };
        let regular = direct(&world, 11 * H + M, 13 * H);
        let peak = direct(&world, 8 * H + M, 10 * H);
        assert_eq!(direct(&reopened, 8 * H + M, 10 * H), peak);
        assert_eq!(regular.first_departure_ms, Some(12 * H));
        assert_eq!(peak.first_departure_ms, Some(8 * H + 20 * M));
        assert_eq!(regular.scheduled_departures, Some(1));
        assert_eq!(peak.scheduled_departures, Some(5));
        assert_eq!(regular.assignment.rail[0].time.wait_seconds, 59.0 * 60.0);
        assert_eq!(peak.assignment.rail[0].time.wait_seconds, 19.0 * 60.0);
        assert!(peak.assignment.rail[0].trips > regular.assignment.rail[0].trips);
        assert_eq!(
            regular.assignment.rail[0].time.ride_seconds,
            peak.assignment.rail[0].time.ride_seconds
        );
        for status in [&regular, &peak] {
            assert!(
                (status.assignment.rail[0].trips + status.assignment.alternative_trips - 100.0)
                    .abs()
                    < 1e-8
            );
        }
        let gap = direct(&world, 8 * H + M, 8 * H + 20 * M);
        assert_eq!(gap.status, ScheduledCorridorStatus::NoDepartureInWindow);
        assert_eq!(gap.assignment.alternative_trips, 100.0);
        let unknown = direct(&world, 5 * H + 55 * M, 6 * H + 5 * M);
        assert_eq!(unknown.status, ScheduledCorridorStatus::UnverifiedSchedule);
        assert_eq!(unknown.assignment.unresolved_trips, 100.0);
        assert_eq!(unknown.assignment.alternative_trips, 0.0);
        for (actor, operator, run) in [
            (PlayerId(player.0 + 1), company, service),
            (player, CompanyId(company.0 + 1), service),
            (player, company, ServiceId(service.0 + 1)),
        ] {
            assert!(world
                .preview_standard_scheduled_corridor(
                    actor,
                    operator,
                    run,
                    &od,
                    8 * H..9 * H,
                    &ChoiceWeights::default()
                )
                .is_err());
        }
        let mut wrong_direction = od;
        std::mem::swap(&mut wrong_direction.access, &mut wrong_direction.egress);
        assert!(world
            .preview_standard_scheduled_corridor(
                player,
                company,
                service,
                &wrong_direction,
                8 * H..9 * H,
                &ChoiceWeights::default()
            )
            .is_err());
        assert_eq!(encode(&world).unwrap(), before);
        assert_eq!(encode(&reopened).unwrap(), before);
    }

    #[cfg(feature = "standard-periodic-rail")]
    #[test]
    fn standard_five_bands_use_actual_world_and_standard_save_without_legacy_phantom() {
        use rosenfu_core::operation::periodic_dispatch::standard::{
            NamedBandOverrides, StandardIntervals,
        };
        use rosenfu_core::world::PlannedCrossingDelayError;
        const H: u64 = 3_600_000;
        const M: u64 = 60_000;
        let (mut world, player, company, service) = fixture(139.02);
        // There were still future absolute births. Explicit paused
        // replacement cancels them rather than combining two sources.
        let mut config = world.operation().unwrap().configs[0].clone();
        config.timetable = Timetable {
            first_departure_ms: 6 * H + 7 * M,
            last_departure_ms: 6 * H + 7 * M,
            headway_ms: 60_000,
        };
        world
            .reconfigure_idle_operation(vec![config], Vec::new(), &service.0.to_string())
            .unwrap();
        let ordinary_before = encode(&world).unwrap();
        assert!(!ordinary_before
            .windows(b"standard_timetable".len())
            .any(|x| x == b"standard_timetable"));
        let expected = world.operation_save().unwrap();
        let authored = StandardIntervals {
            base_minutes: 60,
            boundaries_minutes: [300, 420, 600, 960, 1200, 1500],
            start_ms: 6 * H,
            end_ms: 27 * H,
            overrides: NamedBandOverrides {
                early_morning: None,
                morning_rush: Some(20),
                midday: None,
                evening_rush: Some(30),
                late_night: Some(90),
            },
        };
        assert!(world
            .apply_standard_timetable(
                &expected,
                PlayerId(player.0 + 1),
                company,
                service,
                authored.clone()
            )
            .is_err());
        let mut wrong = expected.clone();
        wrong.runtime.now_ms -= 1;
        assert!(world
            .apply_standard_timetable(&wrong, player, company, service, authored.clone())
            .is_err());
        assert_eq!(encode(&world).unwrap(), ordinary_before);
        world
            .apply_standard_timetable(&expected, player, company, service, authored)
            .unwrap();
        let runtime = &world.operation().unwrap().runtime;
        assert!(runtime.checked_scalar_passenger_offers().is_err());
        assert_eq!(
            world.preview_planned_crossing_delay(6 * H),
            Err(PlannedCrossingDelayError::StandardTimetableUnsupported)
        );
        let grid = runtime
            .standard_departure_grid(&service.0.to_string())
            .unwrap();
        assert_eq!(grid.active_window(), 6 * H..27 * H);
        for at in [
            6 * H,
            7 * H,
            7 * H + 20 * M,
            12 * H,
            16 * H + 30 * M,
            24 * H + 30 * M,
            26 * H,
        ] {
            assert!(grid.is_scheduled(at), "{at}");
        }
        assert!(!grid.is_scheduled(6 * H + 7 * M)); // retired absolute source
        assert!(!grid.is_scheduled(24 * H)); // no artificial midnight reset
        assert!(!grid.is_scheduled(27 * H)); // terminal horizon exclusive
        let _ = world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(24 * H + 30 * M, 4096)
            .unwrap();
        let train = world.operation().unwrap().runtime.trains().next().unwrap();
        assert_eq!(train.key.scheduled_ms, 24 * H + 30 * M);
        let wire = encode(&world).unwrap();
        let mut reopened = decode(&wire).unwrap();
        assert_eq!(wire, encode(&reopened).unwrap());
        let a = world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(28 * H, 4096)
            .unwrap();
        let b = reopened
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(28 * H, 4096)
            .unwrap();
        assert_eq!(a, b);
        assert_eq!(encode(&world).unwrap(), encode(&reopened).unwrap());
        let tampered = String::from_utf8(wire)
            .unwrap()
            .replace("standard_daily_v1", "unknown_v2");
        assert!(decode(tampered.as_bytes()).is_err());
    }
}
