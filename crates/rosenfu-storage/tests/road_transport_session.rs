#[allow(dead_code, unused_imports)]
#[path = "../../../src-tauri/src/session.rs"]
mod session;
#[test]
fn qa_is_explicit_persistent_and_undoable() {
    let mut editor = session::WorldEditor::new();
    let before = editor.snapshot();
    assert!(editor.enable_road_qa_fixture(false).is_err());
    assert_eq!(editor.snapshot(), before);
    let graph = editor.enable_road_qa_fixture(true).unwrap();
    assert_eq!(graph.road_transport.edges.len(), 4);
    let world = rosenfu_core::world::World::new(graph.clone()).unwrap();
    let restored = rosenfu_storage::decode(&rosenfu_storage::encode(&world).unwrap()).unwrap();
    assert_eq!(restored.snapshot(), &graph);
    let undone = editor.undo_world_edit(graph.clone()).unwrap();
    assert_eq!(undone.road_transport, before.road_transport);
    let redone = editor.redo_world_edit(undone).unwrap();
    assert_eq!(redone.road_transport, graph.road_transport);
    assert!(editor.enable_road_qa_fixture(true).is_err());
}
#[test]
fn normal_ipc_cannot_forge_access_or_graph() {
    let mut editor = session::WorldEditor::new();
    let world = editor.enable_road_qa_fixture(true).unwrap();
    let mut changed = world.road_transport.clone();
    changed.edges[0].source = "pretend-live".into();
    assert!(editor
        .replace_road_transport(world.road_transport.clone(), changed)
        .is_err());
    assert_eq!(editor.snapshot(), world);
}
#[test]
fn road_tram_native_lifecycle_is_atomic_and_undoable() {
    let mut editor = session::WorldEditor::new();
    let initial = editor.enable_road_qa_fixture(true).unwrap();
    let invalid = editor.edit_road_tram(session::RoadTramEdit::Build {
        from: "qa:0".into(),
        to: "missing".into(),
        name: "QA".into(),
        shared: true,
    });
    assert!(invalid.is_err());
    assert_eq!(editor.snapshot(), initial);
    let built = editor
        .edit_road_tram(session::RoadTramEdit::Build {
            from: "qa:0".into(),
            to: "qa:1".into(),
            name: "QA軌道".into(),
            shared: true,
        })
        .unwrap();
    let alignment_id = built.road_transport.tram_sections[0].alignment_id;
    assert_eq!(built.alignments.len(), 1);
    let dedicated = editor
        .edit_road_tram(session::RoadTramEdit::ChangePlacement {
            alignment_id,
            from: String::new(),
            to: String::new(),
            shared: false,
        })
        .unwrap();
    assert!(matches!(
        dedicated.road_transport.tram_sections[0].placement,
        rosenfu_core::road_transport::TramPlacement::Dedicated
    ));
    let world = rosenfu_core::world::World::new(dedicated.clone()).unwrap();
    assert_eq!(
        rosenfu_storage::decode(&rosenfu_storage::encode(&world).unwrap())
            .unwrap()
            .snapshot(),
        &dedicated
    );
    let removed = editor
        .edit_road_tram(session::RoadTramEdit::Remove { alignment_id })
        .unwrap();
    assert!(removed.alignments.is_empty() && removed.road_transport.tram_sections.is_empty());
    assert_eq!(editor.undo_world_edit(removed).unwrap(), dedicated);
}

#[test]
fn unknown_native_pilot_and_altered_save_never_authorize_graph() {
    let mut editor = session::WorldEditor::new();
    let before = editor.snapshot();
    assert!(editor
        .install_native_road_pilot("missing-corridor")
        .is_err());
    assert!(editor
        .install_native_road_pilot("../../local-data.json")
        .is_err());
    assert_eq!(editor.snapshot(), before);
    let qa = editor.enable_road_qa_fixture(true).unwrap();
    assert!(editor
        .install_native_road_pilot("missing-corridor")
        .is_err());
    assert_eq!(editor.snapshot(), qa);

    let world = rosenfu_core::world::World::new(qa.clone()).unwrap();
    let encoded = rosenfu_storage::encode(&world).unwrap();
    let encoded = String::from_utf8(encoded).unwrap();
    assert!(encoded.contains("synthetic-qa-not-real-roads"));
    for fake in ["native-pilot:unreviewed", "independent-fake-provider"] {
        let tampered = encoded.replace("synthetic-qa-not-real-roads", fake);
        assert!(rosenfu_storage::decode(tampered.as_bytes()).is_err());
    }
}

