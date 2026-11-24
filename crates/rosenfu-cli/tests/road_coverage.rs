use rosenfu_core::{
    ofm_evidence::{raw_sha256, EvidenceStore, RawEvidence, TileBinding, DECODER_RULE},
    world::{World, WorldDraft},
};
use serde_json::Value;
use std::{fs, path::Path, process::Command};

fn run(path: &Path, bounds: [&str; 4]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rosenfu-cli"))
        .args(["world", "road-coverage"])
        .arg(path)
        .args(bounds)
        .output()
        .unwrap()
}

fn fixture() -> World {
    // Minimal MVT with a valid empty transportation layer, retained in two
    // far-apart regions plus a second tile on the query edge.
    let mut bytes = vec![26, 21, 10, 14];
    bytes.extend(b"transportation");
    bytes.extend([120, 2, 40, 128, 32]);
    let sha = raw_sha256(&bytes);
    let tiles = [(14624, 6016), (14553, 6452), (14552, 6451)]
        .into_iter()
        .map(|(x, y)| TileBinding {
            canonical_url: format!("https://tiles.openfreemap.org/planet/test/14/{x}/{y}.pbf"),
            revision: "test".into(),
            z: 14,
            x,
            y,
            raw_sha256: sha.clone(),
            decoder_rule: DECODER_RULE.into(),
        })
        .collect();
    let evidence =
        EvidenceStore::from_archive_parts(vec![RawEvidence { sha256: sha, bytes }], tiles).unwrap();
    World::new(WorldDraft::default().with_road_evidence(evidence).unwrap()).unwrap()
}

#[test]
fn cli_reads_only_selected_manifest_without_opening_or_writing_world() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nationwide fixture.json");
    rosenfu_storage::save_world(&path, &fixture()).unwrap();
    let original = fs::read(&path).unwrap();
    let version = rosenfu_storage::file_version(&path).unwrap();
    let local = ["14552", "6451", "14553", "6452"];
    let report = run(&path, local);
    assert!(report.status.success(), "{:?}", report.stderr);
    let report: Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(report["status"], "manifest_only_not_validated");
    assert_eq!(report["total_declared_tiles"], 3);
    let selected = report["matching_tiles"].as_array().unwrap();
    assert_eq!(selected.len(), 2);
    assert_eq!(selected[0]["x"], 14552);
    assert_eq!(selected[1]["x"], 14553);
    assert_eq!(selected[1]["y"], 6452);
    assert!(report.get("world").is_none());
    assert!(report.get("raw_blobs").is_none());
    let empty = run(&path, ["0", "0", "0", "0"]);
    assert!(empty.status.success());
    let empty: Value = serde_json::from_slice(&empty.stdout).unwrap();
    assert_eq!(empty["matching_tiles"].as_array().unwrap().len(), 0);
    assert_eq!(empty["total_declared_tiles"], 3);
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(rosenfu_storage::file_version(&path).unwrap(), version);
    assert!(!rosenfu_storage::backup_path(&path).exists());

    // The inspection must not inadvertently grow an authoritative world or
    // validate/recompute raw blobs. The normal entry point still refuses this.
    let mut wire: Value = serde_json::from_slice(&original).unwrap();
    wire["road_evidence"]["raw_blobs"][0]["base64"] = "not a tile".into();
    fs::write(&path, serde_json::to_vec(&wire).unwrap()).unwrap();
    assert!(rosenfu_storage::load_world(&path).is_err());
    let light = run(&path, local);
    assert!(light.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&light.stdout).unwrap(),
        report
    );
}

#[test]
fn cli_rejects_bad_bounds_and_inconsistent_even_out_of_range_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bounded.json");
    let mut wire: Value =
        serde_json::from_slice(&rosenfu_storage::encode(&fixture()).unwrap()).unwrap();
    let bounded = ["14552", "6451", "14553", "6452"];
    let write = |wire: &Value| fs::write(&path, serde_json::to_vec(wire).unwrap()).unwrap();
    let rejects = |bounds| {
        let output = run(&path, bounds);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    };
    write(&wire);
    rejects(["14553", "6452", "14552", "6451"]);
    rejects(["0", "0", "16384", "5"]);

    let rows = wire["road_evidence"]["bindings"].as_array_mut().unwrap();
    let remote = rows.iter().position(|row| row["x"] == 14624).unwrap();
    let mut mixed = rows[remote].clone();
    mixed["revision"] = "other".into();
    mixed["canonical_url"] = "https://tiles.openfreemap.org/planet/other/14/14624/6016.pbf".into();
    rows.push(mixed);
    write(&wire);
    rejects(bounded);
    let rows = wire["road_evidence"]["bindings"].as_array_mut().unwrap();
    rows.pop();
    let duplicate = rows[remote].clone();
    rows.push(duplicate);
    write(&wire);
    rejects(bounded);

    let valid_row = wire["road_evidence"]["bindings"][0].clone();
    wire["road_evidence"]["bindings"] = Value::Array(vec![valid_row; 257]);
    write(&wire);
    rejects(bounded);
    let file = fs::File::options().write(true).open(&path).unwrap();
    file.set_len(rosenfu_storage::MAX_SAVE_BYTES as u64 + 1)
        .unwrap();
    rejects(bounded);
}

