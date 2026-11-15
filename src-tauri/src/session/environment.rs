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
    // This model cannot deserialize vertical profiles. Its only height source is
    // the per-anchor infrastructure; an adapter for profile-enabled models must
    // derive each interval from the physical height/flatness APIs.
    let sections = a
        .anchor_indices
        .as_ref()
        .map_or(1, |indices| indices.len() - 1);
    let ground: Vec<_> = (0..sections)
        .map(|i| {
            a.section_infrastructure(i)
                .is_some_and(|s| s.level == rosenfu_core::world::InfrastructureLevel::Ground)
        })
        .collect();
    assess_alignment_environment(a, &ground, data)
}
fn merge(
    old: Option<&EnvironmentSnapshot>,
    new: EnvironmentSnapshot,
) -> Result<EnvironmentSnapshot, String> {
    new.validate().map_err(|e| e.to_string())?;
    let mut result = old.cloned().unwrap_or_default();
    result.revision = new.revision;
    for b in &new.road_coverage {
        if !result.road_coverage.contains(b) {
            result.road_coverage.push(*b);
        }
    }
    for b in &new.water_coverage {
        if !result.water_coverage.contains(b) {
            result.water_coverage.push(*b);
        }
    }
    // Scope versions spatially: an empty successful refresh removes features only
    // inside that provider's refreshed area. Geometry outside it remains usable.
    for road in &mut result.roads {
        road.coverage = Some(subtract_coverage(
            road.coverage
                .as_deref()
                .unwrap_or(old.map_or(&[], |d| &d.road_coverage)),
            &new.road_coverage,
        )?);
    }
    result
        .roads
        .retain(|r| r.coverage.as_ref().is_some_and(|b| !b.is_empty()));
    for polygon in &mut result.water {
        polygon.coverage = Some(subtract_coverage(
            polygon
                .coverage
                .as_deref()
                .unwrap_or(old.map_or(&[], |d| &d.water_coverage)),
            &new.water_coverage,
        )?);
    }
    result
        .water
        .retain(|p| p.coverage.as_ref().is_some_and(|b| !b.is_empty()));
    for mut road in new.roads {
        if road.coverage.is_none() {
            road.coverage = Some(new.road_coverage.clone());
        }
        result.roads.push(road);
    }
    for mut polygon in new.water {
        if polygon.coverage.is_none() {
            polygon.coverage = Some(new.water_coverage.clone());
        }
        result.water.push(polygon);
    }
    result.validate().map_err(|e| e.to_string())?;
    Ok(result)
}
// Rectangular subtraction preserves all outside coverage without geometric clipping.
fn subtract_coverage(
    old: &[rosenfu_core::world::CoverageBounds],
    fresh: &[rosenfu_core::world::CoverageBounds],
) -> Result<Vec<rosenfu_core::world::CoverageBounds>, String> {
    use rosenfu_core::world::CoverageBounds as B;
    let mut remaining = old.to_vec();
    for n in fresh {
        remaining = remaining
            .into_iter()
            .flat_map(|b| {
                let (w, e, s, north) = (
                    b.west.max(n.west),
                    b.east.min(n.east),
                    b.south.max(n.south),
                    b.north.min(n.north),
                );
                if w >= e || s >= north {
                    return vec![b];
                }
                [
                    B { east: w, ..b },
                    B { west: e, ..b },
                    B {
                        west: w,
                        east: e,
                        north: s,
                        ..b
                    },
                    B {
                        west: w,
                        east: e,
                        south: north,
                        ..b
                    },
                ]
                .into_iter()
                .filter(|r| r.west < r.east && r.south < r.north)
                .collect()
            })
            .collect();
        if remaining.len() > 128 {
            return Err("環境更新の分割上限を超えています。".into());
        }
    }
    Ok(remaining)
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
                coverage: None,
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
    fn empty_refresh_removes_only_its_provider_and_region() {
        let mut old = data();
        old.roads[0].points = vec![p(0.0, -0.08), p(0.0, 0.08)];
        old.water.push(WaterPolygon {
            coverage: None,
            rings: vec![vec![
                p(-0.08, -0.08),
                p(0.08, -0.08),
                p(0.08, 0.08),
                p(-0.08, 0.08),
                p(-0.08, -0.08),
            ]],
        });
        let bounds = CoverageBounds {
            west: 138.98,
            east: 139.02,
            south: 34.98,
            north: 35.02,
        };
        let refreshed = merge(
            Some(&old),
            EnvironmentSnapshot {
                revision: "empty".into(),
                road_coverage: vec![bounds],
                water_coverage: vec![bounds],
                ..Default::default()
            },
        )
        .unwrap();
        let inspect = |d: &EnvironmentSnapshot, y| {
            rosenfu_core::world::assess_environment(
                &[p(-0.01, y), p(0.01, y)],
                InfrastructureLevel::Ground,
                true,
                d,
            )
        };
        let inside = inspect(&refreshed, 0.0);
        assert!(inside.buildable());
        assert!(inside.crossings.is_empty());
        let outside = inspect(&refreshed, 0.05);
        assert!(outside.ground_water);
        assert_eq!(outside.crossings.len(), 1);
        assert!(outside.roads_ready && outside.water_ready);
        let road_only = merge(
            Some(&old),
            EnvironmentSnapshot {
                road_coverage: vec![bounds],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(inspect(&road_only, 0.0).ground_water);
        assert!(inspect(&road_only, 0.0).crossings.is_empty());
        let serialized = serde_json::to_string(&refreshed).unwrap();
        let restored = serde_json::from_str(&serialized).unwrap();
        assert_eq!(inspect(&restored, 0.05), outside);
        // Refreshing the identical region again must not accumulate stale versions.
        let again = merge(
            Some(&refreshed),
            EnvironmentSnapshot {
                road_coverage: vec![bounds],
                water_coverage: vec![bounds],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(again.roads, refreshed.roads);
        assert_eq!(again.water, refreshed.water);
    }
    #[test]
    fn changed_way_retains_old_geometry_only_outside_refresh() {
        let old = data();
        let mut new = data();
        new.road_coverage[0] = CoverageBounds {
            west: 138.995,
            east: 139.005,
            south: 34.995,
            north: 35.005,
        };
        new.water_coverage.clear();
        new.roads[0].points = vec![p(0.003, -0.01), p(0.003, 0.01)];
        let merged = merge(Some(&old), new).unwrap();
        let inside = rosenfu_core::world::assess_environment(
            &[p(-0.004, 0.0), p(0.004, 0.0)],
            InfrastructureLevel::Ground,
            true,
            &merged,
        );
        assert_eq!(inside.crossings.len(), 1);
        assert_eq!(inside.crossings[0].point.longitude, 139.003);
        let outside = rosenfu_core::world::assess_environment(
            &[p(-0.004, 0.008), p(0.004, 0.008)],
            InfrastructureLevel::Ground,
            true,
            &merged,
        );
        assert_eq!(outside.crossings.len(), 1);
        assert_eq!(outside.crossings[0].point.longitude, 139.0);
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
            coverage: None,
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
