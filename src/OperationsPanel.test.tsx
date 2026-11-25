// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {BusOperations,OperationClock,OperationProvider,ServiceOperations,type OperationView} from './OperationsPanel';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();vi.useRealTimers();});
const snapshot:OperationView={now_ms:0,paused:true,speed:1,invalidated:false,catching_up:false,timetable_edit_token:'initial-token',services:[],trains:[]};
const config={service_id:'12',timetable:{first_departure_ms:300000,last_departure_ms:64800000,headway_ms:360000},vehicle:{max_speed_kmh:100,acceleration_mps2:0.8,braking_mps2:1,length_m:80},dwell_ms:30000};
function operation(available=true,epoch=1,onChange=vi.fn()){
 return <OperationProvider available={available} blocked={false} epoch={epoch} onChange={onChange}><OperationClock/><ServiceOperations serviceId="12" enabled/></OperationProvider>;
}
test('no supported engine means no request, controls or background timer',async()=>{
 vi.useFakeTimers();
 render(operation(false));
 await act(async()=>vi.advanceTimersByTime(3000));
 expect(invoke).not.toHaveBeenCalled();expect(screen.queryByRole('group',{name:'ゲーム内時刻'})).toBeNull();
});
test('clock and service settings are authoritative and use the backend IPC units',async()=>{
 vi.mocked(invoke).mockImplementation(async command=>command==='operation_snapshot'?snapshot:{...snapshot,services:[config]});
 render(operation());await screen.findByText('1日目 00:00');
 expect(screen.getByText('この系統のダイヤは未設定です。')).toBeTruthy();
 expect((screen.getByText('再開') as HTMLButtonElement).disabled).toBe(true);
 fireEvent.change(screen.getByLabelText('初発時刻'),{target:{value:'00:05:00'}});
 fireEvent.change(screen.getByLabelText('運転間隔（分）'),{target:{value:'6'}});
 fireEvent.click(screen.getByText('運行を設定'));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('configure_operation_service',{serviceId:'12',expectedOperationToken:'initial-token',timetable:config.timetable,vehicle:config.vehicle,dwellMs:30000}));
 await waitFor(()=>expect((screen.getByText('再開') as HTMLButtonElement).disabled).toBe(false));
 fireEvent.change(screen.getByLabelText('速度'),{target:{value:'8'}});
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('set_operation_clock',{paused:true,speed:8}));
});
test('a pending timer reply does not swallow a manual pause',async()=>{
 vi.useFakeTimers();let release!:(view:OperationView)=>void;
 const gate=new Promise<OperationView>(resolve=>{release=resolve;});
 vi.mocked(invoke).mockImplementation(async(command)=>command==='advance_operation'?gate:command==='set_operation_clock'?{...snapshot,now_ms:500,services:[config]}:{...snapshot,paused:false,services:[config]});
 render(operation());await act(async()=>{});
 await act(async()=>vi.advanceTimersByTime(500));
 expect(invoke).toHaveBeenCalledWith('advance_operation',undefined);
 fireEvent.click(screen.getByText('一時停止'));
 expect(invoke).not.toHaveBeenCalledWith('set_operation_clock',{paused:true,speed:1});
 await act(async()=>release({...snapshot,now_ms:500,paused:false,services:[config]}));
 expect(invoke).toHaveBeenCalledWith('set_operation_clock',{paused:true,speed:1});
 expect(screen.getByText('再開')).toBeTruthy();
});
test('late operation snapshot cannot replace a new world',async()=>{
 let release!:(view:OperationView)=>void;const gate=new Promise<OperationView>(resolve=>{release=resolve;});
 vi.mocked(invoke).mockReturnValueOnce(gate).mockResolvedValue({...snapshot,now_ms:86_400_000});
 const h=render(operation());h.rerender(operation(true,2));
 await screen.findByText('2日目 00:00');
 await act(async()=>release(snapshot));
 expect(screen.queryByText('1日目 00:00')).toBeNull();
});

test('invalidated operation cannot resume, and reset requires explicit confirmation',async()=>{
 vi.mocked(invoke).mockResolvedValueOnce({...snapshot,now_ms:5000,invalidated:true}).mockResolvedValue(snapshot);
 render(operation());await screen.findByText(/運行を停止しました/);
 expect((screen.getByText('再開') as HTMLButtonElement).disabled).toBe(true);
 fireEvent.click(screen.getByText('運行をリセット'));
 fireEvent.click(screen.getByText('リセットを取り消す'));
 expect(invoke).not.toHaveBeenCalledWith('reset_operation',undefined);
 fireEvent.click(screen.getByText('運行をリセット'));
 fireEvent.click(screen.getByText('時刻とダイヤを消去'));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('reset_operation',undefined));
});

