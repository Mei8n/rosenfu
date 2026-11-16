//! Exercise the same desktop plan pipeline as the map UI, without a desktop runtime.
#[allow(dead_code, unused_imports)]
#[path = "../../../src-tauri/src/session.rs"]
mod session;
use serde_json::json;
use session::{PlannedAlignment, WorldEditor};
fn plan(a: f64, b: f64, count: u8) -> PlannedAlignment {
    serde_json::from_value(json!({"name":"Mixed tracks","points":[{"latitude":35.,"longitude":a},{"latitude":35.,"longitude":b}],"trackType":"normal","infrastructure":{"level":"ground","track_count":count},"anchorIndices":[0,1],"anchorLevels":["ground","ground"],"stations":[]})).unwrap()
}
#[test]
fn mixed_counts_connect_from_new_or_built_and_preserve_existing_tracks_and_save() {
    use rosenfu_core::world::{AlignmentEndpoint, World};
    for (a, b) in [(1, 2), (2, 1), (1, 4), (4, 1), (2, 4), (4, 2)] {
        for extend in [false, true] {
            let mut editor = WorldEditor::new();
            let built = editor.build_plan(vec![plan(139.02, 139.03, b)]).unwrap();
            let target = built.alignments[0].id;
            let source = if extend {
                Some(
                    editor
                        .build_plan(vec![plan(139., 139.01, a)])
                        .unwrap()
                        .alignments
                        .last()
                        .unwrap()
                        .id,
                )
            } else {
                None
            };
            let before = editor.snapshot();
            let next = || {
                let mut next = plan(139.01, 139.02, a);
                next.destination = Some(session::PlannedAlignmentExtension {
                    alignment_id: target,
                    endpoint: AlignmentEndpoint::Start,
                });
                next.extension = source.map(|alignment_id| session::PlannedAlignmentExtension {
                    alignment_id,
                    endpoint: AlignmentEndpoint::End,
                });
                next
            };
            let preview = editor
                .preview_construction_plan(vec![next()], vec![])
                .unwrap();
            let after = editor.build_plan(vec![next()]).unwrap();
            assert_eq!(preview, after);
            for old in &before.alignments {
                assert!(after.alignments.contains(old));
            }
            assert_eq!(
                after.alignment_connections.len(),
                if extend { 2 } else { 1 }
            );
            let world = World::new(after.clone()).unwrap();
            assert!(world
                .alignment_lane_movements()
                .iter()
                .any(|movement| movement.point.is_some()));
            let restored =
                rosenfu_storage::decode(&rosenfu_storage::encode(&world).unwrap()).unwrap();
            assert_eq!(restored.snapshot(), &after);
        }
    }
}
