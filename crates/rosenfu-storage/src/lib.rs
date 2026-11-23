//! Provisional alpha JSON storage. One authoritative writer per save path is required.
mod evidence;
#[cfg(feature = "periodic-world-qa")]
pub mod periodic_qa;
use evidence::{BorrowedEvidenceEnvelope, EvidenceEnvelope};
use rosenfu_core::world::{World, WorldDraft, WorldError};
use serde::{
    de::{self, DeserializeSeed, IntoDeserializer, MapAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

pub const FORMAT: &str = "rosenfu-world";
pub const SCHEMA_VERSION: u64 = 27;
// Use a separate outer tag only for the new mixed capability. Older readers
// classify unsupported outer schemas as non-repairable, unlike a known
// schema whose unfamiliar runtime merely appears invalid.
const MIXED_STANDARD_BUS_SCHEMA: u64 = 28;
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

fn current_wire_schema(version: u64) -> bool {
    version == SCHEMA_VERSION
        || (cfg!(feature = "standard-periodic-rail") && version == MIXED_STANDARD_BUS_SCHEMA)
}

fn operation_wire_schema(operation: Option<&rosenfu_core::world::WorldOperationSave>) -> u64 {
    #[cfg(feature = "standard-periodic-rail")]
    if operation.is_some_and(|op| op.has_standard_timetable() && !op.bus_configs.is_empty()) {
        return MIXED_STANDARD_BUS_SCHEMA;
    }
    #[cfg(not(feature = "standard-periodic-rail"))]
    let _ = operation;
    SCHEMA_VERSION
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveDocument<W = WorldDraft> {
    format: String,
    schema_version: u64,
    world: W,
    road_evidence: EvidenceEnvelope,
    operation: Option<rosenfu_core::world::WorldOperationSave>,
    #[serde(default)]
    fixed_od_scenario: Option<rosenfu_core::world::FixedOdScenario>,
}

// Serde may skip unknown values when decoding a typed struct. Unlike its
// Value parser, that skip need not enforce numeric range or recursion limits.
// Walk every value with the normal JSON parser, without building a DOM.
struct CheckedJson;

impl<'de> Deserialize<'de> for CheckedJson {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Check;

        impl<'de> Visitor<'de> for Check {
            type Value = CheckedJson;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a valid JSON value")
            }

            fn visit_bool<E: de::Error>(self, _: bool) -> Result<Self::Value, E> {
                Ok(CheckedJson)
            }

            fn visit_i64<E: de::Error>(self, _: i64) -> Result<Self::Value, E> {
                Ok(CheckedJson)
            }

            fn visit_u64<E: de::Error>(self, _: u64) -> Result<Self::Value, E> {
                Ok(CheckedJson)
            }

            fn visit_f64<E: de::Error>(self, _: f64) -> Result<Self::Value, E> {
                Ok(CheckedJson)
            }

            fn visit_str<E: de::Error>(self, _: &str) -> Result<Self::Value, E> {
                Ok(CheckedJson)
            }

            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(CheckedJson)
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                while seq.next_element::<CheckedJson>()?.is_some() {}
                Ok(CheckedJson)
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                while map.next_entry::<CheckedJson, CheckedJson>()?.is_some() {}
                Ok(CheckedJson)
            }
        }

        deserializer.deserialize_any(Check)
    }
}

// The low-allocation path must check object shape and required fields before
// trusting defaults in WorldDraft. A failed typed parse is retried through the
// Value path, preserving legacy diagnostics and last-wins duplicate handling.
struct CurrentSave(SaveDocument<RequiredWorldDraft>);

impl<'de> Deserialize<'de> for CurrentSave {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct CurrentSaveVisitor;

        impl<'de> Visitor<'de> for CurrentSaveVisitor {
            type Value = CurrentSave;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a current Rosenfu save object")
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let save = SaveDocument::deserialize(de::value::MapAccessDeserializer::new(map))?;
                if !current_wire_schema(save.schema_version) || save.format != FORMAT {
                    return Err(de::Error::custom("not a current Rosenfu save"));
                }
                Ok(CurrentSave(save))
            }
        }

        deserializer.deserialize_map(CurrentSaveVisitor)
    }
}

