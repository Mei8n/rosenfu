//! Headless benchmark of the real summary preview; no fixed CI timing threshold.
#[allow(dead_code, unused_imports)]
#[path = "../../../src-tauri/src/session.rs"]
mod session;
use rosenfu_core::world::{AlignmentId, CoverageBounds, GeoPoint, RoadSnapshot, RoadWay, World};
use session::{PlannedAlignment, WorldEditor};
use std::sync::Mutex;
use std::time::Instant;

fn point(x: f64, y: f64) -> GeoPoint {
    GeoPoint {
        longitude: 139. + x,
        latitude: 35. + y,
    }
}
fn plan_value() -> serde_json::Value {
    serde_json::json!({
        "name": "benchmark", "points":[point(-0.01,0.), point(0.01,0.)],
        "trackType":"normal", "stations":[]
    })
}

fn fixture(size: usize) -> (WorldEditor, usize) {
    let mut editor = WorldEditor::new();
    let mut draft = editor
        .build_construction_plan(vec![serde_json::from_value(plan_value()).unwrap()], vec![])
        .unwrap();
    let alignment = draft.alignments[0].clone();
    for i in 1..size {
        let mut next = alignment.clone();
        next.id = AlignmentId(draft.next_id);
        draft.next_id += 1;
        let x = 0.04 + (i % 110) as f64 * 0.001;
        let y = -0.07 + (i / 110) as f64 * 0.001;
        next.points = vec![point(x, y), point(x + 0.0007, y)];
        draft.alignments.push(next);
    }
    let mut world = World::new(draft).unwrap();
    let ways = (0..1_000)
        .map(|i| {
            let x = -0.009 + (i % 200) as f64 * 0.00009;
            let y = ((i / 200) as f64 - 2.) * 0.007;
            RoadWay {
                id: format!("way/{}", i + 1),
                points: (0..8)
                    .map(|j| point(x + (j % 2) as f64 * 0.00001, y - 0.003 + 0.0009 * j as f64))
                    .collect(),
                coverage: None,
                classification: None,
                layer: None,
                level: None,
                bridge: None,
                tunnel: None,
            }
        })
        .collect();
    world
        .add_road_snapshot(RoadSnapshot {
            coverage_verified: true,
            source: "preview-benchmark".into(),
            source_url: "https://example.org/preview-benchmark.osm.pbf".into(),
            source_sha256: "b".repeat(64),
            attribution: "Synthetic regression".into(),
            license: "ODbL-1.0".into(),
            revision: "2026-11-21T00:00:00Z".into(),
            road_coverage: vec![CoverageBounds {
                west: 138.95,
                east: 139.2,
                south: 34.9,
                north: 35.15,
            }],
            roads: ways,
        })
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("fixture.json");
    rosenfu_storage::create_world(&path, &world).unwrap();
    let save_bytes = std::fs::metadata(&path).unwrap().len() as usize;
    editor.load(&path).unwrap();
    (editor, save_bytes)
}

#[test]
#[ignore = "headless release measurements only"]
fn construction_summary_timings() {
    for n in [100, 1_000, 10_000] {
        let (editor, save_bytes) = fixture(n);
        let roads = Instant::now();
        let count = editor
            .road_assessments()
            .into_iter()
            .map(|(_, r)| r.crossings.len())
            .sum::<usize>();
        let roads_ms = roads.elapsed().as_secs_f64() * 1000.;
        assert_eq!(count, 200);
        let before = editor.snapshot();
        let editor = Mutex::new(editor);
        for sample in 0..7 {
            let request = serde_json::to_vec(&plan_value()).unwrap();
            let decode = Instant::now();
            let plan = serde_json::from_slice::<PlannedAlignment>(&request).unwrap();
            let decode_us = decode.elapsed().as_secs_f64() * 1e6;
            let clone = Instant::now();
            let isolated = editor.lock().unwrap().construction_validation_snapshot();
            let lock_clone_ms = clone.elapsed().as_secs_f64() * 1000.;
            let worker = Instant::now();
            let result = isolated
                .preview_construction_summary(vec![plan], vec![], vec![])
                .unwrap();
            let worker_ms = worker.elapsed().as_secs_f64() * 1000.;
            let encode = Instant::now();
            let bytes = serde_json::to_vec(&result).unwrap();
            let encode_us = encode.elapsed().as_secs_f64() * 1e6;
            assert_eq!(result.new_crossing_count, Some(0));
            assert_eq!(result.roads_unverified, Some(false));
            assert!(result.validation_error.is_none());
            println!(
                "n={n} sample={sample} road_pass_ms={roads_ms:.3} lock_clone_ms={lock_clone_ms:.3} worker_ms={worker_ms:.3} request_decode_us={decode_us:.3} response_encode_us={encode_us:.3} request_bytes={} response_bytes={} save_bytes={save_bytes}",
                request.len(),
                bytes.len()
            );
        }
        assert_eq!(editor.lock().unwrap().snapshot(), before);
    }
}