test('an owned bus route uses the separate native IPC and alone can start the shared clock',async()=>{
 const route={id:'bus:qa-route',name:'検証バスA'};
 const busConfig={route_id:route.id,timetable:{first_departure_ms:0,last_departure_ms:64800000,headway_ms:600000},vehicle:{max_speed_kmh:80,acceleration_mps2:1,braking_mps2:1.2,length_m:12},dwell_ms:30000,assumed_speed_kmh:35};
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='operation_snapshot')return snapshot;
  if(command==='configure_bus_operation')return {...snapshot,bus_services:[busConfig]};
  return {...snapshot,bus_services:[busConfig],trains:[{key:{service_id:'bus:bus:qa-route',scheduled_ms:0},leg:0,delay_ms:500,state:{state:'waiting',ready_ms:123}}]};
 });
 const dirty=vi.fn();
 render(<OperationProvider available blocked={false} epoch={1} onChange={dirty}><OperationClock/><BusOperations routes={[route]} enabled/></OperationProvider>);
 expect(await screen.findByText('検証バスA')).toBeTruthy();
 expect((screen.getByText('再開') as HTMLButtonElement).disabled).toBe(true);
 fireEvent.change(screen.getByLabelText('想定走行速度（km/h）'),{target:{value:'35'}});
 fireEvent.click(screen.getByText('運行を設定'));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('configure_bus_operation',{routeId:route.id,expectedOperationToken:'initial-token',timetable:busConfig.timetable,vehicle:busConfig.vehicle,dwellMs:30000,assumedSpeedKmh:35}));
 await waitFor(()=>expect((screen.getByText('再開') as HTMLButtonElement).disabled).toBe(false));
 expect(dirty).toHaveBeenCalledOnce();
 fireEvent.click(screen.getByText('再開'));
 const list=await screen.findByRole('list',{name:'運行中・待機中のバス'});
 expect(list.textContent).toContain('待機中');
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='configure_operation_service')).toBe(false);
});

test.each([
 ['first ordinary open configuration','openRoundTrip',false,undefined,true],
 ['existing unversioned open configuration','openRoundTrip',true,undefined,false],
 ['existing explicit open configuration','openRoundTrip',true,120000,false],
 ['closed route','closedOneWay',false,undefined,false],
 ['forward route',undefined,false,undefined,false],
] as const)('only a new ordinary open line gets the turnaround default: %s',async (_name,pattern,existing,turnaround,sendsDefault)=>{
 const route={id:'R1',name:'市内線',pattern};
 const prior={route_id:route.id,timetable:config.timetable,vehicle:config.vehicle,dwell_ms:30000,assumed_speed_kmh:30,turnaround_ms:turnaround};
 vi.mocked(invoke).mockResolvedValue({...snapshot,bus_services:existing?[prior]:[]});
 render(<OperationProvider available blocked={false} epoch={1} onChange={vi.fn()}><BusOperations routes={[route]} enabled/></OperationProvider>);
 const save=await screen.findByRole('button',{name:existing?'ダイヤの変更を適用':'運行を設定'});
 expect(!!screen.queryByText('終点の折り返し時間：120秒')).toBe(sendsDefault||turnaround!==undefined);
 fireEvent.click(save);
 await waitFor(()=>expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='configure_bus_operation')).toBe(true));
 const [,payload]=vi.mocked(invoke).mock.calls.find(([command])=>command==='configure_bus_operation')!;
 if(sendsDefault)expect(payload).toHaveProperty('turnaroundMs',120000);
 else expect(payload).not.toHaveProperty('turnaroundMs');
});

test('rail shows scheduled dwell, live path wait, and rear clearance separately',async()=>{
 const trains:OperationView['trains']=[
  {key:{service_id:'12',scheduled_ms:0},leg:1,delay_ms:0,state:{state:'waiting',ready_ms:7000}},
  {key:{service_id:'12',scheduled_ms:1000},leg:1,delay_ms:1100,waiting_for:'junction',state:{state:'waiting',ready_ms:2500}},
  {key:{service_id:'12',scheduled_ms:2000},leg:0,delay_ms:700,state:{state:'moving',entered_ms:2500,arrival_ms:5000,release_ms:5500}},
  {key:{service_id:'12',scheduled_ms:3000},leg:0,delay_ms:0,state:{state:'waiting',ready_ms:5100}},
 ];
 vi.mocked(invoke).mockResolvedValue({...snapshot,now_ms:5100,services:[config],trains});
 render(operation());
 const rows=(await screen.findByRole('list',{name:'運行中・待機中の列車'})).querySelectorAll('li');
 expect(rows[0].textContent).toContain('停車中');
 expect(rows[0].textContent).not.toContain('遅れ');
 expect(rows[1].textContent).toContain('進路待ち（分岐・接続）');
 expect(rows[1].textContent).toContain('遅れ 4秒');
 expect(rows[1].textContent).toContain('今回の進路待ち 3秒');
 expect(rows[2].textContent).toContain('到着済み');
 expect(rows[2].textContent).toContain('この区間の後部通過まで 1秒');
 expect(rows[2].textContent).not.toContain('今回の進路待ち');
 expect(rows[2].textContent).toContain('遅れ 1秒');
 expect(rows[3].textContent).toContain('進路待ち');
 expect(rows[3].textContent).not.toContain('遅れ');
 expect(rows[3].textContent).not.toContain('今回の進路待ち');
});

