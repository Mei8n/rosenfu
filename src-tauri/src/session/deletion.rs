use super::*;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AlignmentSectionSelection {
    pub alignment_id: AlignmentId,
    pub section_indices: Vec<usize>,
}

impl WorldEditor {
    fn validate_section_selection(
        &self,
        selections: &[AlignmentSectionSelection],
        expected_alignments: &[rosenfu_core::world::Alignment],
    ) -> Result<(), String> {
        if selections.is_empty() || selections.len() > 4096 {
            return Err("変更する区間を選択してください。".into());
        }
        let before = self.snapshot();
        let mut seen = std::collections::BTreeSet::new();
        if expected_alignments.len() != selections.len() {
            return Err("選択した線路の情報が一致しません。選択し直してください。".into());
        }
        for selection in selections {
            let id = selection.alignment_id;
            if !seen.insert(id) || selection.section_indices.is_empty() {
                return Err("選択した区間が重複しているか空です。".into());
            }
            let current = before
                .alignments
                .iter()
                .find(|a| a.id == id && a.owner == self.company)
                .ok_or("選択した自社線路がありません。")?;
            let expected = expected_alignments
                .iter()
                .filter(|a| a.id == id)
                .collect::<Vec<_>>();
            if expected.len() != 1 || expected[0] != current {
                return Err("選択した線路が変更されています。選択し直してください。".into());
            }
            let count = current.anchor_indices.as_ref().map_or(1, |a| a.len() - 1);
            let unique = selection
                .section_indices
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>();
            if unique.len() != selection.section_indices.len() || unique.iter().any(|i| *i >= count)
            {
                return Err("選択した区間が不正です。".into());
            }
        }
        Ok(())
    }

    pub fn update_alignment_section_infrastructure(
        &mut self,
        selections: Vec<AlignmentSectionSelection>,
        expected_alignments: Vec<rosenfu_core::world::Alignment>,
        track_count: Option<u8>,
        level: Option<rosenfu_core::world::InfrastructureLevel>,
        plans: rosenfu_core::world::ConstructionPlans,
    ) -> Result<WorldDraft, String> {
        if track_count.is_none() && level.is_none() {
            return Err("線路数または構造を指定してください。".into());
        }
        if track_count.is_some_and(|count| !matches!(count, 1 | 2 | 4)) {
            return Err("線路数は1・2・4から選択してください。".into());
        }
        self.validate_section_selection(&selections, &expected_alignments)?;
        let mut before = self.snapshot();
        before.construction_plans = Some(plans);
        if let Some(target_level) = level {
            let mut candidate = self.clone();
            candidate
                .world
                .replace(before.clone())
                .map_err(|e| e.to_string())?;
            for selection in selections {
                let mut indices = selection.section_indices.clone();
                indices.sort_unstable();
                if indices.windows(2).any(|p| p[1] != p[0] + 1) {
                    return Err(
                        "高さ変更は連続した区間を選択してください。選択外は変更しません。".into(),
                    );
                }
                if let Some(count) = track_count {
                    let mut draft = candidate.snapshot();
                    let line = draft
                        .alignments
                        .iter_mut()
                        .find(|a| a.id == selection.alignment_id)
                        .ok_or("選択した線路がありません。")?;
                    let mut infrastructure = line.effective_infrastructure();
                    for index in &indices {
                        infrastructure[*index].track_count = count;
                    }
                    line.infrastructure = infrastructure;
                    candidate.world.replace(draft).map_err(|e| e.to_string())?;
                }
                let line = candidate
                    .world
                    .snapshot()
                    .alignments
                    .iter()
                    .find(|a| a.id == selection.alignment_id)
                    .expect("validated selection");
                let unchanged_height = if !line.section_profiles.is_empty() {
                    indices.iter().all(|i| {
                        line.section_profiles.get(*i).is_some_and(|p| {
                            p.is_flat() && (p.start_height_m - target_level.height_m()).abs() < 1e-9
                        })
                    })
                } else {
                    match line.vertical_profile {
                        Some(profile) => {
                            profile.is_flat()
                                && (profile.start_height_m - target_level.height_m()).abs() < 1e-9
                        }
                        None => indices.iter().all(|i| {
                            line.section_infrastructure(*i)
                                .is_some_and(|s| s.level == target_level)
                        }),
                    }
                };
                if unchanged_height {
                    let mut draft = candidate.snapshot();
                    let line = draft
                        .alignments
                        .iter_mut()
                        .find(|a| a.id == selection.alignment_id)
                        .unwrap();
                    if line.vertical_profile.is_some() || !line.section_profiles.is_empty() {
                        let mut sections = line.effective_infrastructure();
                        for index in &indices {
                            sections[*index].level = target_level;
                        }
                        line.infrastructure = sections;
                        candidate.world.replace(draft).map_err(|e| e.to_string())?;
                    }
                    continue;
                }
                candidate
                    .world
                    .redevelop_alignment_section_height(
                        candidate.company,
                        selection.alignment_id,
                        indices[0],
                        *indices.last().unwrap(),
                        target_level,
                    )
                    .map_err(|e| e.to_string())?;
            }
            rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
            let after = candidate.snapshot();
            candidate.deleted_alignment = Some((before, after.clone()));
            *self = candidate;
            return Ok(after);
        }
        let mut after = before.clone();
        for selection in selections {
            let alignment = after
                .alignments
                .iter_mut()
                .find(|a| a.id == selection.alignment_id)
                .ok_or("選択した線路がありません。")?;
            if alignment.vertical_profile.is_some() && level.is_some() {
                return Err("勾配区間の変更には縦断の再計算が必要です。".into());
            }
            let mut infrastructure = alignment.effective_infrastructure();
            for index in selection.section_indices {
                if let Some(count) = track_count {
                    infrastructure[index].track_count = count;
                }
                if let Some(level) = level {
                    infrastructure[index].level = level;
                }
            }
            alignment.infrastructure = infrastructure;
        }
        let mut candidate = self.clone();
        candidate.world.replace(after).map_err(|e| e.to_string())?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        let after = candidate.snapshot();
        candidate.deleted_alignment = Some((before, after.clone()));
        *self = candidate;
        Ok(after)
    }

