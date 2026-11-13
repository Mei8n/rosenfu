//! Read-only service drafts. Route membership never implies stop/pass policy.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServiceDraftStation {
    pub station_id: StationId,
    /// Position in this draft, also the index used by OperatingService.stops.
    pub occurrence: u32,
    pub route_occurrence: u32,
    pub route_member: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServiceRoutePreview {
    pub route_id: RouteId,
    pub start: StationId,
    pub tracks: Vec<TrackId>,
    pub stations: Vec<ServiceDraftStation>,
    /// No stop policy is chosen by this API, including for non-member stations.
    pub requires_stop_selection: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServicePlanningError {
    MissingRoute,
    InvalidOccurrence,
    EmptySection,
    AlignmentUnsupported,
    InvalidPath,
}
impl std::fmt::Display for ServicePlanningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::MissingRoute => "路線が見つかりません。",
            Self::InvalidOccurrence => "路線上の駅出現位置が無効です。",
            Self::EmptySection => "異なる駅出現位置を起点・終点に指定してください。",
            Self::AlignmentUnsupported => {
                "選択区間に建設線形が含まれます。建設線形からの系統作成はまだ未対応です。"
            }
            Self::InvalidPath => "路線経路を解決できません。",
        })
    }
}
impl std::error::Error for ServicePlanningError {}
impl World {
    pub fn preview_service_from_route(
        &self,
        route_id: RouteId,
        from_occurrence: u32,
        to_occurrence: u32,
    ) -> Result<ServiceRoutePreview, ServicePlanningError> {
        let route = self
            .data
            .routes
            .iter()
            .find(|r| r.id == route_id)
            .ok_or(ServicePlanningError::MissingRoute)?;
        let stations = self
            .route_path_stations(route_id)
            .map_err(|_| ServicePlanningError::InvalidPath)?;
        let from = from_occurrence as usize;
        let to = to_occurrence as usize;
        if from >= stations.len() || to >= stations.len() {
            return Err(ServicePlanningError::InvalidOccurrence);
        }
        if from == to {
            return Err(ServicePlanningError::EmptySection);
        }
        let sidecar = self
            .data
            .route_paths
            .iter()
            .find(|p| p.route_id == route_id);
        let edges: Vec<Option<TrackId>> = if let Some(path) = sidecar {
            let mut edges = vec![];
            let mut current = route.start;
            for segment in &path.segments {
                let span =
                    route_paths::resolve_path(&self.data, current, std::slice::from_ref(segment))
                        .map_err(|_| ServicePlanningError::InvalidPath)?;
                let track = match segment {
                    RouteSegment::Track { track_id } => Some(*track_id),
                    _ => None,
                };
                edges.extend(std::iter::repeat_n(track, span.len() - 1));
                current = *span.last().ok_or(ServicePlanningError::InvalidPath)?;
            }
            edges
        } else {
            route.tracks.iter().copied().map(Some).collect()
        };
        let mut tracks = edges[from.min(to)..from.max(to)]
            .iter()
            .copied()
            .collect::<Option<Vec<_>>>()
            .ok_or(ServicePlanningError::AlignmentUnsupported)?;
        let positions: Vec<usize> = if from < to {
            (from..=to).collect()
        } else {
            tracks.reverse();
            (to..=from).rev().collect()
        };
        Ok(ServiceRoutePreview {
            route_id,
            start: stations[from],
            tracks,
            stations: positions
                .into_iter()
                .enumerate()
                .map(|(index, position)| ServiceDraftStation {
                    station_id: stations[position],
                    occurrence: index as u32,
                    route_occurrence: position as u32,
                    route_member: sidecar
                        .is_none_or(|p| p.member_stations.contains(&stations[position])),
                })
                .collect(),
            requires_stop_selection: true,
        })
    }
}
