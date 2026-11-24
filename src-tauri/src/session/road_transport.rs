use super::*;
use rosenfu_core::ofm_evidence::BusPattern;
use rosenfu_core::road_transport::{
    import_native_pilot, PilotBusEdit, PilotRoadMode, RoadTransport,
};

fn road_path_help(error: rosenfu_core::road_transport::RoadTransportError) -> String {
    use rosenfu_core::road_transport::RoadTransportError::*;
    match error {
        DisconnectedPath => {
            "選んだ向きでつながる経路がありません。道路の向きと起点・終点を選び直してください。"
        }
        InvalidBusCircuit => "環状線は同じ出発点に戻し、別の経由点を1つ以上指定してください。",
        MissingReturnPath => {
            "選んだ範囲の道路に復路がありません。終点か一方通行の設定を見直してください。"
        }
        UnreachableBusLeg(index) => {
            return format!(
                "第{}区間がつながりません。経由点を選び直してください。",
                index + 1
            )
        }
        MissingBusStopOccurrence => {
            "この停留所は変更後の経路上にありません。停留所を置き直してください。"
        }
        AmbiguousBusStopOccurrence => {
            "同じ道路を通る位置が重なっています。停留所を置く通過を指定してください。"
        }
        InvalidStopOrder => "進行方向に沿って、乗車位置より先に降車位置を置いてください。",
        AccessUnknown | AccessForbidden => {
            "この区間は選んだ交通手段に対応していません。対象の道路を選び直してください。"
        }
        MissingReference | StaleSnapshot => {
            "選んだ位置はこのワールドの道路データにありません。道路と位置を選び直してください。"
        }
        _ => "道路経路を確認できませんでした。道路と位置を選び直してください。",
    }
    .into()
}

#[derive(Debug, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameplayBusPathPreview {
    pub points: Vec<GeoPoint>,
    pub length_m: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_leg_index: Option<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub via_point_indices: Vec<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_path: Option<GameplayBusReturnPreview>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_unavailable_reason: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stop_occurrence_keys: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub interior_stops: Vec<rosenfu_core::road_transport::GameplayBusInteriorStopView>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub auto_stop_suggestions: Vec<rosenfu_core::ofm_evidence::BusInteriorStop>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_fallback_stop: Option<rosenfu_core::road_transport::GameplayBusInteriorStopView>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GameplayBusPreviewOptions {
    pub request_stop_targets: bool,
    pub request_auto_stop_suggestions: bool,
}

#[derive(Debug, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameplayBusReturnPreview {
    pub points: Vec<GeoPoint>,
    pub length_m: f64,
}

enum GameplayBusCandidateError {
    Message(String),
    UnreachableLeg(usize),
}
impl From<String> for GameplayBusCandidateError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}
impl From<&str> for GameplayBusCandidateError {
    fn from(message: &str) -> Self {
        Self::Message(message.into())
    }
}
impl GameplayBusCandidateError {
    fn message(self) -> String {
        match self {
            Self::Message(m) => m,
            Self::UnreachableLeg(i) => road_path_help(
                rosenfu_core::road_transport::RoadTransportError::UnreachableBusLeg(i),
            ),
        }
    }
}
fn return_in_edit_extent(path: &rosenfu_core::ofm_evidence::BusPath) -> bool {
    path.points.iter().all(|p| in_edit_extent(p[1], p[0]))
}

