// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {OperationClock,OperationProvider,ServiceOperations,type OperationView} from './OperationsPanel';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();vi.useRealTimers();});
const snapshot:OperationView={now_ms:0,paused:true,speed:1,services:[],trains:[]};
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
 vi.mocked(invoke).mockResolvedValue(snapshot);
 render(operation());await screen.findByText('1日目 00:00');
 expect(screen.getByText('この系統のダイヤは未設定です。')).toBeTruthy();
 fireEvent.change(screen.getByLabelText('速度'),{target:{value:'8'}});
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('set_operation_clock',{paused:true,speed:8}));
 await waitFor(()=>expect((screen.getByText('運行を設定') as HTMLButtonElement).disabled).toBe(false));
 fireEvent.change(screen.getByLabelText('初発時刻'),{target:{value:'00:05:00'}});
 fireEvent.change(screen.getByLabelText('運転間隔（分）'),{target:{value:'6'}});
 fireEvent.click(screen.getByText('運行を設定'));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('configure_operation_service',{serviceId:'12',timetable:{first_departure_ms:300000,last_departure_ms:64800000,headway_ms:360000},vehicle:{max_speed_kmh:100,acceleration_mps2:0.8,braking_mps2:1,length_m:80},dwellMs:30000}));
});
test('a pending timer reply does not swallow a manual pause',async()=>{
 vi.useFakeTimers();let release!:(view:OperationView)=>void;
 const gate=new Promise<OperationView>(resolve=>{release=resolve;});
 vi.mocked(invoke).mockImplementation(async(command)=>command==='advance_operation'?gate:command==='set_operation_clock'?{...snapshot,now_ms:500}:{...snapshot,paused:false});
 render(operation());await act(async()=>{});
 await act(async()=>vi.advanceTimersByTime(500));
 expect(invoke).toHaveBeenCalledWith('advance_operation',undefined);
 fireEvent.click(screen.getByText('一時停止'));
 expect(invoke).not.toHaveBeenCalledWith('set_operation_clock',{paused:true,speed:1});
 await act(async()=>release({...snapshot,now_ms:500,paused:false}));
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