test('a train entering after a wait does not count the same delay twice',async()=>{
 const key={service_id:'12',scheduled_ms:1000};
 const wait={...snapshot,now_ms:6000,paused:false,services:[config],trains:[{key,leg:0,delay_ms:0,waiting_for:'crossing',state:{state:'waiting',ready_ms:2000}}]} satisfies OperationView;
 const enter={...wait,now_ms:6200,trains:[{key,leg:0,delay_ms:4200,state:{state:'moving',entered_ms:6200,arrival_ms:9000,release_ms:9500}}]} satisfies OperationView;
 vi.useFakeTimers();
 vi.mocked(invoke).mockImplementation(async command=>command==='advance_operation'?enter:wait);
 render(operation());
 await act(async()=>{});
 expect(screen.getByText('進路待ち（平面交差）')).toBeTruthy();
 expect(screen.getByText('遅れ 4秒')).toBeTruthy();
 expect(screen.getByText('今回の進路待ち 4秒')).toBeTruthy();
 await act(async()=>vi.advanceTimersByTime(500));
 expect(screen.getByText('走行中')).toBeTruthy();
 expect(screen.queryByText('進路待ち（平面交差）')).toBeNull();
 expect(screen.getByText('遅れ 5秒')).toBeTruthy();
 expect(screen.queryByText(/今回の進路待ち/)).toBeNull();
 expect(screen.queryByText(/後部通過/)).toBeNull();
 expect(screen.queryByText('進路待ち')).toBeNull();
});

test('a certified occupied platform has a visible rail wait reason until it clears',async()=>{
 const key={service_id:'12',scheduled_ms:1000};
 const stopped={...snapshot,now_ms:10000,paused:false,services:[config],trains:[
  {key,leg:0,delay_ms:0,waiting_for:'platform',state:{state:'waiting',ready_ms:1000}},
 ]} satisfies OperationView;
 const moving={...stopped,now_ms:10500,trains:[
  {key,leg:0,delay_ms:9500,state:{state:'moving',entered_ms:10500,arrival_ms:90000,release_ms:92000}},
 ]} satisfies OperationView;
 vi.useFakeTimers();
 vi.mocked(invoke).mockImplementation(async command=>command==='advance_operation'?moving:stopped);
 render(operation());
 await act(async()=>{});
 expect(screen.getByText('進路待ち（ホームにいる別の列車）')).toBeTruthy();
 expect(screen.getByText('遅れ 9秒')).toBeTruthy();
 await act(async()=>vi.advanceTimersByTime(500));
 expect(screen.getByText('走行中')).toBeTruthy();
 expect(screen.queryByText(/ホームにいる別の列車/)).toBeNull();
 expect(screen.getByText('遅れ 10秒')).toBeTruthy();
});

test('invalidated rail does not show active path wait or add unbooked delay',async()=>{
 vi.mocked(invoke).mockResolvedValue({...snapshot,now_ms:5600,services:[config],invalidated:true,trains:[
  {key:{service_id:'12',scheduled_ms:1000},leg:0,delay_ms:1000,state:{state:'waiting',ready_ms:2000}},
 ]} satisfies OperationView);
 render(operation());
 expect(await screen.findByText('運行停止中')).toBeTruthy();
 expect(screen.queryByText('進路待ち')).toBeNull();
 expect(screen.getByText('遅れ 1秒')).toBeTruthy();
});

test('rail timetable can change after an idle pause without resetting time, and writes the displayed native token',async()=>{
 const later={...snapshot,now_ms:250000,speed:4,timetable_edit_token:'post-run-4',services:[config]};
 const dirty=vi.fn();
 vi.mocked(invoke).mockImplementation(async command=>command==='operation_snapshot'?later:{
  ...later,timetable_edit_token:'next-token',services:[{...config,timetable:{...config.timetable,headway_ms:120000}}],
 });
 render(operation(true,1,dirty));
 expect(await screen.findByText('1日目 00:04')).toBeTruthy();
 expect(screen.getByText(/現在の時刻と他のダイヤはそのまま/)).toBeTruthy();
 expect(screen.getByText('ダイヤの変更を適用').matches(':disabled')).toBe(false);
 fireEvent.change(screen.getByLabelText('運転間隔（分）'),{target:{value:'2'}});
 fireEvent.click(screen.getByText('ダイヤの変更を適用'));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('configure_operation_service',{
  serviceId:'12',expectedOperationToken:'post-run-4',timetable:{...config.timetable,headway_ms:120000},vehicle:config.vehicle,dwellMs:30000,
 }));
 await waitFor(()=>expect(dirty).toHaveBeenCalledOnce());
 expect(screen.getByText('1日目 00:04')).toBeTruthy();
 expect((screen.getByLabelText('速度') as HTMLSelectElement).value).toBe('4');
 expect(invoke).not.toHaveBeenCalledWith('reset_operation',undefined);
});

test('native blocker forbids both timetable modes and explains how to reach the idle boundary',async()=>{
 const reason='走行中・停車中・進路待ちの列車やバスがいます。';
 const route={id:'qa-R',name:'検証バス'};
 const later={...snapshot,now_ms:250000,timetable_edit_token:'active-3',timetable_edit_blocker:reason,services:[config]};
 vi.mocked(invoke).mockResolvedValue(later);
 render(<OperationProvider available blocked={false} epoch={1} onChange={vi.fn()}><OperationClock/><ServiceOperations serviceId="12" enabled/><BusOperations routes={[route]} enabled/></OperationProvider>);
 expect((await screen.findAllByText(reason)).length).toBe(2);
 for(const label of ['ダイヤの変更を適用','運行を設定']){
  const button=screen.getByText(label) as HTMLButtonElement;
  expect(button.matches(':disabled')).toBe(true);
  fireEvent.click(button);
 }
 expect(vi.mocked(invoke).mock.calls.map(([command])=>command)).toEqual(['operation_snapshot']);
});

