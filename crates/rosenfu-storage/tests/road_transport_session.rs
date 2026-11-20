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
