//! Streaming inspection of the declared road-evidence manifest. It is never
//! used to approve construction, open a world, or adopt a recovery file.
use crate::{
    known_combined_profile, regular_file_exists, save_reader_error, StorageError,
    ADJACENT_TRAM_SCHEMA, ADJACENT_TRAM_STANDARD_SCHEMA, COMBINED_CAPABILITIES_SCHEMA,
    CONNECTED_TERMINAL_NECK_SCHEMA, FORMAT, FRACTIONAL_BUS_SCHEMA, MAX_COMBINED_PROFILE_NAMES,
    MAX_SAVE_BYTES, MIXED_STANDARD_BUS_SCHEMA, SCHEMA_VERSION, STREET_TRAM_SCHEMA,
    STREET_TRAM_STANDARD_SCHEMA,
};
use rosenfu_core::ofm_evidence::{TileBinding, MAX_PINNED_BINDINGS, MAX_TILES};
use rosenfu_core::world::WorldError;
use serde::{
    de::{self, IgnoredAny, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use std::{
    collections::BTreeSet,
    fmt,
    fs::File,
    io::{BufReader, Read},
    marker::PhantomData,
    path::Path,
};

/// Inclusive z14 tile-coordinate rectangle; query work is proportional to
/// manifest entries, not the number of coordinates in the rectangle.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct TileBounds {
    min_x: u32,
    min_y: u32,
    max_x: u32,
    max_y: u32,
}

impl TileBounds {
    pub fn new(min_x: u32, min_y: u32, max_x: u32, max_y: u32) -> Option<Self> {
        (min_x <= max_x && min_y <= max_y && max_x < 16384 && max_y < 16384).then_some(Self {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }

    fn includes(self, tile: &TileBinding) -> bool {
        (self.min_x..=self.max_x).contains(&tile.x) && (self.min_y..=self.max_y).contains(&tile.y)
    }
}

/// No raw-blob or world validation has been performed. Matching declarations
/// do not prove real geography, missing declarations do not prove emptiness.
#[derive(Debug, Serialize)]
pub struct RoadCoverage {
    status: &'static str,
    pub schema_version: u64,
    pub zoom: u8,
    pub bounds: TileBounds,
    pub total_declared_tiles: usize,
    pub matching_tiles: Vec<TileBinding>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveIndex {
    format: String,
    schema_version: u64,
    #[serde(default, deserialize_with = "read_capabilities")]
    capabilities: Option<Vec<String>>,
    #[serde(rename = "world")]
    _world: IgnoredAny,
    road_evidence: Object<EvidenceIndex>,
    #[serde(default, rename = "operation")]
    _operation: IgnoredAny,
    #[serde(default, rename = "fixed_od_scenario")]
    _fixed_od_scenario: IgnoredAny,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceIndex {
    #[serde(rename = "raw_blobs")]
    _raw_blobs: IgnoredAny,
    bindings: Bindings,
}

// Restrict discovery as well as retained items. The diagnostic never needs
// to allocate an arbitrary-size capability list or inspect runtime state.
fn read_capabilities<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error> {
    struct Profile;
    impl<'de> Visitor<'de> for Profile {
        type Value = Option<Vec<String>>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "a declared capability list of at most {MAX_COMBINED_PROFILE_NAMES} strings"
            )
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut names = Vec::with_capacity(MAX_COMBINED_PROFILE_NAMES);
            for _ in 0..MAX_COMBINED_PROFILE_NAMES {
                let Some(name) = seq.next_element::<String>()? else {
                    return Ok(Some(names));
                };
                names.push(name);
            }
            if seq.next_element::<IgnoredAny>()?.is_some() {
                return Err(de::Error::custom("too many save capabilities"));
            }
            Ok(Some(names))
        }
    }

    deserializer.deserialize_seq(Profile)
}

// A derived struct also accepts a positional sequence, which is not part of
// this format. Require objects for the enclosing record and manifest entries.
struct Object<T>(T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectVisitor<T> {
            type Value = Object<T>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a save manifest object")
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                T::deserialize(de::value::MapAccessDeserializer::new(map)).map(Object)
            }
        }
        deserializer.deserialize_map(ObjectVisitor(PhantomData))
    }
}

struct Bindings(Vec<TileBinding>);

impl<'de> Deserialize<'de> for Bindings {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct BindingList;
        impl<'de> Visitor<'de> for BindingList {
            type Value = Bindings;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a bounded retained road tile binding list")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut rows = Vec::new();
                for _ in 0..MAX_PINNED_BINDINGS {
                    let Some(Object(row)) = seq.next_element::<Object<TileBinding>>()? else {
                        return Ok(Bindings(rows));
                    };
                    rows.push(row);
                }
                if seq.next_element::<IgnoredAny>()?.is_some() {
                    return Err(de::Error::custom("too many retained road tile bindings"));
                }
                Ok(Bindings(rows))
            }
        }
        deserializer.deserialize_seq(BindingList)
    }
}