test('when invalidated but empty, a rejected stale edit preserves the input and does not dirty the world',async()=>{
 const later={...snapshot,now_ms:250000,invalidated:true,timetable_edit_token:'old-3',services:[config]};
 const dirty=vi.fn();
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='operation_snapshot')return later;
  throw Error('ワールドまたは運行状態が変更されました。');
 });
 render(operation(true,1,dirty));
 await screen.findByText(/ダイヤを適用し直すと/);
 expect((screen.getByText('再開') as HTMLButtonElement).disabled).toBe(true);
 fireEvent.change(screen.getByLabelText('運転間隔（分）'),{target:{value:'7'}});
 fireEvent.click(screen.getByText('ダイヤの変更を適用'));
 const error=await screen.findByRole('alert');
 expect(error.textContent).toContain('ワールドまたは運行状態が変更されました');
 expect((screen.getByLabelText('運転間隔（分）') as HTMLInputElement).value).toBe('7');
 expect(dirty).not.toHaveBeenCalled();
});

test('clock ticks and displays real vehicle state with the lean wire and no demand rows',async()=>{
 vi.useFakeTimers();
 const before={...snapshot,now_ms:59500,paused:false,services:[config]};
 const moving={key:{service_id:'12',scheduled_ms:59500},leg:0,delay_ms:0,state:{state:'moving',entered_ms:59500,arrival_ms:70000,release_ms:72000}} as const;
 const after={...before,now_ms:60000,timetable_edit_token:'second',trains:[moving]};
 vi.mocked(invoke).mockImplementation(async command=>command==='advance_operation'?after:before);
 render(operation());
 await act(async()=>{});
 expect(screen.getByText('1日目 00:00')).toBeTruthy();
 await act(async()=>vi.advanceTimersByTime(500));
 expect(invoke).toHaveBeenCalledWith('advance_operation',undefined);
 expect(screen.getByText('1日目 00:01')).toBeTruthy();
 expect(screen.getByText('走行中')).toBeTruthy();
});

test('a matching compact tick retains separate timetable rows and updates the active clock and tokens',async()=>{
 vi.useFakeTimers();
 const route={id:'R',name:'バス線'};
 const bus={route_id:route.id,timetable:{...config.timetable,headway_ms:900000},vehicle:config.vehicle,dwell_ms:30000,assumed_speed_kmh:35};
 const full={...snapshot,services:[config],bus_services:[bus],paused:false,now_ms:59500,configuration_token:'world:0'};
 const {services:_,bus_services:__,...live}=full;
 vi.mocked(invoke).mockImplementation(async(command)=>command==='advance_operation'
   ?{...live,paused:true,now_ms:60000,timetable_edit_token:'edit-live'}:full);
 render(<OperationProvider available blocked={false} epoch={1} onChange={vi.fn()}><OperationClock/><ServiceOperations serviceId="12" enabled/><BusOperations routes={[route]} enabled/></OperationProvider>);
 await act(async()=>{});
 await act(async()=>vi.advanceTimersByTime(500));
 expect(invoke).toHaveBeenCalledWith('advance_operation',{knownConfigurationToken:'world:0'});
 expect(screen.getByText('1日目 00:01')).toBeTruthy();
 expect(screen.getAllByLabelText('運転間隔（分）').map(x=>(x as HTMLInputElement).value)).toEqual(['6','15']);
 fireEvent.click(screen.getAllByText('ダイヤの変更を適用')[1]);
 await act(async()=>{});
 expect(invoke).toHaveBeenCalledWith('configure_bus_operation',expect.objectContaining({routeId:'R',expectedOperationToken:'edit-live'}));
 expect(vi.mocked(invoke).mock.calls.some(([cmd])=>cmd==='configure_operation_service')).toBe(false);
});

test('a full tick refreshes both timetable arrays after a native revision change',async()=>{
 vi.useFakeTimers();
 const full={...snapshot,services:[config],paused:false,configuration_token:'old'};
 const fresh={...snapshot,services:[{...config,timetable:{...config.timetable,headway_ms:720000}}],bus_services:[],paused:true,configuration_token:'new'};
 vi.mocked(invoke).mockImplementation(async cmd=>cmd==='advance_operation'?fresh:full);
 render(operation());await act(async()=>{});
 await act(async()=>vi.advanceTimersByTime(500));
 expect(invoke).toHaveBeenCalledWith('advance_operation',{knownConfigurationToken:'old'});
 expect((screen.getByLabelText('運転間隔（分）') as HTMLInputElement).value).toBe('12');
 expect(screen.getByText('再開')).toBeTruthy();
});

