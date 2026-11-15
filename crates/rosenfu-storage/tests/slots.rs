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
