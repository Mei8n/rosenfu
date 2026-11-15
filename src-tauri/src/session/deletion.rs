use super::*;

impl WorldEditor {
    pub(super) fn set_anchor_indices(
        &mut self,
        id: AlignmentId,
        indices: Vec<usize>,
    ) -> Result<(), String> {
        let mut draft = self.snapshot();
        draft
            .alignments
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or("線路がありません。")?
            .anchor_indices = Some(indices);
        self.world.replace(draft).map_err(|e| e.to_string())
    }

    /// Remove exactly one original anchor interval, without cascading to dependencies.
    /// Legacy alignments have no recoverable anchors and are treated as one section.
    pub fn delete_alignment_section(
        &mut self,
        id: AlignmentId,
        section_index: usize,
        plans: rosenfu_core::world::ConstructionPlans,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        let mut before = candidate.snapshot();
        before.construction_plans = Some(plans);
        candidate
            .world
            .replace(before.clone())
            .map_err(|e| e.to_string())?;
        let original = before
            .alignments
            .iter()
            .find(|a| a.id == id && a.owner == candidate.company)
            .cloned()
            .ok_or("削除する自社線路が見つかりません。")?;
        let boundaries = original
            .anchor_indices
            .clone()
            .unwrap_or_else(|| vec![0, original.points.len() - 1]);
        let start = *boundaries
            .get(section_index)
            .ok_or("削除する区間がありません。")?;
        let end = *boundaries
            .get(
                section_index
                    .checked_add(1)
                    .ok_or("削除する区間がありません。")?,
            )
            .ok_or("削除する区間がありません。")?;
        // Reuse the full dependency guard on a throwaway clone, never on live data.
        candidate.clone().delete_alignment(id)?;
        if boundaries.len() == 2 {
            candidate.delete_alignment(id)?;
        } else {
            // Source metadata describes an exact span. Never silently lose or falsify it.
            if before
                .alignment_provenance
                .iter()
                .any(|p| p.alignment_id == id)
            {
                return Err("出典付き線路の区間削除には対応していません。".into());
            }
            let mut pieces = vec![];
            if start > 0 {
                pieces.push((
                    original.points[..=start].to_vec(),
                    boundaries[..=section_index].to_vec(),
                ));
            }
            if end < original.points.len() - 1 {
                pieces.push((
                    original.points[end..].to_vec(),
                    boundaries[section_index + 1..]
                        .iter()
                        .map(|i| i - end)
                        .collect(),
                ));
            }
            let mut draft = candidate.snapshot();
            let (points, indices) = pieces.remove(0);
            let surviving = draft
                .alignments
                .iter_mut()
                .find(|a| a.id == id)
                .ok_or("線路がありません。")?;
            surviving.points = points;
            surviving.anchor_indices = Some(indices);
            candidate
                .world
                .replace(draft)
                .map_err(|_| "残る線路が短すぎるため、この区間は削除できません。".to_owned())?;
            for (points, indices) in pieces {
                let new_id = candidate
                    .world
                    .build_alignment_typed(
                        original.owner,
                        original.name.clone(),
                        points,
                        original.track_type,
                    )
                    .map_err(|e| e.to_string())?;
                candidate.set_anchor_indices(new_id, indices)?;
                if let Some(speed) = original.design_speed_kmh {
                    candidate
                        .world
                        .set_alignment_design_speed(new_id, speed)
                        .map_err(|e| e.to_string())?;
                }
            }
        }
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        let after = candidate.snapshot();
        candidate.deleted_alignment = Some((before, after.clone()));
        *self = candidate;
        Ok(after)
    }

