// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,within} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {FixedOdPanel} from './FixedOdPanel';
import {OperationProvider} from './OperationsPanel';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.useRealTimers();vi.resetAllMocks();});
const stations=[{id:'1',name:'東駅'},{id:'2',name:'西駅'}], services=[{id:'3',name:'快速'}];
const result={potential_trips:100,rail:[{stations:['1','2'],services:['3'],generalized_seconds:600,trips:75}],alternative_trips:25,unserved_trips:0,unresolved_trips:0,search_complete:true};
const panel=(version=1)=><FixedOdPanel stations={stations} services={services} enabled worldVersion={version}/>;
function enter(){
 fireEvent.change(screen.getByLabelText('出発駅'),{target:{value:'1'}});
 fireEvent.change(screen.getByLabelText('到着駅'),{target:{value:'2'}});
 fireEvent.click(screen.getByText('需要を試算'));
}
test('uses real selected IDs and native allocation instead of inventing ridership',async()=>{
 vi.mocked(invoke).mockResolvedValue(result);
 render(panel());enter();
 expect(await screen.findByText('75人',{selector:'dd'})).toBeTruthy();
 expect(screen.getByText('快速')).toBeTruthy();
 expect(invoke).toHaveBeenCalledWith('preview_fixed_od',{originStationId:'1',destinationStationId:'2',trips:100,alternativeSeconds:null});
});
test('incomplete native search never counts partial candidates as use',async()=>{
 vi.mocked(invoke).mockResolvedValue({...result,search_complete:false,unresolved_trips:100});render(panel());enter();
 expect(await screen.findByText(/100人は未確定/)).toBeTruthy();
 expect(screen.queryByText('75人')).toBeNull();
});
test('a result from a replaced world is not shown',async()=>{
 let resolve!:(value:typeof result)=>void;vi.mocked(invoke).mockReturnValue(new Promise(done=>{resolve=done;}));
 const view=render(panel());enter();view.rerender(panel(2));await act(async()=>resolve(result));
 expect(screen.queryByText('75人')).toBeNull();expect((screen.getByText('需要を試算') as HTMLButtonElement).disabled).toBe(false);
});

