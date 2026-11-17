import {readFileSync} from 'node:fs';
import {expect,test} from 'vitest';
const source=readFileSync(new URL('../src-tauri/src/main.rs',import.meta.url),'utf8');
test.each(['build_alignment_from_reference','build_alignment','build_alignment_with_stations','build_plan','build_plan_with_roads'])('%s keeps construction and mutex waiting off the UI thread',name=>{
 const command=source.split(`async fn ${name}(`)[1]?.split('#[tauri::command]')[0];
 expect(command).toBeDefined();expect(command).toContain('run_world_edit(app, move |editor|');expect(command).not.toContain('.lock()');
});
test.each(['road_assessments','preview_track_render_geometry','preview_track_movement_geometry'])('%s uses an owned snapshot on a worker',name=>{
 const command=source.split(`async fn ${name}(`)[1].split('#[tauri::command]')[0];
 expect(command).toContain('run_world_snapshot(app, move |snapshot|');expect(command).not.toContain('.lock()');
});
test('snapshot worker releases the live editor before road and rail scans',()=>{
 const worker=source.split("async fn run_world_snapshot<T: Send + 'static>(")[1].split('// Keep the atomic editor transaction')[0];
 expect(worker.indexOf('spawn_blocking(move ||')).toBeLessThan(worker.indexOf('.lock()'));
 expect(worker.indexOf('.clone();')).toBeLessThan(worker.indexOf('operation(snapshot)'));
});
