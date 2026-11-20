//! Provisional alpha JSON storage. One authoritative writer per save path is required.
mod evidence;
use evidence::{BorrowedEvidenceEnvelope, EvidenceEnvelope};
use rosenfu_core::world::{World, WorldDraft, WorldError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

pub const FORMAT: &str = "rosenfu-world";
pub const SCHEMA_VERSION: u64 = 27;
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
    operation: Option<rosenfu_core::world::WorldOperationSave>,
    #[serde(default)]
    fixed_od_scenario: Option<rosenfu_core::world::FixedOdScenario>,
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
    decode_document(serde_json::from_slice(bytes)?)
}

fn decode_document(value: serde_json::Value) -> Result<World, StorageError> {
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
    if version != 26 && version != SCHEMA_VERSION {
        return Err(StorageError::UnsupportedVersion(version));
    }
    if version == 26 && value.get("fixed_od_scenario").is_some() {
        return Err(StorageError::InvalidHeader);
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
    let store = document.road_evidence.restore(version == 26)?;
    document.world.restore_road_evidence(store)?;
    let mut world = World::new(document.world)?;
    world
        .restore_operation(document.operation)
        .map_err(|_| StorageError::InvalidHeader)?;
    world
        .restore_fixed_od_scenario(document.fixed_od_scenario)
        .map_err(|_| StorageError::InvalidHeader)?;
    ensure_world_saveable(&world)?;
    Ok(world)
}

#[derive(Serialize)]
struct BorrowedSaveDocument<'a> {
    format: &'static str,
    schema_version: u64,
    world: &'a WorldDraft,
    road_evidence: BorrowedEvidenceEnvelope<'a>,
    operation: Option<rosenfu_core::world::WorldOperationSave>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fixed_od_scenario: Option<&'a rosenfu_core::world::FixedOdScenario>,
}

/// Keep byte-budget failures distinct from actual disk/stream errors.
struct BoundedWriter<W> {
    inner: W,
    count: usize,
    limit: usize,
    limit_hit: bool,
}
impl<W: Write> BoundedWriter<W> {
    fn new(inner: W, limit: usize) -> Self {
        Self {
            inner,
            count: 0,
            limit,
            limit_hit: false,
        }
    }
}
impl<W: Write> Write for BoundedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.count) {
            self.limit_hit = true;
            return Err(io::Error::other("save size limit"));
        }
        let written = self.inner.write(bytes)?;
        self.count += written;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

struct HashingWriter<W> {
    inner: W,
    digest: Sha256,
}
impl<W: Write> HashingWriter<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            digest: Sha256::new(),
        }
    }
    fn version(self) -> FileVersion {
        FileVersion::Present(self.digest.finalize().into())
    }
}
impl<W: Write> Write for HashingWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(bytes)?;
        self.digest.update(&bytes[..written]);
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn checked_save_document(world: &World) -> Result<BorrowedSaveDocument<'_>, StorageError> {
    let data = world.snapshot();
    data.validate()?;
    Ok(BorrowedSaveDocument {
        format: FORMAT,
        schema_version: SCHEMA_VERSION,
        world: data,
        road_evidence: BorrowedEvidenceEnvelope::from_store(&data.runtime_road_evidence),
        operation: world.operation_save(),
        fixed_od_scenario: world.fixed_od_scenario(),
    })
}

fn write_bounded_save<W: Write>(
    document: &BorrowedSaveDocument<'_>,
    writer: W,
    limit: usize,
) -> Result<W, StorageError> {
    let mut writer = BoundedWriter::new(writer, limit);
    if let Err(error) = serde_json::to_writer_pretty(&mut writer, document) {
        return Err(if writer.limit_hit {
            StorageError::TooLarge
        } else if let Some(kind) = error.io_error_kind() {
            StorageError::Io(io::Error::new(kind, error))
        } else {
            StorageError::Json(error)
        });
    }
    // The wire format includes the final newline; it consumes budget too.
    if let Err(error) = writer.write_all(b"\n") {
        return Err(if writer.limit_hit {
            StorageError::TooLarge
        } else {
            error.into()
        });
    }
    Ok(writer.inner)
}

/// Validate and count the actual pretty-printed save without allocating its output.
pub fn ensure_world_saveable(world: &World) -> Result<(), StorageError> {
    let document = checked_save_document(world)?;
    write_bounded_save(&document, io::sink(), MAX_SAVE_BYTES)?;
    Ok(())
}

pub fn encode(world: &World) -> Result<Vec<u8>, StorageError> {
    let document = checked_save_document(world)?;
    write_bounded_save(&document, Vec::new(), MAX_SAVE_BYTES)
}

