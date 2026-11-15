//! Explicit presentation metadata. Absence preserves the renderer's default.
use super::*;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceAppearance {
    pub service_id: ServiceId,
    pub color: String,
}
fn valid_color(color: &str) -> bool {
    color.len() == 7
        && color.starts_with('#')
        && color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}
impl WorldDraft {
    pub(super) fn validate_service_appearances(&self) -> Result<(), WorldError> {
        let mut ids = BTreeSet::new();
        for row in &self.service_appearances {
            if !valid_color(&row.color) || !ids.insert(row.service_id) {
                return Err(WorldError::InvalidServiceAppearance);
            }
            if !self
                .services
                .iter()
                .any(|service| service.id == row.service_id)
            {
                return Err(WorldError::MissingReference("service_appearance.service"));
            }
        }
        Ok(())
    }
}
impl World {
    pub fn set_service_color(
        &mut self,
        owner: CompanyId,
        service_id: ServiceId,
        color: String,
    ) -> Result<(), WorldError> {
        if !valid_color(&color) {
            return Err(WorldError::InvalidServiceAppearance);
        }
        if !self
            .data
            .services
            .iter()
            .any(|s| s.id == service_id && s.owner == owner)
        {
            return Err(WorldError::MissingReference("owned service"));
        }
        let mut candidate = self.data.clone();
        candidate
            .service_appearances
            .retain(|row| row.service_id != service_id);
        candidate.service_appearances.push(ServiceAppearance {
            service_id,
            color: color.to_ascii_uppercase(),
        });
        self.replace(candidate)
    }
    pub fn duplicate_service(
        &mut self,
        owner: CompanyId,
        service_id: ServiceId,
        name: String,
    ) -> Result<ServiceId, WorldError> {
        let source = self
            .data
            .services
            .iter()
            .find(|s| s.id == service_id && s.owner == owner)
            .cloned()
            .ok_or(WorldError::MissingReference("owned service"))?;
        let path = self
            .data
            .service_paths
            .iter()
            .find(|p| p.service_id == service_id)
            .cloned()
            .ok_or(WorldError::InvalidServicePath)?;
        let binding = self
            .data
            .service_route_bindings
            .iter()
            .find(|b| b.service_id == service_id)
            .cloned();
        let appearance = self
            .data
            .service_appearances
            .iter()
            .find(|b| b.service_id == service_id)
            .cloned();
        self.insert(move |data, id| {
            let id = ServiceId(id);
            data.services.push(OperatingService { id, name, ..source });
            data.service_paths.push(ServicePath {
                service_id: id,
                ..path
            });
            if let Some(row) = binding {
                data.service_route_bindings.push(ServiceRouteBinding {
                    service_id: id,
                    ..row
                });
            }
            if let Some(row) = appearance {
                data.service_appearances.push(ServiceAppearance {
                    service_id: id,
                    ..row
                });
            }
        })
        .map(ServiceId)
    }
}