    /// Batch removal is atomic and forms one undo entry.
    /// Indices refer to the supplied, exact original alignment snapshots.
    pub fn delete_alignment_sections(
        &mut self,
        selections: Vec<AlignmentSectionSelection>,
        expected_alignments: Vec<rosenfu_core::world::Alignment>,
        plans: rosenfu_core::world::ConstructionPlans,
    ) -> Result<WorldDraft, String> {
        if selections.is_empty() || selections.len() > 4096 {
            return Err("削除する区間を選択してください。".into());
        }
        let mut candidate = self.clone();
        let mut before = self.snapshot();
        before.construction_plans = Some(plans.clone());
        candidate
            .world
            .replace(before.clone())
            .map_err(|e| e.to_string())?;
        self.validate_section_selection(&selections, &expected_alignments)?;
        for selection in selections {
            // Collapse adjacent selected intervals before removing them: temporary
            // sub-20m fragments must not reject a valid final contiguous removal.
            let mut indices = selection.section_indices;
            indices.sort_unstable();
            let mut ranges = Vec::new();
            for index in indices {
                match ranges.last_mut() {
                    Some((_, end)) if index == *end + 1 => *end = index,
                    _ => ranges.push((index, index)),
                }
            }
            // Removing from the end keeps the original prefix ID and lower indices stable.
            for (start, end) in ranges.into_iter().rev() {
                if start != end {
                    let current = candidate.snapshot();
                    let alignment = current
                        .alignments
                        .iter()
                        .find(|a| a.id == selection.alignment_id)
                        .ok_or("選択した線路がありません。")?;
                    let mut anchors = alignment
                        .anchor_indices
                        .clone()
                        .unwrap_or_else(|| vec![0, alignment.points.len() - 1]);
                    anchors.drain(start + 1..=end);
                    candidate.set_anchor_indices(selection.alignment_id, anchors)?;
                }
                candidate.delete_alignment_section(selection.alignment_id, start, plans.clone())?;
            }
        }
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        let after = candidate.snapshot();
        candidate.deleted_alignment = Some((before, after.clone()));
        *self = candidate;
        Ok(after)
    }

