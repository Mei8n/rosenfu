use super::*;
use rosenfu_core::road_transport::{
    import_native_pilot, PilotBusEdit, PilotRoadMode, RoadTransport,
};

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
        self.commit_edit(candidate, false);
        Ok(result)
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
        use rosenfu_core::road_transport::{BusRoute, BusStop, BusStopOccurrence};
        let before = self.snapshot();
        let mut draft = before.clone();
        let graph = &mut draft.road_transport;
        let existing = graph.bus_routes.iter().position(|r| r.id == edit.route_id);
        match (existing, edit.expected_route.as_ref()) {
            (None, None) => {}
            (Some(i), Some(expected))
                if graph.bus_routes[i] == *expected
                    && expected.owner == self.company
                    && expected.stops.len() == 2 => {}
            _ => return Err("指定した自社バス系統が変更されています。読み直してください。".into()),
        }
        let edges = graph
            .native_bus_path(&before, &edit.from, &edit.to)
            .map_err(|error| {
                format!("指定した停車位置をつなぐ通行可能な経路がありません: {error:?}")
            })?;
        let last = edges.len() - 1;
        let mut stops = vec![];
        for (point, label, edge_index) in [(edit.from, "起点", 0), (edit.to, "終点", last)] {
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
                    name: label.into(),
                    edge_id: point.edge_id,
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
            edges,
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
                .map_err(|e| format!("検証済みの道路経路がありません: {e:?}"))?;
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
        self.commit_edit(candidate, false);
        Ok(after)
    }
}
