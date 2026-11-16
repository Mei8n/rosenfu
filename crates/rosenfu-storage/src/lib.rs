//! Provisional alpha JSON storage. One authoritative writer per save path is required.
mod evidence;
use evidence::EvidenceEnvelope;
use rosenfu_core::world::{World, WorldDraft, WorldError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

pub const FORMAT: &str = "rosenfu-world";
pub const SCHEMA_VERSION: u64 = 25;
pub const MAX_SAVE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub enum StorageError {
    Io(io::Error),
    Json(serde_json::Error),
    InvalidHeader,
    InvalidFormat,
    UnsupportedVersion(u64),
    LegacyFormat,
    TooLarge,
    FileChanged,
    InvalidWorld(WorldError),
    NotRegularFile(PathBuf),
}
impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "save I/O error: {e}"),
            Self::Json(e) => write!(f, "invalid save JSON: {e}"),
            Self::InvalidHeader => write!(f, "missing or invalid save header"),
            Self::InvalidFormat => write!(f, "not a Rosenfu world file"),
            Self::UnsupportedVersion(_) | Self::LegacyFormat => {
                write!(f, "このセーブ形式は非対応です")
            }
            Self::FileChanged => write!(
                f,
                "save file changed since it was observed; reload before saving"
            ),
            Self::TooLarge => write!(f, "save exceeds the {MAX_SAVE_BYTES}-byte alpha limit"),
            Self::InvalidWorld(e) => write!(f, "invalid world: {e}"),
            Self::NotRegularFile(p) => {
                write!(f, "save path is not a regular file: {}", p.display())
            }
        }
    }
}
impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Json(e) => Some(e),
            Self::InvalidWorld(e) => Some(e),
            _ => None,
        }
    }
}
impl From<io::Error> for StorageError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for StorageError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
impl From<WorldError> for StorageError {
    fn from(e: WorldError) -> Self {
        Self::InvalidWorld(e)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveDocument {
    format: String,
    schema_version: u64,
    world: WorldDraft,
    road_evidence: EvidenceEnvelope,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SaveOutcome {
    Created,
    /// Explicit replacement; existing backup is unchanged.
    Replaced,
    Updated {
        backup: PathBuf,
    },
}

/// Parse the format/version first, then deserialize and validate the world.
pub fn decode(bytes: &[u8]) -> Result<World, StorageError> {
    if bytes.len() > MAX_SAVE_BYTES {
        return Err(StorageError::TooLarge);
    }
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    let format = value
        .get("format")
        .and_then(|v| v.as_str())
        .ok_or(StorageError::InvalidHeader)?;
    if format != FORMAT {
        return Err(StorageError::InvalidFormat);
    }
    let version = value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .ok_or(StorageError::InvalidHeader)?;
    if version != SCHEMA_VERSION {
        return Err(StorageError::UnsupportedVersion(version));
    }
    let data = value.get("world").ok_or(StorageError::InvalidHeader)?;
    if data.get("tracks").is_some()
        || ["routes", "services"].iter().any(|field| {
            data.get(*field)
                .and_then(|v| v.as_array())
                .is_some_and(|items| {
                    items
                        .iter()
                        .any(|item| item.get("tracks").is_some() || item.get("stops").is_some())
                })
        })
        || data
            .get("route_paths")
            .and_then(|v| v.as_array())
            .is_some_and(|paths| {
                paths.iter().any(|p| {
                    p.get("segments")
                        .and_then(|v| v.as_array())
                        .is_some_and(|segments| {
                            segments
                                .iter()
                                .any(|s| s.get("kind").and_then(|k| k.as_str()) == Some("track"))
                        })
                })
            })
        || data
            .get("service_paths")
            .and_then(|v| v.as_array())
            .is_some_and(|paths| {
                paths.iter().any(|p| {
                    p.get("legs")
                        .and_then(|v| v.as_array())
                        .is_some_and(|legs| {
                            legs.iter().any(|l| {
                                l.get("segment")
                                    .and_then(|s| s.get("kind"))
                                    .and_then(|k| k.as_str())
                                    == Some("track")
                            })
                        })
                })
            })
    {
        return Err(StorageError::LegacyFormat);
    }
    for field in [
        "route_paths",
        "service_paths",
        "service_route_bindings",
        "source_records",
        "alignment_provenance",
    ] {
        if !data.get(field).is_some_and(|v| v.is_array()) {
            return Err(StorageError::InvalidHeader);
        }
    }
    let mut document: SaveDocument = serde_json::from_value(value)?;
    let store = document.road_evidence.restore()?;
    document.world.restore_road_evidence(store)?;
    let world = World::new(document.world)?;
    ensure_world_saveable(&world)?;
    Ok(world)
}

#[derive(Serialize)]
struct BorrowedSaveDocument<'a> {
    format: &'static str,
    schema_version: u64,
    world: &'a WorldDraft,
    road_evidence: EvidenceEnvelope,
}

struct BoundedCounter {
    count: usize,
}
impl Write for BoundedCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_SAVE_BYTES.saturating_sub(self.count) {
            return Err(io::Error::other("save size limit"));
        }
        self.count += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Validate and count the actual pretty-printed save without allocating its output.
pub fn ensure_world_saveable(world: &World) -> Result<(), StorageError> {
    world.snapshot().validate()?;
    let document = BorrowedSaveDocument {
        format: FORMAT,
        schema_version: SCHEMA_VERSION,
        world: world.snapshot(),
        road_evidence: EvidenceEnvelope::from_store(&world.snapshot().runtime_road_evidence),
    };
    let mut counter = BoundedCounter { count: 0 };
    serde_json::to_writer_pretty(&mut counter, &document).map_err(|e| {
        if e.is_io() {
            StorageError::TooLarge
        } else {
            StorageError::Json(e)
        }
    })?;
    counter
        .write_all(b"\n")
        .map_err(|_| StorageError::TooLarge)?;
    Ok(())
}

pub fn encode(world: &World) -> Result<Vec<u8>, StorageError> {
    ensure_world_saveable(world)?;
    let document = BorrowedSaveDocument {
        format: FORMAT,
        schema_version: SCHEMA_VERSION,
        world: world.snapshot(),
        road_evidence: EvidenceEnvelope::from_store(&world.snapshot().runtime_road_evidence),
    };
    let mut bytes = serde_json::to_vec_pretty(&document)?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileVersion {
    Missing,
    Present([u8; 32]),
}

/// Fingerprint even a corrupt or oversized save, using bounded memory.
pub fn file_version(path: impl AsRef<Path>) -> Result<FileVersion, StorageError> {
    let path = path.as_ref();
    if !regular_file_exists(path)? {
        return Ok(FileVersion::Missing);
    }
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(FileVersion::Present(digest.finalize().into()))
}

fn bytes_version(bytes: &[u8]) -> FileVersion {
    FileVersion::Present(Sha256::digest(bytes).into())
}

/// The version corresponds to the exact bytes decoded, not a subsequent read.
pub fn load_world_with_version(
    path: impl AsRef<Path>,
) -> Result<(World, FileVersion), StorageError> {
    let path = path.as_ref();
    regular_file_exists(path)?;
    let bytes = read_bounded(path)?;
    Ok((decode(&bytes)?, bytes_version(&bytes)))
}

fn lock_save(path: &Path) -> Result<File, StorageError> {
    let mut name = path.as_os_str().to_os_string();
    name.push(".lock");
    let lock_path = PathBuf::from(name);
    regular_file_exists(&lock_path)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)?;
    file.try_lock()
        .map_err(|error| StorageError::Io(io::Error::other(error)))?;
    Ok(file)
}

fn regular_file_exists(path: &Path) -> Result<bool, StorageError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(true),
        Ok(_) => Err(StorageError::NotRegularFile(path.to_owned())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, StorageError> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take((MAX_SAVE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_SAVE_BYTES {
        return Err(StorageError::TooLarge);
    }
    Ok(bytes)
}

pub fn load_world(path: impl AsRef<Path>) -> Result<World, StorageError> {
    let path = path.as_ref();
    regular_file_exists(path)?;
    decode(&read_bounded(path)?)
}

pub fn backup_path(path: impl AsRef<Path>) -> PathBuf {
    let mut name = path.as_ref().as_os_str().to_os_string();
    name.push(".bak");
    PathBuf::from(name)
}

fn prepare_file(path: &Path, bytes: &[u8]) -> Result<NamedTempFile, StorageError> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temp = NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    Ok(temp)
}

fn persist(temp: NamedTempFile, path: &Path, replace: bool) -> Result<(), StorageError> {
    let result = if replace {
        temp.persist(path)
    } else {
        temp.persist_noclobber(path)
    };
    result.map(|_| ()).map_err(|e| StorageError::Io(e.error))
}

/// Create a new save only; never replace a path that already exists.
pub fn create_world(path: impl AsRef<Path>, world: &World) -> Result<(), StorageError> {
    create_world_if_version(path, world, &FileVersion::Missing).map(|_| ())
}

pub fn create_world_if_version(
    path: impl AsRef<Path>,
    world: &World,
    expected: &FileVersion,
) -> Result<(SaveOutcome, FileVersion), StorageError> {
    if *expected != FileVersion::Missing {
        return Err(StorageError::FileChanged);
    }
    save_world_if_version(path, world, expected)
}

/// Compatibility API. Session owners should use save_world_if_version with their load baseline.
pub fn save_world(path: impl AsRef<Path>, world: &World) -> Result<SaveOutcome, StorageError> {
    let path = path.as_ref();
    let expected = file_version(path)?;
    save_world_if_version(path, world, &expected).map(|(outcome, _)| outcome)
}

/// Serialize cooperating processes with an OS lock and reject stale session baselines.
/// External tools that ignore the lock can still race; this is not a distributed filesystem transaction.
pub fn save_world_if_version(
    path: impl AsRef<Path>,
    world: &World,
    expected: &FileVersion,
) -> Result<(SaveOutcome, FileVersion), StorageError> {
    let path = path.as_ref();
    let bytes = encode(world)?;
    let _lock = lock_save(path)?;
    if file_version(path)? != *expected {
        return Err(StorageError::FileChanged);
    }
    let new_version = bytes_version(&bytes);
    if *expected == FileVersion::Missing {
        persist(prepare_file(path, &bytes)?, path, false)?;
        return Ok((SaveOutcome::Created, new_version));
    }
    let previous = read_bounded(path)?;
    if bytes_version(&previous) != *expected {
        return Err(StorageError::FileChanged);
    }
    decode(&previous)?;
    let backup = backup_path(path);
    let backup_exists = regular_file_exists(&backup)?;
    let backup_version = file_version(&backup)?;
    let new_file = prepare_file(path, &bytes)?;
    let backup_file = prepare_file(&backup, &previous)?;
    if file_version(path)? != *expected || file_version(&backup)? != backup_version {
        return Err(StorageError::FileChanged);
    }
    persist(backup_file, &backup, backup_exists)?;
    if file_version(path)? != *expected {
        return Err(StorageError::FileChanged);
    }
    persist(new_file, path, true)?;
    Ok((SaveOutcome::Updated { backup }, new_version))
}

/// Explicit replacement after user approval (e.g. restoring an autosave over an unreadable main file).
/// Unlike normal save, this does not decode the old file and leaves its existing backup untouched.
/// The caller must have captured and approved this exact baseline, not fetch a fresh one at save time.
pub fn replace_world_if_version(
    path: impl AsRef<Path>,
    world: &World,
    expected: &FileVersion,
) -> Result<(SaveOutcome, FileVersion), StorageError> {
    let path = path.as_ref();
    let bytes = encode(world)?;
    let _lock = lock_save(path)?;
    if file_version(path)? != *expected {
        return Err(StorageError::FileChanged);
    }
    let prepared = prepare_file(path, &bytes)?;
    if file_version(path)? != *expected {
        return Err(StorageError::FileChanged);
    }
    let exists = *expected != FileVersion::Missing;
    persist(prepared, path, exists)?;
    Ok((
        if exists {
            SaveOutcome::Replaced
        } else {
            SaveOutcome::Created
        },
        bytes_version(&bytes),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_clobber_preserves_a_file_created_after_preparation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("save.json");
        let prepared = prepare_file(&path, b"new data").unwrap();
        fs::write(&path, b"other writer").unwrap();
        assert!(persist(prepared, &path, false).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"other writer");
    }
}

mod slots;
pub use slots::{
    create_save_slot, list_deleted_save_slots, list_save_slots, move_save_slot, save_slot_path,
    SaveSlot,
};