#[test]
fn ordinary_tram_intersection_preview_uses_cached_proof_without_mutating_the_world() {
    use base64::Engine;
    use rosenfu_core::ofm_evidence::{AuthoredTramGuide, BusVertex};
    let mut editor = session::WorldEditor::new();
    editor.rename_company("合成道路交差テスト".into()).unwrap();
    let original = editor.snapshot();
    let undo_depth = editor.world_edit_history().undo_depth;
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../rosenfu-core/tests/fixtures/ofm_authored_tram_synthetic.json"
    ))
    .unwrap();
    let tile_key = fixture["tileKey"].as_str().unwrap();
    let digest = fixture["tileRawSha256"].as_str().unwrap();
    let guide = AuthoredTramGuide {
        start_fraction_q: None,
        end_fraction_q: None,
        tile_key: tile_key.into(),
        tile_raw_sha256: digest.into(),
        start: BusVertex {
            feature: 0,
            line: 0,
            vertex: 0,
        },
        end: BusVertex {
            feature: 1,
            line: 0,
            vertex: 1,
        },
    };
    let cached = || {
        let bytes: Vec<u8> = serde_json::from_value(fixture["raw"].clone()).unwrap();
        (
            session::OfmRawInput {
                sha256: digest.into(),
                byte_length: bytes.len(),
                base64: base64::engine::general_purpose::STANDARD.encode(bytes),
            },
            session::OfmBindingInput {
                key: tile_key.into(),
                url: tile_key.into(),
                raw_sha256: digest.into(),
                revision: fixture["revision"].as_str().unwrap().into(),
                z: 14,
                x: 14552,
                y: 6451,
                decoder_rule: "ofm_transportation_z14_v1".into(),
                license: "ODbL-1.0".into(),
                attribution: "© OpenMapTiles © OpenStreetMap contributors".into(),
            },
        )
    };
    let token = editor.ofm_world_token().unwrap();
    assert!(editor
        .preview_authored_tram_road_guide("bad-token", &guide, Some(cached()))
        .is_err());
    assert!(editor
        .preview_authored_tram_road_guide(&token, &guide, None)
        .is_err());
    let points = editor
        .preview_authored_tram_road_guide(&token, &guide, Some(cached()))
        .unwrap();
    assert_eq!(points.len(), 3);
    let reverse = AuthoredTramGuide {
        start: guide.end,
        end: guide.start,
        ..guide
    };
    let opposite = editor
        .preview_authored_tram_road_guide(&token, &reverse, Some(cached()))
        .unwrap();
    assert!(points.iter().rev().eq(&opposite));
    assert_eq!(editor.snapshot(), original);
    assert_eq!(editor.ofm_world_token().unwrap(), token);
    assert_eq!(editor.world_edit_history().undo_depth, undo_depth);

    let alignment = |levels| {
        serde_json::from_value::<session::PlannedAlignment>(serde_json::json!({
            "name": "編集した架空軌道", "trackType": "tram", "points": points,
            "anchorIndices": [0, 2], "anchorLevels": levels, "stations": []
        }))
        .unwrap()
    };
    // The road evidence is horizontal only: game height/gradient still runs
    // through the normal construction gate, not a measured real rail grade.
    assert!(editor
        .build_construction_plan(vec![alignment(["ground", "elevated"])], vec![])
        .is_err());
    assert_eq!(editor.snapshot(), original);
    let built = editor
        .build_construction_plan(vec![alignment(["ground", "ground"])], vec![])
        .unwrap();
    assert_eq!(
        built.alignments[0].track_type,
        rosenfu_core::world::TrackType::Tram
    );
    assert!(built.road_path_provenance.is_empty() && built.road_evidence_descriptors.is_empty());
    let world = rosenfu_core::world::World::new(built.clone()).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("player-authored-tram.json");
    rosenfu_storage::save_world(&path, &world).unwrap();
    assert_eq!(
        rosenfu_storage::load_world_with_version(&path).unwrap().0,
        world
    );
}