test.each([1,2])('a delayed lean response is discarded after a world revision change (next epoch %s)',async(nextEpoch)=>{
 vi.useFakeTimers();
 const full={...snapshot,services:[config],paused:false,configuration_token:'old'};
 const fresh={...snapshot,services:[],bus_services:[],now_ms:86400000,configuration_token:'new'};
 let resolveTick!:(reply:unknown)=>void;
 const pending=new Promise(resolve=>{resolveTick=resolve;});
 let requested=0;
 vi.mocked(invoke).mockImplementation(async cmd=>cmd==='advance_operation'?pending:++requested===1?full:fresh);
 const page=(epoch:number,version:number)=><OperationProvider available blocked={false} epoch={epoch} worldVersion={version} onChange={vi.fn()}><OperationClock/><ServiceOperations serviceId="12" enabled/></OperationProvider>;
 const h=render(page(1,1));await act(async()=>{});
 await act(async()=>vi.advanceTimersByTime(500));
 expect(invoke).toHaveBeenCalledWith('advance_operation',{knownConfigurationToken:'old'});
 h.rerender(page(nextEpoch,2));await act(async()=>{});
 const {services:_,...live}=full;
 await act(async()=>resolveTick({...live,now_ms:60000}));
 expect(screen.getByText('2日目 00:00')).toBeTruthy();
 expect(screen.queryByText('1日目 00:01')).toBeNull();
 expect(screen.getByText('この系統のダイヤは未設定です。')).toBeTruthy();
});

test('a mismatched compact token is rejected instead of borrowing stale timetable rows',async()=>{
 vi.useFakeTimers();
 const full={...snapshot,services:[config],paused:false,configuration_token:'old'};
 const {services:_,...live}=full;
 const fresh={...snapshot,now_ms:86400000,configuration_token:'fresh'};
 let snapshots=0;
 vi.mocked(invoke).mockImplementation(async cmd=>cmd==='advance_operation'
  ?{...live,paused:true,now_ms:60000,configuration_token:'wrong'}
  :++snapshots===1?full:fresh);
 render(operation());await act(async()=>{});
 await act(async()=>vi.advanceTimersByTime(500));
 expect(screen.getByRole('alert').textContent).toContain('運行エンジンの応答を確認できませんでした');
 expect(screen.queryByText('1日目 00:01')).toBeNull();
 fireEvent.click(screen.getByText('運行状態を再取得'));
 await act(async()=>{});
 expect(screen.getByText('2日目 00:00')).toBeTruthy();
 expect(screen.getByText('この系統のダイヤは未設定です。')).toBeTruthy();
});

test('the active handoff exposes only the existing rail timetable and submits unchanged vehicle and dwell', async()=>{
 const paused={...snapshot,now_ms:35000,timetable_edit_blocker:'車両が走行中',rail_timetable_handoff_available:true,services:[config]};
 vi.mocked(invoke).mockResolvedValue(paused);
 render(operation());
 const submit=await screen.findByRole('button',{name:'今後の発車時刻を変更'});
 expect((screen.getByLabelText('駅の停車時間（秒）') as HTMLInputElement).disabled).toBe(true);
 expect((screen.getByLabelText('最高速度（km/h）') as HTMLInputElement).disabled).toBe(true);
 fireEvent.change(screen.getByLabelText('初発時刻'),{target:{value:'00:01:30'}});
 fireEvent.click(submit);
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('configure_operation_service',{
  serviceId:'12',expectedOperationToken:'initial-token',timetable:{...config.timetable,first_departure_ms:90000},
  vehicle:config.vehicle,dwellMs:config.dwell_ms
 }));
});

test('clock warns about older builds only while native reports mixed timetable births',async()=>{
 vi.mocked(invoke).mockImplementation(async command=>command==='operation_snapshot'
  ?{...snapshot,rail_timetable_handoff_in_progress:true,services:[config]}
  :{...snapshot,services:[config]});
 render(operation());
 const notice=await screen.findByText(/この間に保存したデータは旧ビルドでは開けません/);
 expect(notice.getAttribute('role')).toBe('status');
 fireEvent.change(screen.getByLabelText('速度'),{target:{value:'4'}});
 await waitFor(()=>expect(screen.queryByText(/この間に保存したデータは旧ビルドでは開けません/)).toBeNull());
});

test('rail describes certified multi-connection waiting without exposing identifiers or treating normal junction waits as special',async()=>{
 const train=(scheduled_ms:number):OperationView['trains'][number]=>({key:{service_id:'12',scheduled_ms},leg:0,delay_ms:0,waiting_for:'junction',state:{state:'waiting',ready_ms:100}});
 const trains:OperationView['trains']=[
  {...train(100),wait_scope:'held_across_connections'},
  {...train(200),wait_scope:'needs_across_connections'},
  train(300),
  {...train(400),wait_scope:'held_across_connections',state:{state:'waiting',ready_ms:6000}},
 ];
 vi.mocked(invoke).mockResolvedValue({...snapshot,now_ms:5100,services:[config],trains});
 render(operation());
 const rows=(await screen.findByRole('list',{name:'運行中・待機中の列車'})).querySelectorAll('li');
 expect(rows[0].textContent).toContain('相手列車が、接続部をまたぐ進路を確保中');
 expect(rows[1].textContent).toContain('次の停車駅まで、接続部をまたぐ進路が空くのを待っています');
 expect(rows[2].textContent).toContain('進路待ち（分岐・接続）');
 expect(rows[2].textContent).not.toContain('またぐ');
 expect(rows[3].textContent).not.toContain('またぐ');
 expect([...rows].map(r=>r.textContent).join(' ')).not.toMatch(/held_across_connections|needs_across_connections|rail:/u);
});

