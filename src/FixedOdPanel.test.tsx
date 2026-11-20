// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {FixedOdPanel} from './FixedOdPanel';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
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