test('bus stop OD uses native namespaced stop IDs and displays the bus route name',async()=>{
 vi.mocked(invoke).mockResolvedValue({...result,rail:[{stations:['bus:test:a','bus:test:b'],services:['bus:test:route'],generalized_seconds:600,trips:75}]});
 const busStops=[{id:'test:a',name:'東バス停'},{id:'test:b',name:'西バス停'}];
 render(<FixedOdPanel stations={stations} services={services} busStops={busStops} busRoutes={[{id:'test:route',name:'検証バス'}]} enabled worldVersion={1}/>);
 fireEvent.change(screen.getByLabelText('出発地点'),{target:{value:'bus:test:a'}});
 fireEvent.change(screen.getByLabelText('到着地点'),{target:{value:'bus:test:b'}});
 expect((screen.getByRole('button',{name:'運行の遅れを含めて試算'}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.click(screen.getByText('需要を試算'));
 expect(await screen.findByText('検証バス')).toBeTruthy();
 expect(screen.getByText(/駅とバス停の徒歩乗り換えは、試算で指定した場合だけ計算します/)).toBeTruthy();
 expect(invoke).toHaveBeenCalledWith('preview_fixed_od',{originStationId:'bus:test:a',destinationStationId:'bus:test:b',trips:100,alternativeSeconds:null});
});

test('restores recorded conditions, renders unweighted time, and saves only explicit input', async()=>{
 const saved={origin_station_id:'1',destination_station_id:'2',trips:125,alternative_seconds:90};
 vi.mocked(invoke).mockImplementation(async(command)=>command==='fixed_od_scenario'?{token:'old-world',scenario:saved}:{
  ...result,rail:[{...result.rail[0],time:{wait_seconds:30,ride_seconds:180,walk_seconds:0,transfers:0}}]
 });
 const store=vi.fn(async(_old,next)=>({token:'revision-2',scenario:next}));
 render(<FixedOdPanel stations={stations} services={services} enabled worldVersion={1} saveScenario={store}/>);
 await screen.findByText('記録済みの条件は次のセーブに含まれます。');
 expect((screen.getByLabelText('出発駅') as HTMLSelectElement).value).toBe('1');
 expect((screen.getByLabelText('移動需要（人）') as HTMLInputElement).value).toBe('125');
 fireEvent.click(screen.getByText('需要を試算'));
 expect(await screen.findByText('推計: 待ち 0.5分 / 乗車 3分 / 徒歩 0分 / 乗換 0回')).toBeTruthy();
 expect(store).not.toHaveBeenCalled();
 fireEvent.change(screen.getByLabelText('移動需要（人）'),{target:{value:'0'}});
 fireEvent.click(screen.getByText('この試算条件をワールドに記録'));
 expect(store).toHaveBeenCalledWith('old-world',{...saved,trips:0});
});

test('only applies a user-entered rail to bus walk to preview and never saves it', async()=>{
 const busStops=[{id:'test:a',name:'東バス停'},{id:'test:b',name:'西バス停'}];
 const busRoutes=[{id:'test:route',name:'検証バス'}];
 vi.mocked(invoke).mockImplementation(async(command)=>command==='fixed_od_scenario'?{token:'old-world',scenario:null}:{
  ...result,rail:[{...result.rail[0],stations:['1','2','bus:test:a','bus:test:b'],services:['3','bus:test:route'],time:{wait_seconds:60,ride_seconds:180,walk_seconds:180,transfers:1}}]
 });
 const store=vi.fn(async(_old,next)=>({token:'updated',scenario:next}));
 render(<FixedOdPanel stations={stations} services={services} busStops={busStops} busRoutes={busRoutes} enabled worldVersion={1} saveScenario={store}/>);
 await screen.findByRole('button',{name:'この試算条件をワールドに記録'});
 fireEvent.change(screen.getByLabelText('出発地点'),{target:{value:'1'}});
 fireEvent.change(screen.getByLabelText('到着地点'),{target:{value:'bus:test:b'}});
 fireEvent.click(screen.getByLabelText('駅とバス停の乗り換えを試す'));
 fireEvent.change(screen.getByLabelText('到着地点'),{target:{value:'2'}});
 expect((screen.getByRole('button',{name:'運行の遅れを含めて試算'}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.change(screen.getByLabelText('到着地点'),{target:{value:'bus:test:b'}});
 fireEvent.click(screen.getByText('需要を試算'));
 expect((await screen.findByRole('alert')).textContent).toContain('1〜60分');
 expect(invoke).not.toHaveBeenCalledWith('preview_fixed_od',expect.anything());
 fireEvent.change(screen.getByLabelText('乗り換え駅'),{target:{value:'2'}});
 fireEvent.change(screen.getByLabelText('乗り換えバス停'),{target:{value:'bus:test:a'}});
 fireEvent.change(screen.getByLabelText('徒歩の所要時間（分）'),{target:{value:'3'}});
 fireEvent.click(screen.getByText('需要を試算'));
 expect(await screen.findByText(/徒歩 3分 \/ 乗換 1回/)).toBeTruthy();
 const scenario={originStationId:'1',destinationStationId:'bus:test:b',trips:100,alternativeSeconds:null};
 expect(invoke).toHaveBeenCalledWith('preview_fixed_od',{...scenario,transfer:{stationId:'2',busStopId:'bus:test:a',walkSeconds:180}});
 fireEvent.click(screen.getByText('この試算条件をワールドに記録'));
 expect(store).toHaveBeenCalledWith('old-world',{origin_station_id:'1',destination_station_id:'bus:test:b',trips:100,alternative_seconds:null});
 await screen.findByText('記録済みの条件は次のセーブに含まれます。');
 fireEvent.click(screen.getByLabelText('駅とバス停の乗り換えを試す'));
 fireEvent.click(screen.getByText('需要を試算'));
 expect(invoke).toHaveBeenCalledWith('preview_fixed_od',scenario);
});

test('observed allocation is an explicit rail-only request and shows physical elapsed time',async()=>{
 vi.mocked(invoke).mockResolvedValue({status:'reached',potential_trips:100,rail_trips:37,alternative_trips:63,unresolved_trips:0,
  itinerary:{ready_ms:0,arrival_ms:1740000,platform_wait_ms:600000,onboard_ms:1140000}});
 render(panel());
 const observe=screen.getByRole('button',{name:'運行の遅れを含めて試算'}) as HTMLButtonElement;
 expect(observe.disabled).toBe(true);
 fireEvent.change(screen.getByLabelText('出発駅'),{target:{value:'1'}});
 fireEvent.change(screen.getByLabelText('到着駅'),{target:{value:'2'}});
 fireEvent.change(screen.getByLabelText('鉄道以外の所要時間（分・任意）'),{target:{value:'26'}});
 expect(invoke).not.toHaveBeenCalled();
 fireEvent.click(observe);
 expect(await screen.findByText('所要 29分（待ち 10分、乗車 19分）')).toBeTruthy();
 expect(screen.getByText('37人',{selector:'dd'})).toBeTruthy();
 expect(screen.getByText('観測開始: 1日目 00:00')).toBeTruthy();
 expect(invoke).toHaveBeenCalledExactlyOnceWith('preview_observed_fixed_od',{query:{originStationId:'1',destinationStationId:'2',trips:100,alternativeSeconds:1560}});
});

test.each(['input','world'])('keeps separate timetable and actual-operation trial results; clears both for a different %s',async change=>{
 const replay={status:'reached',potential_trips:100,rail_trips:37,alternative_trips:63,unresolved_trips:0,
  itinerary:{ready_ms:0,arrival_ms:1740000,platform_wait_ms:600000,onboard_ms:1140000}};
 let scheduledRequests=0;
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='preview_observed_fixed_od')return replay;
  return ++scheduledRequests===1?result:{...result,rail:[{...result.rail[0],trips:72}],alternative_trips:28};
 });
 const view=render(panel());enter();
 expect(await screen.findByText('75人',{selector:'dd'})).toBeTruthy();
 expect(vi.mocked(invoke).mock.calls.map(([command])=>command)).toEqual(['preview_fixed_od']);
 fireEvent.click(screen.getByRole('button',{name:'運行の遅れを含めて試算'}));
 expect(await screen.findByText('37人',{selector:'dd'})).toBeTruthy();
 expect(screen.queryByText('75人',{selector:'dd'})).not.toBeNull();
 const scheduled=within(screen.getByRole('region',{name:'ダイヤを使った需要試算結果'}));
 const observed=within(screen.getByRole('region',{name:'遅れを含む需要試算結果'}));
 expect(scheduled.getByText('75人',{selector:'dd'})).toBeTruthy();
 expect(observed.getByText('37人')).toBeTruthy();
 expect(screen.getByText(/人数の差は遅れだけによるものではありません/)).toBeTruthy();
 fireEvent.click(screen.getByRole('button',{name:'需要を試算'}));
 expect(await screen.findByText('72人',{selector:'dd'})).toBeTruthy();
 expect(observed.getByText('37人')).toBeTruthy();
 expect(vi.mocked(invoke).mock.calls.map(([command])=>command)).toEqual([
  'preview_fixed_od','preview_observed_fixed_od','preview_fixed_od',
 ]);
 if(change==='world')view.rerender(panel(2));
 else fireEvent.change(screen.getByLabelText('移動需要（人）'),{target:{value:'50'}});
 expect(screen.queryByText('72人',{selector:'dd'})).toBeNull();
 expect(screen.queryByText('37人',{selector:'dd'})).toBeNull();
 expect(screen.queryByText(/人数の差は遅れだけによるものではありません/)).toBeNull();
});

test('observed partial or finite-window no arrival does not allocate substitute trips',async()=>{
 vi.mocked(invoke).mockResolvedValue({status:'no_arrival_within_window',potential_trips:100,rail_trips:0,alternative_trips:0,unresolved_trips:100});
 render(panel());
 fireEvent.change(screen.getByLabelText('出発駅'),{target:{value:'1'}});
 fireEvent.change(screen.getByLabelText('到着駅'),{target:{value:'2'}});
 fireEvent.click(screen.getByRole('button',{name:'運行の遅れを含めて試算'}));
 expect(await screen.findByText(/100人は未確定/)).toBeTruthy();
 expect(screen.queryByText('別の移動手段を利用')).toBeNull();
});

test('keeps the captured minute and marks it stale after a clock tick without requesting another OD preview',async()=>{
 vi.useFakeTimers();
 const observedMs=6*60*60_000;
 const snapshot={now_ms:observedMs,paused:false,speed:1,trains:[],services:[],configuration_token:'scope',timetable_edit_token:'old'};
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='operation_snapshot')return snapshot;
  if(command==='advance_operation')return {...snapshot,now_ms:observedMs+60_000,timetable_edit_token:'new'};
  if(command==='preview_observed_fixed_od')return {
   status:'reached',potential_trips:100,rail_trips:37,alternative_trips:63,unresolved_trips:0,
   itinerary:{ready_ms:observedMs,arrival_ms:observedMs+1740000,platform_wait_ms:600000,onboard_ms:1140000}
  };
  throw Error(command);
 });
 await act(async()=>{render(<OperationProvider available blocked={false} epoch={1} onChange={()=>{}}>{panel()}</OperationProvider>);});
 fireEvent.change(screen.getByLabelText('出発駅'),{target:{value:'1'}});
 fireEvent.change(screen.getByLabelText('到着駅'),{target:{value:'2'}});
 await act(async()=>{fireEvent.click(screen.getByRole('button',{name:'運行の遅れを含めて試算'}));});
 expect(screen.getByText('観測開始: 1日目 06:00')).toBeTruthy();
 expect(screen.queryByText(/時刻が進みました/)).toBeNull();
 await act(async()=>{await vi.advanceTimersByTimeAsync(500);});
 expect(screen.getByRole('status').textContent).toContain('最新の時刻で見るには再試算');
 expect(screen.getByText('観測開始: 1日目 06:00')).toBeTruthy();
 expect(screen.getByText('37人',{selector:'dd'})).toBeTruthy();
 expect(vi.mocked(invoke).mock.calls.map(([command])=>command)).toEqual([
  'operation_snapshot','preview_observed_fixed_od','advance_operation'
 ]);
});
