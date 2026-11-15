//! One assessment path is used by preview and commit. Costs are intentionally absent.
use super::*;
use rosenfu_core::world::{
    assess_alignment_environment, Alignment, AutomaticCrossing, EnvironmentAssessment,
    EnvironmentSnapshot,
};
#[derive(serde::Serialize)]
pub struct ConstructionEnvironmentPreview {
    pub assessments: Vec<EnvironmentAssessment>,
    pub crossings: Vec<AutomaticCrossing>,
    pub buildable: bool,
}
fn assessment(a: &Alignment, data: &EnvironmentSnapshot) -> EnvironmentAssessment {
    // This baseline has no vertical profile field. Ramp integration must supply false
    // for non-flat profiles and test the actual height before allowing ground crossings.
    assess_alignment_environment(a, true, data)
}
fn merge(
    old: Option<&EnvironmentSnapshot>,
    new: EnvironmentSnapshot,
) -> Result<EnvironmentSnapshot, String> {
    new.validate().map_err(|e| e.to_string())?;
    let mut result = old.cloned().unwrap_or_default();
    result.revision = new.revision;
    for b in new.road_coverage {
        if !result.road_coverage.contains(&b) {
            result.road_coverage.push(b);
        }
    }
    for b in new.water_coverage {
        if !result.water_coverage.contains(&b) {
            result.water_coverage.push(b);
        }
    }
    for road in new.roads {
        if let Some(old) = result.roads.iter_mut().find(|r| r.id == road.id) {
            *old = road;
        } else {
            result.roads.push(road);
        }
    }
    for polygon in new.water {
        if !result.water.contains(&polygon) {
            result.water.push(polygon);
        }
    }
    result.validate().map_err(|e| e.to_string())?;
    Ok(result)
}
impl WorldEditor {
    pub(super) fn check_environment_changes(&self, before: &WorldDraft) -> Result<(), String> {
        if let Some(data) = &self.world.snapshot().environment {
            for a in &self.world.snapshot().alignments {
                if !before.alignments.contains(a) && !assessment(a, data).buildable() {
                    return Err(
                        "道路・水域の取得範囲外、または水域建設制約に違反する変更です。".into(),
                    );
                }
            }
            let changed: Vec<_> = self
                .world
                .snapshot()
                .alignments
                .iter()
                .filter(|a| !before.alignments.contains(a))
                .map(|a| a.id)
                .collect();
            rosenfu_core::world::validate_connected_water(self.world.snapshot(), &changed, data)
                .map_err(|_| "接続線路を含む水面横断の制約に違反しています。".to_string())?;
        }
        Ok(())
    }

