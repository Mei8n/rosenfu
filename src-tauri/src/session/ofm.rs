//! Session-scoped evidence operations. Raw evidence never enters a UI snapshot.
use super::*;

impl WorldEditor {
    /// Optimistic concurrency token for an evidence operation. It binds the
    /// complete current snapshot, including saved plans and evidence manifest,
    /// and the active destination. It is not an authentication credential.
    pub fn ofm_world_token(&self) -> Result<String, String> {
        let bytes = serde_json::to_vec(&(
            self.world.snapshot(),
            self.company,
            &self.active_slot_id,
            &self.loaded_path,
            self.company_setup_required,
        ))
        .map_err(|_| "ワールドの状態を確認できません。".to_owned())?;
        Ok(rosenfu_core::ofm_evidence::raw_sha256(&bytes))
    }

    /// UI previews receive descriptors but not raw evidence. Reattach only the
    /// current immutable table, rejecting changed or stale manifests.
    pub fn world_for_ui_preview(&self, draft: WorldDraft) -> Result<World, String> {
        self.world
            .replace_retaining_road_evidence(draft)
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_token_is_read_only_and_changes_with_world_or_slot() {
        let mut editor = WorldEditor::new();
        let before = editor.snapshot();
        let token = editor.ofm_world_token().unwrap();
        assert_eq!(token, editor.ofm_world_token().unwrap());
        assert_eq!(editor.snapshot(), before);
        editor.active_slot_id = Some("another-slot".into());
        assert_ne!(token, editor.ofm_world_token().unwrap());
        editor.active_slot_id = None;
        assert_eq!(token, editor.ofm_world_token().unwrap());
        editor.add_station("新駅".into(), 35.0, 139.0).unwrap();
        assert_ne!(token, editor.ofm_world_token().unwrap());
    }

    #[test]
    fn ui_preview_restores_current_evidence_but_rejects_changed_manifest() {
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
        let mut editor = WorldEditor::new();
        let old_token = editor.ofm_world_token().unwrap();
        editor.world = World::new(editor.snapshot().with_road_evidence(store).unwrap()).unwrap();
        assert_ne!(old_token, editor.ofm_world_token().unwrap());
        let original = editor.snapshot();
        let mut from_ui: WorldDraft =
            serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
        assert!(World::new(from_ui.clone()).is_err());
        assert_eq!(
            editor
                .world_for_ui_preview(from_ui.clone())
                .unwrap()
                .snapshot(),
            &original
        );
        from_ui.road_evidence_descriptors.clear();
        assert!(editor.world_for_ui_preview(from_ui).is_err());
        assert_eq!(editor.snapshot(), original);
    }

    #[test]
    fn ui_preview_does_not_replace_the_current_world() {
        let editor = WorldEditor::new();
        let original = editor.snapshot();
        let preview = editor.world_for_ui_preview(original.clone()).unwrap();
        assert_eq!(preview.snapshot(), &original);
        assert_eq!(editor.snapshot(), original);
    }
}