const REQUIRED_WORLD_ARRAYS: [&str; 5] = [
    "route_paths",
    "service_paths",
    "service_route_bindings",
    "source_records",
    "alignment_provenance",
];

struct RequiredWorldDraft(WorldDraft);
impl From<RequiredWorldDraft> for WorldDraft {
    fn from(draft: RequiredWorldDraft) -> Self {
        draft.0
    }
}

impl<'de> Deserialize<'de> for RequiredWorldDraft {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct WorldVisitor;

        impl<'de> Visitor<'de> for WorldVisitor {
            type Value = RequiredWorldDraft;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a world object with explicit path and provenance arrays")
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                struct RequiredFields<'a, A> {
                    map: A,
                    found: &'a mut [bool; REQUIRED_WORLD_ARRAYS.len()],
                }

                impl<'de, A: MapAccess<'de>> MapAccess<'de> for RequiredFields<'_, A> {
                    type Error = A::Error;

                    fn next_key_seed<K: DeserializeSeed<'de>>(
                        &mut self,
                        seed: K,
                    ) -> Result<Option<K::Value>, Self::Error> {
                        let Some(key) = self.map.next_key::<String>()? else {
                            return Ok(None);
                        };
                        if let Some(index) = REQUIRED_WORLD_ARRAYS.iter().position(|&f| f == key) {
                            self.found[index] = true;
                        }
                        seed.deserialize(key.into_deserializer()).map(Some)
                    }

                    fn next_value_seed<V: DeserializeSeed<'de>>(
                        &mut self,
                        seed: V,
                    ) -> Result<V::Value, Self::Error> {
                        self.map.next_value_seed(seed)
                    }
                }

                let mut found = [false; REQUIRED_WORLD_ARRAYS.len()];
                let draft = WorldDraft::deserialize(de::value::MapAccessDeserializer::new(
                    RequiredFields {
                        map,
                        found: &mut found,
                    },
                ))?;
                for (present, field) in found.into_iter().zip(REQUIRED_WORLD_ARRAYS) {
                    if !present {
                        return Err(de::Error::missing_field(field));
                    }
                }
                Ok(RequiredWorldDraft(draft))
            }
        }

        deserializer.deserialize_map(WorldVisitor)
    }
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
    if let Ok(CurrentSave(document)) = serde_json::from_slice(bytes) {
        if serde_json::from_slice::<CheckedJson>(bytes).is_ok() {
            let version = document.schema_version;
            return restore_document(document, version);
        }
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
    if version != 26 && !current_wire_schema(version) {
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
    for field in REQUIRED_WORLD_ARRAYS {
        if !data.get(field).is_some_and(|v| v.is_array()) {
            return Err(StorageError::InvalidHeader);
        }
    }
    let document: SaveDocument = serde_json::from_value(value)?;
    restore_document(document, version)
}

fn restore_document<W: Into<WorldDraft>>(
    document: SaveDocument<W>,
    version: u64,
) -> Result<World, StorageError> {
    // Schema 26 predates runtime formats after v2 and the newer rail reservations. Keep that ingress
    // closed even when the current schema supports newer runtime formats.
    if version == 26
        && document.operation.as_ref().is_some_and(|operation| {
            operation.runtime.format != 2
                || operation.crossing_reservations
                || operation.parallel_join_lanes
        })
    {
        return Err(StorageError::InvalidHeader);
    }
    #[cfg(feature = "standard-periodic-rail")]
    if version == 26
        && document
            .operation
            .as_ref()
            .is_some_and(|o| o.has_standard_timetable())
    {
        return Err(StorageError::InvalidHeader);
    }
    if version != 26 && operation_wire_schema(document.operation.as_ref()) != version {
        return Err(StorageError::InvalidHeader);
    }
    let draft = document.world.into();
    if version == 26 && !draft.ofm_gameplay_bus_routes.is_empty() {
        return Err(StorageError::InvalidHeader);
    }
    if version == 26
        && draft.road_transport.bus_stops.iter().any(|stop| {
            stop.location_kind == rosenfu_core::road_transport::BusStopLocationKind::PlayerCreated
        })
    {
        return Err(StorageError::InvalidHeader);
    }
    let store = document.road_evidence.restore(version == 26)?;
    let mut world = World::from_saved_draft(draft, store)?;
    world
        .restore_operation(document.operation)
        .map_err(|_| StorageError::InvalidHeader)?;
    world
        .restore_fixed_od_scenario(document.fixed_od_scenario)
        .map_err(|_| StorageError::InvalidHeader)?;
    // The draft was validated above. Restoring operation and demand only
    // changes World state outside the draft. The canonical size check still
    // uses the complete document, including those restored fields.
    validate_save_footprint(&save_document(&world))?;
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
    world.snapshot().validate()?;
    #[cfg(feature = "standard-periodic-rail")]
    if world.has_standard_timetable() && world.operation().is_some_and(|o| o.invalidated) {
        // The normal loader cannot reopen a periodic source whose exact
        // physical specification no longer exists. Never write such a file.
        return Err(StorageError::InvalidHeader);
    }
    Ok(save_document(world))
}

fn save_document(world: &World) -> BorrowedSaveDocument<'_> {
    let data = world.snapshot();
    let operation = world.operation_save();
    BorrowedSaveDocument {
        format: FORMAT,
        schema_version: operation_wire_schema(operation.as_ref()),
        world: data,
        road_evidence: BorrowedEvidenceEnvelope::from_store(&data.runtime_road_evidence),
        operation,
        fixed_od_scenario: world.fixed_od_scenario(),
    }
}