/// In a fresh process, measure an editor overwrite and a cold editor reopen of
/// that same slot. The data are synthetic and preloaded; OS caches are not reset.
#[test]
#[ignore = "headless edit/save/reopen measurements only"]
fn construction_edit_save_reopen_timings() {
    let n = match std::env::var("ROSENFU_EDIT_BENCH_ALIGNMENTS").as_deref() {
        Ok("100") => 100,
        Ok("1000") | Err(std::env::VarError::NotPresent) => 1_000,
        Ok("10000") => 10_000,
        _ => panic!("ROSENFU_EDIT_BENCH_ALIGNMENTS must be 100, 1000 or 10000"),
    };
    let request = serde_json::json!({
        "name": "benchmark edit", "points":[point(-0.01,0.01), point(0.01,0.01)],
        "trackType":"normal", "stations":[]
    });
    let view = rosenfu_core::world::ViewState {
        latitude: 35.02,
        longitude: 139.03,
        zoom: 12.,
    };
    for sample in 0..7 {
        // Set up an established slot on each sample, outside timed work.
        let root = tempfile::tempdir().unwrap();
        let (mut editor, _) = fixture(n);
        assert!(
            editor
                .save_active_slot(root.path(), None, None)
                .unwrap()
                .saved
        );
        let slot = editor
            .list_save_slots(root.path())
            .unwrap()
            .active_slot_id
            .unwrap();
        let path = editor.active_save_path(root.path()).unwrap();
        let plan = serde_json::from_value(request.clone()).unwrap();

        let overall = Instant::now();
        let build = Instant::now();
        let mut reply = editor.build_construction_plan(vec![plan], vec![]).unwrap();
        let build_ms = build.elapsed().as_secs_f64() * 1000.;
        let encode = Instant::now();
        let reply_bytes = serde_json::to_vec(&reply).unwrap().len();
        let encode_ms = encode.elapsed().as_secs_f64() * 1000.;
        let save = Instant::now();
        let saved = editor
            .save_active_slot(root.path(), None, Some(view))
            .unwrap();
        let save_ms = save.elapsed().as_secs_f64() * 1000.;
        // A fresh session, not a disk cache flush. Drop the first editor as
        // it would be when the app closes before reopening this same slot.
        drop(editor);
        let reopen = Instant::now();
        let reopened = WorldEditor::new()
            .load_save_slot(root.path(), slot)
            .unwrap();
        let reopen_ms = reopen.elapsed().as_secs_f64() * 1000.;
        let total_ms = overall.elapsed().as_secs_f64() * 1000.;
        let save_bytes = std::fs::metadata(&path).unwrap().len();

        assert!(saved.saved, "edit never reached the archive");
        assert_eq!(reply.alignments.len(), n + 1);
        reply.view_state = Some(view);
        assert_eq!(reopened, reply, "save/reopen changed the edited world");
        println!("n={n} sample={sample} build_ms={build_ms:.3} reply_encode_ms={encode_ms:.3} overwrite_ms={save_ms:.3} reopen_ms={reopen_ms:.3} total_ms={total_ms:.3} response_bytes={reply_bytes} save_bytes={save_bytes}");
    }
    #[cfg(target_os = "linux")]
    if let Some(peak) = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find_map(|line| line.strip_prefix("VmHWM:").map(str::trim))
                .map(str::to_owned)
        })
    {
        println!("n={n} whole_process_peak_rss={peak}");
    }
}
