// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,within} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
// @ts-expect-error Node fs is available in Vitest, not in the browser type environment.
import {readFileSync} from 'node:fs';
const css=readFileSync('src/style.css','utf8');
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();localStorage.clear();});
test('speed lives inside initially closed details and retains edited values across closing',async()=>{
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'details-test'};
  if(command==='world_snapshot')return {next_id:'3',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
  if(command==='list_save_slots')return {active_slot_id:'test-slot',slots:[]};
  if(command==='autosave_status')return {available:false};
  throw Error('unexpected '+command);
 });
 await act(async()=>{render(<App/>);});
 const details=document.querySelector<HTMLDetailsElement>('.track-advanced-settings')!;
 expect(details).not.toBeNull();expect(details.open).toBe(false);
 expect(details.querySelector('summary')?.textContent).toBe('詳細設定');
 const lower=details.closest('.construction-options-row')!;
 expect(lower.contains(screen.getByLabelText('敷設する線路数'))).toBe(true);
 const mode=document.querySelector('.construction-mode-row')!;
 expect(mode.contains(screen.getByLabelText('敷設する線路数'))).toBe(false);
 expect(mode.compareDocumentPosition(lower)&Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
 const height=document.querySelector('.height-controls')!;
 expect(height.compareDocumentPosition(mode)&Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
 const field=within(details).getByLabelText('線路の目標最高速度') as HTMLInputElement;
 expect(field.closest('details')).toBe(details);
 // jsdom does not perform native summary keyboard activation/layout; toggle its native state.
 details.open=true;fireEvent(details,new Event('toggle'));
 fireEvent.change(field,{target:{value:'180'}});
 fireEvent.click(within(details).getByRole('button',{name:'目標最高速度を20 km/h下げる'}));
 expect(field.value).toBe('160');
 details.open=false;fireEvent(details,new Event('toggle'));
 details.open=true;fireEvent(details,new Event('toggle'));
 expect(field.value).toBe('160');
 fireEvent.click(within(details).getByRole('button',{name:'目標最高速度を20 km/h上げる'}));
 expect(field.value).toBe('180');expect(field.disabled).toBe(false);
 expect(screen.getAllByLabelText('線路の目標最高速度')).toHaveLength(1);
});
test('narrow toolbar allows label and controls to wrap without widening the disclosure',()=>{
 expect(css).toMatch(/\.track-advanced-settings \{[^}]*min-width:0;[^}]*max-width:100%/);
 expect(css).toMatch(/\.track-advanced-settings \.target-speed-control \{[^}]*flex-wrap:wrap/);
});

test('explicit lower row remains full width and wraps its controls on narrow panels',()=>{
 expect(css).toMatch(/\.construction-options-row \{[^}]*flex-wrap:wrap;[^}]*flex-basis:100%;[^}]*min-width:0;[^}]*max-width:100%/);
 expect(css).toContain('@media (max-width:480px)');
});
