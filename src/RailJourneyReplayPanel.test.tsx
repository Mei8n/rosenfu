// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {RailJourneyReplayPanel} from './RailJourneyReplayPanel';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
const stations=[{id:'1',name:'東駅'},{id:'2',name:'西駅'}];
const services=[{id:'3',name:'快速'}];
const panel=(worldVersion=1)=><RailJourneyReplayPanel stations={stations} services={services} enabled worldVersion={worldVersion}/>;
const result={version:1,ready_ms:0,requested_until_ms:1_800_000,observed_until_ms:1_800_000,status:'reached',arrival_ms:186_480,platform_wait_ms:94_240,onboard_ms:92_240,rides:[{service_id:'3',from:'1',to:'2',departed_ms:94_240,arrived_ms:186_480}]};
function enter(){
 fireEvent.change(screen.getByLabelText('乗車駅'),{target:{value:'1'}});
 fireEvent.change(screen.getByLabelText('降車駅'),{target:{value:'2'}});
 fireEvent.click(screen.getByText('運行を試算'));
}
test('show departure and arrival from native replay; never send a planned OD request',async()=>{
 vi.mocked(invoke).mockResolvedValue(result);
 render(panel());enter();
 expect(await screen.findByText('到着 00:03:06 / 駅での待ち 00:01:34 / 車内 00:01:32 / 乗換 0回')).toBeTruthy();
 expect(screen.getByText(/東駅 → .*西駅（快速）/)).toBeTruthy();
 expect(vi.mocked(invoke).mock.calls).toEqual([['preview_rail_journey_replay',{origin:'1',destination:'2'}]]);
});
test('event-truncated and observed-window-limited results stay distinct',async()=>{
 vi.mocked(invoke).mockResolvedValueOnce({...result,status:'incomplete',arrival_ms:null,rides:[]})
  .mockResolvedValueOnce({...result,status:'no_arrival_within_window',arrival_ms:null,rides:[]});
 render(panel());enter();
 expect(await screen.findByText(/到着できるかは未確定/)).toBeTruthy();
 fireEvent.click(screen.getByText('運行を試算'));
 expect(await screen.findByText(/以降の運行は未確認/)).toBeTruthy();
});
test('suppress stale replay when world/operation changes',async()=>{
 let resolve!:(value:typeof result)=>void;
 vi.mocked(invoke).mockReturnValue(new Promise(done=>{resolve=done;}));
 const view=render(panel());enter();view.rerender(panel(2));await act(async()=>resolve(result));
 expect(screen.queryByText(/駅での待ち/)).toBeNull();
 expect((screen.getByText('運行を試算') as HTMLButtonElement).disabled).toBe(false);
});
