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
  {key:{service_id:'12',scheduled_ms:1000},leg:1,delay_ms:1100,state:{state:'waiting',ready_ms:2500}},
  {key:{service_id:'12',scheduled_ms:2000},leg:0,delay_ms:700,state:{state:'moving',entered_ms:2500,arrival_ms:5000,release_ms:5500}},
  {key:{service_id:'12',scheduled_ms:3000},leg:0,delay_ms:0,state:{state:'waiting',ready_ms:5100}},
 ];
 vi.mocked(invoke).mockResolvedValue({...snapshot,now_ms:5100,services:[config],trains});
 render(operation());
 const rows=(await screen.findByRole('list',{name:'運行中・待機中の列車'})).querySelectorAll('li');
 expect(rows[0].textContent).toContain('停車中');
 expect(rows[0].textContent).not.toContain('遅れ');
 expect(rows[1].textContent).toContain('進路待ち');
 expect(rows[1].textContent).toContain('遅れ 4秒');
 expect(rows[2].textContent).toContain('到着済み');
 expect(rows[2].textContent).toContain('遅れ 1秒');
 expect(rows[3].textContent).toContain('進路待ち');
 expect(rows[3].textContent).not.toContain('遅れ');
});

test('a train entering after a wait does not count the same delay twice',async()=>{
 const key={service_id:'12',scheduled_ms:1000};
 const wait={...snapshot,now_ms:6000,paused:false,services:[config],trains:[{key,leg:0,delay_ms:0,state:{state:'waiting',ready_ms:2000}}]} satisfies OperationView;
 const enter={...wait,now_ms:6200,trains:[{key,leg:0,delay_ms:4200,state:{state:'moving',entered_ms:6200,arrival_ms:9000,release_ms:9500}}]} satisfies OperationView;
 vi.useFakeTimers();
 vi.mocked(invoke).mockImplementation(async command=>command==='advance_operation'?enter:wait);
 render(operation());
 await act(async()=>{});
 expect(screen.getByText('進路待ち')).toBeTruthy();
 expect(screen.getByText('遅れ 4秒')).toBeTruthy();
 await act(async()=>vi.advanceTimersByTime(500));
 expect(screen.getByText('走行中')).toBeTruthy();
 expect(screen.getByText('遅れ 5秒')).toBeTruthy();
 expect(screen.queryByText('進路待ち')).toBeNull();
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
