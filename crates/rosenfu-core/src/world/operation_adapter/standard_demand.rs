//! Explicit demand inspection for one activated Standard outbound service.
use super::*;
use crate::demand::{
    assign_scheduled_corridor, scheduled_od_span, ChoiceWeights, FixedOd, PassengerLeg,
    ScheduledBoarding, ScheduledCorridorAssignment,
};
use std::ops::Range;

impl World {
    /// Scheduled scenario from the currently configured Standard service. The
    /// input time is when riders are ready at the chosen outbound platform,
    /// not current world time. This uses no present or replayed vehicle state.
    /// The source's activation window shifts with the nominal elapsed time to
    /// that platform. An incompletely covered window remains unverified. No
    /// reverse, transfer, or automatic directional demand is inferred.
    ///
    /// The caller must pass the authenticated actor/company, not accept
    /// arbitrary owner IDs from an external view request.
    pub fn preview_standard_scheduled_corridor(
        &self,
        player: PlayerId,
        company: CompanyId,
        service: ServiceId,
        od: &FixedOd,
        window: Range<u64>,
        weights: &ChoiceWeights,
    ) -> Result<ScheduledCorridorAssignment, String> {
        let operation = self
            .operation()
            .filter(|operation| !operation.invalidated)
            .ok_or("standard train timetable unavailable")?;
        if operation.standard_owner != Some((player, company))
            || !self
                .data
                .companies
                .iter()
                .any(|c| c.id == company && c.owner == player)
            || !self
                .data
                .services
                .iter()
                .any(|s| s.id == service && s.owner == company)
        {
            return Err("standard scheduled demand unavailable to this actor".into());
        }
        let ready_ms = window.start;
        let end_ms = window.end;
        scheduled_od_span(ready_ms, end_ms).ok_or("invalid scheduled OD window")?;
        let runtime = &operation.runtime;
        let source = service.0.to_string();
        let grid = runtime
            .standard_departure_grid(&source)
            .ok_or("select the activated standard timetable service")?;
        let spec = runtime
            .service_spec(&source)
            .ok_or("standard train route unavailable")?;
        let (origin, destination) = match (od.access.as_slice(), od.egress.as_slice()) {
            ([origin], [destination]) => (&origin.station, &destination.station),
            _ => return Err("select one origin and destination station".into()),
        };
        // Standard admission requires an exact mirrored return and unique
        // outbound stops. A passed station does not appear as a compiled stop.
        let outbound = &spec.legs[..spec.legs.len() / 2];
        let boarding_index = outbound
            .iter()
            .position(|leg| &leg.from == origin)
            .ok_or("origin is not a boardable outbound scheduled stop")?;
        let corridor = &outbound[boarding_index..];
        let destination_index = corridor
            .iter()
            .position(|leg| &leg.to == destination)
            .ok_or("destination is not a later outbound scheduled stop")?;

        // The scheduler's nominal departure from an intermediate platform is
        // the originating birth plus previous rides and max(dwell, clearance).
        // This is not an estimate of actual waiting for reserved track.
        let offset = (0..boarding_index)
            .try_fold(0u64, |sum, index| {
                let (ride, stand) = runtime.unblocked_leg_timing(&source, index)?;
                sum.checked_add(ride)?.checked_add(stand)
            })
            .ok_or("compiled scheduled boarding time unavailable")?;
        let mut legs = Vec::with_capacity(destination_index + 1);
        for (sequence, leg) in corridor[..=destination_index].iter().enumerate() {
            let index = boarding_index + sequence;
            let (ride, _) = runtime
                .unblocked_leg_timing(&source, index)
                .ok_or("compiled scheduled corridor timing unavailable")?;
            let through = if sequence == 0 {
                0
            } else {
                runtime
                    .unblocked_leg_timing(&source, index - 1)
                    .ok_or("compiled scheduled dwell unavailable")?
                    .1
            };
            legs.push(PassengerLeg {
                leg_id: format!("{source}:{index}"),
                sequence,
                board_allowed: sequence == 0,
                alight_allowed: sequence == destination_index,
                service_id: source.clone(),
                from: leg.from.clone(),
                to: leg.to.clone(),
                ride_seconds: ride as f64 / 1_000.0,
                through_seconds: through as f64 / 1_000.0,
                // No scalar headway is manufactured from a multi-band source.
                headway_seconds: 0.0,
            });
        }
        let coverage = grid.active_window();
        let schedule = match (ready_ms.checked_sub(offset), end_ms.checked_sub(offset)) {
            (Some(start), Some(end)) if start >= coverage.start && end <= coverage.end => {
                let first_departure_ms = grid
                    .cursor_after(start.checked_sub(1))
                    .next_departure()
                    .filter(|at| *at < end)
                    .and_then(|at| at.checked_add(offset));
                ScheduledBoarding::Verified {
                    ready_ms,
                    end_ms,
                    first_departure_ms,
                    scheduled_departures: grid.departure_count(start..end),
                }
            }
            _ => ScheduledBoarding::Unverified,
        };
        assign_scheduled_corridor(od, &legs, schedule, weights)
    }
}