impl WorldEditor {
    pub fn replace_road_transport(
        &mut self,
        expected: RoadTransport,
        replacement: RoadTransport,
    ) -> Result<WorldDraft, String> {
        let before = self.snapshot();
        if before.road_transport != expected {
            return Err("道路交通の状態が変更されています。読み直してください。".into());
        }
        if replacement.nodes != expected.nodes
            || replacement.edges != expected.edges
            || replacement.evidence != expected.evidence
        {
            return Err("道路グラフの変更には検証済みの取得処理が必要です。".into());
        }
        if expected.uses_native_pilot_source() {
            fn retained<T: PartialEq>(before: &[T], proposed: &[T]) -> bool {
                let mut old = before.iter();
                proposed
                    .iter()
                    .all(|item| old.any(|previous| previous == item))
            }
            // Native create/edit resolves the complete directed path and its
            // approved stop positions itself. This endpoint remains for removal.
            if !retained(&expected.bus_stops, &replacement.bus_stops)
                || !retained(&expected.bus_routes, &replacement.bus_routes)
                || replacement.tram_sections != expected.tram_sections
            {
                return Err("承認済み地域の路線変更には専用の操作を使用してください。".into());
            }
        }
        let foreign_stops = |d: &RoadTransport| {
            d.bus_stops
                .iter()
                .filter(|s| s.owner != self.company)
                .cloned()
                .collect::<Vec<_>>()
        };
        let foreign_routes = |d: &RoadTransport| {
            d.bus_routes
                .iter()
                .filter(|s| s.owner != self.company)
                .cloned()
                .collect::<Vec<_>>()
        };
        if foreign_stops(&replacement) != foreign_stops(&expected)
            || foreign_routes(&replacement) != foreign_routes(&expected)
        {
            return Err("他社の道路交通は変更できません。".into());
        }
        let foreign_trams = |d: &RoadTransport| {
            d.tram_sections
                .iter()
                .filter(|s| {
                    before
                        .alignments
                        .iter()
                        .any(|a| a.id == s.alignment_id && a.owner != self.company)
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        if foreign_trams(&replacement) != foreign_trams(&expected) {
            return Err("他社の併用軌道は変更できません。".into());
        }
        let mut draft = before.clone();
        draft.road_transport = replacement;
        self.commit_road_transport(draft)
    }
    fn commit_road_transport(&mut self, draft: WorldDraft) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        candidate.world.replace(draft).map_err(|e| e.to_string())?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        let result = candidate.snapshot();
        self.commit_edit(candidate, false)?;
        Ok(result)
    }
    pub fn preview_authored_tram_road_guide(
        &self,
        expected_world_token: &str,
        guide: &rosenfu_core::ofm_evidence::AuthoredTramGuide,
        cached: Option<(OfmRawInput, OfmBindingInput)>,
    ) -> Result<Vec<GeoPoint>, String> {
        if self.company_setup_required || expected_world_token != self.ofm_world_token()? {
            return Err("ワールドが変更されました。道路を選び直してください。".into());
        }
        let (raw, bindings): (Vec<_>, Vec<_>) = cached.into_iter().unzip();
        let (raw, bindings) = super::ofm::inputs(raw, bindings)?;
        let route = self
            .snapshot()
            .authored_tram_road_guide(guide, raw.into_iter().zip(bindings).collect())
            .map_err(road_path_help)?
            .filter(|points| (2..=4096).contains(&points.len()))
            .ok_or(
                "この道路の接続を確認できません。短い区間を選ぶか、道路追従をオフにしてください。",
            )?;
        route
            .into_iter()
            .map(|[longitude, latitude]| {
                if !in_edit_extent(latitude, longitude) {
                    return Err("線路は編集範囲内に敷設してください。".into());
                }
                Ok(GeoPoint {
                    longitude,
                    latitude,
                })
            })
            .collect()
    }

    /// Bank exactly the confirmed game-authored street path. A world switch,
    /// tile change or changed geometry may not silently downgrade to free track.
    pub fn build_authored_street_tram(
        &mut self,
        expected_world_token: &str,
        guide: &rosenfu_core::ofm_evidence::AuthoredTramGuide,
        cached: Option<(OfmRawInput, OfmBindingInput)>,
        mut alignment: PlannedAlignment,
        roads: Vec<rosenfu_core::world::RoadSnapshot>,
    ) -> Result<WorldDraft, String> {
        use rosenfu_core::world::{InfrastructureLevel::Ground, TrackType};
        if self.company_setup_required || expected_world_token != self.ofm_world_token()? {
            return Err("ワールドが変更されました。道路を選び直してください。".into());
        }
        if alignment.track_type != TrackType::Tram
            || alignment.infrastructure.level != Ground
            || alignment.start_level.is_some_and(|level| level != Ground)
            || alignment
                .anchor_levels
                .as_ref()
                .is_some_and(|levels| levels.as_slice() != [Ground, Ground])
            || alignment.ofm_path.is_some()
            || alignment.reference.is_some()
            || alignment.extension.is_some()
            || alignment.destination.is_some()
            || alignment.junction_source.is_some()
        {
            return Err("併用軌道は地上の独立した一区間として敷設してください。".into());
        }
        let (raw, bindings): (Vec<_>, Vec<_>) = cached.into_iter().unzip();
        let (raw, bindings) = super::ofm::inputs(raw, bindings)?;
        let (world, derived) = self
            .world
            .staged_street_tram_for_construction(guide, raw.into_iter().zip(bindings).collect())
            .map_err(road_path_help)?;
        if alignment.points.len() != derived.points.len()
            || alignment.points.iter().zip(&derived.points).any(|(p, q)| {
                !in_edit_extent(p.latitude, p.longitude)
                    || (p.longitude - q[0]).abs() > 1e-10
                    || (p.latitude - q[1]).abs() > 1e-10
            })
        {
            return Err("確認した道路と敷設位置が一致しません。道路を選び直してください。".into());
        }
        alignment.anchor_indices = None;
        alignment.anchor_levels = None;
        alignment.start_level = None;
        alignment.ofm_path = Some(derived.descriptor);
        let mut candidate = self.clone();
        candidate.world = world;
        for road in roads {
            candidate
                .world
                .add_road_snapshot(road)
                .map_err(|e| e.to_string())?;
        }
        candidate.prepare_construction_candidate(self.world.snapshot(), vec![alignment], vec![])?;
        candidate.deleted_alignment = None;
        self.commit_edit(candidate, true)?;
        Ok(self.snapshot())
    }

    /// A player action over immutable OFM evidence, independent of the regional
    /// catalog selector. An optional already fetched tile is admitted and used
    /// in the same transaction, without creating or changing a rail plan.
    pub fn put_gameplay_ofm_bus(
        &mut self,
        expected_world_token: &str,
        route_id: String,
        name: String,
        path: rosenfu_core::ofm_evidence::BusPathBinding,
        cached: Vec<(OfmRawInput, OfmBindingInput)>,
    ) -> Result<WorldDraft, String> {
        let world = self
            .gameplay_ofm_bus_candidate(expected_world_token, route_id, name, path, cached)
            .map_err(GameplayBusCandidateError::message)?;
        rosenfu_storage::ensure_world_saveable(&world).map_err(|e| e.to_string())?;
        let mut candidate = self.clone();
        candidate.world = world;
        self.commit_edit(candidate, false)?;
        Ok(self.snapshot())
    }

    #[cfg(test)]
    pub fn preview_gameplay_ofm_bus(
        &self,
        expected_world_token: &str,
        route_id: String,
        name: String,
        path: rosenfu_core::ofm_evidence::BusPathBinding,
        cached: Vec<(OfmRawInput, OfmBindingInput)>,
    ) -> Result<GameplayBusPathPreview, String> {
        self.preview_gameplay_ofm_bus_details(
            expected_world_token,
            route_id,
            name,
            path,
            cached,
            GameplayBusPreviewOptions::default(),
        )
    }

    pub fn preview_gameplay_ofm_bus_details(
        &self,
        expected_world_token: &str,
        route_id: String,
        name: String,
        path: rosenfu_core::ofm_evidence::BusPathBinding,
        cached: Vec<(OfmRawInput, OfmBindingInput)>,
        options: GameplayBusPreviewOptions,
    ) -> Result<GameplayBusPathPreview, String> {
        // A partial outward preview must not permit a partial round-trip commit.
        let mut outward = path.clone();
        if path.pattern == Some(BusPattern::OpenRoundTrip) {
            outward.pattern = None;
        }
        let world = match self.gameplay_ofm_bus_candidate(
            expected_world_token,
            route_id.clone(),
            name,
            outward.clone(),
            cached,
        ) {
            Ok(world) => world,
            Err(GameplayBusCandidateError::UnreachableLeg(index)) => {
                return Ok(GameplayBusPathPreview {
                    points: vec![],
                    length_m: 0.,
                    failed_leg_index: Some(index),
                    via_point_indices: vec![],
                    return_path: None,
                    return_unavailable_reason: None,
                    stop_occurrence_keys: vec![],
                    interior_stops: vec![],
                    auto_stop_suggestions: vec![],
                    loop_fallback_stop: None,
                })
            }
            Err(other) => return Err(other.message()),
        };
        let draft = world.snapshot();
        let bus = &draft.road_transport;
        let legs = bus.bus_legs(draft, &route_id).map_err(road_path_help)?;
        let route = bus
            .bus_routes
            .iter()
            .find(|r| r.id == route_id)
            .ok_or("バス路線が見つかりません。")?;
        let edges: std::collections::BTreeMap<_, _> = bus
            .edges
            .iter()
            .map(|edge| (edge.id.as_str(), edge))
            .collect();
        let mut points = Vec::new();
        for id in &route.edges {
            let edge = edges
                .get(id.as_str())
                .ok_or("バス路線の道路が見つかりません。")?;
            points.extend(edge.geometry.iter().copied());
        }
        points.dedup();
        let mut verified_return = None;
        let (return_path, return_unavailable_reason) =
            if path.pattern == Some(BusPattern::OpenRoundTrip) {
                match draft
                    .gameplay_bus_return_path(&outward)
                    .map_err(road_path_help)?
                {
                    Some(back) if return_in_edit_extent(&back) => {
                        let preview = GameplayBusReturnPreview {
                            points: back
                                .points
                                .iter()
                                .map(|p| GeoPoint {
                                    latitude: p[1],
                                    longitude: p[0],
                                })
                                .collect(),
                            length_m: back.length_m,
                        };
                        verified_return = Some(back);
                        (Some(preview), None)
                    }
                    Some(_) => (
                        None,
                        Some("復路が編集範囲の外へ出るため確定できません。".into()),
                    ),
                    None => (
                        None,
                        Some(road_path_help(
                            rosenfu_core::road_transport::RoadTransportError::MissingReturnPath,
                        )),
                    ),
                }
            } else {
                (None, None)
            };
        let (via_point_indices, stop_occurrence_keys) =
            if outward.via.is_empty() && !options.request_stop_targets {
                (vec![], vec![])
            } else {
                let selected = rosenfu_core::ofm_evidence::reconstruct_gameplay_bus_path(
                    &draft.runtime_road_evidence,
                    &outward,
                )
                .map_err(|_| "経由点の位置を取得できません。")?
                .ok_or("経由点の位置を取得できません。")?;
                let keys = if options.request_stop_targets {
                    let keys = selected.stop_occurrence_keys();
                    if keys.len() + 1 != points.len() {
                        return Err("停留所の候補を確認できません。".into());
                    }
                    keys
                } else {
                    vec![]
                };
                (selected.via_point_indices, keys)
            };
        let interior_stops = if path.interior_stops.is_empty() {
            vec![]
        } else {
            draft
                .gameplay_bus_interior_stop_views(&route_id, &path, verified_return.as_deref())
                .map_err(road_path_help)?
        };
        let auto_stop_suggestions =
            if options.request_auto_stop_suggestions && return_unavailable_reason.is_none() {
                draft
                    .gameplay_bus_auto_stop_suggestions(&outward)
                    .map_err(road_path_help)?
            } else {
                vec![]
            };
        Ok(GameplayBusPathPreview {
            points,
            length_m: legs.iter().map(|leg| leg.length_m).sum(),
            failed_leg_index: None,
            via_point_indices,
            return_path,
            return_unavailable_reason,
            stop_occurrence_keys,
            interior_stops,
            auto_stop_suggestions,
            loop_fallback_stop: draft
                .gameplay_bus_loop_fallback_view(&route_id, &path)
                .map_err(road_path_help)?,
        })
    }

    fn gameplay_ofm_bus_candidate(
        &self,
        expected_world_token: &str,
        route_id: String,
        name: String,
        path: rosenfu_core::ofm_evidence::BusPathBinding,
        cached: Vec<(OfmRawInput, OfmBindingInput)>,
    ) -> Result<World, GameplayBusCandidateError> {
        if self.company_setup_required || expected_world_token != self.ofm_world_token()? {
            return Err("ワールドが変更されました。道路を選び直してください。".into());
        }
        let (raw, bindings): (Vec<_>, Vec<_>) = cached.into_iter().unzip();
        let (raw, bindings) = super::ofm::inputs(raw, bindings)?;
        let cached = raw.into_iter().zip(bindings).collect();
        let world = self
            .world
            .candidate_with_gameplay_ofm_bus(self.company, route_id.clone(), name, path, cached)
            .map_err(|err| match err {
                rosenfu_core::road_transport::RoadTransportError::UnreachableBusLeg(i) => {
                    GameplayBusCandidateError::UnreachableLeg(i)
                }
                other => GameplayBusCandidateError::Message(road_path_help(other)),
            })?;
        if world
            .snapshot()
            .road_transport
            .edges
            .iter()
            .flat_map(|e| &e.geometry)
            .any(|p| !in_edit_extent(p.latitude, p.longitude))
        {
            return Err("バス路線は編集範囲内に作成してください。".into());
        }
        if let Some(path) = world
            .snapshot()
            .ofm_gameplay_bus_routes
            .get(&route_id)
            .filter(|p| p.pattern == Some(BusPattern::OpenRoundTrip))
        {
            if world
                .snapshot()
                .gameplay_bus_return_path(path)
                .map_err(road_path_help)?
                .is_some_and(|back| !return_in_edit_extent(&back))
            {
                return Err("復路が編集範囲の外へ出るため確定できません。".into());
            }
        }
        Ok(world)
    }

    pub fn remove_gameplay_ofm_bus(
        &mut self,
        expected_world_token: &str,
        route_id: &str,
    ) -> Result<WorldDraft, String> {
        if self.company_setup_required || expected_world_token != self.ofm_world_token()? {
            return Err("ワールドが変更されました。対象のバス路線を開き直してください。".into());
        }
        let draft = self
            .snapshot()
            .remove_ofm_gameplay_bus(self.company, route_id)
            .map_err(road_path_help)?;
        self.commit_road_transport(draft)
    }
    /// Explicit opt-in QA fixture: it is never attributed to a real map provider.
    pub fn enable_road_qa_fixture(
        &mut self,
        confirmed_synthetic: bool,
    ) -> Result<WorldDraft, String> {
        if !confirmed_synthetic {
            return Err("実在道路ではないテストデータの確認が必要です。".into());
        }
        self.install_road_graph(RoadTransport::synthetic_qa_graph())
    }

    pub fn install_native_road_pilot(&mut self, asset_id: &str) -> Result<WorldDraft, String> {
        let graph = import_native_pilot(asset_id)
            .map_err(|error| format!("承認済みの道路交通データを取得できません: {error:?}"))?;
        self.install_road_graph(graph)
    }

    fn install_road_graph(&mut self, graph: RoadTransport) -> Result<WorldDraft, String> {
        let mut draft = self.snapshot();
        if draft.road_transport != RoadTransport::default() {
            return Err("道路交通データが既に存在します。上書きはしません。".into());
        }
        draft.road_transport = graph;
        self.commit_road_transport(draft)
    }
}

impl WorldEditor {
    pub fn edit_native_pilot_bus(&mut self, edit: PilotBusEdit) -> Result<WorldDraft, String> {
        use rosenfu_core::road_transport::{
            BusRoute, BusStop, BusStopOccurrence, MAX_BUS_WAYPOINTS,
        };
        let via = edit.via.as_deref().unwrap_or_default();
        if via.len() > MAX_BUS_WAYPOINTS - 2 {
            return Err(format!(
                "経由点は{}件まで指定できます。",
                MAX_BUS_WAYPOINTS - 2
            ));
        }
        let before = self.snapshot();
        let mut draft = before.clone();
        let graph = &mut draft.road_transport;
        let existing = graph.bus_routes.iter().position(|r| r.id == edit.route_id);
        match (existing, edit.expected_route.as_ref()) {
            (None, None) => {}
            (Some(i), Some(expected))
                if graph.bus_routes[i] == *expected
                    && expected.owner == self.company
                    && (edit.via.is_some() || expected.stops.len() == 2) => {}
            _ => return Err("指定した自社バス系統が変更されています。読み直してください。".into()),
        }
        let points: Vec<_> = std::iter::once((&edit.from, true))
            .chain(via.iter().map(|v| (&v.point, v.stop)))
            .chain(std::iter::once((&edit.to, true)))
            .collect();
        let plan = graph
            .native_bus_path_via(&before, &points)
            .map_err(road_path_help)?;
        let last = points.len() - 1;
        let mut stops = vec![];
        for (i, ((point, stop), edge_index)) in points
            .into_iter()
            .zip(plan.waypoint_edge_indices)
            .enumerate()
        {
            if !stop {
                continue;
            }
            let label = if i == 0 {
                "起点".into()
            } else if i == last {
                "終点".into()
            } else {
                format!("経由停留所 {i}")
            };

            let stop_id = if let Some(stop) = graph.bus_stops.iter().find(|s| {
                s.owner == self.company
                    && s.edge_id == point.edge_id
                    && s.position == point.position
                    && s.location_kind == point.location_kind
            }) {
                stop.id.clone()
            } else {
                // Reuse the same stop on re-edit. A revision suffix avoids
                // reassigning an old stop still used by another local service.
                let occupied: std::collections::BTreeSet<_> =
                    graph.bus_stops.iter().map(|s| s.id.as_str()).collect();
                let stop_id = (0..=graph.bus_stops.len())
                    .map(|i| format!("{}:{label}:{i}", edit.route_id))
                    .find(|id| !occupied.contains(id.as_str()))
                    .ok_or("停留所IDを発行できません。")?;
                graph.bus_stops.push(BusStop {
                    id: stop_id.clone(),
                    owner: self.company,
                    name: label,
                    edge_id: point.edge_id.clone(),
                    position: point.position,
                    location_kind: point.location_kind,
                });
                stop_id
            };
            stops.push(BusStopOccurrence {
                stop_id,
                edge_index,
            });
        }
        let route = BusRoute {
            id: edit.route_id,
            owner: self.company,
            name: edit.name,
            edges: plan.edges,
            stops,
        };
        if let Some(i) = existing {
            graph.bus_routes[i] = route;
        } else {
            graph.bus_routes.push(route);
        }
        self.commit_road_transport(draft)
    }
}

#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RoadTramEdit {
    Build {
        from: String,
        to: String,
        name: String,
        shared: bool,
    },
    ChangePlacement {
        alignment_id: AlignmentId,
        from: String,
        to: String,
        shared: bool,
    },
    Remove {
        alignment_id: AlignmentId,
    },
}

impl WorldEditor {
    /// Validate the road path on the authoritative world and commit rail plus its
    /// road association as one history entry. No client-supplied rail geometry.
    pub fn edit_road_tram(&mut self, edit: RoadTramEdit) -> Result<WorldDraft, String> {
        use rosenfu_core::road_transport::{TramPlacement, TramSection};
        use rosenfu_core::world::TrackType;
        let before = self.snapshot();
        let mut candidate = self.clone();
        let route = |from: &str, to: &str| -> Result<(Vec<String>, Vec<GeoPoint>), String> {
            let ids = before
                .road_transport
                .native_shortest_path(&before, from, to, PilotRoadMode::Tram)
                .map_err(road_path_help)?;
            let mut points: Vec<_> = ids
                .iter()
                .flat_map(|id| {
                    before
                        .road_transport
                        .edges
                        .iter()
                        .find(|e| &e.id == id)
                        .unwrap()
                        .geometry
                        .iter()
                        .copied()
                })
                .collect();
            points.dedup();
            if points.len() > 4096 {
                return Err("区間が長すぎます。分けて敷設してください。".into());
            }
            Ok((ids, points))
        };
        let section = |id: AlignmentId, points, shared, ids| TramSection {
            alignment_id: id,
            alignment_geometry: points,
            from_position: 0.0,
            to_position: 1.0,
            placement: if shared {
                TramPlacement::SharedRoad { edges: ids }
            } else {
                TramPlacement::Dedicated
            },
        };
        let own_road_tram = |id| {
            before
                .alignments
                .iter()
                .find(|a| {
                    a.id == id
                        && a.owner == self.company
                        && a.track_type == TrackType::Tram
                        && before
                            .road_transport
                            .tram_sections
                            .iter()
                            .any(|t| t.alignment_id == id)
                })
                .ok_or("自社の道路交通から着工した軌道ではありません。")
        };
        match edit {
            RoadTramEdit::Build {
                from,
                to,
                name,
                shared,
            } => {
                if name.trim().is_empty() || name.len() > 200 {
                    return Err("軌道の名前を入力してください。".into());
                }
                let (ids, points) = route(&from, &to)?;
                if before
                    .alignments
                    .iter()
                    .any(|a| a.points == points || a.points.iter().rev().eq(points.iter()))
                {
                    return Err(
                        "同じ場所に線路があります。既存軌道の関連を変更してください。".into(),
                    );
                }

                candidate.build_construction_plan(
                    vec![PlannedAlignment {
                        ofm_path: None,
                        anchor_levels: None,
                        junction_source: None,
                        start_level: None,
                        infrastructure: Default::default(),
                        anchor_indices: None,
                        name,
                        points: points.clone(),
                        track_type: TrackType::Tram,
                        design_speed_kmh: None,
                        stations: vec![],
                        reference: None,
                        extension: None,
                        destination: None,
                    }],
                    vec![],
                )?;
                let mut draft = candidate.snapshot();
                let created = draft
                    .alignments
                    .iter()
                    .find(|a| !before.alignments.iter().any(|b| b.id == a.id))
                    .ok_or("着工した軌道がありません。")?;
                draft
                    .road_transport
                    .tram_sections
                    .push(section(created.id, points, shared, ids));
                candidate.world.replace(draft).map_err(|e| e.to_string())?;
            }
            RoadTramEdit::ChangePlacement {
                alignment_id,
                from,
                to,
                shared,
            } => {
                let rail = own_road_tram(alignment_id)?;
                let (ids, points) = if shared {
                    route(&from, &to)?
                } else {
                    (vec![], rail.points.clone())
                };
                if rail.points != points {
                    return Err("選択した道路と既設軌道の形状が一致しません。".into());
                }
                let mut draft = before.clone();
                draft
                    .road_transport
                    .tram_sections
                    .retain(|s| s.alignment_id != alignment_id);
                draft
                    .road_transport
                    .tram_sections
                    .push(section(alignment_id, points, shared, ids));
                candidate.world.replace(draft).map_err(|e| e.to_string())?;
            }
            RoadTramEdit::Remove { alignment_id } => {
                own_road_tram(alignment_id)?;
                let mut draft = before.clone();
                draft
                    .road_transport
                    .tram_sections
                    .retain(|s| s.alignment_id != alignment_id);
                candidate.world.replace(draft).map_err(|e| e.to_string())?;
                candidate.delete_alignment(alignment_id)?;
            }
        }
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        candidate.deleted_alignment = None;
        let after = candidate.snapshot();
        self.commit_edit(candidate, false)?;
        Ok(after)
    }
}

#[cfg(all(test, feature = "dev-road-lab"))]
mod tests {
    use super::*;
    use rosenfu_core::operation::{Timetable, Vehicle};
    use rosenfu_core::road_transport::{
        BusRoute, BusStopLocationKind, PilotBusWaypoint, StopPosition,
    };