    /// Synchronize unsaved plans before checking station removal; never drop them.
    pub fn delete_station_with_plans(
        &mut self,
        id: StationId,
        plans: rosenfu_core::world::ConstructionPlans,
    ) -> Result<WorldDraft, String> {
        let mut candidate = self.clone();
        let mut before = candidate.snapshot();
        before.construction_plans = Some(plans);
        candidate
            .world
            .replace(before.clone())
            .map_err(|e| e.to_string())?;
        candidate.delete_station(id)?;
        let after = candidate.snapshot();
        candidate.deleted_alignment = Some((before, after.clone()));
        *self = candidate;
        Ok(after)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rosenfu_core::world::{ConstructionPlans, TrackType};
    fn plans() -> ConstructionPlans {
        serde_json::from_value(serde_json::json!({"version":1,"plans":[{"id":1,"name":"計画"}],"parkedPaths":[],"activePathId":1,"activePlanId":1,"draftPoints":[],"redoPoints":[],"draftExtension":null,"trackType":"normal","designSpeedKmh":120,"roadFollowing":false,"plannedStations":[],"draftFinished":false})).unwrap()
    }
    fn setup() -> (WorldEditor, AlignmentId) {
        let mut editor = WorldEditor::new();
        let points = (0..7)
            .map(|i| GeoPoint {
                latitude: 35.,
                longitude: 139. + i as f64 * 0.001,
            })
            .collect();
        let snapshot = editor
            .build_plan(vec![PlannedAlignment {
                anchor_indices: Some(vec![0, 2, 4, 6]),
                name: "線路".into(),
                points,
                track_type: TrackType::Normal,
                design_speed_kmh: Some(140.),
                stations: vec![],
                reference: None,
                extension: None,
                destination: None,
            }])
            .unwrap();
        let id = snapshot.alignments[0].id;
        (editor, id)
    }
    #[test]
    fn section_deletion_splits_exact_geometry_and_undo_restores_ids() {
        let (mut editor, id) = setup();
        let mut before = editor.snapshot();
        before.construction_plans = Some(plans());
        let after = editor.delete_alignment_section(id, 1, plans()).unwrap();
        assert_eq!(after.alignments.len(), 2);
        assert_eq!(after.alignments[0].id, id);
        assert_eq!(
            after.alignments[0].points,
            before.alignments[0].points[..=2]
        );
        assert_eq!(after.alignments[1].points, before.alignments[0].points[4..]);
        assert_eq!(after.alignments[1].anchor_indices, Some(vec![0, 2]));
        assert_eq!(after.alignments[1].design_speed_kmh, Some(140.));
        assert_eq!(editor.undo_alignment_deletion().unwrap(), before);
    }
    #[test]
    fn section_deletion_trim_legacy_and_save_roundtrip() {
        let (mut editor, id) = setup();
        let after = editor.delete_alignment_section(id, 0, plans()).unwrap();
        assert_eq!(after.alignments[0].anchor_indices, Some(vec![0, 2, 4]));
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("section.json");
        editor.save(&path).unwrap();
        let mut reloaded = WorldEditor::new();
        let loaded = reloaded.load(&path).unwrap();
        assert_eq!(loaded.alignments, after.alignments);
        reloaded.delete_alignment_section(id, 1, plans()).unwrap();
        assert_eq!(reloaded.snapshot().alignments[0].points.len(), 3);
        reloaded.delete_alignment_section(id, 0, plans()).unwrap();
        assert!(reloaded.snapshot().alignments.is_empty());
        let mut editor = WorldEditor::new();
        let snapshot = editor
            .build_alignment(
                "legacy".into(),
                vec![
                    GeoPoint {
                        latitude: 35.,
                        longitude: 139.,
                    },
                    GeoPoint {
                        latitude: 35.,
                        longitude: 139.01,
                    },
                    GeoPoint {
                        latitude: 35.,
                        longitude: 139.02,
                    },
                ],
            )
            .unwrap();
        assert!(editor
            .delete_alignment_section(snapshot.alignments[0].id, 0, plans())
            .unwrap()
            .alignments
            .is_empty());
    }
    #[test]
    fn invalid_section_and_dependencies_leave_world_unchanged() {
        let (mut editor, id) = setup();
        let before = editor.snapshot();
        assert!(editor
            .delete_alignment_section(id, usize::MAX, plans())
            .is_err());
        assert_eq!(editor.snapshot(), before);
        editor
            .place_station_on_alignment("駅".into(), id, 0.5, 40., 12.)
            .unwrap();
        let before = editor.snapshot();
        assert!(editor.delete_alignment_section(id, 0, plans()).is_err());
        assert_eq!(editor.snapshot(), before);
        let (mut editor, id) = setup();
        let before = editor.snapshot();
        let mut p = plans();
        p.draft_extension = Some(
            serde_json::from_value(serde_json::json!({"alignmentId":id,"endpoint":"end"})).unwrap(),
        );
        assert!(editor.delete_alignment_section(id, 0, p).is_err());
        assert_eq!(editor.snapshot(), before);
    }
    #[test]
    fn station_removal_preserves_plans_and_is_reversible() {
        let (mut editor, id) = setup();
        let snapshot = editor
            .place_station_on_alignment("駅".into(), id, 0.5, 40., 12.)
            .unwrap();
        let station = snapshot.stations[0].id;
        let after = editor.delete_station_with_plans(station, plans()).unwrap();
        assert!(after.stations.is_empty());
        assert_eq!(after.construction_plans, Some(plans()));
        let undo = editor.undo_alignment_deletion().unwrap();
        assert_eq!(undo.stations, snapshot.stations);
        assert_eq!(undo.station_placements, snapshot.station_placements);
    }
    #[test]
    fn invalid_anchor_indices_are_atomic_and_extension_preserves_boundaries() {
        let (mut editor, id) = setup();
        let before = editor.snapshot();
        assert!(editor.set_anchor_indices(id, vec![0, 2, 1, 6]).is_err());
        assert_eq!(editor.snapshot(), before);
        editor
            .build_plan(vec![PlannedAlignment {
                anchor_indices: Some(vec![0, 1, 2]),
                name: "延伸".into(),
                points: vec![
                    GeoPoint {
                        latitude: 35.,
                        longitude: 139.006,
                    },
                    GeoPoint {
                        latitude: 35.,
                        longitude: 139.007,
                    },
                    GeoPoint {
                        latitude: 35.,
                        longitude: 139.008,
                    },
                ],
                track_type: TrackType::Normal,
                design_speed_kmh: Some(140.),
                stations: vec![],
                reference: None,
                extension: Some(PlannedAlignmentExtension {
                    alignment_id: id,
                    endpoint: rosenfu_core::world::AlignmentEndpoint::End,
                }),
                destination: None,
            }])
            .unwrap();
        assert_eq!(
            editor.snapshot().alignments[0].anchor_indices,
            Some(vec![0, 2, 4, 6, 7, 8])
        );
    }
}