test.each(['rail only','second rail','bus','catch-up'] as const)('Standard cold Apply does not require a dummy run: %s',async kind=>{
 const bus={route_id:'other',timetable:config.timetable,vehicle:config.vehicle,dwell_ms:30000,assumed_speed_kmh:20};
 const view={...snapshot,configuration_token:'cold',services:kind==='second rail'?[config,{...config,service_id:'13'}]:[config],
  bus_services:kind==='bus'?[bus]:[],catching_up:kind==='catch-up',
  timetable_edit_blocker:kind==='catch-up'?'未処理の運行が残っています。':null};
 vi.mocked(invoke).mockImplementation(async command=>command==='standard_schedule_status'
  ?{configurationToken:'cold',serviceId:'12',canApply:true,activeElsewhere:false,authored:null,departureCount:null,destinations:[]}
  :view);
 render(<OperationProvider available blocked={false} epoch={1} onChange={vi.fn()}><ServiceOperations serviceId="12" enabled standardEnabled/></OperationProvider>);
 const apply=await screen.findByRole('button',{name:'時間帯ダイヤを適用'});
 expect((apply as HTMLButtonElement).disabled).toBe(kind!=='rail only');
 expect(screen.getByText(/この系統の従来ダイヤを置き換えます/)).toBeTruthy();
 fireEvent.click(apply);
 expect(vi.mocked(invoke).mock.calls.some(([which])=>which==='apply_standard_schedule')).toBe(kind==='rail only');
 expect(vi.mocked(invoke).mock.calls.some(([which])=>which==='set_operation_clock'||which==='advance_operation')).toBe(false);
});

test('the Standard form follows live catch-up and pause without renewing cached eligibility',async()=>{
 vi.useFakeTimers();
 const pending={...snapshot,now_ms:60000,paused:false,catching_up:true,configuration_token:'same',services:[config],
  timetable_edit_blocker:'ダイヤを変更するには先に運行を一時停止してください。'};
 const caughtUp={...pending,now_ms:60500,catching_up:false};
 vi.mocked(invoke).mockImplementation(async command=>command==='standard_schedule_status'
  ?{configurationToken:'same',serviceId:'12',canApply:true,activeElsewhere:false,authored:null,departureCount:null,destinations:[]}
  :command==='advance_operation'?caughtUp:command==='set_operation_clock'
  ?{...caughtUp,paused:true,timetable_edit_token:'paused-token',timetable_edit_blocker:null}:pending);
 render(<OperationProvider available blocked={false} epoch={1} onChange={vi.fn()}>
  <OperationClock/><ServiceOperations serviceId="12" enabled standardEnabled/>
 </OperationProvider>);
 await act(async()=>{});
 const form=screen.getByRole('region',{name:'標準の時間帯ダイヤ'});
 const status=()=>form.querySelector('[role="status"]')?.textContent;
 const apply=()=>screen.getByRole('button',{name:'時間帯ダイヤを適用'}) as HTMLButtonElement;
 expect(status()).toContain('運行が時刻に追いついてから');
 expect(apply().disabled).toBe(true);
 await act(async()=>vi.advanceTimersByTime(500));
 expect(status()).toBe(pending.timetable_edit_blocker);
 expect(apply().disabled).toBe(true);
 await act(async()=>fireEvent.click(screen.getByRole('button',{name:'一時停止'})));
 expect(status()).toBeUndefined();
 expect(apply().disabled).toBe(false);
 expect(vi.mocked(invoke).mock.calls.filter(([which])=>which==='standard_schedule_status')).toHaveLength(1);
});

test('time-band defaults follow the selected service and AM/PM overrides remain independent',async()=>{
 const view={...snapshot,now_ms:60_000,configuration_token:'freq',services:[config,
  {...config,service_id:'13',timetable:{...config.timetable,headway_ms:900_000}}]};
 const ipc=vi.mocked(invoke);
 ipc.mockImplementation(async (command,args)=>command==='standard_schedule_status'
  ?{configurationToken:'freq',serviceId:(args as {service:string}).service,canApply:true,activeElsewhere:false,
    authored:null,departureCount:null,destinations:[]}
  :view);
 const display=(id:string)=><OperationProvider available blocked={false} epoch={1} onChange={vi.fn()}>
  <ServiceOperations serviceId={id} enabled standardEnabled/>
 </OperationProvider>;
 const {rerender}=render(display('13'));
 expect((await screen.findByLabelText('基本の間隔（分）') as HTMLInputElement).value).toBe('15');
 fireEvent.change(screen.getByLabelText('朝ラッシュの間隔（分）'),{target:{value:'4'}});
 rerender(display('12'));
 expect((await screen.findByLabelText('基本の間隔（分）') as HTMLInputElement).value).toBe('6');
 const morning=screen.getByLabelText('朝ラッシュの間隔（分）') as HTMLInputElement;
 expect(morning.value).toBe('');
 fireEvent.change(morning,{target:{value:'3'}});
 fireEvent.change(screen.getByLabelText('夕ラッシュの間隔（分）'),{target:{value:'5'}});
 expect(ipc.mock.calls.some(([command])=>command==='apply_standard_schedule')).toBe(false);
 fireEvent.click(screen.getByRole('button',{name:'時間帯ダイヤを適用'}));
 await waitFor(()=>expect(ipc).toHaveBeenCalledWith('apply_standard_schedule',{
  service:'12',expected:'initial-token',baseMinutes:6,
  overrides:{earlyMorning:null,morningRush:3,midday:null,eveningRush:5,lateNight:null},
  boundariesMinutes:[0,360,600,960,1200,1440],
 }));
});

