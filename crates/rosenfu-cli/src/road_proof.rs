use rosenfu_core::ofm_evidence::raw_sha256;
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    error::Error,
    fs::{self, File},
    io::Read,
    path::Path,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const MANIFEST_LIMIT: usize = 256 * 1024;

fn read_local(path: &Path, max: usize) -> Result<Vec<u8>> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.len() > max as u64 {
        return Err("input must be a bounded regular file".into());
    }
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err("input must be a bounded regular file".into());
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    file.take((max + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > max {
        return Err("input exceeds its byte limit".into());
    }
    Ok(bytes)
}

fn route_point_tile(latitude: f64, longitude: f64) -> Option<(u32, u32)> {
    const GRID: f64 = 16384.0;
    if !(-85.051_128_78..85.051_128_78).contains(&latitude) || !(-180.0..180.0).contains(&longitude)
    {
        return None;
    }
    let x = (longitude + 180.0) * GRID / 360.0;
    let y = (1.0 - latitude.to_radians().tan().asinh() / std::f64::consts::PI) * GRID / 2.0;
    if !(0.0..GRID).contains(&x) || !(0.0..GRID).contains(&y) {
        return None;
    }
    Some((x.floor() as u32, y.floor() as u32))
}

fn checked(approved: &Value, save: &[u8]) -> Result<Value> {
    let obj = approved.as_object().ok_or("invalid approved manifest")?;
    if obj.len() != 6 || approved["format"] != "rosenfu-road-path-scope-v1" {
        return Err("invalid approved manifest format".into());
    }
    let provenance = match approved["origin"].as_str() {
        Some("captured") => "captured (as declared by the local operator)",
        Some("synthetic") => "synthetic geometry only",
        _ => return Err("manifest must explicitly declare captured or synthetic origin".into()),
    };
    let actual_save_digest = raw_sha256(save);
    if approved["save_sha256"].as_str() != Some(actual_save_digest.as_str()) {
        return Err("save digest differs from the approved capture".into());
    }
    let id = approved["alignment_id"]
        .as_str()
        .filter(|id| {
            id.parse::<u64>()
                .is_ok_and(|n| n > 0 && n.to_string() == **id)
        })
        .ok_or("manifest alignment_id must be a canonical positive decimal string")?;
    // This calls the production storage reader on exactly the bytes we hashed.
    // It rehydrates and re-verifies bound raw bytes, decoder identities, scope
    // and retained geometry; the manifest never grants new source authority.
    let world = rosenfu_storage::decode(save)?;
    let draft = world.snapshot();
    let alignment = draft
        .alignments
        .iter()
        .find(|v| v.id.0.to_string() == id)
        .ok_or("selected alignment is absent")?;
    let mut spans: Vec<_> = draft
        .road_path_provenance
        .iter()
        .filter(|p| p.alignment_id == alignment.id)
        .collect();
    if spans.iter().any(|p| p.path.carries_street_tram_proof()) {
        return Err("street-tram geometry is not a multi-cell road proof".into());
    }
    spans.sort_unstable_by_key(|p| p.start_point_index);
    if spans.first().is_none_or(|p| p.start_point_index != 0)
        || spans
            .last()
            .is_none_or(|p| p.end_point_index.checked_add(1) != Some(alignment.points.len()))
        || spans
            .windows(2)
            .any(|w| w[0].end_point_index != w[1].start_point_index)
    {
        return Err("selected alignment is not fully covered by contiguous proved spans".into());
    }

    let actual_spans: Vec<_> = spans
        .iter()
        .map(serde_json::to_value)
        .collect::<std::result::Result<_, _>>()?;
    if approved["spans"].as_array() != Some(&actual_spans) {
        return Err("ordered spans differ from approved scope".into());
    }
    let mut seen = BTreeSet::new();
    let mut ordered_tiles = Vec::new();
    let mut coords = BTreeSet::new();
    for span in &actual_spans {
        let keys = span["path"]["requiredBindingKeys"]
            .as_array()
            .ok_or("missing required keys")?;
        for key in keys {
            let key = key.as_str().ok_or("invalid required key")?;
            if !seen.insert(key) {
                continue;
            }
            let found = draft
                .road_evidence_descriptors
                .iter()
                .find(|d| d.binding.canonical_url == key)
                .ok_or("approved path tile is absent")?;
            // Native verifier already checks canonical URL, revision and
            // raw/canonical SHA; include exact known metadata for comparison.
            coords.insert((found.binding.x, found.binding.y));
            ordered_tiles.push(serde_json::to_value(found)?);
        }
    }
    let point_tiles = alignment
        .points
        .iter()
        .map(|p| route_point_tile(p.latitude, p.longitude))
        .collect::<Option<BTreeSet<_>>>()
        .ok_or("selected alignment leaves valid z14 Web Mercator coordinates")?;
    if point_tiles.len() < 2 || !point_tiles.is_subset(&coords) {
        return Err("selected alignment does not cross at least two bound z14 cells".into());
    }
    if approved["tiles"].as_array() != Some(&ordered_tiles) {
        return Err("ordered tile bindings/digests differ from approved scope".into());
    }
    let ordered_spans = actual_spans
        .iter()
        .map(|span| {
            let bytes = serde_json::to_vec(span)?;
            Ok(json!({
                "start_point_index": span["start_point_index"],
                "end_point_index": span["end_point_index"],
                "binding_count": span["path"]["requiredBindingKeys"].as_array().map(Vec::len),
                "span_sha256": raw_sha256(&bytes),
            }))
        })
        .collect::<std::result::Result<Vec<Value>, serde_json::Error>>()?;
    Ok(json!({
        "format":"rosenfu-road-path-receipt-v1",
        "origin":provenance,
        "native_verified":true,
        "coverage_or_access_certified":false,
        "alignment_id":id,
        "save_sha256":actual_save_digest,
        "points":alignment.points.len(),
        "bound_tiles":coords.len(),
        "tiles_containing_route_points":point_tiles.len(),
        "ordered_tile_metadata_sha256":raw_sha256(&serde_json::to_vec(&ordered_tiles)?),
        "spans":ordered_spans,
    }))
}

pub(super) fn verify(approved: &Path, expected_sha256: &str, capture: &Path) -> Result<()> {
    let approved = read_local(approved, MANIFEST_LIMIT)?;
    let manifest_digest = raw_sha256(&approved);
    if expected_sha256 != manifest_digest {
        return Err("manifest digest differs from approval".into());
    }
    let manifest: Value = serde_json::from_slice(&approved)?;
    let save = read_local(capture, rosenfu_storage::MAX_SAVE_BYTES)?;
    let mut receipt = checked(&manifest, &save)?;
    receipt["approved_manifest_sha256"] = json!(manifest_digest);
    println!("{}", serde_json::to_string(&receipt)?);
    Ok(())
}
