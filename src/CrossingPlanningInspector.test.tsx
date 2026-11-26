// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,beforeEach,expect,test,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {CrossingPlanningInspector} from './CrossingPlanningInspector';

vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
const ipc=vi.mocked(invoke);
const hour=3_600_000n;
const sample=(configuration_token='rev:1',start_ms='0')=>({
 configuration_token,start_ms,end_ms:String(BigInt(start_ms)+hour),available:true,estimate_count:1,
 estimates:[{alignment_ids:['3','9007199254740993'],approach_alignment_id:'3',towards_alignment_end:true,
 scheduled_passages:[6,12],requested_utilization:.6,estimated_delay_seconds_per_train:24}],
 incomplete_count:1,incomplete_crossings:[['5','6']],sparse_count:1,sparse_crossings:[['7','8']]
});
const press=()=>fireEvent.click(screen.getByRole('button',{name:'1時間を試算'}));
const open=()=>fireEvent.click(screen.getByText('平面交差の計画試算'));
const node=(token:string|undefined='rev:1',enabled=true)=><CrossingPlanningInspector enabled={enabled} configurationToken={token} nowMs={0}/>;

beforeEach(()=>ipc.mockReset());
afterEach(cleanup);

test('only a manual click triggers a nominal-window read, separating three result categories',async()=>{
 ipc.mockResolvedValue(sample());
 render(node());
 open();
 await act(async()=>{});
 expect(ipc).not.toHaveBeenCalled();
 expect(screen.getByText(/終了時刻は含まない/)).toBeTruthy();
 expect(screen.getByText(/次の実際の発車・通過時刻は予測しません/)).toBeTruthy();
 press();
 expect(ipc).toHaveBeenCalledTimes(1);
 expect(ipc).toHaveBeenCalledWith('preview_crossing_planning',{expectedConfigurationToken:'rev:1',startMs:'0'});
 expect(await screen.findByText(/平均遅れ 24.0秒\/本/)).toBeTruthy();
 expect(screen.getByText(/試算対象の進路 1件 \/ 計算できない交差 1箇所 \/ 本数が少なく試算対象外 1箇所/)).toBeTruthy();
 expect(screen.getByText(/交差 1・進路 B/)).toBeTruthy();
});

test('a late response cannot replace the current world or window; one read is in flight',async()=>{
 let old!:(value:unknown)=>void;
 ipc.mockImplementationOnce(()=>new Promise(resolve=>{old=resolve;}));
 const {rerender}=render(node());
 open();
 press();
 expect(screen.getByText('試算中…')).toBeTruthy();
 rerender(node('rev:2'));
 ipc.mockResolvedValueOnce(sample('rev:2','86400000'));
 fireEvent.change(screen.getByLabelText('開始日（ゲーム内）'),{target:{value:'2'}});
 press();
 expect(ipc).toHaveBeenCalledTimes(1);
 await act(async()=>old({...sample(),estimates:[{...sample().estimates[0],estimated_delay_seconds_per_train:99}]}));
 expect(screen.queryByText(/99.0秒\/本/)).toBeNull();
 press();
 expect(await screen.findByText(/平均遅れ 24.0秒\/本/)).toBeTruthy();
 fireEvent.change(screen.getByLabelText('開始時刻'),{target:{value:'01:00'}});
 expect(screen.queryByText(/24.0秒\/本/)).toBeNull();
});

test('stale native window, unavailable results, disabled state and invalid day are honest',async()=>{
 ipc.mockResolvedValueOnce(sample('rev:1','3600000'));
 const {rerender}=render(node());
 open();
 press();
 expect(await screen.findByRole('alert')).toBeTruthy();
 expect(screen.queryByText(/24.0秒\/本/)).toBeNull();
 ipc.mockResolvedValueOnce({...sample(),available:false,estimate_count:0,estimates:[],incomplete_count:0,incomplete_crossings:[],sparse_count:0,sparse_crossings:[]});
 press();
 expect(await screen.findByText('利用できる鉄道ダイヤがありません。')).toBeTruthy();
 rerender(node('rev:1',false));
 expect(screen.queryByText('利用できる鉄道ダイヤがありません。')).toBeNull();
 press();
 expect(ipc).toHaveBeenCalledTimes(2);
 rerender(node());
 fireEvent.change(screen.getByLabelText('開始日（ゲーム内）'),{target:{value:'99999999999999999999'}});
 press();
 await waitFor(()=>expect(screen.getByText('開始日と時刻を確認してください。')).toBeTruthy());
 expect(ipc).toHaveBeenCalledTimes(2);
});


test('player-facing labels hide alignment identities in all categories and in IPC errors',async()=>{
 const privateIds=['9007199254740993','9007199254741099','9007199254742100','9007199254742101','9007199254743100','9007199254743101'];
 const eligible=sample().estimates[0];
 const pair=privateIds.slice(0,2);
 ipc.mockResolvedValueOnce({...sample(),estimate_count:4,estimates:[
  {...eligible,alignment_ids:pair,approach_alignment_id:pair[0],towards_alignment_end:false},
  {...eligible,alignment_ids:pair,approach_alignment_id:pair[0],towards_alignment_end:true},
  {...eligible,alignment_ids:pair,approach_alignment_id:pair[1],towards_alignment_end:false},
  {...eligible,alignment_ids:pair,approach_alignment_id:pair[1],towards_alignment_end:true},
 ],incomplete_crossings:[privateIds.slice(2,4)],sparse_crossings:[privateIds.slice(4,6)]});
 const {container}=render(node());
 open();
 press();
 expect(await screen.findByText(/交差 1・進路 A/)).toBeTruthy();
 for(const letter of ['B','C','D'])expect(screen.getByText(new RegExp(`交差 1・進路 ${letter}`))).toBeTruthy();
 expect(screen.getByText('計算できない交差：交差 2')).toBeTruthy();
 expect(screen.getByText('本数が少ない交差：交差 3')).toBeTruthy();
 for(const id of privateIds)expect(container.innerHTML).not.toContain(id);

 ipc.mockRejectedValueOnce(Error(`alignment ${privateIds[0]} and ${privateIds[5]} cannot calculate`));
 press();
 expect((await screen.findByRole('alert')).textContent).toBe('試算を取得できませんでした。最新の運行設定で調べ直してください。');
 for(const id of privateIds)expect(container.innerHTML).not.toContain(id);
});