    pub(super) fn set_anchor_indices(
        &mut self,
        id: AlignmentId,
        indices: Vec<usize>,
    ) -> Result<(), String> {
        let mut draft = self.snapshot();
        let alignment = draft
            .alignments
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or("線路がありません。")?;
        if !alignment.infrastructure.is_empty() {
            let old = alignment
                .anchor_indices
                .clone()
                .unwrap_or_else(|| vec![0, alignment.points.len() - 1]);
            let attrs = alignment.effective_infrastructure();
            alignment.infrastructure = indices
                .windows(2)
                .map(|pair| {
                    let section = old
                        .partition_point(|point| *point <= pair[0])
                        .saturating_sub(1);
                    attrs[section.min(attrs.len() - 1)]
                })
                .collect();
        }
        alignment.anchor_indices = Some(indices);
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
        // Only station placements and surviving endpoint links may be remapped.
        // Paths, plans and provenance retain their stricter protection.
        let references = |segment: &rosenfu_core::world::RouteSegment| match segment {
            rosenfu_core::world::RouteSegment::Alignment { alignment_id, .. } => {
                *alignment_id == id
            }
            rosenfu_core::world::RouteSegment::ConnectedAlignments { connections, .. } => {
                connections
                    .iter()
                    .any(|c| c.a.alignment_id == id || c.b.alignment_id == id)
            }
        };
        if before
            .route_paths
            .iter()
            .any(|p| p.segments.iter().any(&references))
            || before
                .service_paths
                .iter()
                .any(|p| p.legs.iter().any(|l| references(&l.segment)))
        {
            return Err("路線・系統が使用中の線路の区間削除には対応していません。".into());
        }
        if boundaries.len() == 2 {
            // Disconnect only this alignment. Validation rejects route/service paths
            // that still require the link; equipment and plans retain their guards.
            let mut disconnected = before.clone();
            disconnected
                .alignment_connections
                .retain(|c| c.a.alignment_id != id && c.b.alignment_id != id);
            candidate
                .world
                .replace(disconnected)
                .map_err(|e| e.to_string())?;
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
            let mut stripped = before.clone();
            // Groups require every member to have a placement. This private
            // intermediate candidate temporarily omits placements; restore the
            // exact groups only after all retained placements are remapped.
            stripped.station_groups.clear();
            stripped.station_placements.retain(|p| p.alignment_id != id);
            stripped
                .alignment_connections
                .retain(|c| c.a.alignment_id != id && c.b.alignment_id != id);
            candidate
                .world
                .replace(stripped)
                .map_err(|e| e.to_string())?;
            candidate.clone().delete_alignment(id)?;
            let mut prefix = original.clone();
            prefix.points = original.points[..=start].to_vec();
            let cut_start = prefix.length_m();
            prefix.points = original.points[..=end].to_vec();
            let cut_end = prefix.length_m();
            let old_length = original.length_m();
            for placement in before
                .station_placements
                .iter()
                .filter(|p| p.alignment_id == id)
            {
                let center = placement.position * old_length;
                if !(start > 0 && center + placement.length_m / 2.0 <= cut_start
                    || end < original.points.len() - 1
                        && center - placement.length_m / 2.0 >= cut_end)
                {
                    return Err("駅のホームに重なる区間は削除できません。".into());
                }
            }
            let mut piece_ids = vec![id];
            let mut pieces = vec![];
            if start > 0 {
                pieces.push((
                    original.points[..=start].to_vec(),
                    boundaries[..=section_index].to_vec(),
                    original.effective_infrastructure()[..section_index].to_vec(),
                    original
                        .vertical_profile
                        .and_then(|p| p.clip(0.0, cut_start / old_length)),
                    if original.section_profiles.is_empty() {
                        vec![]
                    } else {
                        original.section_profiles[..section_index].to_vec()
                    },
                ));
            }
            if end < original.points.len() - 1 {
                pieces.push((
                    original.points[end..].to_vec(),
                    boundaries[section_index + 1..]
                        .iter()
                        .map(|i| i - end)
                        .collect(),
                    original.effective_infrastructure()[section_index + 1..].to_vec(),
                    original
                        .vertical_profile
                        .and_then(|p| p.clip(cut_end / old_length, 1.0)),
                    if original.section_profiles.is_empty() {
                        vec![]
                    } else {
                        original.section_profiles[section_index + 1..].to_vec()
                    },
                ));
            }
            let mut draft = candidate.snapshot();
            let (points, indices, infrastructure, profile, section_profiles) = pieces.remove(0);
            let surviving = draft
                .alignments
                .iter_mut()
                .find(|a| a.id == id)
                .ok_or("線路がありません。")?;
            surviving.points = points;
            surviving.anchor_indices = Some(indices);
            surviving.infrastructure = infrastructure;
            surviving.vertical_profile = profile;
            surviving.section_profiles = section_profiles;
            candidate
                .world
                .replace(draft)
                .map_err(|_| "残る線路が短すぎるため、この区間は削除できません。".to_owned())?;
            for (points, indices, infrastructure, profile, section_profiles) in pieces {
                let new_id = candidate
                    .world
                    .build_alignment_typed(
                        original.owner,
                        original.name.clone(),
                        points,
                        original.track_type,
                    )
                    .map_err(|e| e.to_string())?;
                piece_ids.push(new_id);
                candidate.set_anchor_indices(new_id, indices)?;
                let mut data = candidate.snapshot();
                data.alignments
                    .iter_mut()
                    .find(|a| a.id == new_id)
                    .unwrap()
                    .infrastructure = infrastructure;
                data.alignments
                    .iter_mut()
                    .find(|a| a.id == new_id)
                    .unwrap()
                    .vertical_profile = profile;
                let line = data.alignments.iter_mut().find(|a| a.id == new_id).unwrap();
                line.section_profiles = section_profiles;
                candidate.world.replace(data).map_err(|e| e.to_string())?;
                if let Some(speed) = original.design_speed_kmh {
                    candidate
                        .world
                        .set_alignment_design_speed(new_id, speed)
                        .map_err(|e| e.to_string())?;
                }
            }
            let mut restored = candidate.snapshot();
            let right_id = *piece_ids.last().ok_or("残る線路がありません。")?;
            for placement in before
                .station_placements
                .iter()
                .filter(|p| p.alignment_id == id)
            {
                let mut placement = placement.clone();
                let center = placement.position * old_length;
                let offset = if start > 0 && center + placement.length_m / 2.0 <= cut_start {
                    0.0
                } else {
                    cut_end
                };
                placement.alignment_id = if offset == 0.0 { id } else { right_id };
                let length = restored
                    .alignments
                    .iter()
                    .find(|a| a.id == placement.alignment_id)
                    .ok_or("残る線路がありません。")?
                    .length_m();
                placement.position = (center - offset) / length;
                restored.station_placements.push(placement);
            }
            // Preserve placement order as well as IDs, dimensions and station coordinates.
            restored.station_placements.sort_by_key(|p| {
                before
                    .station_placements
                    .iter()
                    .position(|old| old.station_id == p.station_id)
            });
            restored.station_groups = before.station_groups.clone();
            restored.alignment_connections = before
                .alignment_connections
                .iter()
                .filter(|connection| {
                    ![connection.a, connection.b].iter().any(|endpoint| {
                        endpoint.alignment_id == id
                            && match endpoint.endpoint {
                                rosenfu_core::world::AlignmentEndpoint::Start => start == 0,
                                rosenfu_core::world::AlignmentEndpoint::End => {
                                    end == original.points.len() - 1
                                }
                            }
                    })
                })
                .cloned()
                .collect();
            for connection in &mut restored.alignment_connections {
                for endpoint in [&mut connection.a, &mut connection.b] {
                    if endpoint.alignment_id == id
                        && endpoint.endpoint == rosenfu_core::world::AlignmentEndpoint::End
                    {
                        endpoint.alignment_id = right_id;
                    }
                }
            }
            candidate
                .world
                .replace(restored)
                .map_err(|e| e.to_string())?;
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
                anchor_levels: None,
                junction_source: None,
                start_level: None,
                infrastructure: Default::default(),
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
    fn selecting_current_height_does_not_split_short_track_or_reject_station() {
        use rosenfu_core::world::InfrastructureLevel;
        let (mut editor, id) = setup();
        editor
            .place_station_on_alignment("station".into(), id, 0.5, 20., 10.)
            .unwrap();
        let before = editor.snapshot();
        let after = editor
            .update_alignment_section_infrastructure(
                vec![AlignmentSectionSelection {
                    alignment_id: id,
                    section_indices: vec![1],
                }],
                before.alignments.clone(),
                None,
                Some(InfrastructureLevel::Ground),
                plans(),
            )
            .unwrap();
        assert_eq!(before.alignments, after.alignments);
        assert_eq!(before.station_placements, after.station_placements);
        assert_eq!(before.next_id, after.next_id);
    }

    #[test]
    fn selected_height_redevelopment_is_one_undo_and_short_run_is_atomic() {
        use rosenfu_core::world::InfrastructureLevel;
        let (mut editor, id) = setup();
        let mut draft = editor.snapshot();
        for point in &mut draft.alignments[0].points {
            point.longitude = 139. + (point.longitude - 139.) * 10.;
        }
        editor.world.replace(draft).unwrap();
        let mut before = editor.snapshot();
        before.construction_plans = Some(plans());
        let after = editor
            .update_alignment_section_infrastructure(
                vec![AlignmentSectionSelection {
                    alignment_id: id,
                    section_indices: vec![1],
                }],
                before.alignments.clone(),
                None,
                Some(InfrastructureLevel::Elevated),
                plans(),
            )
            .unwrap();
        assert_eq!(after.alignments.len(), 5);
        assert!(after.alignments.iter().any(|a| a
            .vertical_profile
            .is_some_and(|p| p.is_flat() && p.start_height_m == 6.)));
        assert_eq!(editor.undo_alignment_deletion().unwrap(), before);
        let (mut short, id) = setup();
        let before = short.snapshot();
        let error = short
            .update_alignment_section_infrastructure(
                vec![AlignmentSectionSelection {
                    alignment_id: id,
                    section_indices: vec![1],
                }],
                before.alignments.clone(),
                None,
                Some(InfrastructureLevel::Elevated),
                plans(),
            )
            .unwrap_err();
        assert!(error.contains("区間を長く"));
        assert_eq!(short.snapshot(), before);
    }

    #[test]
    fn section_infrastructure_is_local_atomic_and_saved_with_one_undo() {
        use rosenfu_core::world::{InfrastructureLevel, SectionInfrastructure};
        let (mut editor, id) = setup();
        editor
            .place_station_on_alignment("駅".into(), id, 0.1, 20., 12.)
            .unwrap();
        let mut before = editor.snapshot();
        before.construction_plans = Some(plans());
        let after = editor
            .update_alignment_section_infrastructure(
                vec![AlignmentSectionSelection {
                    alignment_id: id,
                    section_indices: vec![1],
                }],
                before.alignments.clone(),
                Some(4),
                None,
                plans(),
            )
            .unwrap();
        assert_eq!(
            after.alignments[0].effective_infrastructure(),
            vec![
                SectionInfrastructure::default(),
                SectionInfrastructure {
                    track_count: 4,
                    level: InfrastructureLevel::Ground
                },
                SectionInfrastructure::default()
            ]
        );
        assert_eq!(after.alignments[0].points, before.alignments[0].points);
        assert_eq!(after.station_placements, before.station_placements);
        assert_eq!(editor.undo_alignment_deletion().unwrap(), before);
        for count in [None, Some(0), Some(3)] {
            assert!(editor
                .update_alignment_section_infrastructure(
                    vec![AlignmentSectionSelection {
                        alignment_id: id,
                        section_indices: vec![1]
                    }],
                    before.alignments.clone(),
                    count,
                    None,
                    plans()
                )
                .is_err());
            assert_eq!(editor.snapshot(), before);
        }
        let after = editor
            .update_alignment_section_infrastructure(
                vec![AlignmentSectionSelection {
                    alignment_id: id,
                    section_indices: vec![0, 2],
                }],
                before.alignments,
                Some(2),
                None,
                plans(),
            )
            .unwrap();
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("infra.json");
        editor.save(&path).unwrap();
        assert_eq!(
            WorldEditor::new().load(&path).unwrap().alignments,
            after.alignments
        );
    }

    #[test]
    fn piecewise_height_cut_keeps_platform_height_and_undo() {
        use rosenfu_core::world::VerticalProfile;
        let (mut editor, id) = setup();
        let mut draft = editor.snapshot();
        for point in &mut draft.alignments[0].points {
            point.longitude = 139. + (point.longitude - 139.) * 10.;
        }
        draft.alignments[0].section_profiles = [(0., 6.), (6., 6.), (6., 0.)]
            .into_iter()
            .map(|(start_height_m, end_height_m)| VerticalProfile {
                start_height_m,
                end_height_m,
                start_tangent_m: 0.,
                end_tangent_m: 0.,
            })
            .collect();
        editor.world.replace(draft).unwrap();
        editor
            .place_station_on_alignment("plateau".into(), id, 0.5, 100., 10.)
            .unwrap();
        let mut before = editor.snapshot();
        before.construction_plans = Some(plans());
        let after = editor.delete_alignment_section(id, 0, plans()).unwrap();
        assert_eq!(after.alignments[0].section_profiles.len(), 2);
        assert_eq!(after.stations, before.stations);
        let placement = &after.station_placements[0];
        assert!(
            (after.alignments[0]
                .height_at_position_m(placement.position)
                .unwrap()
                - 6.)
                .abs()
                < 1e-9
        );
        assert_eq!(editor.undo_alignment_deletion().unwrap(), before);
    }

    #[test]
    fn ramp_section_cut_preserves_profile_and_undo() {
        use rosenfu_core::world::{InfrastructureLevel, VerticalProfile};
        let (mut editor, id) = setup();
        let mut draft = editor.snapshot();
        let profile = VerticalProfile {
            start_height_m: 0.,
            end_height_m: 6.,
            start_tangent_m: 0.,
            end_tangent_m: 0.,
        };
        draft.alignments[0].vertical_profile = Some(profile);
        draft.alignments[0].infrastructure = vec![
            rosenfu_core::world::SectionInfrastructure {
                track_count: 2,
                level: InfrastructureLevel::Elevated
            };
            3
        ];
        editor.world.replace(draft).unwrap();
        let original = editor.snapshot().alignments[0].clone();
        let before = editor.snapshot();
        let after = editor.delete_alignment_section(id, 1, plans()).unwrap();
        assert_eq!(after.alignments.len(), 2);
        for i in 0..11 {
            let t = i as f64 / 10.;
            assert!(
                (after.alignments[0].height_at_position_m(t).unwrap()
                    - original.height_at_position_m(t / 3.).unwrap())
                .abs()
                    < 1e-8
            );
            assert!(
                (after.alignments[1].height_at_position_m(t).unwrap()
                    - original.height_at_position_m(2. / 3. + t / 3.).unwrap())
                .abs()
                    < 1e-8
            );
        }
        let restored = editor.undo_alignment_deletion().unwrap();
        assert_eq!(restored.alignments, before.alignments);
    }

    #[test]
    fn section_attributes_survive_split_and_both_endpoint_extensions() {
        use rosenfu_core::world::{AlignmentEndpoint, InfrastructureLevel, SectionInfrastructure};
        for endpoint in [AlignmentEndpoint::Start, AlignmentEndpoint::End] {
            let (mut editor, id) = setup();
            let mut draft = editor.snapshot();
            let attrs = vec![
                SectionInfrastructure {
                    track_count: 2,
                    level: InfrastructureLevel::Ground,
                },
                SectionInfrastructure::default(),
                SectionInfrastructure {
                    track_count: 4,
                    level: InfrastructureLevel::Ground,
                },
            ];
            draft.alignments[0].infrastructure = attrs.clone();
            editor.world.replace(draft).unwrap();
            let after = editor.delete_alignment_section(id, 1, plans()).unwrap();
            assert_eq!(after.alignments[0].infrastructure, attrs[..1]);
            assert_eq!(after.alignments[1].infrastructure, attrs[2..]);
            editor.undo_alignment_deletion().unwrap();
            let original = editor.snapshot().alignments[0].clone();
            let point = if endpoint == AlignmentEndpoint::Start {
                original.points[0]
            } else {
                *original.points.last().unwrap()
            };
            let delta = if endpoint == AlignmentEndpoint::Start {
                -0.001
            } else {
                0.001
            };
            editor
                .extend_alignment(
                    id,
                    endpoint,
                    vec![
                        point,
                        GeoPoint {
                            longitude: point.longitude + delta,
                            ..point
                        },
                    ],
                )
                .unwrap();
            let updated = editor.snapshot().alignments[0].clone();
            let expected = if endpoint == AlignmentEndpoint::Start {
                vec![attrs[0], attrs[0], attrs[1], attrs[2]]
            } else {
                vec![attrs[0], attrs[1], attrs[2], attrs[2]]
            };
            assert_eq!(updated.infrastructure, expected);
        }
    }

    #[test]
    fn batch_adjacent_short_sections_can_be_removed_as_one_range() {
        let (mut editor, id) = setup();
        let mut before = editor.snapshot();
        for (i, point) in before.alignments[0].points.iter_mut().enumerate() {
            point.longitude = 139.0 + i as f64 * 0.00005;
        }
        editor.world.replace(before.clone()).unwrap();
        let after = editor
            .delete_alignment_sections(
                vec![AlignmentSectionSelection {
                    alignment_id: id,
                    section_indices: vec![0, 1, 2],
                }],
                before.alignments,
                plans(),
            )
            .unwrap();
        assert!(after.alignments.is_empty());
    }

    #[test]
    fn batch_sections_are_one_atomic_undo_and_save_roundtrip() {
        let (mut editor, id) = setup();
        let mut before = editor.snapshot();
        before.construction_plans = Some(plans());
        let after = editor
            .delete_alignment_sections(
                vec![AlignmentSectionSelection {
                    alignment_id: id,
                    section_indices: vec![0, 2],
                }],
                before.alignments.clone(),
                plans(),
            )
            .unwrap();
        assert_eq!(after.alignments.len(), 1);
        assert_eq!(
            after.alignments[0].points,
            before.alignments[0].points[2..=4]
        );
        assert_eq!(editor.undo_alignment_deletion().unwrap(), before);
        let after = editor
            .delete_alignment_sections(
                vec![AlignmentSectionSelection {
                    alignment_id: id,
                    section_indices: vec![2, 0, 1],
                }],
                before.alignments.clone(),
                plans(),
            )
            .unwrap();
        assert!(after.alignments.is_empty());
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("batch.json");
        editor.save(&path).unwrap();
        assert!(WorldEditor::new()
            .load(&path)
            .unwrap()
            .alignments
            .is_empty());
    }

    #[test]
    fn batch_rejects_stale_duplicate_invalid_and_partial_failure_without_mutation() {
        let (mut editor, id) = setup();
        editor
            .place_station_on_alignment("駅".into(), id, 0.5, 40., 12.)
            .unwrap();
        let before = editor.snapshot();
        for indices in [vec![1, 2], vec![0, 0], vec![99], vec![]] {
            assert!(editor
                .delete_alignment_sections(
                    vec![AlignmentSectionSelection {
                        alignment_id: id,
                        section_indices: indices
                    }],
                    before.alignments.clone(),
                    plans()
                )
                .is_err());
            assert_eq!(editor.snapshot(), before);
        }
        let mut stale = before.alignments.clone();
        stale[0].name = "古い選択".into();
        assert!(editor
            .delete_alignment_sections(
                vec![AlignmentSectionSelection {
                    alignment_id: id,
                    section_indices: vec![2]
                }],
                stale,
                plans()
            )
            .is_err());
        assert_eq!(editor.snapshot(), before);
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
        assert!(editor.delete_alignment_section(id, 1, plans()).is_err());
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
    fn distant_stations_survive_split_trim_save_and_undo() {
        for section in 0..3 {
            let (mut editor, id) = setup();
            let positions: Vec<f64> = match section {
                0 => vec![0.5, 0.85],
                1 => vec![0.15, 0.85],
                _ => vec![0.15, 0.5],
            };
            for position in positions {
                editor
                    .place_station_on_alignment("駅".into(), id, position, 40., 12.)
                    .unwrap();
            }
            let mut before = editor.snapshot();
            before.construction_plans = Some(plans());
            let after = editor
                .delete_alignment_section(id, section, plans())
                .unwrap();
            assert_eq!(after.stations, before.stations);
            assert_eq!(after.station_placements.len(), 2);
            for placement in &after.station_placements {
                let old = before
                    .station_placements
                    .iter()
                    .find(|p| p.station_id == placement.station_id)
                    .unwrap();
                assert_eq!(placement.length_m, old.length_m);
                assert_eq!(placement.width_m, old.width_m);
                let alignment = after
                    .alignments
                    .iter()
                    .find(|a| a.id == placement.alignment_id)
                    .unwrap();
                let actual = alignment.point_at(placement.position);
                let expected = before.alignments[0].point_at(old.position);
                assert!((actual.longitude - expected.longitude).abs() < 1e-10);
                assert!((actual.latitude - expected.latitude).abs() < 1e-10);
            }
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("world.json");
            editor.save(&path).unwrap();
            let mut loaded = WorldEditor::new();
            let loaded = loaded.load(&path).unwrap();
            assert_eq!(loaded.stations, after.stations);
            for (actual, expected) in loaded
                .station_placements
                .iter()
                .zip(&after.station_placements)
            {
                assert_eq!(actual.station_id, expected.station_id);
                assert_eq!(actual.alignment_id, expected.alignment_id);
                assert!((actual.position - expected.position).abs() < 1e-14);
                assert_eq!(actual.length_m, expected.length_m);
            }
            assert_eq!(editor.undo_alignment_deletion().unwrap(), before);
        }
    }
    #[test]
    fn surviving_endpoint_connection_remaps_and_removed_endpoint_disconnects() {
        use rosenfu_core::world::{AlignmentEndpoint, AlignmentEndpointRef};
        for section in 0..3 {
            let (mut editor, id) = setup();
            let original = editor.snapshot().alignments[0].clone();
            let next = editor
                .world
                .build_alignment_typed(
                    editor.company,
                    "隣接".into(),
                    vec![
                        original.points[6],
                        GeoPoint {
                            latitude: 35.,
                            longitude: 139.008,
                        },
                    ],
                    TrackType::Normal,
                )
                .unwrap();
            editor
                .world
                .connect_alignment_endpoints(
                    editor.company,
                    AlignmentEndpointRef {
                        alignment_id: id,
                        endpoint: AlignmentEndpoint::End,
                    },
                    AlignmentEndpointRef {
                        alignment_id: next,
                        endpoint: AlignmentEndpoint::Start,
                    },
                )
                .unwrap();
            let mut before = editor.snapshot();
            let result = editor.delete_alignment_section(id, section, plans());
            if section == 2 {
                let after = result.unwrap();
                assert!(after.alignment_connections.is_empty());
                assert_eq!(
                    after.alignments.iter().find(|a| a.id == next),
                    before.alignments.iter().find(|a| a.id == next)
                );
                before.construction_plans = Some(plans());
                assert_eq!(editor.undo_alignment_deletion().unwrap(), before);
            } else {
                let after = result.unwrap();
                // Both retained pieces must remain owned by the original company.
                assert!(after.alignments.iter().all(|a| a.owner == original.owner));
                let connection = &after.alignment_connections[0];
                assert_eq!(connection.b, before.alignment_connections[0].b);
                let retained = after
                    .alignments
                    .iter()
                    .find(|a| a.id == connection.a.alignment_id)
                    .unwrap();
                assert_eq!(retained.points.last(), original.points.last());
                assert_eq!(
                    after.alignments.iter().find(|a| a.id == next),
                    before.alignments.iter().find(|a| a.id == next)
                );
                before.construction_plans = Some(plans());
                assert_eq!(editor.undo_alignment_deletion().unwrap(), before);
            }
        }
    }
    #[test]
    fn route_reference_is_protected_even_when_removed_section_is_distant() {
        use rosenfu_core::world::RouteSegment;
        let (mut editor, id) = setup();
        let a = editor
            .world
            .place_station_on_alignment(editor.company, "A".into(), id, 0.7, 20., 12.)
            .unwrap();
        let b = editor
            .world
            .place_station_on_alignment(editor.company, "B".into(), id, 0.9, 20., 12.)
            .unwrap();
        editor
            .save_planned_route(
                None,
                "路線".into(),
                a,
                vec![RouteSegment::Alignment {
                    alignment_id: id,
                    from: a,
                    to: b,
                }],
                vec![],
            )
            .unwrap();
        let before = editor.snapshot();
        assert!(editor.delete_alignment_section(id, 0, plans()).is_err());
        assert_eq!(editor.snapshot(), before);
    }
    #[test]
    fn start_connection_survives_middle_split_without_retargeting() {
        use rosenfu_core::world::{AlignmentEndpoint, AlignmentEndpointRef};
        let (mut editor, id) = setup();
        let original = editor.snapshot().alignments[0].clone();
        let next = editor
            .world
            .build_alignment_typed(
                editor.company,
                "左".into(),
                vec![
                    GeoPoint {
                        latitude: 35.,
                        longitude: 138.998,
                    },
                    original.points[0],
                ],
                TrackType::Normal,
            )
            .unwrap();
        editor
            .world
            .connect_alignment_endpoints(
                editor.company,
                AlignmentEndpointRef {
                    alignment_id: id,
                    endpoint: AlignmentEndpoint::Start,
                },
                AlignmentEndpointRef {
                    alignment_id: next,
                    endpoint: AlignmentEndpoint::End,
                },
            )
            .unwrap();
        let before = editor.snapshot();
        let disconnected = editor
            .clone()
            .delete_alignment_section(id, 0, plans())
            .unwrap();
        assert!(disconnected.alignment_connections.is_empty());
        assert_eq!(
            disconnected.alignments.iter().find(|a| a.id == next),
            before.alignments.iter().find(|a| a.id == next)
        );
        let after = editor.delete_alignment_section(id, 1, plans()).unwrap();
        assert_eq!(after.alignment_connections, before.alignment_connections);
    }
    #[test]
    fn curved_split_preserves_both_stations_and_both_endpoint_links() {
        use rosenfu_core::world::{AlignmentEndpoint, AlignmentEndpointRef};
        let (mut editor, id) = setup();
        let mut draft = editor.snapshot();
        for (index, point) in draft.alignments[0].points.iter_mut().enumerate() {
            point.latitude += (index as f64 * 0.4).sin() * 0.0005;
        }
        editor.world.replace(draft).unwrap();
        let original = editor.snapshot().alignments[0].clone();
        for (end, outer, inner) in [
            (AlignmentEndpoint::Start, 0, 1),
            (AlignmentEndpoint::End, 6, 5),
        ] {
            let point = original.points[outer];
            let inside = original.points[inner];
            let away = GeoPoint {
                latitude: 2. * point.latitude - inside.latitude,
                longitude: 2. * point.longitude - inside.longitude,
            };
            let adjacent = editor
                .world
                .build_alignment_typed(
                    editor.company,
                    "接続先".into(),
                    vec![point, away],
                    TrackType::Normal,
                )
                .unwrap();
            editor
                .world
                .connect_alignment_endpoints(
                    editor.company,
                    AlignmentEndpointRef {
                        alignment_id: id,
                        endpoint: end,
                    },
                    AlignmentEndpointRef {
                        alignment_id: adjacent,
                        endpoint: AlignmentEndpoint::Start,
                    },
                )
                .unwrap();
        }
        for position in [0.15, 0.85] {
            editor
                .place_station_on_alignment("駅".into(), id, position, 40., 12.)
                .unwrap();
        }
        let mut before = editor.snapshot();
        before.construction_plans = Some(plans());
        let after = editor.delete_alignment_section(id, 1, plans()).unwrap();
        assert_eq!(after.stations, before.stations);
        assert_eq!(
            after.alignment_connections[0],
            before.alignment_connections[0]
        );
        assert_eq!(
            after.alignment_connections[1].b,
            before.alignment_connections[1].b
        );
        assert_eq!(
            after.alignment_connections[1].a.alignment_id,
            after.station_placements[1].alignment_id
        );
        for p in &after.station_placements {
            let a = after
                .alignments
                .iter()
                .find(|a| a.id == p.alignment_id)
                .unwrap();
            let old = before
                .station_placements
                .iter()
                .find(|old| old.station_id == p.station_id)
                .unwrap();
            let actual = a.point_at(p.position);
            let expected = original.point_at(old.position);
            assert!((actual.latitude - expected.latitude).abs() < 1e-10);
            assert!((actual.longitude - expected.longitude).abs() < 1e-10);
        }
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("curve.json");
        editor.save(&path).unwrap();
        let mut loaded = WorldEditor::new();
        let restored = loaded.load(&path).unwrap();
        assert_eq!(restored.alignment_connections, after.alignment_connections);
        assert_eq!(restored.alignments, after.alignments);
        assert_eq!(restored.stations, after.stations);
        assert_eq!(editor.undo_alignment_deletion().unwrap(), before);
    }
    #[test]
    fn grouped_station_survives_distant_section_removal_save_and_undo() {
        let (mut editor, id) = setup();
        editor
            .place_station_on_alignment("交差駅".into(), id, 1.0 / 6.0, 40., 12.)
            .unwrap();
        let crossing = editor
            .world
            .build_alignment(
                editor.company,
                "交差線".into(),
                vec![
                    GeoPoint {
                        latitude: 34.999,
                        longitude: 139.001,
                    },
                    GeoPoint {
                        latitude: 35.001,
                        longitude: 139.001,
                    },
                ],
            )
            .unwrap();
        editor
            .place_station_on_alignment("仮名".into(), crossing, 0.5, 40., 12.)
            .unwrap();
        let mut before = editor.snapshot();
        assert_eq!(before.station_groups.len(), 1);
        before.construction_plans = Some(plans());
        let after = editor.delete_alignment_section(id, 1, plans()).unwrap();
        assert_eq!(after.station_groups, before.station_groups);
        assert_eq!(after.stations, before.stations);
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("group-section.json");
        editor.save(&path).unwrap();
        let mut loaded = WorldEditor::new();
        let restored = loaded.load(&path).unwrap();
        assert_eq!(restored.station_groups, after.station_groups);
        assert_eq!(restored.stations, after.stations);
        assert_eq!(editor.undo_alignment_deletion().unwrap(), before);
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
                anchor_levels: None,
                junction_source: None,
                start_level: None,
                infrastructure: Default::default(),
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
    #[test]
    fn split_remove_gap_reconnect_preserves_owners_endpoints_and_deletion_guards() {
        use rosenfu_core::world::AlignmentEndpoint;
        let mut editor = WorldEditor::new();
        let points: Vec<_> = (0..5)
            .map(|i| GeoPoint {
                latitude: 35.,
                longitude: 139. + i as f64 * 0.002,
            })
            .collect();
        let built = editor
            .build_plan(vec![PlannedAlignment {
                anchor_levels: None,
                junction_source: None,
                start_level: None,
                infrastructure: Default::default(),
                anchor_indices: Some(vec![0, 1, 2, 3, 4]),
                name: "ABCDE".into(),
                points: points.clone(),
                track_type: TrackType::Normal,
                design_speed_kmh: Some(120.),
                stations: vec![],
                reference: None,
                extension: None,
                destination: None,
            }])
            .unwrap();
        let left = built.alignments[0].id;
        // Remove BC, then CD from the surviving CDE piece: AB and DE remain.
        let split = editor.delete_alignment_section(left, 1, plans()).unwrap();
        let right = split.alignments.iter().find(|a| a.id != left).unwrap().id;
        let gap = editor.delete_alignment_section(right, 0, plans()).unwrap();
        assert_eq!(gap.alignments[0].points, points[..2]);
        assert_eq!(gap.alignments[1].points, points[3..]);
        let joined = editor
            .build_plan(vec![PlannedAlignment {
                anchor_levels: None,
                junction_source: None,
                start_level: None,
                infrastructure: Default::default(),
                anchor_indices: Some(vec![0, 1]),
                name: "BD".into(),
                points: vec![points[1], points[3]],
                track_type: TrackType::Normal,
                design_speed_kmh: Some(120.),
                stations: vec![],
                reference: None,
                extension: Some(PlannedAlignmentExtension {
                    alignment_id: left,
                    endpoint: AlignmentEndpoint::End,
                }),
                destination: Some(PlannedAlignmentExtension {
                    alignment_id: right,
                    endpoint: AlignmentEndpoint::Start,
                }),
            }])
            .unwrap();
        assert_eq!(joined.alignments.len(), 2);
        assert!(joined.alignments.iter().all(|a| a.owner == editor.company));
        assert_eq!(
            joined.alignments[0].points,
            vec![points[0], points[1], points[3]]
        );
        assert_eq!(joined.alignments[0].anchor_indices, Some(vec![0, 1, 2]));
        assert_eq!(joined.alignments[1].points, points[3..]);
        assert_eq!(joined.alignment_connections.len(), 1);
        let connection = &joined.alignment_connections[0];
        assert_eq!(connection.a.alignment_id, left);
        assert_eq!(connection.a.endpoint, AlignmentEndpoint::End);
        assert_eq!(connection.b.alignment_id, right);
        assert_eq!(connection.b.endpoint, AlignmentEndpoint::Start);
        assert_eq!(
            joined.alignments[0].points.last(),
            joined.alignments[1].points.first()
        );
        // A connected route/service still owns its corridor: never detach under it.
        let mut dependent = editor.clone();
        let station_a = dependent
            .world
            .place_station_on_alignment(dependent.company, "A駅".into(), left, 0.1, 20., 12.)
            .unwrap();
        let station_e = dependent
            .world
            .place_station_on_alignment(dependent.company, "E駅".into(), right, 0.5, 20., 12.)
            .unwrap();
        let retained_stations = dependent.snapshot();
        let without_bridge = dependent
            .clone()
            .delete_alignment_section(left, 1, plans())
            .unwrap();
        assert_eq!(without_bridge.stations, retained_stations.stations);
        assert!(without_bridge.alignment_connections.is_empty());
        assert_eq!(
            without_bridge
                .station_placements
                .iter()
                .find(|p| p.station_id == station_e),
            retained_stations
                .station_placements
                .iter()
                .find(|p| p.station_id == station_e)
        );
        let mut blocked = dependent.clone();
        assert!(blocked.delete_alignment_section(right, 0, plans()).is_err());
        assert_eq!(blocked.snapshot(), retained_stations);
        let mut overlap = editor.clone();
        overlap
            .world
            .place_station_on_alignment(overlap.company, "橋区間駅".into(), left, 0.7, 20., 12.)
            .unwrap();
        let before_overlap = overlap.snapshot();
        assert!(overlap.delete_alignment_section(left, 1, plans()).is_err());
        assert_eq!(overlap.snapshot(), before_overlap);
        let segment = rosenfu_core::world::RouteSegment::ConnectedAlignments {
            from: station_a,
            to: station_e,
            connections: joined.alignment_connections.clone(),
        };
        let mut routed = dependent.clone();
        routed
            .save_planned_route(
                None,
                "連絡路線".into(),
                station_a,
                vec![segment.clone()],
                vec![],
            )
            .unwrap();
        let preview = dependent
            .preview_service_path(station_a, vec![segment.clone()])
            .unwrap();
        dependent
            .create_service_path(
                "連絡系統".into(),
                station_a,
                vec![segment],
                preview
                    .occurrences
                    .into_iter()
                    .map(|o| o.reference)
                    .collect(),
            )
            .unwrap();
        for mut protected in [routed, dependent] {
            for (id, section) in [(left, 1), (right, 0)] {
                let before = protected.snapshot();
                assert!(protected
                    .delete_alignment_section(id, section, plans())
                    .unwrap_err()
                    .contains("路線・系統"));
                assert_eq!(protected.snapshot(), before);
            }
        }
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("reconnected.json");
        editor.save(&path).unwrap();
        let mut loaded = WorldEditor::new();
        assert_eq!(
            loaded.load(&path).unwrap().alignment_connections,
            joined.alignment_connections
        );
        for (id, section) in [(left, 1), (right, 0)] {
            let mut isolated = loaded.clone();
            let mut before = isolated.snapshot();
            before.construction_plans = Some(plans());
            let after = isolated
                .delete_alignment_section(id, section, plans())
                .unwrap();
            assert!(after.alignment_connections.is_empty());
            let other = if id == left { right } else { left };
            assert_eq!(
                after.alignments.iter().find(|a| a.id == other),
                before.alignments.iter().find(|a| a.id == other)
            );
            let deletion_save = temp.path().join(format!("deleted-{section}.json"));
            let mut saved = isolated.clone();
            saved.save(&deletion_save).unwrap();
            let reopened = WorldEditor::new().load(&deletion_save).unwrap();
            assert_eq!(reopened.alignment_connections, after.alignment_connections);
            assert_eq!(reopened.alignments, after.alignments);
            assert_eq!(isolated.undo_alignment_deletion().unwrap(), before);
            assert_eq!(
                isolated
                    .delete_alignment_section(id, section, plans())
                    .unwrap(),
                after
            );
        }
        // AB does not include the connected D endpoint, so removing AB remains safe.
        let remaining = loaded.delete_alignment_section(left, 0, plans()).unwrap();
        assert_eq!(remaining.alignments[0].points, vec![points[1], points[3]]);
        assert_eq!(
            remaining.alignment_connections,
            joined.alignment_connections
        );
    }
}
