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
