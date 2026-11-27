//! Offline draft generation only. No authority comes from this command's output.
use super::road_proof::read_local;
use rosenfu_core::ofm_evidence::{
    raw_sha256, replay_westbound_seam_traversal, EvidenceStore, RawEvidence, RawValue, TileBinding,
    WestboundSeamReceipt, MAX_TILE_BYTES, WESTBOUND_SEAM_WITNESS_RULE,
};
use serde::Deserialize;
use std::{collections::BTreeMap, error::Error, fs::OpenOptions, io::Write, path::Path};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const APPROVAL_FORMAT: &str = "rosenfu-seam-archive-approval-v1";
const ID_RULE: &str = "planetiler_openmaptiles_osm_way_10_plus_2_v1";
const EXTENT: f64 = 4096.;
const Z14_GRID: f64 = 16384.;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Approval {
    format: String,
    producer_id_rule: String,
    tiles_west_east: [TileBinding; 2],
    osm_way_id: u64,
    osm_way_version: u32,
    reviewed_way_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewedWay {
    id: u64,
    version: u32,
    points: Vec<[f64; 2]>,
    tags: BTreeMap<String, String>,
}

fn source_cut(way: &ReviewedWay, east: &TileBinding) -> Result<f64> {
    if way.points.len() < 2 || way.points.len() > 20_000 {
        return Err("source way has an unsupported number of points".into());
    }
    let mut points = Vec::with_capacity(way.points.len());
    for &[lon, lat] in &way.points {
        if !(-180.0..180.0).contains(&lon) || !(-85.05112878..85.05112878).contains(&lat) {
            return Err("source point outside supported coordinates".into());
        }
        let x = (lon + 180.) * Z14_GRID / 360.;
        let y = (1. - lat.to_radians().tan().asinh() / std::f64::consts::PI) * Z14_GRID / 2.;
        points.push([
            (x - f64::from(east.x)) * EXTENT,
            (y - f64::from(east.y)) * EXTENT,
        ]);
    }
    if !points.windows(2).all(|pair| pair[0][0] > pair[1][0]) {
        return Err("source way must be strictly westbound".into());
    }
    let mut cut = None;
    for pair in points.windows(2) {
        let ([x0, y0], [x1, y1]) = (pair[0], pair[1]);
        if x0 >= 0. && x1 < 0. {
            let y = y0 + (y1 - y0) * x0 / (x0 - x1);
            if !(0. ..EXTENT).contains(&y) || cut.replace(y).is_some() {
                return Err("source way does not cross the approved row unambiguously".into());
            }
        }
    }
    cut.ok_or_else(|| "source way does not cross the approved tile boundary".into())
}

fn derive(approval: Approval, raw: [Vec<u8>; 2], source: &[u8]) -> Result<WestboundSeamReceipt> {
    let [west, east] = &approval.tiles_west_east;
    west.validate()?;
    east.validate()?;
    if approval.format != APPROVAL_FORMAT
        || approval.producer_id_rule != ID_RULE
        || east.x != west.x + 1
        || west.y != east.y
        || west.revision != east.revision
        || raw_sha256(source) != approval.reviewed_way_sha256
    {
        return Err("archive approval scope does not match these inputs".into());
    }
    let way: ReviewedWay = serde_json::from_slice(source)?;
    if way.id == 0
        || way.id != approval.osm_way_id
        || way.version == 0
        || way.version != approval.osm_way_version
        || way.tags.get("oneway").map(String::as_str) != Some("yes")
        || way.tags.keys().any(|tag| {
            let key = tag.split(':').next().unwrap_or_default();
            [
                "access",
                "vehicle",
                "motor_vehicle",
                "motorcar",
                "bus",
                "psv",
                "bridge",
                "tunnel",
                "layer",
                "ford",
                "junction",
            ]
            .contains(&key)
        })
    {
        return Err("source way identity, one-way direction or rights/grade is unsupported".into());
    }
    let source_highway = way
        .tags
        .get("highway")
        .ok_or("source way has no highway class")?;
    let source_y = source_cut(&way, east)?;
    let raw_feature_id = way
        .id
        .checked_mul(10)
        .and_then(|id| id.checked_add(2))
        .ok_or("approved source way ID overflows its producer mapping")?;
    let [raw_west, raw_east] = raw;
    let store = EvidenceStore::from_archive_parts(
        [raw_west, raw_east]
            .into_iter()
            .zip([west, east])
            .map(|(bytes, binding)| RawEvidence {
                sha256: binding.raw_sha256.clone(),
                bytes,
            })
            .collect(),
        vec![west.clone(), east.clone()],
    )?
    .scope(&[west.canonical_url.clone(), east.canonical_url.clone()])?;
    let vertex = |binding: &TileBinding, offset: i32| -> Result<u32> {
        let tile = store.decoded(&binding.canonical_url)?;
        let line = tile
            .unique_raw_road_line(raw_feature_id)
            .ok_or("expected unique typed road ID is absent")?;
        if line.attributes.get("class") != Some(&RawValue::String(source_highway.clone())) {
            return Err("tile class differs from reviewed source class".into());
        }
        let mut candidates = line
            .coordinates
            .windows(2)
            .enumerate()
            .filter(|(_, p)| p[0][0] > offset && p[1][0] < offset);
        let (idx, pair) = candidates
            .next()
            .ok_or("approved way does not cross tile")?;
        if candidates.next().is_some() {
            return Err("ambiguous seam crossing".into());
        }
        let ([x0, y0], [x1, y1]) = (pair[0], pair[1]);
        let y = f64::from(y0) + f64::from(y1 - y0) * f64::from(x0 - offset) / f64::from(x0 - x1);
        // Source-snapshot consistency check, not a snapping tolerance. Native
        // replay below still requires an exact shared atom and a 0.1-grid cut.
        if (y - source_y).abs() > 2. {
            return Err("archive cut differs from independently reviewed source way".into());
        }
        Ok(u32::try_from(idx)?)
    };
    let vertices_west_east = [vertex(west, 4096)?, vertex(east, 0)?];
    let receipt = WestboundSeamReceipt {
        rule: WESTBOUND_SEAM_WITNESS_RULE.into(),
        tiles_west_east: approval.tiles_west_east,
        raw_feature_id,
        vertices_west_east,
    };
    let bytes = serde_json::to_vec(&receipt)?;
    // This supplies a self-hash only to exercise the complete native geometry
    // verifier; it does not authenticate or install the authoring result.
    replay_westbound_seam_traversal(&store, &bytes, &raw_sha256(&bytes))?;
    Ok(receipt)
}

pub(super) fn prepare(
    approved: &Path,
    expected_sha256: &str,
    tiles: [&Path; 2],
    source: &Path,
    output: &Path,
) -> Result<()> {
    let approved = read_local(approved, 16 * 1024)?;
    if raw_sha256(&approved) != expected_sha256 {
        return Err("manifest digest differs from independent approval".into());
    }
    let approval = serde_json::from_slice(&approved)?;
    let source = read_local(source, 1024 * 1024)?;
    let raw = [
        read_local(tiles[0], MAX_TILE_BYTES)?,
        read_local(tiles[1], MAX_TILE_BYTES)?,
    ];
    let receipt = derive(approval, raw, &source)?;
    let mut contents = serde_json::to_vec(&serde_json::json!({"version":1,"receipts":[receipt]}))?;
    contents.push(b'\n');
    // Never overwrite a previously submitted candidate or the tracked catalog.
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(output)?.write_all(&contents)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    fn words(values: &[u32]) -> Vec<u8> {
        let mut out = Vec::new();
        for &value in values {
            let mut n = value;
            while n >= 128 {
                out.push((n as u8 & 0x7f) | 0x80);
                n >>= 7;
            }
            out.push(n as u8);
        }
        out
    }

    fn field(n: u32, body: &[u8]) -> Vec<u8> {
        [words(&[n * 8 + 2, body.len() as u32]), body.to_vec()].concat()
    }

    fn tile(points: &[[i32; 2]]) -> Vec<u8> {
        let zig = |n: i32| ((n << 1) ^ (n >> 31)) as u32;
        let [x, y] = points[0];
        let mut geom = vec![9, zig(x), zig(y), 8 * (points.len() as u32 - 1) + 2];
        for pair in points.windows(2) {
            geom.extend([zig(pair[1][0] - pair[0][0]), zig(pair[1][1] - pair[0][1])]);
        }
        let feature = [
            words(&[8, 22, 24, 2]),
            field(2, &words(&[0, 0, 1, 1])),
            field(4, &words(&geom)),
        ]
        .concat();
        field(
            3,
            &[
                words(&[120, 2, 40, 4096]),
                field(1, b"transportation"),
                field(2, &feature),
                field(3, b"class"),
                field(3, b"oneway"),
                field(4, &field(1, b"secondary")),
                field(4, &words(&[40, 1])),
            ]
            .concat(),
        )
    }

    #[test]
    fn offline_candidate_is_repeatable_and_refuses_mismatched_inputs_or_output() {
        let [west, east] = [
            tile(&[[4176, 1800], [4156, 1790], [3856, 1640]]),
            tile(&[[180, 1850], [80, 1800], [60, 1790], [-1385, 1067]]),
        ];
        let dir = tempfile::tempdir().unwrap();
        let approved = dir.path().join("approved.json");
        let source_file = dir.path().join("way.json");
        let [w, e] = ["west", "east"].map(|p| dir.path().join(p));
        let [draft, repeat, rejected] =
            ["draft.json", "repeat.json", "rejected.json"].map(|p| dir.path().join(p));
        fs::write(&w, &west).unwrap();
        fs::write(&e, &east).unwrap();
        let points = [[180., 1850.], [60., 1790.], [-240., 1640.]].map(|[x, y]| {
            let lon = (14553. + x / EXTENT) * 360. / Z14_GRID - 180.;
            let lat = (std::f64::consts::PI * (1. - 2. * (6451. + y / EXTENT) / Z14_GRID))
                .sinh()
                .atan()
                .to_degrees();
            [lon, lat]
        });
        let mut source = json!({"id":2,"version":1,"points":points,"tags":{"highway":"secondary","oneway":"yes"}});
        let binding = |x, raw| TileBinding {
            canonical_url: format!(
                "https://tiles.openfreemap.org/planet/qa_archived/14/{x}/6451.pbf"
            ),
            revision: "qa_archived".into(),
            z: 14,
            x,
            y: 6451,
            raw_sha256: raw_sha256(raw),
            decoder_rule: rosenfu_core::ofm_evidence::DECODER_RULE.into(),
        };
        let mut manifest = json!({
            "format":APPROVAL_FORMAT,"producer_id_rule":ID_RULE,
            "tiles_west_east":[binding(14552,&west), binding(14553,&east)],
            "osm_way_id":2,"osm_way_version":1,"reviewed_way_sha256":""
        });
        let update = |manifest: &mut serde_json::Value, source: &serde_json::Value| {
            let bytes = serde_json::to_vec(source).unwrap();
            fs::write(&source_file, &bytes).unwrap();
            manifest["reviewed_way_sha256"] = raw_sha256(&bytes).into();
            let bytes = serde_json::to_vec(manifest).unwrap();
            fs::write(&approved, &bytes).unwrap();
            raw_sha256(&bytes)
        };
        let digest = update(&mut manifest, &source);
        let call = |digest, dest| prepare(&approved, digest, [&w, &e], &source_file, dest);
        call(&digest, &draft).unwrap();
        call(&digest, &repeat).unwrap();
        let initial = fs::read(&draft).unwrap();
        assert_eq!(initial, fs::read(&repeat).unwrap());
        let output: serde_json::Value = serde_json::from_slice(&initial).unwrap();
        assert_eq!(output["receipts"][0]["verticesWestEast"], json!([1, 2]));
        assert_eq!(output["receipts"][0]["rawFeatureId"], 22);
        assert!(call(&digest, &draft).is_err());
        assert_eq!(initial, fs::read(&draft).unwrap());
        assert!(call("not independently approved", &rejected).is_err());

        // Today's bytes at a dated alias can even be a valid MVT. They are
        // still not the producer archive pinned by the approval.
        fs::write(&w, tile(&[[4176, 1801], [4156, 1791], [3856, 1641]])).unwrap();
        assert!(call(&digest, &rejected).is_err());
        fs::write(&w, &west).unwrap();

        fs::write(&source_file, b"changed snapshot").unwrap();
        assert!(call(&digest, &rejected).is_err());
        source["points"] = json!([points[2], points[1], points[0]]);
        let digest = update(&mut manifest, &source);
        assert!(call(&digest, &rejected).is_err());
        source["points"] = json!(points);
        source["tags"]["bus:conditional"] = "no".into();
        let digest = update(&mut manifest, &source);
        assert!(call(&digest, &rejected).is_err());
        assert!(!rejected.exists());
    }
}