    fn environment_candidate(
        &self,
        alignments: Vec<PlannedAlignment>,
        stations: Vec<PlannedBuiltStation>,
        data: EnvironmentSnapshot,
    ) -> Result<(Self, ConstructionEnvironmentPreview), String> {
        let data = merge(self.world.snapshot().environment.as_ref(), data)?;
        let before = self.snapshot();
        let mut candidate = self.clone();
        // Extensions validate while building; install the merged coverage first.
        let mut draft = candidate.snapshot();
        draft.environment = Some(data.clone());
        candidate.world.replace(draft).map_err(|e| e.to_string())?;
        candidate.build_construction_plan(alignments, stations)?;
        let assessments: Vec<_> = candidate
            .world
            .snapshot()
            .alignments
            .iter()
            .filter(|a| !before.alignments.contains(a))
            .map(|a| assessment(a, &data))
            .collect();
        let changed: Vec<_> = candidate
            .world
            .snapshot()
            .alignments
            .iter()
            .filter(|a| !before.alignments.contains(a))
            .map(|a| a.id)
            .collect();
        let buildable = assessments.iter().all(EnvironmentAssessment::buildable)
            && rosenfu_core::world::validate_connected_water(
                candidate.world.snapshot(),
                &changed,
                &data,
            )
            .is_ok();
        let existing = self.automatic_crossings();
        let mut crossings: Vec<AutomaticCrossing> = vec![];
        for a in &assessments {
            for c in &a.crossings {
                if !existing
                    .iter()
                    .chain(crossings.iter())
                    .any(|old| rosenfu_core::world::same_crossing_point(old.point, c.point))
                {
                    crossings.push(c.clone());
                }
            }
        }
        let mut draft = candidate.snapshot();
        draft.environment = Some(data);
        candidate.world.replace(draft).map_err(|e| e.to_string())?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        Ok((
            candidate,
            ConstructionEnvironmentPreview {
                assessments,
                crossings,
                buildable,
            },
        ))
    }
    pub fn preview_environment(
        &self,
        alignments: Vec<PlannedAlignment>,
        stations: Vec<PlannedBuiltStation>,
        data: EnvironmentSnapshot,
    ) -> Result<ConstructionEnvironmentPreview, String> {
        Ok(self.environment_candidate(alignments, stations, data)?.1)
    }
    pub fn build_plan_with_environment(
        &mut self,
        alignments: Vec<PlannedAlignment>,
        stations: Vec<PlannedBuiltStation>,
        data: EnvironmentSnapshot,
    ) -> Result<WorldDraft, String> {
        let (candidate, preview) = self.environment_candidate(alignments, stations, data)?;
        if !preview.buildable {
            return Err("道路・水域が未取得、交差高さが不明、または水域建設制約に違反しています。計画は着工していません。".into());
        }
        *self = candidate;
        Ok(self.snapshot())
    }
    pub fn automatic_crossings(&self) -> Vec<AutomaticCrossing> {
        let Some(data) = &self.world.snapshot().environment else {
            return vec![];
        };
        let mut crossings: Vec<AutomaticCrossing> = vec![];
        for a in &self.world.snapshot().alignments {
            for c in assessment(a, data).crossings {
                if !crossings
                    .iter()
                    .any(|old| rosenfu_core::world::same_crossing_point(old.point, c.point))
                {
                    crossings.push(c);
                }
            }
        }
        crossings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rosenfu_core::world::{
        CoverageBounds, GeoPoint, InfrastructureLevel, RoadWay, SectionInfrastructure, TrackType,
        WaterPolygon,
    };
    fn p(x: f64, y: f64) -> GeoPoint {
        GeoPoint {
            longitude: 139.0 + x,
            latitude: 35.0 + y,
        }
    }
    fn request() -> PlannedAlignment {
        PlannedAlignment {
            infrastructure: SectionInfrastructure::default(),
            anchor_indices: None,
            name: "環境試験".into(),
            points: vec![p(-0.01, 0.0), p(0.01, 0.0)],
            track_type: TrackType::Normal,
            design_speed_kmh: None,
            stations: vec![],
            reference: None,
            extension: None,
            destination: None,
        }
    }
    fn data() -> EnvironmentSnapshot {
        EnvironmentSnapshot {
            revision: "fixture".into(),
            road_coverage: vec![CoverageBounds {
                west: 138.9,
                east: 139.1,
                south: 34.9,
                north: 35.1,
            }],
            water_coverage: vec![CoverageBounds {
                west: 138.9,
                east: 139.1,
                south: 34.9,
                north: 35.1,
            }],
            roads: vec![RoadWay {
                id: "1".into(),
                points: vec![p(0.0, -0.01), p(0.0, 0.01)],
                layer: None,
                level: None,
                bridge: None,
                tunnel: None,
            }],
            water: vec![],
        }
    }
    #[test]
    fn preview_commit_save_restore_delete_undo_are_consistent() {
        let mut e = WorldEditor::new();
        let before = e.snapshot();
        let preview = e
            .preview_environment(vec![request()], vec![], data())
            .unwrap();
        assert!(preview.buildable);
        assert_eq!(preview.crossings.len(), 1);
        assert_eq!(e.snapshot(), before);
        let built = e
            .build_plan_with_environment(vec![request()], vec![], data())
            .unwrap();
        assert_eq!(e.automatic_crossings(), preview.crossings);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("environment.rosenfu.json");
        rosenfu_storage::save_world(&path, &e.world).unwrap();
        e.world = rosenfu_storage::load_world(&path).unwrap();
        assert_eq!(e.automatic_crossings(), preview.crossings);
        let id = built.alignments.last().unwrap().id;
        e.delete_alignment(id).unwrap();
        assert!(e.automatic_crossings().is_empty());
        e.undo_alignment_deletion().unwrap();
        assert_eq!(e.automatic_crossings(), preview.crossings);
    }
    #[test]
    fn missing_coverage_and_water_reject_atomically() {
        let mut e = WorldEditor::new();
        let before = e.snapshot();
        assert!(e
            .build_plan_with_environment(vec![request()], vec![], EnvironmentSnapshot::default())
            .is_err());
        assert_eq!(e.snapshot(), before);
        let mut d = data();
        d.water.push(WaterPolygon {
            rings: vec![vec![
                p(-0.005, -0.005),
                p(0.005, -0.005),
                p(0.005, 0.005),
                p(-0.005, 0.005),
                p(-0.005, -0.005),
            ]],
        });
        assert!(e
            .build_plan_with_environment(vec![request()], vec![], d.clone())
            .is_err());
        assert_eq!(e.snapshot(), before);
        let mut r = request();
        r.infrastructure.level = InfrastructureLevel::Elevated;
        assert!(e.build_plan_with_environment(vec![r], vec![], d).is_ok());
        assert!(e.automatic_crossings().is_empty());
    }
    #[test]
    fn extension_uses_new_coverage_without_mutating_preview_source() {
        let mut e = WorldEditor::new();
        let mut initial = data();
        initial.road_coverage[0].east = 139.01;
        initial.water_coverage[0].east = 139.01;
        let built = e
            .build_plan_with_environment(vec![request()], vec![], initial)
            .unwrap();
        let before = e.snapshot();
        let mut r = request();
        r.points = vec![p(0.01, 0.0), p(0.02, 0.0)];
        r.extension = Some(PlannedAlignmentExtension {
            alignment_id: built.alignments.last().unwrap().id,
            endpoint: rosenfu_core::world::AlignmentEndpoint::End,
        });
        assert!(
            e.preview_environment(vec![r], vec![], data())
                .unwrap()
                .buildable
        );
        assert_eq!(e.snapshot(), before);
        let mut r = request();
        r.points = vec![p(0.01, 0.0), p(0.02, 0.0)];
        r.extension = Some(PlannedAlignmentExtension {
            alignment_id: built.alignments.last().unwrap().id,
            endpoint: rosenfu_core::world::AlignmentEndpoint::End,
        });
        assert!(e
            .build_plan_with_environment(vec![r], vec![], data())
            .is_ok());
    }
    #[test]
    fn extension_does_not_report_existing_crossings_as_new() {
        let mut e = WorldEditor::new();
        let built = e
            .build_plan_with_environment(vec![request()], vec![], data())
            .unwrap();
        let id = built.alignments.last().unwrap().id;
        let mut r = request();
        r.points = vec![p(0.01, 0.0), p(0.02, 0.0)];
        r.extension = Some(PlannedAlignmentExtension {
            alignment_id: id,
            endpoint: rosenfu_core::world::AlignmentEndpoint::End,
        });
        let preview = e.preview_environment(vec![r], vec![], data()).unwrap();
        assert!(preview.buildable);
        assert!(preview.crossings.is_empty());
    }
}
