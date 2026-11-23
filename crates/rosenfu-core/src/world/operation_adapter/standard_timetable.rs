//! Native feature gate. A configured operation retains its physical model,
//! saves one explicit named-band source, and rejects stale cold updates.
use super::*;
use crate::operation::periodic_dispatch::{
    standard::{StandardDailyV1, StandardIntervals},
    ScheduleSource,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "version", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum StandardWorldSchedule {
    StandardDailyV1 {
        player: PlayerId,
        company: CompanyId,
        source: StandardDailyV1,
    },
}

impl World {
    pub fn has_standard_timetable(&self) -> bool {
        self.operation()
            .is_some_and(|op| op.runtime.has_standard_timetable())
    }

    fn check_standard_owner(
        &self,
        player: PlayerId,
        company: CompanyId,
        service: ServiceId,
    ) -> Result<(), OperationError> {
        if !self
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
            return Err(OperationError::InvalidPath);
        }
        Ok(())
    }

    /// Standalone native commit. Caller must authorize player/company and pass
    /// the operation snapshot it actually reviewed (also guards period edits
    /// at an unchanged world clock). No source attaches to another service or
    /// automatically reconfigures its physical turns and reservations.
    pub fn apply_standard_timetable(
        &mut self,
        expected: &WorldOperationSave,
        player: PlayerId,
        company: CompanyId,
        service: ServiceId,
        authored: StandardIntervals,
    ) -> Result<(), OperationError> {
        self.check_standard_owner(player, company, service)?;
        if self.operation_save().as_ref() != Some(expected) {
            return Err(OperationError::ReservationScopeChanged);
        }
        let op = self.operation().ok_or(OperationError::InvalidClock)?;
        if op.invalidated
            || op.pending_target.is_some()
            || !op.runtime.paused()
            || !op.runtime.has_started()
            || op.runtime.trains().next().is_some()
            || op.runtime.rail_timetable_handoff_in_progress()
            || op.runtime.has_standard_timetable()
        {
            return Err(OperationError::InvalidClock);
        }
        self.validate_standard_physics(expected, service)?;
        let op = self.operation_mut().ok_or(OperationError::InvalidClock)?;
        op.runtime
            .set_standard_timetable(service.0.to_string(), authored)?;
        op.standard_owner = Some((player, company));
        Ok(())
    }

    /// Check the same exact physical compatibility and archived runtime that
    /// the ordinary loader would require after a proposed world edit.
    pub fn validate_retained_standard_timetable(&self) -> Result<(), OperationError> {
        let saved = self.operation_save().ok_or(OperationError::InvalidClock)?;
        if self.restore_standard_timetable(&saved)?.is_none() {
            return Err(OperationError::ReservationScopeChanged);
        }
        Ok(())
    }

    fn validate_standard_physics(
        &self,
        saved: &WorldOperationSave,
        target: ServiceId,
    ) -> Result<RailAdmissionModel, OperationError> {
        let model = match saved.runtime.format {
            2 => RailAdmissionModel::Fixed,
            4 => RailAdmissionModel::TurnbackHold,
            _ => return Err(OperationError::ReservationScopeChanged),
        };
        if saved.invalidated
            || !saved.bus_configs.is_empty()
            || !(1..=4).contains(&saved.configs.len())
            || !saved.configs.iter().any(|c| c.service_id == target)
        {
            return Err(OperationError::ReservationScopeChanged);
        }
        let exact = self.compile_all_operation(saved.configs.clone(), Vec::new(), Some(model))?;
        if exact.runtime.save().services != saved.runtime.services {
            return Err(OperationError::ReservationScopeChanged);
        }
        Ok(model)
    }

    pub(super) fn restore_standard_timetable(
        &self,
        saved: &WorldOperationSave,
    ) -> Result<Option<(OperationRuntime, PlayerId, CompanyId)>, OperationError> {
        let Some(StandardWorldSchedule::StandardDailyV1 {
            player,
            company,
            source,
        }) = &saved.standard_timetable
        else {
            return Ok(None);
        };
        let target: ServiceId = source
            .service_id
            .clone()
            .try_into()
            .map_err(|_| OperationError::InvalidPath)?;
        self.check_standard_owner(*player, *company, target)?;
        let admitted = self.validate_standard_physics(saved, target)?;
        let runtime = OperationRuntime::restore_checked_with_source(
            saved.runtime.clone(),
            Some(admitted),
            Some(&ScheduleSource::StandardDailyV1(source.clone())),
        )?;
        Ok(Some((runtime, *player, *company)))
    }
}

impl WorldOperationSave {
    pub fn has_standard_timetable(&self) -> bool {
        self.standard_timetable.is_some()
    }
}
