//! Experimental frozen-world binding, not a live World operation mode.
use super::*;
use crate::operation::periodic_dispatch::{PeriodicOperation, PeriodicOperationSave};
use crate::operation::service_planning::periodic::DailyServiceBand;
use crate::operation::{AdvanceResult, PassengerReplay, PassengerReplayError, Train};
use sha2::{Digest, Sha256};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundPeriodicQaSave {
    service_id: ServiceId,
    player: PlayerId,
    company: CompanyId,
    physical_digest: [u8; 32],
    runtime: PeriodicOperationSave,
}

pub struct BoundPeriodicQa {
    service_id: ServiceId,
    player: PlayerId,
    company: CompanyId,
    physical_digest: [u8; 32],
    runtime: PeriodicOperation,
}

impl World {
    fn periodic_qa_binding(
        &self,
        player: PlayerId,
        company: CompanyId,
        service_id: ServiceId,
    ) -> Result<(WorldOperationSave, [u8; 32]), OperationError> {
        if !self
            .data
            .companies
            .iter()
            .any(|c| c.id == company && c.owner == player)
            || !self
                .data
                .services
                .iter()
                .any(|s| s.id == service_id && s.owner == company)
        {
            return Err(OperationError::InvalidPath);
        }
        let op = self.operation().ok_or(OperationError::InvalidPath)?;
        if op.invalidated
            || op.pending_target.is_some()
            || !op.runtime.has_started()
            || !op.runtime.paused()
            || op.runtime.trains().next().is_some()
            || op.runtime.admission_model() != RailAdmissionModel::Fixed
            || op.runtime.rail_timetable_handoff_in_progress()
        {
            return Err(OperationError::InvalidClock);
        }
        if !op.bus_configs.is_empty()
            || !(1..=4).contains(&op.configs.len())
            || !op.configs.iter().any(|c| c.service_id == service_id)
        {
            return Err(OperationError::InvalidPath);
        }
        let frozen = self.operation_save().ok_or(OperationError::InvalidPath)?;
        // Compile using the existing dispatch reservation adapter, retaining
        // the actual admitted model: no fallback from progressive/turnback.
        let exact = self.compile_all_operation(
            op.configs.clone(),
            Vec::new(),
            Some(RailAdmissionModel::Fixed),
        )?;
        if exact.runtime.save().services != frozen.runtime.services {
            return Err(OperationError::ReservationScopeChanged);
        }
        let binding = serde_json::to_vec(&(player, company, service_id, &frozen))
            .map_err(|_| OperationError::InvalidPath)?;
        Ok((frozen, Sha256::digest(binding).into()))
    }

    /// A separate QA engine; the authoritative ordinary world is not edited.
    pub fn start_bound_periodic_qa(
        &self,
        player: PlayerId,
        company: CompanyId,
        service_id: ServiceId,
        window: Range<u64>,
        spacing_ms: u64,
        bands: Vec<DailyServiceBand>,
    ) -> Result<BoundPeriodicQa, OperationError> {
        let (_, physical_digest) = self.periodic_qa_binding(player, company, service_id)?;
        let runtime = PeriodicOperation::start_from_idle(
            self.operation()
                .ok_or(OperationError::InvalidPath)?
                .runtime
                .clone(),
            service_id.0.to_string(),
            window,
            spacing_ms,
            bands,
        )?;
        Ok(BoundPeriodicQa {
            service_id,
            player,
            company,
            physical_digest,
            runtime,
        })
    }

    pub fn restore_bound_periodic_qa(
        &self,
        player: PlayerId,
        company: CompanyId,
        save: BoundPeriodicQaSave,
    ) -> Result<BoundPeriodicQa, OperationError> {
        if save.player != player || save.company != company {
            return Err(OperationError::InvalidPath);
        }
        let (frozen, expected) = self.periodic_qa_binding(player, company, save.service_id)?;
        if save.physical_digest != expected
            || !save
                .runtime
                .matches_frozen(&save.service_id.0.to_string(), &frozen.runtime)
        {
            return Err(OperationError::ReservationScopeChanged);
        }
        let runtime = PeriodicOperation::restore(save.runtime)?;
        Ok(BoundPeriodicQa {
            service_id: save.service_id,
            player,
            company,
            physical_digest: expected,
            runtime,
        })
    }
}

impl BoundPeriodicQa {
    pub fn advance_to(
        &mut self,
        until: u64,
        event_budget: usize,
    ) -> Result<AdvanceResult, OperationError> {
        self.runtime.advance_to(until, event_budget)
    }

    pub fn now_ms(&self) -> u64 {
        self.runtime.now_ms()
    }

    pub fn trains(&self) -> impl Iterator<Item = &Train> {
        self.runtime.trains()
    }

    // The wrapper deliberately exposes observed replay only. Scheduled OD
    // and the crossing planning inspector cannot consume the frozen grid.
    pub fn preview_passenger_replay(
        &self,
        until: u64,
        event_budget: usize,
    ) -> Result<PassengerReplay, PassengerReplayError> {
        self.runtime.preview_passenger_replay(until, event_budget)
    }

    pub fn save(&self) -> BoundPeriodicQaSave {
        BoundPeriodicQaSave {
            service_id: self.service_id,
            player: self.player,
            company: self.company,
            physical_digest: self.physical_digest,
            runtime: self.runtime.save(),
        }
    }
}
