import {readFileSync} from 'node:fs';
import {expect,test} from 'vitest';
const source=readFileSync(new URL('../src-tauri/src/main.rs',import.meta.url),'utf8');
// Guard the desktop boundary, not transaction semantics (covered by Rust tests).
test.each(['delete_station','delete_alignment','delete_alignment_section','delete_alignment_sections','undo_alignment_deletion'])('%s dispatches its transaction off the UI thread',name=>{
 const command=source.split(`async fn ${name}(`)[1]?.split('#[tauri::command]')[0];
 expect(command).toBeDefined();
 expect(command).toContain('run_world_edit(app, move |editor|');
 expect(command).not.toContain('.lock()');
});
test('the worker acquires the editor mutex rather than waiting before dispatch',()=>{
 const worker=source.split('async fn run_world_edit(')[1].split('#[tauri::command]')[0];
 expect(worker.indexOf('spawn_blocking(move ||')).toBeLessThan(worker.indexOf('state.lock()'));
 expect(worker).toContain('operation(&mut editor)');
});
