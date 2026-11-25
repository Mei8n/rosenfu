// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {afterEach,beforeEach,expect,test,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {StandardSchedulePanel} from './StandardSchedulePanel';

vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
const ipc=vi.mocked(invoke);
const apply=vi.fn(async()=>{});
const plain={configurationToken:'cfg',serviceId:'3',canApply:true,activeElsewhere:false,
 authored:null,departureCount:null,origin:{id:'4',name:'A駅'},destinations:[{id:'5',name:'B駅'}]};
const bounds=[0,360,600,960,1200,1440];
const rates={earlyMorning:null,morningRush:6,midday:null,eveningRush:null,lateNight:null};
const panel=(canApply=true,currentHeadwayMs=600_000)=><StandardSchedulePanel service="3" revision="cfg" operationToken="edit" currentHeadwayMs={currentHeadwayMs} enabled canApply={canApply} apply={apply}/>;

beforeEach(()=>{ipc.mockReset();apply.mockClear();});
afterEach(cleanup);

test('five standard fields with opt-in ownership token; changing input does not apply',async()=>{
 ipc.mockResolvedValueOnce(plain);
 render(panel());
 const morning=await screen.findByLabelText('朝ラッシュの間隔（分）');
 fireEvent.change(morning,{target:{value:'6'}});
 expect(apply).not.toHaveBeenCalled();
 expect((screen.getByLabelText('有効期間（ゲーム日）') as HTMLInputElement).value).toBe('1');
 expect(screen.queryByText(/旧0.2.1では開けません/)).toBeNull();
 fireEvent.click(screen.getByRole('button',{name:'時間帯ダイヤを適用'}));
 expect(apply).toHaveBeenCalledWith({service:'3',expected:'edit',baseMinutes:10,overrides:rates,boundariesMinutes:bounds});
});

test('applied native timetable displays the finite window and explicit OD, not assumed service',async()=>{
 ipc.mockResolvedValueOnce({...plain,authored:{baseMinutes:10,overrides:rates,boundariesMinutes:bounds,startMs:60_000,endMs:86_460_000},departureCount:93});
 ipc.mockResolvedValueOnce({status:'scheduled',first_departure_ms:420_000,scheduled_departures:4,
 assignment:{rail:[{trips:70,time:{wait_seconds:360,ride_seconds:150}}],alternative_trips:30,unserved_trips:0,unresolved_trips:0}});
 render(panel(false));
 expect(await screen.findByText(/終了後に自動で延長しません/)).toBeTruthy();
 expect(screen.getByText(/朝ラッシュ 06:00～10:00：6分おき/)).toBeTruthy();
 expect(screen.queryByRole('button',{name:'時間帯ダイヤを適用'})).toBeNull();
 fireEvent.click(screen.getByRole('button',{name:'この時間の固定ODを試算'}));
 expect(await screen.findByText(/鉄道 70.0人、待ち 360秒、乗車 150秒/)).toBeTruthy();
 expect(ipc).toHaveBeenCalledWith('preview_standard_schedule_od',{service:'3',revision:'cfg',
 query:{origin:'4',destination:'5',trips:100,alternativeSeconds:null,startMs:60_000,endMs:3_660_000}});
 ipc.mockResolvedValueOnce({status:'unverified_schedule',first_departure_ms:null,scheduled_departures:null,
 assignment:{rail:[],alternative_trips:0,unserved_trips:0,unresolved_trips:100}});
 fireEvent.click(screen.getByRole('button',{name:'この時間の固定ODを試算'}));
 expect(await screen.findByText(/発車ゼロとは扱いません/)).toBeTruthy();
});

test('warns before Apply and after reopening a Standard+bus save, but never auto-applies',async()=>{
 const mixed={...plain,hasBusConfigs:true};
 ipc.mockResolvedValueOnce(mixed).mockResolvedValueOnce({...mixed,canApply:false,
  authored:{baseMinutes:10,overrides:rates,boundariesMinutes:bounds,startMs:60_000,endMs:86_460_000},departureCount:93});
 const {rerender}=render(panel());
 expect(await screen.findByText(/旧0.2.1では開けません/)).toBeTruthy();
 expect(screen.getByRole('button',{name:'時間帯ダイヤを適用'})).toBeTruthy();
 expect(apply).not.toHaveBeenCalled();
 rerender(<StandardSchedulePanel service="3" revision="cfg" operationToken="edit" enabled canApply={false} apply={apply} key="reopened"/>);
 expect(await screen.findByText(/終了後に自動で延長しません/)).toBeTruthy();
 expect(screen.getByText(/適用前のセーブを別の場所に残してください/)).toBeTruthy();
 expect(screen.queryByRole('button',{name:'時間帯ダイヤを適用'})).toBeNull();
 expect(apply).not.toHaveBeenCalled();
});

