use rosenfu_core::world::World;
use rosenfu_storage::*;
#[test]
fn independent_slots_preserve_legacy_labels_and_backups() {
    let root = tempfile::tempdir().unwrap();
    let world = World::default();
    let legacy = root.path().join("world.rosenfu.json");
    std::fs::write(&legacy, b"old unsupported bytes").unwrap();
    let (a, av) = create_save_slot(root.path(), "First", &world).unwrap();
    let (b, bv) = create_save_slot(root.path(), "First", &world).unwrap();
    assert_ne!(a.id, b.id);
    let ap = save_slot_path(root.path(), &a.id).unwrap();
    let bp = save_slot_path(root.path(), &b.id).unwrap();
    assert_eq!(file_version(&ap).unwrap(), av);
    assert_eq!(file_version(&bp).unwrap(), bv);
    assert_eq!(list_save_slots(root.path()).unwrap().len(), 3);
    assert_eq!(std::fs::read(&legacy).unwrap(), b"old unsupported bytes");
    let list = list_save_slots(root.path()).unwrap();
    assert!(!list.iter().find(|s| s.id == "legacy").unwrap().loadable);
    assert_eq!(list.iter().find(|s| s.id == a.id).unwrap().name, "First");
    save_world_if_version(&ap, &world, &av).unwrap();
    assert!(backup_path(&ap).exists());
    assert!(!backup_path(&bp).exists());
}
#[test]
fn untrusted_ids_names_and_incomplete_slots() {
    let root = tempfile::tempdir().unwrap();
    let world = World::default();
    for id in [
        "../world",
        "/etc/passwd",
        "slot-../../oops",
        "slot-a/bbbb",
        "slot-",
        "slot-あいうえお",
    ] {
        assert!(save_slot_path(root.path(), id).is_err());
    }
    for name in ["", "   ", "bad\nname"] {
        assert!(create_save_slot(root.path(), name, &world).is_err());
    }
    let (slot, _) = create_save_slot(root.path(), "../display label", &world).unwrap();
    assert_eq!(slot.name, "../display label");
    let incomplete = root.path().join("saves/slot-abcdef");
    std::fs::create_dir(&incomplete).unwrap();
    std::fs::write(incomplete.join("manifest.json"), b"{}").unwrap();
    assert_eq!(list_save_slots(root.path()).unwrap().len(), 1);
}
#[cfg(unix)]
#[test]
fn slot_symlinks_never_reach_external_worlds() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("saves")).unwrap();
    symlink(outside.path(), root.path().join("saves/slot-abcdef")).unwrap();
    assert!(save_slot_path(root.path(), "slot-abcdef").is_err());
    assert!(list_save_slots(root.path()).unwrap().is_empty());
}

#[test]
fn deletion_is_recoverable_and_keeps_related_files() {
    let root = tempfile::tempdir().unwrap();
    let (slot, _) = create_save_slot(root.path(), "delete me", &World::default()).unwrap();
    let file = save_slot_path(root.path(), &slot.id).unwrap();
    let before = std::fs::read(&file).unwrap();
    std::fs::write(file.with_file_name("world.autosave.rosenfu.json"), b"auto").unwrap();
    move_save_slot(root.path(), &slot.id, false).unwrap();
    assert!(list_save_slots(root.path()).unwrap().is_empty());
    assert_eq!(
        list_deleted_save_slots(root.path()).unwrap()[0].name,
        "delete me"
    );
    assert!(move_save_slot(root.path(), "../escape", true).is_err());
    move_save_slot(root.path(), &slot.id, true).unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), before);
    assert_eq!(
        std::fs::read(file.with_file_name("world.autosave.rosenfu.json")).unwrap(),
        b"auto"
    );
    assert!(list_deleted_save_slots(root.path()).unwrap().is_empty());
    std::fs::write(
        root.path().join("world.rosenfu.json"),
        b"unsupported legacy",
    )
    .unwrap();
    move_save_slot(root.path(), "legacy", false).unwrap();
    std::fs::write(root.path().join("world.rosenfu.json"), b"new legacy").unwrap();
    assert!(move_save_slot(root.path(), "legacy", true).is_err());
    assert_eq!(
        std::fs::read(root.path().join("world.rosenfu.json")).unwrap(),
        b"new legacy"
    );
}