#[cfg(test)]
#[test]
fn bounded_write_matches_pretty_bytes_and_includes_final_newline() {
    let world = World::default();
    let document = checked_save_document(&world).unwrap();
    let mut old_bytes = serde_json::to_vec_pretty(&document).unwrap();
    old_bytes.push(b'\n');
    let limit = old_bytes.len();
    assert_eq!(
        write_bounded_save(&document, Vec::new(), limit).unwrap(),
        old_bytes
    );
    assert!(write_bounded_save(&document, io::sink(), limit).is_ok());
    assert!(matches!(
        write_bounded_save(&document, io::sink(), limit - 1),
        Err(StorageError::TooLarge)
    ));
    assert!(matches!(
        write_bounded_save(&document, Vec::new(), limit - 1),
        Err(StorageError::TooLarge)
    ));
    assert_eq!(encode(&world).unwrap(), old_bytes);
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
    let lock_path = slots::slot_lock_path(path).unwrap_or_else(|| PathBuf::from(name));
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

fn temp_sibling(path: &Path) -> Result<NamedTempFile, StorageError> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    Ok(NamedTempFile::new_in(parent)?)
}

fn prepare_file(path: &Path, bytes: &[u8]) -> Result<NamedTempFile, StorageError> {
    let mut temp = temp_sibling(path)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    Ok(temp)
}

fn prepare_world_file(
    path: &Path,
    document: &BorrowedSaveDocument<'_>,
) -> Result<(NamedTempFile, FileVersion), StorageError> {
    let mut temp = temp_sibling(path)?;
    let version = {
        let output = BufWriter::new(HashingWriter::new(temp.as_file_mut()));
        let output = write_bounded_save(document, output, MAX_SAVE_BYTES)?;
        output
            .into_inner()
            .map_err(|error| error.into_error())?
            .version()
    };
    temp.as_file().sync_all()?;
    Ok((temp, version))
}