test.each([330_000,14_460_000,0])('does not round or silently replace unsupported original headway %s',async currentHeadwayMs=>{
 ipc.mockResolvedValueOnce(plain);
 render(panel(true,currentHeadwayMs));
 const base=await screen.findByLabelText('基本の間隔（分）') as HTMLInputElement;
 expect(base.value).toBe('');
 expect(screen.getByText(/従来ダイヤの間隔をそのまま引き継げないため/)).toBeTruthy();
 fireEvent.submit(base.closest('form')!);
 expect(apply).not.toHaveBeenCalled();
 fireEvent.change(base,{target:{value:'7'}});
 fireEvent.click(screen.getByRole('button',{name:'時間帯ダイヤを適用'}));
 expect(apply).toHaveBeenCalledWith({service:'3',expected:'edit',baseMinutes:7,
  overrides:{earlyMorning:null,morningRush:null,midday:null,eveningRush:null,lateNight:null},boundariesMinutes:bounds});
});

test('multi-day time bands preserve the prior duration on edit and send the validated day count',async()=>{
 ipc.mockResolvedValueOnce({...plain,authored:{baseMinutes:10,overrides:rates,boundariesMinutes:bounds,startMs:60_000,endMs:60_000+7*86_400_000},departureCount:651});
 render(panel());
 fireEvent.click(await screen.findByRole('button',{name:'時間帯ダイヤを変更'}));
 const days=screen.getByLabelText('有効期間（ゲーム日）') as HTMLInputElement;
 expect(days.value).toBe('7');
 fireEvent.change(days,{target:{value:'31'}});
 fireEvent.submit(days.closest('form')!);
 expect(screen.getByRole('alert').textContent).toMatch(/1〜30ゲーム日/);
 expect(apply).not.toHaveBeenCalled();
 fireEvent.change(days,{target:{value:'14'}});
 fireEvent.click(screen.getByRole('button',{name:'変更を適用'}));
 expect(apply).toHaveBeenCalledWith({service:'3',expected:'edit',baseMinutes:10,overrides:rates,boundariesMinutes:bounds,activeDays:14});
});

test('a time-band fleet count yields an explicit rounded draft interval without changing other bands',async()=>{
 ipc.mockResolvedValueOnce({...plain,nominalCycleMs:725_000});
 render(panel());
 fireEvent.click(await screen.findByText('編成数から時間帯の間隔を決める'));
 fireEvent.change(screen.getByLabelText('夕ラッシュの間隔（分）'),{target:{value:'7'}});
 fireEvent.change(screen.getByLabelText('間隔に反映する時間帯'),{target:{value:'morningRush'}});
 const count=screen.getByLabelText('試算用の編成数');
 fireEvent.change(count,{target:{value:'3'}});
 expect(fireEvent.keyDown(count,{key:'Enter'})).toBe(false);
 expect(apply).not.toHaveBeenCalled();
 expect(screen.getByText('試算結果：5分おき')).toBeTruthy();
 fireEvent.click(screen.getByRole('button',{name:'試算した間隔を入力欄に反映'}));
 expect((screen.getByLabelText('朝ラッシュの間隔（分）') as HTMLInputElement).value).toBe('5');
 expect((screen.getByLabelText('基本の間隔（分）') as HTMLInputElement).value).toBe('10');
 expect(apply).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole('button',{name:'時間帯ダイヤを適用'}));
 expect(apply).toHaveBeenCalledWith({service:'3',expected:'edit',baseMinutes:10,boundariesMinutes:bounds,
  overrides:{earlyMorning:null,morningRush:5,midday:null,eveningRush:7,lateNight:null}});
});

test('out-of-range fleet intervals cannot override a timetable draft',async()=>{
 ipc.mockResolvedValueOnce({...plain,nominalCycleMs:14_400_001});
 render(panel());
 fireEvent.click(await screen.findByText('編成数から時間帯の間隔を決める'));
 const count=screen.getByLabelText('試算用の編成数');
 const button=screen.getByRole('button',{name:'試算した間隔を入力欄に反映'}) as HTMLButtonElement;
 fireEvent.change(count,{target:{value:'1'}});
 expect(button.disabled).toBe(true);
 expect(screen.getByText(/240分を超えます/)).toBeTruthy();
 fireEvent.change(count,{target:{value:'0'}});
 expect(button.disabled).toBe(true);
 expect((screen.getByLabelText('基本の間隔（分）') as HTMLInputElement).value).toBe('10');
 expect(apply).not.toHaveBeenCalled();
});