#[test]
fn deletion_refuses_active_save_lock_without_moving_data() {
    let root = tempfile::tempdir().unwrap();
    let (slot, _) = create_save_slot(root.path(), "busy", &World::default()).unwrap();
    let file = save_slot_path(root.path(), &slot.id).unwrap();
    for filename in [
        "world.rosenfu.json.lock",
        "world.autosave.rosenfu.json.lock",
    ] {
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(
                root.path()
                    .join("saves")
                    .join(format!("{}.{}", slot.id, filename)),
            )
            .unwrap();
        lock.try_lock().unwrap();
        assert!(move_save_slot(root.path(), &slot.id, false).is_err());
        assert!(file.is_file());
        assert!(list_deleted_save_slots(root.path()).unwrap().is_empty());
        drop(lock);
    }
    move_save_slot(root.path(), &slot.id, false).unwrap();
    move_save_slot(root.path(), &slot.id, true).unwrap();
    // Ordinary saves use the exact same stable lock identity.
    let lock_path = root
        .path()
        .join("saves")
        .join(format!("{}.world.rosenfu.json.lock", slot.id));
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(lock_path)
        .unwrap();
    lock.try_lock().unwrap();
    assert!(save_world(&file, &World::default()).is_err());
    assert!(file.is_file());
}

#[test]
fn rename_label_preserves_identity_bytes_versions_and_recovery() {
    let root = tempfile::tempdir().unwrap();
    let (slot, version) = create_save_slot(root.path(), "Before", &World::default()).unwrap();
    let path = save_slot_path(root.path(), &slot.id).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let auto = path.with_file_name("world.autosave.rosenfu.json");
    std::fs::write(&auto, b"autosave unchanged").unwrap();
    std::fs::write(backup_path(&path), b"backup unchanged").unwrap();
    rename_save_slot(root.path(), &slot.id, "Before", "  東京 🚆  ").unwrap();
    assert_eq!(list_save_slots(root.path()).unwrap()[0].name, "東京 🚆");
    assert_eq!(file_version(&path).unwrap(), version);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(std::fs::read(&auto).unwrap(), b"autosave unchanged");
    assert_eq!(
        std::fs::read(backup_path(&path)).unwrap(),
        b"backup unchanged"
    );
    assert!(rename_save_slot(root.path(), &slot.id, "Before", "Stale").is_err());
    for name in ["", " ", "bad\nname"] {
        assert!(rename_save_slot(root.path(), &slot.id, "東京 🚆", name).is_err());
    }
    assert!(rename_save_slot(root.path(), &slot.id, "東京 🚆", &"a".repeat(101)).is_err());
    move_save_slot(root.path(), &slot.id, false).unwrap();
    assert!(rename_save_slot(root.path(), &slot.id, "東京 🚆", "Archived").is_err());
    move_save_slot(root.path(), &slot.id, true).unwrap();
    assert_eq!(list_save_slots(root.path()).unwrap()[0].name, "東京 🚆");
    assert!(rename_save_slot(root.path(), "legacy", "以前のワールド", "new").is_err());
}

#[cfg(unix)]
#[test]
fn rename_rejects_manifest_symlink_without_touching_target() {
    let root = tempfile::tempdir().unwrap();
    let (slot, _) = create_save_slot(root.path(), "Before", &World::default()).unwrap();
    let manifest = save_slot_path(root.path(), &slot.id)
        .unwrap()
        .with_file_name("manifest.json");
    let outside = root.path().join("outside.json");
    std::fs::write(&outside, br#"{"name":"Before"}"#).unwrap();
    std::fs::remove_file(&manifest).unwrap();
    std::os::unix::fs::symlink(&outside, &manifest).unwrap();
    assert!(rename_save_slot(root.path(), &slot.id, "Before", "After").is_err());
    assert_eq!(std::fs::read(&outside).unwrap(), br#"{"name":"Before"}"#);
}
