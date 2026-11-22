//! Session-only, bounded chronological World transactions. Never serialized.
use super::*;
use std::sync::Arc;

#[derive(Clone, Default)]
pub(super) struct EditHistory {
    undo: Vec<Arc<EditEntry>>,
    redo: Vec<Arc<EditEntry>>,
    revision: u64,
}
struct EditEntry {
    before: WorldDraft,
    after: WorldDraft,
    guarded_service_path: Option<ServiceId>,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldEditHistory {
    pub revision: u64,
    pub undo_depth: usize,
    pub redo_depth: usize,
}
fn editable(mut world: WorldDraft) -> WorldDraft {
    world.construction_plans = None;
    world.view_state = None;
    world
}
impl WorldEditor {
    pub fn world_edit_history(&self) -> WorldEditHistory {
        WorldEditHistory {
            revision: self.edit_history.revision,
            undo_depth: self.edit_history.undo.len(),
            redo_depth: self.edit_history.redo.len(),
        }
    }
    pub(super) fn clear_edit_history(&mut self) {
        self.edit_history.undo.clear();
        self.edit_history.redo.clear();
        self.edit_history.revision = self.edit_history.revision.saturating_add(1);
    }
    /// One outer transaction supersedes any intermediate candidate transactions.
    pub(super) fn commit_edit(&mut self, mut candidate: Self, final_edit: bool) {
        candidate.world.retain_operation_from(&self.world);
        candidate.world.retain_fixed_od_scenario_from(&self.world);
        self.publish_reconciled_edit(candidate, final_edit);
    }
    /// Publish a validated candidate whose runtime and fixed OD were reconciled
    /// to this same live baseline. Does not perform any fallible work.
    pub(super) fn publish_reconciled_edit(&mut self, candidate: Self, final_edit: bool) {
        self.publish_reconciled_edit_with_service_path(candidate, final_edit, None);
    }
    pub(super) fn publish_reconciled_edit_with_service_path(
        &mut self,
        mut candidate: Self,
        final_edit: bool,
        guarded_service_path: Option<ServiceId>,
    ) {
        let before = editable(self.snapshot());
        let after = editable(candidate.snapshot());
        candidate.edit_history = self.edit_history.clone();
        if before != after {
            if final_edit {
                candidate.clear_edit_history();
                candidate.deleted_alignment = None;
            } else {
                candidate.edit_history.undo.push(Arc::new(EditEntry {
                    before,
                    after,
                    guarded_service_path,
                }));
                if candidate.edit_history.undo.len() > 50 {
                    candidate.edit_history.undo.remove(0);
                }
                candidate.edit_history.redo.clear();
                candidate.edit_history.revision = candidate.edit_history.revision.saturating_add(1);
            }
        } else {
            candidate.deleted_alignment = self.deleted_alignment.clone();
        }
        *self = candidate;
    }
    pub fn undo_world_edit(&mut self, expected_world: WorldDraft) -> Result<WorldDraft, String> {
        self.restore_world_edit(expected_world, false)
    }
    pub fn redo_world_edit(&mut self, expected_world: WorldDraft) -> Result<WorldDraft, String> {
        self.restore_world_edit(expected_world, true)
    }
    fn restore_world_edit(
        &mut self,
        mut expected: WorldDraft,
        redo: bool,
    ) -> Result<WorldDraft, String> {
        let current = editable(self.snapshot());
        // IPC omits the backend-only immutable evidence store. Reattach only
        // the current store for comparison, never change the claimed manifest.
        // The historical target below retains its own evidence, not this store.
        expected.runtime_road_evidence = current.runtime_road_evidence.clone();
        // The IPC draft is already owned; move the UI fields aside instead of
        // cloning every alignment just to exclude them from the CAS check.
        let construction_plans = expected.construction_plans.take();
        let view_state = expected.view_state.take();
        if current != expected {
            return Err("ワールドが変更されています。最新の状態を確認してください。".into());
        }
        let entry = if redo {
            self.edit_history.redo.last()
        } else {
            self.edit_history.undo.last()
        }
        .ok_or("元に戻せる／やり直せる操作がありません。")?
        .clone();
        let (target, baseline) = if redo {
            (&entry.after, &entry.before)
        } else {
            (&entry.before, &entry.after)
        };
        if &current != baseline {
            return Err("履歴の対象外でワールドが変更されたため復元できません。".into());
        }
        let mut draft = target.clone();
        // UI owns live plans and viewport. Validate them against restored IDs,
        // never overwrite them with an older saved or historical snapshot.
        draft.construction_plans = construction_plans;
        draft.view_state = view_state;
        let mut restored = World::new(draft).map_err(|e| e.to_string())?;
        let rebuilt_operation = if let Some(service_id) = entry.guarded_service_path {
            self.reconcile_service_path_edit(&mut restored, service_id)?
        } else {
            restored.retain_operation_from(&self.world);
            restored.retain_fixed_od_scenario_from(&self.world);
            false
        };
        rosenfu_storage::ensure_world_saveable(&restored).map_err(|e| e.to_string())?;
        self.world = restored;
        if rebuilt_operation {
            self.operation_tick = None;
        }
        self.deleted_alignment = None;
        if redo {
            self.edit_history.redo.pop();
            self.edit_history.undo.push(entry);
        } else {
            self.edit_history.undo.pop();
            self.edit_history.redo.push(entry);
        }
        self.edit_history.revision = self.edit_history.revision.saturating_add(1);
        Ok(self.snapshot())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ipc_history_retains_evidence_and_rejects_changed_manifest() {
        use rosenfu_core::ofm_evidence::{
            raw_sha256, EvidenceStore, RawEvidence, TileBinding, DECODER_RULE,
        };
        let mut layer = vec![10, 14];
        layer.extend(b"transportation");
        layer.extend([120, 2, 40, 128, 32]);
        let mut raw = vec![26, layer.len() as u8];
        raw.extend(layer);
        let sha = raw_sha256(&raw);
        let store = EvidenceStore::from_parts(
            vec![RawEvidence {
                sha256: sha.clone(),
                bytes: raw,
            }],
            vec![TileBinding {
                canonical_url: "https://tiles.openfreemap.org/planet/test/14/1/2.pbf".into(),
                revision: "test".into(),
                z: 14,
                x: 1,
                y: 2,
                raw_sha256: sha,
                decoder_rule: DECODER_RULE.into(),
            }],
        )
        .unwrap();
        let ipc = |draft: &WorldDraft| -> WorldDraft {
            serde_json::from_slice(&serde_json::to_vec(draft).unwrap()).unwrap()
        };
        let mut e = WorldEditor::new();
        e.add_station("A".into(), 35.68, 139.76).unwrap();
        e.world = World::new(e.snapshot().with_road_evidence(store.clone()).unwrap()).unwrap();
        let before = e.snapshot();
        let id = before.stations[0].id;
        e.rename_station(id, "B".into()).unwrap();
        let after = e.snapshot();
        assert_ne!(ipc(&after).runtime_road_evidence, store);
        let mut invalid = ipc(&after);
        invalid.road_evidence_descriptors.clear();
        let revision = e.world_edit_history().revision;
        assert!(e.undo_world_edit(invalid).is_err());
        assert_eq!(e.snapshot(), after);
        assert_eq!(e.world_edit_history().revision, revision);
        assert_eq!(e.world_edit_history().undo_depth, 1);
        assert_eq!(e.undo_world_edit(ipc(&after)).unwrap(), before);
        assert_eq!(e.snapshot().runtime_road_evidence, store);
        let mut invalid = ipc(&before);
        invalid.road_evidence_descriptors.clear();
        assert!(e.redo_world_edit(invalid).is_err());
        assert_eq!(e.snapshot(), before);
        assert_eq!(e.world_edit_history().redo_depth, 1);
        assert_eq!(e.redo_world_edit(ipc(&before)).unwrap(), after);
        assert_eq!(e.snapshot().runtime_road_evidence, store);
    }
    #[test]
    fn world_history_live_plans_view_and_invalid_restore_are_atomic() {
        let mut e = WorldEditor::new();
        e.rename_company("A".into()).unwrap();
        let mut expected = e.snapshot();
        expected.construction_plans = Some(serde_json::from_value(serde_json::json!({"version":1,"plans":[{"id":1,"name":"live plan"}],"parkedPaths":[],"activePathId":1,"activePlanId":1,"draftPoints":[],"redoPoints":[],"draftExtension":null,"trackType":"normal","designSpeedKmh":120,"roadFollowing":false,"plannedStations":[],"draftFinished":false})).unwrap());
        expected.view_state = Some(rosenfu_core::world::ViewState {
            latitude: 35.,
            longitude: 139.,
            zoom: 12.,
        });
        let restored = e.undo_world_edit(expected.clone()).unwrap();
        assert_eq!(restored.construction_plans, expected.construction_plans);
        assert_eq!(restored.view_state, expected.view_state);
        let revision = e.world_edit_history().revision;
        let mut invalid = restored.clone();
        invalid.view_state.as_mut().unwrap().zoom = 100.;
        assert!(e.redo_world_edit(invalid).is_err());
        assert_eq!(e.snapshot(), restored);
        assert_eq!(e.world_edit_history().revision, revision);
        assert_eq!(e.world_edit_history().redo_depth, 1);
        let mut stale = restored.clone();
        stale.companies[0].name = "stale claim".into();
        assert_eq!(
            e.redo_world_edit(stale).unwrap_err(),
            "ワールドが変更されています。最新の状態を確認してください。"
        );
        assert_eq!(e.snapshot(), restored);
        assert_eq!(e.world_edit_history().revision, revision);
        assert_eq!(e.world_edit_history().redo_depth, 1);
        let mut expected = restored;
        expected.view_state.as_mut().unwrap().zoom = 14.;
        let redone = e.redo_world_edit(expected.clone()).unwrap();
        assert_eq!(redone.companies[0].name, "A");
        assert_eq!(redone.construction_plans, expected.construction_plans);
        assert_eq!(redone.view_state, expected.view_state);
    }
    #[test]
    fn world_history_chronology_identity_and_redo_invalidation() {
        let mut e = WorldEditor::new();
        e.add_station("A".into(), 35.68, 139.76).unwrap();
        let id = e.snapshot().stations[0].id;
        e.rename_station(id, "B".into()).unwrap();
        e.rename_company("Company".into()).unwrap();
        assert_eq!(e.world_edit_history().undo_depth, 2);
        e.undo_world_edit(e.snapshot()).unwrap();
        assert_eq!(e.snapshot().stations[0].name, "B");
        e.undo_world_edit(e.snapshot()).unwrap();
        assert_eq!(e.snapshot().stations[0].name, "A");
        assert_eq!(e.snapshot().stations[0].id, id);
        e.redo_world_edit(e.snapshot()).unwrap();
        assert_eq!(e.snapshot().stations[0].name, "B");
        e.rename_station(id, "C".into()).unwrap();
        assert_eq!(e.world_edit_history().redo_depth, 0);
    }
    #[test]
    fn world_history_failure_noop_and_final_boundaries() {
        let mut e = WorldEditor::new();
        e.rename_company("A".into()).unwrap();
        let revision = e.world_edit_history().revision;
        e.rename_company("A".into()).unwrap();
        assert!(e.rename_company("".into()).is_err());
        assert_eq!(e.world_edit_history().revision, revision);
        let stale = e.snapshot();
        e.rename_company("B".into()).unwrap();
        assert!(e.undo_world_edit(stale).is_err());
        assert_eq!(e.world_edit_history().undo_depth, 2);
        e.add_station("built".into(), 35.68, 139.76).unwrap();
        assert_eq!(e.world_edit_history().undo_depth, 0);
        assert!(e.undo_world_edit(e.snapshot()).is_err());
    }
    #[test]
    fn world_history_save_keeps_history_load_clears_and_limit_is_bounded() {
        let mut e = WorldEditor::new();
        for i in 0..60 {
            e.rename_company(format!("Company {i}")).unwrap();
        }
        assert_eq!(e.world_edit_history().undo_depth, 50);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.rosenfu");
        e.save(&path).unwrap();
        assert_eq!(e.world_edit_history().undo_depth, 50);
        e.undo_world_edit(e.snapshot()).unwrap();
        assert_eq!(e.snapshot().companies[0].name, "Company 58");
        let revision = e.world_edit_history().revision;
        e.load(&path).unwrap();
        assert_eq!(e.world_edit_history().undo_depth, 0);
        assert!(e.world_edit_history().revision > revision);
    }
    #[test]
    fn world_history_station_delete_restores_same_id() {
        let mut e = WorldEditor::new();
        e.add_station("station".into(), 35.68, 139.76).unwrap();
        let before = e.snapshot();
        e.delete_station(before.stations[0].id).unwrap();
        let deleted = e.snapshot();
        assert_eq!(e.undo_world_edit(deleted.clone()).unwrap(), before);
        assert_eq!(e.redo_world_edit(before).unwrap(), deleted);
    }
}