fn prepare_backup_file(
    path: &Path,
    backup: &Path,
    expected: &FileVersion,
) -> Result<NamedTempFile, StorageError> {
    // Copy and hash the same bytes eventually validated and published as backup.
    // Reopening the temp uses its file identity rather than re-reading a live save.
    let mut input = File::open(path)?.take((MAX_SAVE_BYTES + 1) as u64);
    let mut temp = temp_sibling(backup)?;
    let version = {
        let output = BufWriter::new(HashingWriter::new(temp.as_file_mut()));
        let mut checked = BoundedWriter::new(output, MAX_SAVE_BYTES);
        if let Err(error) = io::copy(&mut input, &mut checked) {
            return Err(if checked.limit_hit {
                StorageError::TooLarge
            } else {
                error.into()
            });
        }
        checked
            .inner
            .into_inner()
            .map_err(|error| error.into_error())?
            .version()
    };
    if version != *expected {
        return Err(StorageError::FileChanged);
    }
    // Use the same world, version, and legacy validation as the ordinary load
    // path, without also holding a full Vec of the copied JSON.
    let value = serde_json::from_reader(BufReader::new(temp.reopen()?)).map_err(|error| {
        if let Some(kind) = error.io_error_kind() {
            StorageError::Io(io::Error::new(kind, error))
        } else {
            StorageError::Json(error)
        }
    })?;
    decode_document(value)?;
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
    let document = checked_save_document(world)?;
    let _lock = lock_save(path)?;
    if file_version(path)? != *expected {
        return Err(StorageError::FileChanged);
    }
    // Slot moves share this lock. No temporary file should be created in
    // the slot until the lock is held, or moved out from under cleanup.
    let (new_file, new_version) = prepare_world_file(path, &document)?;
    if *expected == FileVersion::Missing {
        persist(new_file, path, false)?;
        return Ok((SaveOutcome::Created, new_version));
    }
    let backup = backup_path(path);
    let backup_exists = regular_file_exists(&backup)?;
    let backup_version = file_version(&backup)?;
    let backup_file = prepare_backup_file(path, &backup, expected)?;
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
    let document = checked_save_document(world)?;
    let _lock = lock_save(path)?;
    if file_version(path)? != *expected {
        return Err(StorageError::FileChanged);
    }
    let (prepared, new_version) = prepare_world_file(path, &document)?;
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
        new_version,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_clobber_preserves_a_file_created_after_preparation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("save.json");
        let world = World::default();
        let document = checked_save_document(&world).unwrap();
        let (prepared, _) = prepare_world_file(&path, &document).unwrap();
        fs::write(&path, b"other writer").unwrap();
        assert!(persist(prepared, &path, false).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"other writer");
    }

    #[test]
    fn streaming_save_with_held_lock_leaves_no_sibling_or_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("save.json");
        let world = World::default();
        create_world(&path, &world).unwrap();
        let version = file_version(&path).unwrap();
        let lock = lock_save(&path).unwrap();
        let siblings = || fs::read_dir(dir.path()).unwrap().count();
        let before = siblings();
        assert!(matches!(
            save_world_if_version(&path, &world, &version),
            Err(StorageError::Io(_))
        ));
        assert!(matches!(
            replace_world_if_version(&path, &world, &version),
            Err(StorageError::Io(_))
        ));
        assert_eq!(siblings(), before);
        assert!(!backup_path(&path).exists());
        assert_eq!(file_version(&path).unwrap(), version);
        drop(lock);
    }

    #[test]
    fn streaming_save_uses_the_slot_lock_before_touching_an_archived_slot() {
        let root = tempfile::tempdir().unwrap();
        let world = World::default();
        let (slot, version) =
            slots::create_save_slot(root.path(), "Before archive", &world).unwrap();
        let active = slots::save_slot_path(root.path(), &slot.id).unwrap();
        slots::move_save_slot(root.path(), &slot.id, false).unwrap();
        let archived = slots::save_slot_path(&root.path().join("deleted-saves"), &slot.id).unwrap();
        let held = lock_save(&active).unwrap();

        // Nonblocking lock contention, not NotFound from attempting to create
        // a temporary file in the directory which has already moved away.
        for result in [
            save_world_if_version(&active, &world, &version),
            replace_world_if_version(&active, &world, &version),
        ] {
            let Err(StorageError::Io(error)) = result else {
                panic!("expected slot lock contention");
            };
            assert!(matches!(
                error
                    .get_ref()
                    .and_then(|cause| cause.downcast_ref::<std::fs::TryLockError>()),
                Some(std::fs::TryLockError::WouldBlock)
            ));
        }
        drop(held);
        assert_eq!(file_version(&archived).unwrap(), version);
        assert!(!active.parent().unwrap().exists());
        slots::move_save_slot(root.path(), &slot.id, true).unwrap();
        assert_eq!(file_version(&active).unwrap(), version);
    }

    #[cfg(unix)]
    #[test]
    fn streaming_save_rejects_a_symlink_backup_without_following_it() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("save.json");
        let other = dir.path().join("other.json");
        fs::write(&other, b"preserve this file").unwrap();
        let world = World::default();
        create_world(&path, &world).unwrap();
        let version = file_version(&path).unwrap();
        let backup = backup_path(&path);
        symlink(&other, &backup).unwrap();
        assert!(matches!(
            save_world_if_version(&path, &world, &version),
            Err(StorageError::NotRegularFile(ref rejected)) if *rejected == backup
        ));
        assert_eq!(fs::read(&other).unwrap(), b"preserve this file");
        assert_eq!(fs::read_link(&backup).unwrap(), other);
        assert_eq!(file_version(&path).unwrap(), version);
    }

    #[test]
    fn streaming_save_reports_disk_failure_separately_from_size_limit() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "fake disk failure",
                ))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let world = World::default();
        let document = checked_save_document(&world).unwrap();
        assert!(matches!(
            write_bounded_save(&document, Broken, MAX_SAVE_BYTES),
            Err(StorageError::Io(ref error)) if error.kind()==io::ErrorKind::PermissionDenied
        ));
    }

    #[test]
    fn streaming_save_matches_bytes_versions_and_exact_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        let world = World::default();
        let (_, first) = create_world_if_version(&path, &world, &FileVersion::Missing).unwrap();
        assert_eq!(first, bytes_version(&encode(&world).unwrap()));
        // Prior valid JSON may contain different harmless whitespace; backup
        // must preserve the original file, not pretty-print a reconstruction.
        let mut prior = fs::read(&path).unwrap();
        prior.extend(b"  \n");
        fs::write(&path, &prior).unwrap();
        let prior_version = bytes_version(&prior);
        let mut edited = world.clone();
        edited.add_player("streaming check".to_owned()).unwrap();
        let (_, current) = save_world_if_version(&path, &edited, &prior_version).unwrap();
        assert_eq!(fs::read(backup_path(&path)).unwrap(), prior);
        assert_eq!(fs::read(&path).unwrap(), encode(&edited).unwrap());
        assert_eq!(file_version(&path).unwrap(), current);
        assert!(matches!(
            save_world_if_version(&path, &world, &prior_version),
            Err(StorageError::FileChanged)
        ));
    }
}

mod slots;
pub use slots::{
    create_save_slot, list_deleted_save_slots, list_save_slots, move_save_slot, rename_save_slot,
    save_slot_path, SaveSlot,
};
