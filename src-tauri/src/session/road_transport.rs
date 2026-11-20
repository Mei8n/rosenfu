use super::*;
use rosenfu_core::road_transport::RoadTransport;

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
        use rosenfu_core::road_transport::{Access, RoadEdge, RoadNode};
        if !confirmed_synthetic {
            return Err("実在道路ではないテストデータの確認が必要です。".into());
        }
        let mut draft = self.snapshot();
        if draft.road_transport != RoadTransport::default() {
            return Err("道路交通データが既に存在します。上書きはしません。".into());
        }
        let pts = [
            GeoPoint {
                latitude: 35.0,
                longitude: 135.0,
            },
            GeoPoint {
                latitude: 35.0,
                longitude: 135.005,
            },
            GeoPoint {
                latitude: 35.005,
                longitude: 135.005,
            },
        ];
        let mut graph = RoadTransport::default();
        for (i, point) in pts.iter().enumerate() {
            graph.nodes.push(RoadNode {
                id: format!("qa:{i}"),
                point: *point,
            });
        }
        for (a, b) in [(0, 1), (1, 0), (1, 2), (2, 1)] {
            graph.edges.push(RoadEdge {
                id: format!("qa:{a}-{b}"),
                from: format!("qa:{a}"),
                to: format!("qa:{b}"),
                source: "synthetic-qa-not-real-roads".into(),
                revision: "1".into(),
                geometry: vec![pts[a], pts[b]],
                bus_access: Access::Allowed,
                tram_access: Access::Allowed,
                level: Some(rosenfu_core::world::InfrastructureLevel::Ground),
            });
        }
        draft.road_transport = graph;
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
                .shortest_path(&before, from, to, true)
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
