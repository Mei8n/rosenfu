// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {ServicePathCreator} from './ServicePathCreator';
import {resolveServicePath} from './servicePaths';
import {ServiceRouteBindingEditor} from './ServiceRouteBinding';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
const segment=(from:string,to:string)=>({kind:'alignment' as const,alignment_id:'line',from,to});
const routes=[{id:'r1',name:'A線',start:'a'},{id:'r2',name:'B線',start:'b'}];
const routePaths=[{route_id:'r1',segments:[segment('a','b')],member_stations:['a','b']},{route_id:'r2',segments:[segment('b','c')],member_stations:['b','c']}];
const stations=[{id:'a',name:'始発'},{id:'b',name:'境界'},{id:'c',name:'終点'}];
const world={station_placements:stations.map((s,i)=>({station_id:s.id,alignment_id:'line',position:i*.5}))};
test('creates a through service with explicit routes and uninterrupted stops',async()=>{
 const save=vi.fn().mockResolvedValue(undefined);
 vi.mocked(invoke).mockImplementation(async(_command,args:any)=>{const legs=args.segments.map((segment:any,i:number)=>({key:String(i+1),segment}));return {legs,occurrences:resolveServicePath(args.start,legs,world),requires_stop_selection:true};});
 render(<ServicePathCreator routes={routes} routePaths={routePaths} stations={stations} world={world} enabled canStart onDirty={vi.fn()} onSave={save}/>);
 fireEvent.change(screen.getByLabelText('コピーする路線'),{target:{value:'r1'}});
 fireEvent.click(screen.getByRole('button',{name:'直通区間に追加'}));
 fireEvent.change(screen.getByLabelText('コピーする路線'),{target:{value:'r2'}});
 fireEvent.click(screen.getByRole('button',{name:'選択区間の停車候補を確認'}));
 await screen.findByLabelText('新形式停車 3');
 fireEvent.change(screen.getByLabelText('新形式の系統名'),{target:{value:'直通'}});
 fireEvent.click(screen.getByLabelText('全駅停車（所属外を含む）'));
 fireEvent.click(screen.getByRole('button',{name:'新形式系統を適用'}));
 await waitFor(()=>expect(save).toHaveBeenCalledOnce());
 expect(save.mock.calls[0][0]).toEqual({sourceRouteIds:['r1','r2'],name:'直通',start:'a',segments:[segment('a','b'),segment('b','c')],stops:[{kind:'start'},{kind:'on_leg',leg_key:'1',station_id:'b'},{kind:'on_leg',leg_key:'2',station_id:'c'}]});
});
test('does not bridge a discontinuity and permits cancelling the staged route',async()=>{
 const save=vi.fn().mockRejectedValue(Error('conflict'));
 render(<ServicePathCreator routes={routes} routePaths={routePaths} stations={stations} world={world} enabled canStart onDirty={vi.fn()} onSave={save}/>);
 fireEvent.change(screen.getByLabelText('コピーする路線'),{target:{value:'r1'}});fireEvent.click(screen.getByRole('button',{name:'直通区間に追加'}));
 fireEvent.change(screen.getByLabelText('コピーする路線'),{target:{value:'r1'}});
 expect((screen.getByRole('button',{name:'選択区間の停車候補を確認'}) as HTMLButtonElement).disabled).toBe(true);
 expect(screen.getByRole('alert').textContent).toContain('同じ駅');
 expect(invoke).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole('button',{name:'直通区間を取り消す'}));
 expect(screen.queryByLabelText('直通する路線')).toBeNull();
});
test('multiple route association is explicit and cannot be saved with only one route',async()=>{
 const save=vi.fn().mockResolvedValue(undefined);
 render(<ServiceRouteBindingEditor binding={{kind:'route',route_id:'r1'}} routes={routes} enabled onDirty={vi.fn()} onSave={save}/>);
 fireEvent.change(screen.getByLabelText('関連する路線'),{target:{value:'routes'}});
 fireEvent.click(screen.getByLabelText('A線'));
 expect((screen.getByRole('button',{name:'関連付けを適用'}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.click(screen.getByLabelText('B線'));fireEvent.click(screen.getByRole('button',{name:'関連付けを適用'}));
 await waitFor(()=>expect(save).toHaveBeenCalledWith({kind:'routes',route_ids:['r1','r2']}));
});