#[test]
fn cli_inspects_known_envelopes_without_admitting_unknown_schemas() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("manifest-envelopes.rosenfu");
    let mut header: Value =
        serde_json::from_slice(&rosenfu_storage::encode(&fixture()).unwrap()).unwrap();
    // Only the evidence envelope is in scope. In particular, these relabeled
    // fixtures do not claim to be valid playable 28–34 saves.
    for version in [26, 27, 28, 29, 30, 31, 32, 33, 34] {
        header["schema_version"] = version.into();
        let bytes = serde_json::to_vec(&header).unwrap();
        fs::write(&path, &bytes).unwrap();
        let output = run(&path, ["14552", "6451", "14552", "6451"]);
        assert!(
            output.status.success(),
            "schema {version}: {:?}",
            output.stderr
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["schema_version"], version);
        assert_eq!(report["status"], "manifest_only_not_validated");
        assert_eq!(report["matching_tiles"].as_array().unwrap().len(), 1);
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    for version in [25, 36] {
        header["schema_version"] = version.into();
        fs::write(&path, serde_json::to_vec(&header).unwrap()).unwrap();
        let output = run(&path, ["14552", "6451", "14552", "6451"]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn cli_requires_a_canonical_known_combined_profile_without_claiming_world_validation() {
    use serde_json::json;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("capability-manifest.rosenfu");
    let mut header: Value =
        serde_json::from_slice(&rosenfu_storage::encode(&fixture()).unwrap()).unwrap();
    let bounds = ["14552", "6451", "14552", "6451"];
    let inspect = |wire: &[u8]| {
        fs::write(&path, wire).unwrap();
        let output = run(&path, bounds);
        assert_eq!(fs::read(&path).unwrap(), wire);
        assert!(!rosenfu_storage::backup_path(&path).exists());
        output
    };
    header["schema_version"] = 35.into();
    for profile in [
        json!(["classic_bus_run_v1", "rail_terminal_neck_v1"]),
        json!(["fractional_bus_same_ground_atom_v1", "street_tram_single_cell_v1"]),
        json!(["street_tram_chain_v1"]),
        json!(["fractional_bus_ground_route_v1"]),
        json!([
            "fractional_bus_ground_route_v1",
            "street_tram_adjacent_cells_v1"
        ]),
        json!([
            "fractional_bus_ground_route_v1",
            "fractional_bus_ground_stops_v1",
            "street_tram_chain_v1"
        ]),
    ] {
        header["capabilities"] = profile;
        let bytes = serde_json::to_vec(&header).unwrap();
        let output = inspect(&bytes);
        assert!(output.status.success(), "{:?}", output.stderr);
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["schema_version"], 35);
        assert_eq!(report["status"], "manifest_only_not_validated");
        assert!(report.get("capabilities").is_none());
        assert!(rosenfu_storage::load_world(&path).is_err());
    }

    for profile in [
        Value::Null,
        json!([]),
        json!([1]),
        json!({"profile": "street_tram_chain_v1"}),
        json!(["unsupported_v1"]),
        json!(["street_tram_chain_v1", "street_tram_chain_v1"]),
        json!(["rail_terminal_neck_v1", "classic_bus_run_v1"]),
        json!(["street_tram_chain_v1", "classic_bus_run_v1", "rail_terminal_neck_v1"]),
        json!(["fractional_bus_ground_stops_v1", "fractional_bus_ground_route_v1"]),
        json!([
            "fractional_bus_ground_route_v1",
            "fractional_bus_ground_stops_v1",
            "street_tram_single_cell_v1",
            "street_tram_chain_v1"
        ]),
    ] {
        header["capabilities"] = profile;
        let output = inspect(&serde_json::to_vec(&header).unwrap());
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    header.as_object_mut().unwrap().remove("capabilities");
    assert!(!inspect(&serde_json::to_vec(&header).unwrap()).status.success());

    header["capabilities"] = json!(["street_tram_chain_v1"]);
    let encoded = serde_json::to_string(&header).unwrap();
    let duplicate = format!("{{\"capabilities\":[\"street_tram_chain_v1\"],{}", &encoded[1..]);
    assert!(!inspect(duplicate.as_bytes()).status.success());
    let duplicate_version = format!("{{\"schema_version\":35,{}", &encoded[1..]);
    assert!(!inspect(duplicate_version.as_bytes()).status.success());

    header["schema_version"] = 34.into();
    assert!(!inspect(&serde_json::to_vec(&header).unwrap()).status.success());
}
