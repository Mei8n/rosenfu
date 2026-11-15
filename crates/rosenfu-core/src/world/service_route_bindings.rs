//! Explicit route association, separate from a service's immutable physical path.
//! An absent row is unknown, never silently migrated to independent.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ServiceRouteBindingKind {
    Independent,
    Route { route_id: RouteId },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceRouteBinding {
    pub service_id: ServiceId,
    pub binding: ServiceRouteBindingKind,
}
#[derive(Clone, Copy, PartialEq)]
enum Infrastructure {
    Alignment(AlignmentId),
}
#[derive(Clone, Copy)]
struct Span {
    infrastructure: Infrastructure,
    begin: f64,
    end: f64,
}
impl Span {
    fn forward(self) -> bool {
        self.end > self.begin
    }
    fn contains(self, point: f64) -> bool {
        self.begin.min(self.end) <= point && point <= self.begin.max(self.end)
    }
}
fn spans(
    data: &WorldDraft,
    start: StationId,
    segments: &[RouteSegment],
) -> Result<Vec<Span>, WorldError> {
    let invalid = WorldError::InvalidServiceRouteBinding;
    let placement = |id| {
        data.station_placements
            .iter()
            .find(|p| p.station_id == id)
            .ok_or(invalid.clone())
    };
    let mut output: Vec<Span> = vec![];
    let mut append = |span: Span| {
        if span.begin == span.end {
            return;
        }
        if let Some(last) = output.last_mut() {
            if last.infrastructure == span.infrastructure
                && last.end == span.begin
                && last.forward() == span.forward()
            {
                last.end = span.end;
                return;
            }
        }
        output.push(span);
    };
    let mut current = start;
    for segment in segments {
        match segment {
            RouteSegment::Alignment {
                alignment_id,
                from,
                to,
            } => {
                if current != *from {
                    return Err(invalid);
                }
                let a = placement(*from)?;
                let b = placement(*to)?;
                if a.alignment_id != *alignment_id || b.alignment_id != *alignment_id {
                    return Err(invalid);
                }
                append(Span {
                    infrastructure: Infrastructure::Alignment(*alignment_id),
                    begin: a.position,
                    end: b.position,
                });
                current = *to;
            }
            RouteSegment::ConnectedAlignments {
                from,
                to,
                connections,
            } => {
                if current != *from {
                    return Err(invalid);
                }
                alignment_connections::resolve_corridor(data, *from, *to, connections)
                    .map_err(|_| invalid.clone())?;
                let a = placement(*from)?;
                let b = placement(*to)?;
                let mut id = a.alignment_id;
                let mut begin = a.position;
                for link in connections {
                    if id != link.a.alignment_id {
                        return Err(invalid);
                    }
                    let end = if link.a.endpoint == AlignmentEndpoint::Start {
                        0.0
                    } else {
                        1.0
                    };
                    append(Span {
                        infrastructure: Infrastructure::Alignment(id),
                        begin,
                        end,
                    });
                    id = link.b.alignment_id;
                    begin = if link.b.endpoint == AlignmentEndpoint::Start {
                        0.0
                    } else {
                        1.0
                    };
                }
                if id != b.alignment_id {
                    return Err(invalid);
                }
                append(Span {
                    infrastructure: Infrastructure::Alignment(id),
                    begin,
                    end: b.position,
                });
                current = *to;
            }
        }
    }
    Ok(output)
}
fn contains_path(route: &[Span], service: &[Span]) -> bool {
    if service.is_empty() {
        return false;
    }
    (0..route.len()).any(|offset| {
        if offset + service.len() > route.len() {
            return false;
        }
        service.iter().enumerate().all(|(i, span)| {
            let r = route[offset + i];
            r.infrastructure == span.infrastructure
                && r.forward() == span.forward()
                && if i == 0 {
                    r.contains(span.begin)
                } else {
                    r.begin == span.begin
                }
                && if i + 1 == service.len() {
                    r.contains(span.end)
                } else {
                    r.end == span.end
                }
        })
    })
}
impl WorldDraft {
    pub(super) fn validate_service_route_bindings(&self) -> Result<(), WorldError> {
        let mut seen = BTreeSet::new();
        for row in &self.service_route_bindings {
            if !seen.insert(row.service_id) {
                return Err(WorldError::InvalidServiceRouteBinding);
            }
            let service = self
                .services
                .iter()
                .find(|s| s.id == row.service_id)
                .ok_or(WorldError::InvalidServiceRouteBinding)?;
            if let ServiceRouteBindingKind::Route { route_id } = row.binding {
                let route = self
                    .routes
                    .iter()
                    .find(|r| r.id == route_id && r.owner == service.owner)
                    .ok_or(WorldError::InvalidServiceRouteBinding)?;
                let route_segments = &self
                    .route_paths
                    .iter()
                    .find(|p| p.route_id == route_id)
                    .ok_or(WorldError::InvalidServiceRouteBinding)?
                    .segments;
                let service_segments: Vec<_> = self
                    .service_paths
                    .iter()
                    .find(|p| p.service_id == service.id)
                    .ok_or(WorldError::InvalidServiceRouteBinding)?
                    .legs
                    .iter()
                    .map(|l| l.segment.clone())
                    .collect();
                let mut route_spans = spans(self, route.start, &route_segments)?;
                let service_spans = spans(self, service.start, &service_segments)?;
                if !contains_path(&route_spans, &service_spans) {
                    route_spans.reverse();
                    for span in &mut route_spans {
                        std::mem::swap(&mut span.begin, &mut span.end);
                    }
                    if !contains_path(&route_spans, &service_spans) {
                        return Err(WorldError::InvalidServiceRouteBinding);
                    }
                }
            }
        }
        Ok(())
    }
}
impl World {
    pub fn set_service_route_binding(
        &mut self,
        owner: CompanyId,
        service_id: ServiceId,
        binding: ServiceRouteBindingKind,
    ) -> Result<(), WorldError> {
        if !self
            .data
            .services
            .iter()
            .any(|s| s.id == service_id && s.owner == owner)
        {
            return Err(WorldError::InvalidServiceRouteBinding);
        }
        let mut candidate = self.data.clone();
        candidate
            .service_route_bindings
            .retain(|row| row.service_id != service_id);
        candidate.service_route_bindings.push(ServiceRouteBinding {
            service_id,
            binding,
        });
        candidate.validate()?;
        self.data = candidate;
        Ok(())
    }
}