    #[test]
    fn lab_route_first_commits_guarded_path_and_stops_then_restores_operation() {
        let mut editor = WorldEditor::new();
        editor.create_new_world("Route test".into()).unwrap();
        editor
            .install_native_road_pilot("shimada-otsu-chubu-261120-lab-bus")
            .unwrap();
        let issue = |n, position| StopPosition {
            edge_id: format!("osm:392108440:{n}:forward"),
            position,
            location_kind: BusStopLocationKind::PlayerCreated,
        };
        let via = || {
            vec![
                PilotBusWaypoint {
                    point: issue(2, 0.5),
                    stop: false,
                },
                PilotBusWaypoint {
                    point: issue(3, 0.4),
                    stop: true,
                },
            ]
        };
        let request = |expected_route: Option<BusRoute>, via| PilotBusEdit {
            route_id: "owned-bus".into(),
            name: "via stops".into(),
            expected_route,
            from: issue(1, 0.3),
            to: issue(4, 0.7),
            via,
        };
        let mut backwards = request(None, Some(via()));
        backwards.via.as_mut().unwrap()[0].point = issue(1, 0.2);
        let untouched = editor.snapshot();
        let rev = editor.world_edit_history().revision;
        assert!(editor.edit_native_pilot_bus(backwards).is_err());
        assert_eq!(editor.snapshot(), untouched);
        assert_eq!(editor.world_edit_history().revision, rev);

        let made = editor
            .edit_native_pilot_bus(request(None, Some(via())))
            .unwrap();
        let route = made.road_transport.bus_routes[0].clone();
        assert_eq!(
            route.edges,
            (1..=4).map(|n| issue(n, 0.5).edge_id).collect::<Vec<_>>()
        );
        assert_eq!(
            route.stops.iter().map(|s| s.edge_index).collect::<Vec<_>>(),
            [0, 2, 3]
        );
        assert_eq!(made.road_transport.bus_stops.len(), 3);
        let legacy_edit = request(Some(route.clone()), None);
        assert!(editor.edit_native_pilot_bus(legacy_edit).is_err());
        assert_eq!(editor.snapshot(), made);
        let mut stale = route.clone();
        stale.name = "Not current".into();
        let saved_revision = editor.world_edit_history().revision;
        assert!(editor
            .edit_native_pilot_bus(request(Some(stale), Some(via())))
            .is_err());
        assert_eq!(editor.world_edit_history().revision, saved_revision);
        assert_eq!(editor.snapshot(), made);

        editor.undo_world_edit(editor.snapshot()).unwrap();
        assert_eq!(editor.snapshot(), untouched);
        editor.redo_world_edit(editor.snapshot()).unwrap();
        assert_eq!(editor.snapshot(), made);
        let operation_token = editor.operation_snapshot().timetable_edit_token.clone();
        editor
            .configure_bus_operation(
                route.id.clone(),
                Timetable {
                    first_departure_ms: 0,
                    last_departure_ms: 0,
                    headway_ms: 60_000,
                },
                Vehicle {
                    max_speed_kmh: 70.0,
                    acceleration_mps2: 1.0,
                    braking_mps2: 1.2,
                    length_m: 10.0,
                },
                30.0,
                3_000,
                None,
                &operation_token,
            )
            .unwrap();
        let saved = editor.world.operation_save().unwrap();
        assert_eq!(saved.runtime.services[0].legs.len(), 2);
        assert_eq!(
            saved.runtime.services[0].legs[0].to,
            saved.runtime.services[0].legs[1].from
        );
        let folder = tempfile::tempdir().unwrap();
        let file = folder.path().join("road-via.json");
        editor.save(&file).unwrap();
        editor.create_new_world("Other".into()).unwrap();
        editor.load(&file).unwrap();
        assert_eq!(editor.snapshot().road_transport, made.road_transport);
        assert_eq!(editor.world.operation_save().unwrap(), saved);
    }
}
