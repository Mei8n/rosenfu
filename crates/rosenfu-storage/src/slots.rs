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
