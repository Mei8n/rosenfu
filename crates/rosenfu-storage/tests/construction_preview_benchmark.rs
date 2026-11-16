//! Headless release microbenchmark. Fixture setup and request creation are not timed.
#[allow(dead_code, unused_imports)]
#[path = "../../../src-tauri/src/session.rs"]
mod session;
use rosenfu_core::world::{AlignmentId, World};
use session::{PlannedAlignment, WorldEditor};
use std::time::Instant;
fn plan() -> PlannedAlignment {
    serde_json::from_value(serde_json::json!({"name":"bench", "points":[
        {"longitude":139.7671,"latitude":35.6812},
        {"longitude":139.7681,"latitude":35.6812}],
        "trackType":"normal","stations":[]})).unwrap()
}
fn editor(size: usize) -> WorldEditor {
    let mut editor = WorldEditor::new();
    let mut draft = editor.build_construction_plan(vec![plan()], vec![]).unwrap();
    let template = draft.alignments[0].clone();
    for _ in 1..size {
        let mut alignment = template.clone();
        alignment.id = AlignmentId(draft.next_id);
        draft.next_id += 1;
        draft.alignments.push(alignment);
    }
    let world = World::new(draft).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bench.json");
    rosenfu_storage::create_world(&path, &world).unwrap();
    editor.load(&path).unwrap();
    editor
}
#[test]
fn isolated_native_preview_entry_preserves_results_and_source() {
    let editor = editor(4);
    let before = editor.snapshot();
    let old = editor.clone().preview_construction_plan(vec![plan()], vec![]);
    let new = editor.clone().build_construction_plan(vec![plan()], vec![]);
    assert_eq!(old, new);
    assert_eq!(editor.snapshot(), before);
    let bad = || { let mut p=plan(); p.points.clear(); p };
    assert_eq!(editor.clone().preview_construction_plan(vec![bad()],vec![]),
        editor.clone().build_construction_plan(vec![bad()],vec![]));
    assert_eq!(editor.snapshot(), before);
}
#[test]
#[ignore = "release timing, not a CI performance threshold"]
fn construction_preview_timings() {
    let sizes = std::env::var("ROSENFU_BENCH_SIZES").unwrap_or("100,1000".into());
    for size in sizes.split(',').map(|s| s.parse::<usize>().unwrap()) {
        assert!(size <= 10_000);
        let editor = editor(size);
        let before = editor.snapshot();
        for mode in ["legacy-entry", "isolated-build", "with-roads"] {
            let mut times = Vec::new();
            for iteration in 0..12 {
                let p=plan();
                let start=Instant::now();
                let result=match mode {
                    "legacy-entry" => editor.clone().preview_construction_plan(vec![p],vec![]),
                    "isolated-build" => editor.clone().build_construction_plan(vec![p],vec![]),
                    _ => editor.clone().build_construction_plan_with_roads(vec![p],vec![],vec![]),
                }.unwrap();
                std::hint::black_box(result);
                if iteration >= 2 { times.push(start.elapsed().as_secs_f64()*1000.0); }
            }
            times.sort_by(f64::total_cmp);
            println!("alignments={size} mode={mode} median_ms={:.3} min_ms={:.3} max_ms={:.3}",
                (times[4]+times[5])/2.0,times[0],times[9]);
        }
        assert_eq!(editor.snapshot(),before);
    }
}