test('time-band fixed OD can board an intermediate stop, only toward later stops',async()=>{
 const conditions={configurationToken:'outbound',serviceId:'12',canApply:false,activeElsewhere:false,
  authored:{baseMinutes:10,overrides:{earlyMorning:null,morningRush:6,midday:null,eveningRush:null,lateNight:null},
   boundariesMinutes:[0,360,600,960,1200,1440],startMs:60_000,endMs:86_460_000},
  departureCount:144,origin:{id:'101',name:'起点駅'},destinations:[{id:'102',name:'中間駅'},{id:'103',name:'終点駅'}]};
 const result={status:'scheduled',first_departure_ms:185_250,scheduled_departures:1,
  assignment:{rail:[{trips:75,time:{wait_seconds:65.25,ride_seconds:95.5}}],alternative_trips:25,unserved_trips:0,unresolved_trips:0}};
 const ipc=vi.mocked(invoke);
 ipc.mockImplementation(async command=>command==='standard_schedule_status'?conditions:
  command==='preview_standard_schedule_od'?result:
  {...snapshot,configuration_token:'outbound',standard_schedule_active:true,services:[config]});
 render(<OperationProvider available blocked={false} epoch={1} onChange={vi.fn()}>
  <ServiceOperations serviceId="12" enabled standardEnabled/>
 </OperationProvider>);
 const boarding=(await screen.findByLabelText('乗車駅')) as HTMLSelectElement;
 const arrival=screen.getByLabelText('到着駅') as HTMLSelectElement;
 const submit=screen.getByRole('button',{name:'この時間の固定ODを試算'});
 expect(Array.from(boarding.options,option=>option.value)).toEqual(['101','102']);
 expect(arrival.value).toBe('102');
 fireEvent.change(boarding,{target:{value:'102'}});
 expect(Array.from(arrival.options,option=>option.value)).toEqual(['103']);
 expect(arrival.value).toBe('103');
 expect(ipc.mock.calls.some(([command])=>command==='preview_standard_schedule_od')).toBe(false);
 fireEvent.change(screen.getByLabelText('起点のダイヤ適用開始から何分後に乗車駅で待つか'),{target:{value:'1'}});
 fireEvent.click(submit);
 expect(await screen.findByText('鉄道 75.0人、待ち 65秒、乗車 96秒')).toBeTruthy();
 expect(ipc).toHaveBeenCalledWith('preview_standard_schedule_od',{service:'12',revision:'outbound',
  query:{origin:'102',destination:'103',trips:100,alternativeSeconds:null,startMs:120_000,endMs:3_720_000}});
 fireEvent.change(boarding,{target:{value:'101'}});
 expect(screen.queryByText(/鉄道 75\.0人/u)).toBeNull();
 expect(Array.from(arrival.options,option=>option.value)).toEqual(['102','103']);
 expect(arrival.value).toBe('103');
 expect(ipc.mock.calls.filter(([command])=>command==='preview_standard_schedule_od')).toHaveLength(1);
});

test('an active time-band timetable can be edited without resetting the clock',async()=>{
 const view={...snapshot,now_ms:1_000_000,configuration_token:'active',timetable_edit_token:'active:1000000',
  services:[config],standard_schedule_active:true};
 const authored={baseMinutes:30,overrides:{earlyMorning:null,morningRush:4,midday:null,eveningRush:7,lateNight:null},
  boundariesMinutes:[60,420,600,960,1200,1440],startMs:1_080_000,endMs:87_480_000};
 const ipc=vi.mocked(invoke);
 ipc.mockImplementation(async command=>command==='standard_schedule_status'
  ?{configurationToken:'active',serviceId:'12',canApply:true,activeElsewhere:false,
    authored,departureCount:50,origin:null,destinations:[]}
  :view);
 render(<OperationProvider available blocked={false} epoch={1} onChange={vi.fn()}>
  <ServiceOperations serviceId="12" enabled standardEnabled/>
 </OperationProvider>);
 const edit=await screen.findByRole('button',{name:'時間帯ダイヤを変更'});
 fireEvent.click(edit);
 expect((screen.getByLabelText('基本の間隔（分）') as HTMLInputElement).value).toBe('30');
 expect((screen.getByLabelText('朝ラッシュの間隔（分）') as HTMLInputElement).value).toBe('4');
 expect((screen.getByLabelText('夕ラッシュの間隔（分）') as HTMLInputElement).value).toBe('7');
 fireEvent.change(screen.getByLabelText('朝ラッシュの間隔（分）'),{target:{value:'2'}});
 fireEvent.click(screen.getByRole('button',{name:'変更を取り消す'}));
 expect(ipc.mock.calls.some(([name])=>name==='apply_standard_schedule'||name==='reset_operation')).toBe(false);
 fireEvent.click(await screen.findByRole('button',{name:'時間帯ダイヤを変更'}));
 expect((screen.getByLabelText('朝ラッシュの間隔（分）') as HTMLInputElement).value).toBe('4');
 fireEvent.change(screen.getByLabelText('朝ラッシュの間隔（分）'),{target:{value:'3'}});
 expect(screen.getByText(/他の系統や時刻はそのまま/u)).toBeTruthy();
 fireEvent.click(screen.getByRole('button',{name:'変更を適用'}));
 await waitFor(()=>expect(ipc).toHaveBeenCalledWith('apply_standard_schedule',{
  service:'12',expected:'active:1000000',baseMinutes:30,
  overrides:{earlyMorning:null,morningRush:3,midday:null,eveningRush:7,lateNight:null},boundariesMinutes:[60,420,600,960,1200,1440],
 }));
 expect(ipc.mock.calls.some(([name])=>name==='reset_operation')).toBe(false);
});

