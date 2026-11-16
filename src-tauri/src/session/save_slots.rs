//! Slot identity is session state, never inferred from a company name or UI path.
use super::*;
#[derive(serde::Serialize)]
pub struct SaveSlotList {
    pub active_slot_id: Option<String>,
    pub slots: Vec<rosenfu_storage::SaveSlot>,
}
impl WorldEditor {
    pub fn list_save_slots(&self, root: &Path) -> Result<SaveSlotList, String> {
        Ok(SaveSlotList {
            active_slot_id: self.active_slot_id.clone(),
            slots: rosenfu_storage::list_save_slots(root).map_err(|e| e.to_string())?,
        })
    }
    pub fn create_new_world(&mut self, company_name: String) -> Result<WorldDraft, String> {
        let mut candidate = Self::new();
        // Provider configuration and immutable data cache belong to the app, not a save slot.
        candidate.water_model = self.water_model.clone();
        candidate.rename_company(company_name)?;
        *self = candidate;
        Ok(self.snapshot())
    }
    pub fn active_save_path(&self, root: &Path) -> Result<PathBuf, String> {
        let id = self
            .active_slot_id
            .as_deref()
            .ok_or("先にワールドを保存してください。")?;
        let path = rosenfu_storage::save_slot_path(root, id).map_err(|e| e.to_string())?;
        if self.loaded_path.as_deref() != Some(path.as_path()) {
            return Err("保存先の状態が変わりました。ワールドを読み込み直してください。".into());
        }
        Ok(path)
    }
    pub fn active_autosave_path(&self, root: &Path) -> Result<PathBuf, String> {
        Ok(self
            .active_save_path(root)?
            .with_file_name("world.autosave.rosenfu.json"))
    }
    pub fn require_active_slot(&self, expected_slot_id: &str) -> Result<(), String> {
        if self.active_slot_id.as_deref() != Some(expected_slot_id) {
            return Err("ワールドが切り替わったため、以前の操作を中止しました。".into());
        }
        Ok(())
    }
    pub fn autosave_active_slot(
        &mut self,
        root: &Path,
        expected_slot_id: &str,
        plans: Option<rosenfu_core::world::ConstructionPlans>,
        view: Option<rosenfu_core::world::ViewState>,
    ) -> Result<(), String> {
        if self.active_slot_id.as_deref() != Some(expected_slot_id) {
            return Err("ワールドが切り替わったため、以前の自動保存を中止しました。".into());
        }
        self.save_with_view(&self.active_autosave_path(root)?, plans, view, true)
    }
    pub fn slot_autosave_status(&mut self, root: &Path) -> Result<bool, String> {
        if self.active_slot_id.is_none() {
            return Ok(false);
        }
        self.autosave_status(&self.active_autosave_path(root)?)
    }
    pub fn load_save_slot(&mut self, root: &Path, id: String) -> Result<WorldDraft, String> {
        let path = rosenfu_storage::save_slot_path(root, &id).map_err(|e| e.to_string())?;
        let mut candidate = self.clone();
        candidate.load(&path)?;
        candidate.active_slot_id = Some(id);
        *self = candidate;
        Ok(self.snapshot())
    }
    pub fn load_slot_autosave(&mut self, root: &Path) -> Result<WorldDraft, String> {
        let id = self.active_slot_id.clone();
        let manual = self.active_save_path(root)?;
        let automatic = self.active_autosave_path(root)?;
        let mut candidate = self.clone();
        candidate.load_autosave(&automatic, &manual)?;
        candidate.active_slot_id = id;
        *self = candidate;
        Ok(self.snapshot())
    }
    pub fn save_world_as_slot(
        &mut self,
        root: &Path,
        name: String,
        plans: Option<rosenfu_core::world::ConstructionPlans>,
        view: Option<rosenfu_core::world::ViewState>,
    ) -> Result<ManualSaveResult, String> {
        let mut candidate = self.clone();
        let mut draft = candidate.snapshot();
        if let Some(plans) = plans {
            draft.construction_plans = Some(plans);
        }
        if let Some(view) = view {
            draft.view_state = Some(view);
        }
        candidate.world = World::new(draft).map_err(|e| e.to_string())?;
        let (slot, version) = rosenfu_storage::create_save_slot(root, &name, &candidate.world)
            .map_err(|e| e.to_string())?;
        // Creation returned a validated, unique directory; derive without another fallible IO
        // step so a successful file publication cannot leave session identity behind.
        candidate.loaded_path = Some(root.join("saves").join(&slot.id).join("world.rosenfu.json"));
        candidate.active_slot_id = Some(slot.id);
        candidate.manual_version = Some(version);
        candidate.manual_repair_allowed = false;
        candidate.pending_manual_replacement = None;
        candidate.autosave_version = None;
        candidate.observed_autosave = None;
        candidate.autosave_blocked = false;
        candidate.autosave_replace_allowed = false;
        *self = candidate;
        Ok(ManualSaveResult {
            saved: true,
            replacement_required: None,
        })
    }
    pub fn save_active_slot(
        &mut self,
        root: &Path,
        plans: Option<rosenfu_core::world::ConstructionPlans>,
        view: Option<rosenfu_core::world::ViewState>,
    ) -> Result<ManualSaveResult, String> {
        if self.active_slot_id.is_none() {
            let name = self
                .snapshot()
                .companies
                .iter()
                .find(|c| c.id == self.company)
                .ok_or("会社が見つかりません。")?
                .name
                .clone();
            return self.save_world_as_slot(root, name, plans, view);
        }
        self.save_with_view(&self.active_save_path(root)?, plans, view, false)?;
        Ok(ManualSaveResult {
            saved: true,
            replacement_required: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bytes(path: &Path) -> Vec<u8> {
        std::fs::read(path).unwrap()
    }
    #[test]
    fn new_world_and_save_as_are_isolated_and_do_not_overwrite_legacy() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let legacy = root.join("world.rosenfu.json");
        std::fs::write(&legacy, br#"{"schema_version":17}"#).unwrap();
        let before = bytes(&legacy);
        let mut editor = WorldEditor::new();
        editor.create_new_world("Company A".into()).unwrap();
        assert!(!editor.slot_autosave_status(root).unwrap());
        assert!(editor.active_autosave_path(root).is_err());
        editor.save_active_slot(root, None, None).unwrap();
        let a = editor.active_slot_id.clone().unwrap();
        let a_path = editor.active_save_path(root).unwrap();
        let a_bytes = bytes(&a_path);
        editor.rename_company("Company B".into()).unwrap();
        editor
            .save_world_as_slot(root, "Separate label".into(), None, None)
            .unwrap();
        let b = editor.active_slot_id.clone().unwrap();
        assert_ne!(a, b);
        assert_eq!(bytes(&a_path), a_bytes);
        assert_eq!(bytes(&legacy), before);
        let list = editor.list_save_slots(root).unwrap();
        assert!(list
            .slots
            .iter()
            .any(|s| s.id == b && s.name == "Separate label"));
        assert_eq!(editor.snapshot().companies[0].name, "Company B");
        editor.load_save_slot(root, a.clone()).unwrap();
        assert_eq!(editor.snapshot().companies[0].name, "Company A");
        editor.create_new_world("Company C".into()).unwrap();
        assert!(editor.active_slot_id.is_none());
        assert!(editor.manual_version.is_none());
        assert_eq!(bytes(&a_path), a_bytes);
        editor.save_active_slot(root, None, None).unwrap();
        assert_ne!(editor.active_slot_id.as_ref(), Some(&a));
    }
    #[test]
    fn failed_slot_switch_and_save_keep_world_identity_and_revision() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut editor = WorldEditor::new();
        editor.create_new_world("A".into()).unwrap();
        editor.save_active_slot(root, None, None).unwrap();
        editor.rename_company("unsaved".into()).unwrap();
        let snapshot = editor.snapshot();
        let id = editor.active_slot_id.clone();
        let path = editor.loaded_path.clone();
        let version = editor.manual_version.clone();
        std::fs::write(root.join("world.rosenfu.json"), br#"{"schema_version":17}"#).unwrap();
        for target in ["legacy", "../escape", "slot-missing"] {
            assert!(editor.load_save_slot(root, target.into()).is_err());
            assert_eq!(editor.snapshot(), snapshot);
            assert_eq!(editor.active_slot_id, id);
            assert_eq!(editor.loaded_path, path);
            assert_eq!(editor.manual_version, version);
        }
        assert!(editor
            .save_world_as_slot(root, "".into(), None, None)
            .is_err());
        assert_eq!(editor.snapshot(), snapshot);
        assert_eq!(editor.active_slot_id, id);
        assert_eq!(editor.manual_version, version);
        assert!(editor.create_new_world("".into()).is_err());
        assert_eq!(editor.snapshot(), snapshot);
        assert_eq!(editor.active_slot_id, id);
        let file = editor.active_save_path(root).unwrap();
        let mut outside = WorldEditor::new();
        outside.load(&file).unwrap();
        outside.rename_company("external".into()).unwrap();
        outside.save(&file).unwrap();
        let changed = bytes(&file);
        assert!(editor.save_active_slot(root, None, None).is_err());
        assert_eq!(bytes(&file), changed);
        assert_eq!(editor.snapshot(), snapshot);
        assert_eq!(editor.active_slot_id, id);
        assert_eq!(editor.manual_version, version);
    }
    #[test]
    fn autosaves_are_per_slot_and_recovery_preserves_identity() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut editor = WorldEditor::new();
        editor.create_new_world("A".into()).unwrap();
        editor.save_active_slot(root, None, None).unwrap();
        let a = editor.active_slot_id.clone().unwrap();
        let a_auto = editor.active_autosave_path(root).unwrap();
        editor.rename_company("auto A".into()).unwrap();
        editor.save_with_view(&a_auto, None, None, true).unwrap();
        let saved_auto = bytes(&a_auto);
        editor
            .save_world_as_slot(root, "B".into(), None, None)
            .unwrap();
        let b = editor.active_slot_id.clone().unwrap();
        let b_auto = editor.active_autosave_path(root).unwrap();
        assert_ne!(a_auto, b_auto);
        let before = editor.snapshot();
        assert!(editor.autosave_active_slot(root, &a, None, None).is_err());
        assert_eq!(editor.snapshot(), before);
        assert!(!b_auto.exists());
        assert!(editor.autosave_version.is_none());
        assert!(editor.observed_autosave.is_none());
        assert!(!editor.slot_autosave_status(root).unwrap());
        editor.rename_company("auto B".into()).unwrap();
        editor.save_with_view(&b_auto, None, None, true).unwrap();
        assert_eq!(bytes(&a_auto), saved_auto);
        editor.load_save_slot(root, a.clone()).unwrap();
        assert_eq!(editor.snapshot().companies[0].name, "A");
        assert!(editor.slot_autosave_status(root).unwrap());
        editor.load_slot_autosave(root).unwrap();
        assert_eq!(editor.snapshot().companies[0].name, "auto A");
        assert_eq!(editor.active_slot_id, Some(a));
        editor.save_active_slot(root, None, None).unwrap();
        editor.load_save_slot(root, b).unwrap();
        assert_eq!(editor.snapshot().companies[0].name, "auto A");
        editor.load_slot_autosave(root).unwrap();
        assert_eq!(editor.snapshot().companies[0].name, "auto B");
    }
    #[test]
    fn same_name_saves_do_not_collide_and_switch_clears_only_old_world_history() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut editor = WorldEditor::new();
        editor.create_new_world("A".into()).unwrap();
        editor
            .save_world_as_slot(root, "same".into(), None, None)
            .unwrap();
        let a = editor.active_slot_id.clone().unwrap();
        editor.deleted_alignment = Some((editor.snapshot(), editor.snapshot()));
        editor
            .save_world_as_slot(root, "same".into(), None, None)
            .unwrap();
        assert!(editor.deleted_alignment.is_some());
        assert_eq!(editor.list_save_slots(root).unwrap().slots.len(), 2);
        assert_ne!(editor.active_slot_id.as_ref(), Some(&a));
        assert!(editor.load_save_slot(root, "slot-missing".into()).is_err());
        assert!(editor.deleted_alignment.is_some());
        editor.load_save_slot(root, a).unwrap();
        assert!(editor.deleted_alignment.is_none());
        assert!(editor.undo_alignment_deletion().is_err());
        editor.deleted_alignment = Some((editor.snapshot(), editor.snapshot()));
        editor.create_new_world("B".into()).unwrap();
        assert!(editor.deleted_alignment.is_none());
    }
}
