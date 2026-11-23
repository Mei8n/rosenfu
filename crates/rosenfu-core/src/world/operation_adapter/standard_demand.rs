//! Explicit demand inspection for the activated Standard origin stream.
use super::*;
use crate::demand::{
    assign_scheduled_corridor, scheduled_od_span, ChoiceWeights, FixedOd, PassengerLeg,
    ScheduledBoarding, ScheduledCorridorAssignment,
};
use std::ops::Range;

impl World {
    /// Scheduled scenario from the currently configured Standard service. The
    /// input time is when riders are ready at its origin platform, not current
    /// world time; this uses no present or replayed vehicle state. For this
    /// first slice only origin→one outbound stop is supported. A window not
    /// fully inside this source's coverage remains unverified rather than
    /// being extended past activation or expiry. No reverse or automatic
    /// directional demand is inferred from the origin's served path.
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
        if spec.legs.first().is_none_or(|leg| &leg.from != origin) || origin == destination {
            return Err("scheduled corridor must start at the selected service origin".into());
        }
        // Standard schedule admission already requires the exact mirrored
        // return; use only the outbound half. A passed station never appears
        // as a compiled stop, so it cannot be chosen as a destination.
        let outbound = &spec.legs[..spec.legs.len() / 2];
        let destination_index = outbound
            .iter()
            .position(|leg| &leg.to == destination)
            .ok_or("destination is not an outbound scheduled stop")?;
        let mut legs = Vec::with_capacity(destination_index + 1);
        for (sequence, leg) in outbound[..=destination_index].iter().enumerate() {
            let (ride, _) = runtime
                .unblocked_leg_timing(&source, sequence)
                .ok_or("compiled scheduled corridor timing unavailable")?;
            let through = sequence
                .checked_sub(1)
                .map_or(Some(0), |previous| {
                    runtime
                        .unblocked_leg_timing(&source, previous)
                        .map(|(_, stand)| stand)
                })
                .ok_or("compiled scheduled dwell unavailable")?;
            legs.push(PassengerLeg {
                leg_id: format!("{source}:{sequence}"),
                sequence,
                board_allowed: sequence == 0,
                alight_allowed: sequence == destination_index,
                service_id: source.clone(),
                from: leg.from.clone(),
                to: leg.to.clone(),
                ride_seconds: ride as f64 / 1_000.0,
                through_seconds: through as f64 / 1_000.0,
                // No scalar headway is manufactured from a multi-band source.
                // The dedicated solver receives the actual scheduled wait.
                headway_seconds: 0.0,
            });
        }
        let coverage = grid.active_window();
        let schedule = if ready_ms < coverage.start || end_ms > coverage.end {
            ScheduledBoarding::Unverified
        } else {
            let first_departure_ms = grid
                .cursor_after(ready_ms.checked_sub(1))
                .next_departure()
                .filter(|at| *at < end_ms);
            let scheduled_departures = grid.departure_count(window);
            ScheduledBoarding::Verified {
                ready_ms,
                end_ms,
                first_departure_ms,
                scheduled_departures,
            }
        };
        assign_scheduled_corridor(od, &legs, schedule, weights)
    }
}
