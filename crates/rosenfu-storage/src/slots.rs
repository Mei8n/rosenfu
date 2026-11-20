//! Independent save slots. Labels never become paths and creation never overwrites a slot.
use super::*;
#[derive(Debug, Clone, Serialize)]
pub struct SaveSlot {
    pub id: String,
    pub name: String,
    pub updated_at: Option<u64>,
    pub loadable: bool,
    pub error: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    name: String,
}
fn directory(path: &Path) -> Result<(), StorageError> {
    if !fs::symlink_metadata(path)?.file_type().is_dir() {
        return Err(StorageError::NotRegularFile(path.to_owned()));
    }
    Ok(())
}
fn valid_id(id: &str) -> bool {
    id.starts_with("slot-")
        && (11..=64).contains(&id.len())
        && id[5..].bytes().all(|b| b.is_ascii_alphanumeric())
}
// Keep locks outside the directory moved by archive/restore. Windows refuses
// directory rename while any descendant file handle is open.
pub(super) fn slot_lock_path(path: &Path) -> Option<PathBuf> {
    let slot = path.parent()?;
    let id = slot.file_name()?.to_str()?;
    let saves = slot.parent()?;
    if !valid_id(id) || saves.file_name()? != "saves" {
        return None;
    }
    let mut name = std::ffi::OsString::from(id);
    name.push(".");
    name.push(path.file_name()?);
    name.push(".lock");
    Some(saves.join(name))
}
pub fn save_slot_path(root: &Path, id: &str) -> Result<PathBuf, StorageError> {
    directory(root)?;
    if id == "legacy" {
        return Ok(root.join("world.rosenfu.json"));
    }
    if !valid_id(id) {
        return Err(StorageError::InvalidHeader);
    }
    let saves = root.join("saves");
    directory(&saves)?;
    let slot = saves.join(id);
    directory(&slot)?;
    Ok(slot.join("world.rosenfu.json"))
}
fn describe(id: String, name: String, path: &Path) -> SaveSlot {
    let error = load_world(path).err().map(|e| e.to_string());
    let updated_at = fs::symlink_metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs());
    SaveSlot {
        id,
        name,
        updated_at,
        loadable: error.is_none(),
        error,
    }
}
pub fn list_save_slots(root: &Path) -> Result<Vec<SaveSlot>, StorageError> {
    if !root.exists() {
        return Ok(vec![]);
    }
    directory(root)?;
    let mut result = vec![];
    let legacy = root.join("world.rosenfu.json");
    if fs::symlink_metadata(&legacy).is_ok() {
        result.push(describe("legacy".into(), "以前のワールド".into(), &legacy));
    }
    let saves = root.join("saves");
    if !saves.exists() {
        return Ok(result);
    }
    directory(&saves)?;
    for entry in fs::read_dir(saves)? {
        let entry = entry?;
        let id = entry.file_name().to_string_lossy().into_owned();
        if !valid_id(&id) || !entry.file_type()?.is_dir() {
            continue;
        }
        let path = save_slot_path(root, &id)?;
        if !path.exists() {
            continue;
        }
        let manifest = entry.path().join("manifest.json");
        let name = if regular_file_exists(&manifest)? {
            serde_json::from_slice::<Manifest>(&read_bounded(&manifest)?)
                .map(|m| m.name)
                .unwrap_or_else(|_| id.clone())
        } else {
            id.clone()
        };
        result.push(describe(id, name, &path));
    }
    result.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.id.cmp(&b.id)));
    Ok(result)
}
pub fn create_save_slot(
    root: &Path,
    name: &str,
    world: &World,
) -> Result<(SaveSlot, FileVersion), StorageError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 || name.chars().any(char::is_control) {
        return Err(StorageError::InvalidHeader);
    }
    let bytes = encode(world)?;
    fs::create_dir_all(root)?;
    directory(root)?;
    let saves = root.join("saves");
    match fs::create_dir(&saves) {
        Ok(()) => (),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.into()),
    }
    directory(&saves)?;
    let slot = tempfile::Builder::new()
        .prefix("slot-")
        .tempdir_in(&saves)?;
    let id = slot
        .path()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let manifest = slot.path().join("manifest.json");
    persist(
        prepare_file(
            &manifest,
            &serde_json::to_vec(&Manifest { name: name.into() })?,
        )?,
        &manifest,
        false,
    )?;
    let path = slot.path().join("world.rosenfu.json");
    persist(prepare_file(&path, &bytes)?, &path, false)?;
    let result = describe(id, name.into(), &path);
    let _ = slot.keep();
    Ok((result, bytes_version(&bytes)))
}

/// Rename only the display label; world identity, save versions and backups stay intact.
pub fn rename_save_slot(
    root: &Path,
    id: &str,
    expected_name: &str,
    name: &str,
) -> Result<(), StorageError> {
    let name = name.trim();
    if id == "legacy"
        || name.is_empty()
        || name.chars().count() > 100
        || name.chars().any(char::is_control)
    {
        return Err(StorageError::InvalidHeader);
    }
    let path = save_slot_path(root, id)?;
    let _lock = lock_save(&path)?;
    if !regular_file_exists(&path)? {
        return Err(StorageError::InvalidHeader);
    }
    let manifest = path.with_file_name("manifest.json");
    if !regular_file_exists(&manifest)? {
        return Err(StorageError::InvalidHeader);
    }
    let previous: Manifest = serde_json::from_slice(&read_bounded(&manifest)?)?;
    if previous.name != expected_name {
        return Err(StorageError::FileChanged);
    }
    persist(
        prepare_file(
            &manifest,
            &serde_json::to_vec(&Manifest { name: name.into() })?,
        )?,
        &manifest,
        true,
    )
}

/// Recoverable removal. Move the whole slot (including autosave/backups) atomically;
/// never recursively delete user files or replace an existing recovery entry.
pub fn move_save_slot(root: &Path, id: &str, restore: bool) -> Result<(), StorageError> {
    directory(root)?;
    let trash = root.join("deleted-saves");
    if !trash.exists() {
        fs::create_dir(&trash)?;
    }
    directory(&trash)?;
    let (source_root, target_root) = if restore {
        (trash.as_path(), root)
    } else {
        (root, trash.as_path())
    };
    let file = save_slot_path(source_root, id)?;
    if !regular_file_exists(&file)? {
        return Err(StorageError::InvalidHeader);
    }
    let _manual_lock = lock_save(&file)?;
    let _auto_lock = lock_save(&file.with_file_name("world.autosave.rosenfu.json"))?;
    let (source, target) = if id == "legacy" {
        (file, target_root.join("world.rosenfu.json"))
    } else {
        let saves = target_root.join("saves");
        if !saves.exists() {
            fs::create_dir(&saves)?;
        }
        directory(&saves)?;
        (file.parent().unwrap().to_owned(), saves.join(id))
    };
    if fs::symlink_metadata(&target).is_ok() {
        return Err(StorageError::FileChanged);
    }
    if id == "legacy" {
        let _target_lock = lock_save(&target)?;
        // hard_link fails if the destination exists, unlike Unix rename.
        // Both directories are on the same volume; unlink only after a
        // second name exists. A failed unlink preserves the original.
        fs::hard_link(&source, &target)?;
        if let Err(error) = fs::remove_file(&source) {
            let _ = fs::remove_file(&target);
            return Err(error.into());
        }
    } else {
        fs::rename(source, target)?;
    }
    Ok(())
}
pub fn list_deleted_save_slots(root: &Path) -> Result<Vec<SaveSlot>, StorageError> {
    list_save_slots(&root.join("deleted-saves"))
}
