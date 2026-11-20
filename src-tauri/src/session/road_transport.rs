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
        for section in replacement
            .tram_sections
            .iter()
            .chain(expected.tram_sections.iter())
        {
            if before
                .alignments
                .iter()
                .any(|a| a.id == section.alignment_id && a.owner != self.company)
                && replacement.tram_sections != expected.tram_sections
            {
                return Err("他社の併用軌道は変更できません。".into());
            }
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
            });
        }
        draft.road_transport = graph;
        self.commit_road_transport(draft)
    }
}