/// Scan the bounded JSON input without allocating world geometry or decoding
/// tile blobs. Never use this manifest-only response instead of load_world.
pub fn read_road_coverage(
    path: impl AsRef<Path>,
    bounds: TileBounds,
) -> Result<RoadCoverage, StorageError> {
    let path = path.as_ref();
    regular_file_exists(path)?;
    let file = File::open(path)?;
    let limit = MAX_SAVE_BYTES as u64;
    if file.metadata()?.len() > limit {
        return Err(StorageError::TooLarge);
    }
    let mut reader = BufReader::new(file.take(limit + 1));
    let index = serde_json::from_reader::<_, Object<SaveIndex>>(&mut reader);
    if reader.get_ref().limit() == 0 {
        return Err(StorageError::TooLarge);
    }
    let Object(index) = index.map_err(save_reader_error)?;
    if index.format != FORMAT {
        return Err(StorageError::InvalidFormat);
    }
    let version = index.schema_version;
    // All of these envelopes encode the same evidence manifest. A CLI built
    // without periodic rail can inspect it, not load or edit those worlds.
    if !matches!(
        version,
        26 | SCHEMA_VERSION
            | MIXED_STANDARD_BUS_SCHEMA
            | STREET_TRAM_SCHEMA
            | STREET_TRAM_STANDARD_SCHEMA
            | ADJACENT_TRAM_SCHEMA
            | ADJACENT_TRAM_STANDARD_SCHEMA
            | CONNECTED_TERMINAL_NECK_SCHEMA
            | FRACTIONAL_BUS_SCHEMA
            | COMBINED_CAPABILITIES_SCHEMA
    ) {
        return Err(StorageError::UnsupportedVersion(version));
    }
    let known_header = match (version, index.capabilities.as_deref()) {
        (COMBINED_CAPABILITIES_SCHEMA, Some(declared)) => known_combined_profile(declared),
        (COMBINED_CAPABILITIES_SCHEMA, None) | (_, Some(_)) => false,
        (_, None) => true,
    };
    if !known_header {
        return Err(StorageError::UnsupportedVersion(
            COMBINED_CAPABILITIES_SCHEMA,
        ));
    }
    let rows = index.road_evidence.0.bindings.0;
    let invalid = || StorageError::InvalidWorld(WorldError::InvalidRoadEvidence);
    if version == 26 && rows.len() > MAX_TILES {
        return Err(invalid());
    }
    let mut seen = BTreeSet::new();
    let revision = rows.first().map(|b| b.revision.as_str());
    for row in &rows {
        row.validate().map_err(|_| invalid())?;
        if revision != Some(row.revision.as_str()) || !seen.insert((row.x, row.y)) {
            return Err(invalid());
        }
    }
    let total_declared_tiles = rows.len();
    let mut matching_tiles: Vec<_> = rows
        .into_iter()
        .filter(|row| bounds.includes(row))
        .collect();
    matching_tiles.sort_unstable_by_key(|b| (b.x, b.y));
    Ok(RoadCoverage {
        status: "manifest_only_not_validated",
        schema_version: version,
        zoom: 14,
        bounds,
        total_declared_tiles,
        matching_tiles,
    })
}