test('a fleet estimate applies the exact headway through the current native edit token without resetting time',async()=>{
 const current={...snapshot,now_ms:900_000,timetable_edit_token:'current-edit',configuration_token:'fleet-config',services:[
  {...config,timetable:{...config.timetable,first_departure_ms:301_234}},
 ]};
 const adjusted={...current.services[0],timetable:{...current.services[0].timetable,headway_ms:126_667}};
 const estimate={cycleMs:380_000,runningMs:300_000,dwellAndClearanceMs:80_000,headwayMs:126_667,trainsets:3};
 vi.mocked(invoke).mockImplementation(async command=>command==='operation_snapshot'?current:
  command==='preview_nominal_trainsets'?estimate:{...current,configuration_token:'fleet-config-next',services:[adjusted]});
 render(operation());
 fireEvent.click(await screen.findByText('編成数と運転間隔の試算'));
 fireEvent.change(screen.getByLabelText('試算の基準'),{target:{value:'trainsetCount'}});
 fireEvent.change(screen.getByLabelText('試算する編成数'),{target:{value:'3'}});
 fireEvent.click(screen.getByRole('button',{name:'編成数・間隔を試算'}));
 const apply=await screen.findByRole('button',{name:'試算した間隔を現在のダイヤに反映'});
 expect(invoke).toHaveBeenCalledWith('preview_nominal_trainsets',{serviceId:'12',configurationToken:'fleet-config',request:{mode:'trainsetCount',trainsets:3}});
 expect(invoke).not.toHaveBeenCalledWith('configure_operation_service',expect.anything());
 fireEvent.click(apply);
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('configure_operation_service',{
  serviceId:'12',expectedOperationToken:'current-edit',timetable:adjusted.timetable,vehicle:adjusted.vehicle,dwellMs:adjusted.dwell_ms,
 }));
 expect(invoke).not.toHaveBeenCalledWith('reset_operation',undefined);
 expect(screen.getByText('1日目 00:15')).toBeTruthy();
});

test('read-only fleet estimates are available but cannot mutate a blocked timetable',async()=>{
 const current={...snapshot,configuration_token:'fleet-config',timetable_edit_blocker:'運行中の車両がいます。',services:[config]};
 vi.mocked(invoke).mockImplementation(async command=>command==='operation_snapshot'?current:
  {cycleMs:240_000,runningMs:180_000,dwellAndClearanceMs:60_000,headwayMs:120_000,trainsets:2});
 render(operation());
 fireEvent.click(await screen.findByText('編成数と運転間隔の試算'));
 fireEvent.change(screen.getByLabelText('試算の基準'),{target:{value:'trainsetCount'}});
 fireEvent.click(screen.getByRole('button',{name:'編成数・間隔を試算'}));
 expect(await screen.findByText('最短運転間隔の目安')).toBeTruthy();
 expect(screen.queryByRole('button',{name:'試算した間隔を現在のダイヤに反映'})).toBeNull();
 expect(invoke).not.toHaveBeenCalledWith('configure_operation_service',expect.anything());
});

test('returns only the selected rail service to its existing simple schedule with no clock reset',async()=>{
 const running={...snapshot,now_ms:70_000_000,timetable_edit_token:'standard-stop',configuration_token:'std',
  standard_schedule_active:true,services:[config]};
 const status={configurationToken:'std',serviceId:'12',canApply:true,activeElsewhere:false,departureCount:200,origin:null,destinations:[],
  authored:{baseMinutes:6,overrides:{earlyMorning:null,morningRush:null,midday:null,eveningRush:null,lateNight:null},
  boundariesMinutes:[0,360,600,960,1200,1440],startMs:60_000,endMs:86_460_000}};
 vi.mocked(invoke).mockImplementation(async command=>command==='standard_schedule_status'?status:
  command==='restore_simple_rail_schedule'?{...running,configuration_token:'simple',standard_schedule_active:false}:running);
 render(<OperationProvider available blocked={false} epoch={1} onChange={vi.fn()}>
  <OperationClock/><ServiceOperations serviceId="12" enabled standardEnabled/>
 </OperationProvider>);
 const restore=await screen.findByRole('button',{name:'元の簡易ダイヤに戻す'});
 expect(screen.getByText(/元の終発は過ぎています/)).toBeTruthy();
 expect(invoke).not.toHaveBeenCalledWith('restore_simple_rail_schedule',expect.anything());
 fireEvent.click(restore);
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('restore_simple_rail_schedule',{service:'12',expected:'standard-stop'}));
 expect(invoke).not.toHaveBeenCalledWith('reset_operation',undefined);
 expect(screen.getByText('1日目 19:26')).toBeTruthy();
});