#[derive(Clone, Copy)]
enum SaveLayout {
    Pretty,
    Compact,
}

/// Keep the old wire bytes whenever they fit. Only a measured wire-size
/// limit, not a serialization or disk failure, can choose compact JSON.
fn prefer_pretty_save<T>(
    mut attempt: impl FnMut(SaveLayout) -> Result<T, StorageError>,
) -> Result<T, StorageError> {
    match attempt(SaveLayout::Pretty) {
        Err(StorageError::TooLarge) => attempt(SaveLayout::Compact),
        result => result,
    }
}

fn write_bounded_save<W: Write>(
    document: &BorrowedSaveDocument<'_>,
    writer: W,
    limit: usize,
    layout: SaveLayout,
) -> Result<W, StorageError> {
    let mut writer = BoundedWriter::new(writer, limit);
    let written = match layout {
        SaveLayout::Pretty => serde_json::to_writer_pretty(&mut writer, document),
        SaveLayout::Compact => serde_json::to_writer(&mut writer, document),
    };
    if let Err(error) = written {
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

fn validate_save_footprint(document: &BorrowedSaveDocument<'_>) -> Result<(), StorageError> {
    prefer_pretty_save(|layout| write_bounded_save(document, io::sink(), MAX_SAVE_BYTES, layout))?;
    Ok(())
}

/// Validate and count the actual save representation without allocating its output.
pub fn ensure_world_saveable(world: &World) -> Result<(), StorageError> {
    validate_save_footprint(&checked_save_document(world)?)
}

pub fn encode(world: &World) -> Result<Vec<u8>, StorageError> {
    let document = checked_save_document(world)?;
    prefer_pretty_save(|layout| write_bounded_save(&document, Vec::new(), MAX_SAVE_BYTES, layout))
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
        write_bounded_save(&document, Vec::new(), limit, SaveLayout::Pretty).unwrap(),
        old_bytes
    );
    assert!(write_bounded_save(&document, io::sink(), limit, SaveLayout::Pretty).is_ok());
    assert!(matches!(
        write_bounded_save(&document, io::sink(), limit - 1, SaveLayout::Pretty),
        Err(StorageError::TooLarge)
    ));
    assert!(matches!(
        write_bounded_save(&document, Vec::new(), limit - 1, SaveLayout::Pretty),
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
    prepare_world_file_bounded(path, document, MAX_SAVE_BYTES)
}

fn prepare_world_file_bounded(
    path: &Path,
    document: &BorrowedSaveDocument<'_>,
    limit: usize,
) -> Result<(NamedTempFile, FileVersion), StorageError> {
    prefer_pretty_save(|layout| {
        // Every attempt owns its sibling and digest. An oversized pretty
        // sibling is discarded before compact starts; the live save/backup
        // remains untouched until the selected bounded output is synchronized.
        let mut temp = temp_sibling(path)?;
        let version = {
            let output = BufWriter::new(HashingWriter::new(temp.as_file_mut()));
            let output = write_bounded_save(document, output, limit, layout)?;
            output
                .into_inner()
                .map_err(|error| error.into_error())?
                .version()
        };
        temp.as_file().sync_all()?;
        Ok((temp, version))
    })
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
    // Parse the copied bytes without holding a full Vec or generic DOM on
    // the common path. The fallback reopens the exact same temp file identity.
    let typed = parse_save_temp::<CurrentSave>(&temp)?;
    let restored_typed = if let Some(CurrentSave(document)) = typed {
        if parse_save_temp::<CheckedJson>(&temp)?.is_some() {
            let version = document.schema_version;
            restore_document(document, version)?;
            true
        } else {
            false
        }
    } else {
        false
    };
    if !restored_typed {
        let value =
            serde_json::from_reader(BufReader::new(temp.reopen()?)).map_err(save_reader_error)?;
        decode_document(value)?;
    }
    temp.as_file().sync_all()?;
    Ok(temp)
}

fn parse_save_temp<T: de::DeserializeOwned>(
    temp: &NamedTempFile,
) -> Result<Option<T>, StorageError> {
    match serde_json::from_reader(BufReader::new(temp.reopen()?)) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.io_error_kind().is_some() => Err(save_reader_error(error)),
        Err(_) => Ok(None),
    }
}

fn save_reader_error(error: serde_json::Error) -> StorageError {
    if let Some(kind) = error.io_error_kind() {
        StorageError::Io(io::Error::new(kind, error))
    } else {
        StorageError::Json(error)
    }
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
    fn schema_26_does_not_accept_player_created_road_stops() {
        let mut data: serde_json::Value =
            serde_json::from_slice(&encode(&World::default()).unwrap()).unwrap();
        data["schema_version"] = 26.into();
        assert!(decode(&serde_json::to_vec(&data).unwrap()).is_ok());
        data["world"]["road_transport"] = serde_json::json!({
            "nodes": [], "edges": [], "bus_routes": [], "tram_sections": [],
            "bus_stops": [{
                "id":"s", "owner":"1", "name":"Player stop", "edge_id":"candidate",
                "position":0.4, "location_kind":"player_created"
            }]
        });
        assert!(matches!(
            decode(&serde_json::to_vec(&data).unwrap()),
            Err(StorageError::InvalidHeader)
        ));
        data["schema_version"] = SCHEMA_VERSION.into();
        assert!(matches!(
            decode(&serde_json::to_vec(&data).unwrap()),
            Err(StorageError::InvalidWorld(
                rosenfu_core::world::WorldError::InvalidRoadTransport
            ))
        ));
    }

    #[test]
    fn typed_save_and_value_fallback_agree_on_shape_and_wire_bytes() {
        let mut world = World::default();
        world.add_player("test / 駅".into()).unwrap();
        let bytes = encode(&world).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let CurrentSave(typed) = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            encode(&restore_document(typed, SCHEMA_VERSION).unwrap()).unwrap(),
            bytes
        );

        let same = |data: &[u8]| {
            let original = serde_json::from_slice(data)
                .map_err(StorageError::Json)
                .and_then(decode_document);
            match (decode(data), original) {
                (Ok(a), Ok(b)) => {
                    assert_eq!(a, b);
                    assert_eq!(encode(&a).unwrap(), encode(&b).unwrap());
                }
                (Err(a), Err(b)) => assert_eq!(a.to_string(), b.to_string()),
                result => panic!("decoder disagreement: {result:?}"),
            }
        };
        same(&bytes);
        same(&serde_json::to_vec(&value).unwrap());
        same(&[bytes.as_slice(), b" \n\r\t"].concat());
        same(&[bytes.as_slice(), b"null"].concat());
        same(&bytes[..bytes.len() - 3]);
        same(b"[]");
        same(b"null");
        let name = String::from_utf8(bytes.clone()).unwrap();
        // Prior Value-based loads accepted duplicates, with the last value
        // winning. Do not broaden or change that behavior on rewrite.
        same(
            name.replacen(
                r#""schema_version": 27,"#,
                r#""schema_version": "discard", "schema_version": 27,"#,
                1,
            )
            .as_bytes(),
        );
        same(
            name.replacen(r#""name":"#, r#""name": false, "name":"#, 1)
                .as_bytes(),
        );
        same(
            name.replacen(r#""route_paths":"#, r#""route_\u0070aths":"#, 1)
                .as_bytes(),
        );
        let mut cases = vec![];
        for key in REQUIRED_WORLD_ARRAYS {
            let mut missing = value.clone();
            missing["world"].as_object_mut().unwrap().remove(key);
            cases.push(missing);
            let mut wrong_type = value.clone();
            wrong_type["world"][key] = serde_json::json!({"empty":true});
            cases.push(wrong_type);
        }
        for (key, val) in [
            ("schema_version", serde_json::json!(26)),
            ("schema_version", serde_json::json!(28)),
            ("schema_version", serde_json::json!(27.0)),
            ("format", serde_json::json!("other")),
            ("unrecognized", serde_json::json!(true)),
            ("world", serde_json::json!([])),
        ] {
            let mut changed = value.clone();
            changed[key] = val;
            cases.push(changed);
        }
        let mut legacy = value.clone();
        legacy["world"]["tracks"] = serde_json::json!([]);
        cases.push(legacy);
        let mut legacy_path = value.clone();
        legacy_path["world"]["route_paths"] = serde_json::json!([{"segments":[{"kind":"track"}]}]);
        cases.push(legacy_path);
        let mut prior_with_future_field = value.clone();
        prior_with_future_field["schema_version"] = serde_json::json!(26);
        prior_with_future_field["fixed_od_scenario"] = serde_json::Value::Null;
        cases.push(prior_with_future_field);
        for changed in cases {
            same(&serde_json::to_vec(&changed).unwrap());
        }
    }

    #[test]
    fn streaming_backup_preserves_exact_current_bytes_with_legacy_duplicate_handling() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("duplicate.json");
        let world = World::default();
        let bytes = String::from_utf8(encode(&world).unwrap())
            .unwrap()
            .replacen(r#""format":"#, r#""format": null, "format":"#, 1)
            .into_bytes();
        assert!(serde_json::from_slice::<CurrentSave>(&bytes).is_err());
        fs::write(&path, &bytes).unwrap();
        assert_eq!(load_world(&path).unwrap(), world);
        let prior = file_version(&path).unwrap();
        save_world_if_version(&path, &world, &prior).unwrap();
        assert_eq!(fs::read(backup_path(&path)).unwrap(), bytes);
        assert_eq!(fs::read(&path).unwrap(), encode(&world).unwrap());
    }

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
    fn compact_fallback_uses_one_bounded_representation_and_no_leftover_temp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bounded.json");
        let mut world = World::default();
        world.add_player("駅\n\" quoted".into()).unwrap();
        let document = checked_save_document(&world).unwrap();
        let mut expected = serde_json::to_vec(&document).unwrap();
        expected.push(b'\n');
        let limit = expected.len();
        assert!(matches!(
            write_bounded_save(&document, io::sink(), limit, SaveLayout::Pretty),
            Err(StorageError::TooLarge)
        ));
        assert_eq!(
            prefer_pretty_save(|layout| write_bounded_save(&document, Vec::new(), limit, layout))
                .unwrap(),
            expected
        );
        let (prepared, version) = prepare_world_file_bounded(&path, &document, limit).unwrap();
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        assert_eq!(fs::read(prepared.path()).unwrap(), expected);
        assert_eq!(version, bytes_version(&expected));
        drop(prepared);
        assert!(matches!(
            prepare_world_file_bounded(&path, &document, limit - 1),
            Err(StorageError::TooLarge)
        ));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn exactly_16_mib_compact_save_reopens_and_keeps_exact_backups_and_cas() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capacity.json");
        let mut world = World::default();
        world.add_player("x".into()).unwrap();
        let document = checked_save_document(&world).unwrap();
        let padding = MAX_SAVE_BYTES - serde_json::to_vec(&document).unwrap().len() - 1;
        let mut draft = world.snapshot().clone();
        draft.players[0].name.push_str(&"x".repeat(padding));
        world.replace(draft).unwrap();
        assert!(matches!(
            write_bounded_save(
                &save_document(&world),
                io::sink(),
                MAX_SAVE_BYTES,
                SaveLayout::Pretty
            ),
            Err(StorageError::TooLarge)
        ));
        ensure_world_saveable(&world).unwrap();
        let canonical = encode(&world).unwrap();
        assert_eq!(canonical.len(), MAX_SAVE_BYTES);
        assert_eq!(canonical.last(), Some(&b'\n'));
        let (_, initial) = create_world_if_version(&path, &world, &FileVersion::Missing).unwrap();
        assert_eq!(initial, bytes_version(&canonical));
        assert_eq!(fs::read(&path).unwrap(), canonical);
        assert_eq!(
            load_world_with_version(&path).unwrap(),
            (world.clone(), initial.clone())
        );

        let mut draft = world.snapshot().clone();
        draft.players[0].name.replace_range(..1, "y");
        world.replace(draft).unwrap();
        let (_, updated) = save_world_if_version(&path, &world, &initial).unwrap();
        let backup = backup_path(&path);
        assert_eq!(fs::read(&backup).unwrap(), canonical);
        assert_eq!(
            load_world(&backup).unwrap().snapshot().players[0]
                .name
                .as_bytes()[0],
            b'x'
        );
        assert_eq!(file_version(&path).unwrap(), updated);
        assert_ne!(updated, initial);
        assert!(matches!(
            save_world_if_version(&path, &world, &initial),
            Err(StorageError::FileChanged)
        ));
        let sibling_count = fs::read_dir(dir.path()).unwrap().count();
        let mut draft = world.snapshot().clone();
        draft.players[0].name.push('z');
        world.replace(draft).unwrap();
        assert!(matches!(
            ensure_world_saveable(&world),
            Err(StorageError::TooLarge)
        ));
        assert!(matches!(encode(&world), Err(StorageError::TooLarge)));
        assert!(matches!(
            save_world_if_version(&path, &world, &updated),
            Err(StorageError::TooLarge)
        ));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), sibling_count);
        assert_eq!(file_version(&path).unwrap(), updated);
        assert_eq!(fs::read(&backup).unwrap(), canonical);
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
        let mut attempts = 0;
        assert!(matches!(
            prefer_pretty_save(|layout| {
                attempts += 1;
                write_bounded_save(&document, Broken, MAX_SAVE_BYTES, layout)
            }),
            Err(StorageError::Io(ref error)) if error.kind()==io::ErrorKind::PermissionDenied
        ));
        assert_eq!(attempts, 1);
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
    save_slot_path, ArchivedSaveSlot, SaveSlot,
};
