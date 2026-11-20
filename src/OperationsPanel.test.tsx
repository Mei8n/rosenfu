// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {BusOperations,OperationClock,OperationProvider,ServiceOperations,type OperationView} from './OperationsPanel';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();vi.useRealTimers();});
const snapshot:OperationView={now_ms:0,paused:true,speed:1,invalidated:false,catching_up:false,services:[],trains:[]};
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
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('configure_operation_service',{serviceId:'12',timetable:config.timetable,vehicle:config.vehicle,dwellMs:30000}));
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
 render(operation());await screen.findByText(/再開するには運行をリセット/);
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
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('configure_bus_operation',{routeId:route.id,timetable:busConfig.timetable,vehicle:busConfig.vehicle,dwellMs:30000,assumedSpeedKmh:35}));
 await waitFor(()=>expect((screen.getByText('再開') as HTMLButtonElement).disabled).toBe(false));
 expect(dirty).toHaveBeenCalledOnce();
 fireEvent.click(screen.getByText('再開'));
 const list=await screen.findByRole('list',{name:'運行中・待機中のバス'});
 expect(list.textContent).toContain('待機中');
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='configure_operation_service')).toBe(false);
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
