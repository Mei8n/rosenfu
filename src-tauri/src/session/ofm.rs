//! Session-scoped evidence operations. Raw evidence never enters a UI snapshot.
use super::*;

use base64::Engine;
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OfmRawInput {
    pub sha256: String,
    pub byte_length: usize,
    pub base64: String,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OfmBindingInput {
    pub key: String,
    pub raw_sha256: String,
    pub url: String,
    pub revision: String,
    pub z: u8,
    pub x: u32,
    pub y: u32,
    pub decoder_rule: String,
    pub license: String,
    pub attribution: String,
}

fn inputs(
    raw: Vec<OfmRawInput>,
    bindings: Vec<OfmBindingInput>,
) -> Result<
    (
        Vec<rosenfu_core::ofm_evidence::RawEvidence>,
        Vec<rosenfu_core::ofm_evidence::TileBinding>,
    ),
    String,
> {
    use rosenfu_core::ofm_evidence::{
        RawEvidence, TileBinding, MAX_REQUEST_BYTES, MAX_TILES, MAX_TILE_BYTES,
    };
    let err = || "道路の参照情報が不正または容量上限を超えています。".to_owned();
    if raw.len() > MAX_TILES || bindings.len() > MAX_TILES {
        return Err(err());
    }
    let mut total = 0usize;
    let mut decoded = Vec::new();
    for item in raw {
        if item.byte_length == 0
            || item.byte_length > MAX_TILE_BYTES
            || item.base64.len() != 4 * item.byte_length.div_ceil(3)
        {
            return Err(err());
        }
        total = total
            .checked_add(item.byte_length)
            .filter(|n| *n <= MAX_REQUEST_BYTES)
            .ok_or_else(err)?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&item.base64)
            .map_err(|_| err())?;
        if bytes.len() != item.byte_length
            || base64::engine::general_purpose::STANDARD.encode(&bytes) != item.base64
        {
            return Err(err());
        }
        decoded.push(RawEvidence {
            sha256: item.sha256,
            bytes,
        });
    }
    let mut validated = Vec::new();
    for b in bindings {
        if b.key != b.url
            || b.license != "ODbL-1.0"
            || b.attribution != "© OpenMapTiles © OpenStreetMap contributors"
        {
            return Err(err());
        }
        validated.push(TileBinding {
            canonical_url: b.url,
            revision: b.revision,
            z: b.z,
            x: b.x,
            y: b.y,
            raw_sha256: b.raw_sha256,
            decoder_rule: b.decoder_rule,
        });
    }
    Ok((decoded, validated))
}

impl WorldEditor {
    /// Park a complete draft and its raw evidence together. Failure leaves world,
    /// IDs, deletion Undo, save destination and previously parked drafts unchanged.
    pub fn stage_ofm_construction_plan(
        &mut self,
        expected_token: &str,
        plans: rosenfu_core::world::ConstructionPlans,
        raw: Vec<OfmRawInput>,
        bindings: Vec<OfmBindingInput>,
    ) -> Result<WorldDraft, String> {
        if expected_token != self.ofm_world_token()? {
            return Err("ワールドが変更されたため道路計画の追加を中止しました。".into());
        }
        let (raw, bindings) = inputs(raw, bindings)?;
        let mut candidate = self.clone();
        candidate
            .world
            .stage_ofm_construction_plans(plans, raw, bindings)
            .map_err(|e| e.to_string())?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        *self = candidate;
        Ok(self.snapshot())
    }

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

    fn fixture() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../../crates/rosenfu-core/tests/fixtures/ofm_session_synthetic.json"
        ))
        .unwrap()
    }
    fn stage(editor: &mut WorldEditor) {
        let f = fixture();
        let raw = f["rawEvidence"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                let bytes: Vec<u8> = serde_json::from_value(r["bytes"].clone()).unwrap();
                OfmRawInput {
                    sha256: r["sha256"].as_str().unwrap().into(),
                    byte_length: bytes.len(),
                    base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                }
            })
            .collect();
        let bindings = f["tileBindings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| OfmBindingInput {
                key: r["canonical_url"].as_str().unwrap().into(),
                url: r["canonical_url"].as_str().unwrap().into(),
                raw_sha256: r["raw_sha256"].as_str().unwrap().into(),
                revision: r["revision"].as_str().unwrap().into(),
                z: 14,
                x: r["x"].as_u64().unwrap() as u32,
                y: r["y"].as_u64().unwrap() as u32,
                decoder_rule: r["decoder_rule"].as_str().unwrap().into(),
                license: "ODbL-1.0".into(),
                attribution: "© OpenMapTiles © OpenStreetMap contributors".into(),
            })
            .collect();
        editor
            .stage_ofm_construction_plan(
                &editor.ofm_world_token().unwrap(),
                serde_json::from_value(f["plans"].clone()).unwrap(),
                raw,
                bindings,
            )
            .unwrap();
    }
    fn request() -> PlannedAlignment {
        serde_json::from_value(serde_json::json!({
            "name":"境界道路", "points":[{"latitude":0.,"longitude":0.},{"latitude":0.,"longitude":1.}],
            "trackType":"normal","stations":[],"ofmPath":fixture()["path"]
        })).unwrap()
    }
    #[test]
    fn parked_evidence_build_and_deletion_undo_roundtrip() {
        let mut editor = WorldEditor::new();
        stage(&mut editor);
        let parked = editor.snapshot();
        assert_eq!(parked.road_evidence_descriptors.len(), 2);
        assert_eq!(
            rosenfu_storage::decode(&rosenfu_storage::encode(&editor.world).unwrap()).unwrap(),
            editor.world
        );
        editor.build_plan(vec![request()]).unwrap();
        let built = editor.snapshot();
        let line = built.alignments.last().unwrap();
        assert!(line.points.iter().all(|p| p.latitude > 30.)); // client coordinates ignored
        assert_eq!(built.road_path_provenance.len(), 1);
        assert_eq!(
            rosenfu_storage::decode(&rosenfu_storage::encode(&editor.world).unwrap()).unwrap(),
            editor.world
        );
        editor.delete_alignment(line.id).unwrap();
        assert_eq!(editor.undo_alignment_deletion().unwrap(), built);
    }
    #[test]
    fn stale_stage_and_invalid_second_path_preserve_world() {
        let mut editor = WorldEditor::new();
        stage(&mut editor);
        let before = editor.snapshot();
        assert!(editor
            .stage_ofm_construction_plan(
                "stale",
                serde_json::from_value(fixture()["plans"].clone()).unwrap(),
                vec![],
                vec![]
            )
            .is_err());
        assert_eq!(editor.snapshot(), before);
        let mut bad = request();
        bad.ofm_path.as_mut().unwrap().start.fraction = 2.;
        assert!(editor.build_plan(vec![request(), bad]).is_err());
        assert_eq!(editor.snapshot(), before);
        let raw = OfmRawInput {
            sha256: "0".repeat(64),
            byte_length: 1,
            base64: "AB==".into(),
        };
        assert!(editor
            .stage_ofm_construction_plan(
                &editor.ofm_world_token().unwrap(),
                serde_json::from_value(fixture()["plans"].clone()).unwrap(),
                vec![raw],
                vec![]
            )
            .is_err());
        assert_eq!(editor.snapshot(), before);
    }

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
