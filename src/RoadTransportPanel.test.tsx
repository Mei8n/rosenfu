// @vitest-environment jsdom
import {render,screen,fireEvent,waitFor,cleanup} from '@testing-library/react';
import {describe,it,expect,vi,afterEach} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {RoadTransportPanel,type RoadTransportData} from './RoadTransportPanel';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
const empty:RoadTransportData={nodes:[],edges:[],bus_stops:[],bus_routes:[],tram_sections:[]};
describe('road QA gate',()=>{
 it('requires explicit acknowledgement and never claims real road coverage',async()=>{
  const run=vi.fn().mockResolvedValue({});render(<RoadTransportPanel data={empty} companyId="1" run={run}/>);
  const button=screen.getByRole('button',{name:'検証用道路を追加'});expect((button as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole('checkbox'));fireEvent.click(button);
  await waitFor(()=>expect(run).toHaveBeenCalledWith('enable_road_qa_fixture',{confirmedSynthetic:true}));
 });
 it('respects parent busy and world guards',()=>{
  const run=vi.fn();render(<RoadTransportPanel data={empty} companyId="1" run={run} enabled={false}/>);
  fireEvent.click(screen.getByRole('checkbox'));fireEvent.click(screen.getByRole('button',{name:'検証用道路を追加'}));expect(run).not.toHaveBeenCalled();
 });
});

it('can use map endpoints to commit a native tram construction',async()=>{
 const run=vi.fn().mockResolvedValue({});
 const point={latitude:35,longitude:135};
 const data:RoadTransportData={...empty,nodes:[{id:'qa:0',point},{id:'qa:1',point}],edges:[{id:'qa:0-1',from:'qa:0',to:'qa:1',source:'synthetic-qa-not-real-roads',revision:'1',geometry:[point,point],bus_access:'allowed',tram_access:'allowed'}]};
 const pick=vi.fn();
 const view=render(<RoadTransportPanel data={data} companyId="1" run={run} tramEnabled onPickOnMap={pick}/>);
 fireEvent.click(screen.getByRole('button',{name:'地図で起点・終点を選ぶ'}));
 expect(pick).toHaveBeenCalledOnce();
 view.rerender(<RoadTransportPanel data={data} companyId="1" run={run} tramEnabled onPickOnMap={pick} mapSelection={{from:'qa:0',to:'qa:1',revision:1}}/>);
 fireEvent.click(screen.getByRole('button',{name:'軌道を着工'}));
 await waitFor(()=>expect(run).toHaveBeenCalledWith('edit_road_tram',{edit:{kind:'build',from:'qa:0',to:'qa:1',name:'軌道',shared:true}}));
});

const from={edge_id:'road-a',position:0.25}, to={edge_id:'road-b',position:0.75};
const region={id:'fixture-region',region:'試験地域',origin:'Test origin',revision:'2',attribution:'Test attribution',bus_stop_positions:[from,to]};
const a={latitude:35,longitude:135},b={latitude:35,longitude:135.001},c={latitude:35,longitude:135.002};
const native:RoadTransportData={...empty,nodes:[{id:'a',point:a},{id:'b',point:b},{id:'c',point:c}],edges:[
 {id:'road-a',from:'a',to:'b',source:'native-pilot:fixture-region',revision:'2',geometry:[a,b],bus_access:'allowed',tram_access:'allowed'},
 {id:'road-b',from:'b',to:'c',source:'native-pilot:fixture-region',revision:'2',geometry:[b,c],bus_access:'allowed',tram_access:'allowed'},
]};
it('separates an empty release registry from a listing failure',async()=>{
 const run=vi.fn();
 vi.mocked(invoke).mockResolvedValueOnce([]);
 const view=render(<RoadTransportPanel data={empty} companyId="1" run={run}/>);
 await screen.findByText('この版で追加できる承認済みの実在地域はありません。');
 view.unmount();
 vi.mocked(invoke).mockRejectedValueOnce(new Error('missing registry'));
 render(<RoadTransportPanel data={empty} companyId="1" run={run}/>);
 await screen.findByText('道路データの一覧を確認できませんでした。');
 expect(run).not.toHaveBeenCalled();
});
it('imports only the listed asset ID, then cannot overwrite it',async()=>{
 vi.mocked(invoke).mockResolvedValue([region]);
 const run=vi.fn().mockResolvedValue({});
 const view=render(<RoadTransportPanel data={empty} companyId="1" run={run}/>);
 expect(await screen.findByText('出所: Test origin / 版: 2 / 帰属: Test attribution')).toBeTruthy();
 fireEvent.click(screen.getByRole('button',{name:'試験地域を追加'}));
 await waitFor(()=>expect(run).toHaveBeenCalledWith('install_native_road_pilot',{assetId:'fixture-region'}));
 view.rerender(<RoadTransportPanel data={native} companyId="1" run={run}/>);
 expect(screen.getByText('このワールドで使用中')).toBeTruthy();
 expect(screen.queryByRole('button',{name:'試験地域を追加'})).toBeNull();
 expect(screen.queryByRole('button',{name:'検証用道路を追加'})).toBeNull();
});
it('creates, edits and removes a native bus using the exact approved boarding fractions',async()=>{
 vi.mocked(invoke).mockResolvedValue([region]);
 const run=vi.fn().mockResolvedValue({});
 const view=render(<RoadTransportPanel data={native} companyId="1" run={run}/>);
 const boarding=await screen.findByLabelText('乗車');
 const drop=screen.getByLabelText('降車');
 fireEvent.change(boarding,{target:{value:JSON.stringify([from.edge_id,from.position])}});
 fireEvent.change(drop,{target:{value:JSON.stringify([to.edge_id,to.position])}});
 fireEvent.click(screen.getByText('承認済み停留所でバス系統を作成'));
 await waitFor(()=>expect(run).toHaveBeenCalledWith('edit_native_pilot_bus',{edit:{route_id:expect.stringMatching(/^bus:/),name:'バス系統',expected_route:null,from,to}}));
 const saved:RoadTransportData={...native,bus_stops:[{id:'s1',owner:'1',name:'A',...from},{id:'s2',owner:'1',name:'B',...to}],bus_routes:[{id:'route',owner:'1',name:'Original',edges:['road-a','road-b'],stops:[{stop_id:'s1',edge_index:0},{stop_id:'s2',edge_index:1}]}]};
 view.rerender(<RoadTransportPanel data={saved} companyId="1" run={run}/>);
 fireEvent.click(screen.getByText('系統を編集'));
 fireEvent.change(screen.getByLabelText('名称'),{target:{value:'Edited'}});
 fireEvent.click(screen.getByText('バス系統の変更を適用'));
 await waitFor(()=>expect(run).toHaveBeenCalledWith('edit_native_pilot_bus',{edit:{route_id:'route',name:'Edited',expected_route:saved.bus_routes[0],from,to}}));
 fireEvent.click(screen.getByText('系統を削除'));
 await waitFor(()=>expect(run).toHaveBeenCalledWith('replace_road_transport',{expected:saved,replacement:{...saved,bus_routes:[]}}));
});

it('does not turn editing a route removed in the meantime into creation',async()=>{
 vi.mocked(invoke).mockResolvedValue([region]);
 const route={id:'route',owner:'1',name:'First version',edges:['road-a','road-b'],stops:[{stop_id:'s1',edge_index:0},{stop_id:'s2',edge_index:1}]};
 const data={...native,bus_stops:[{id:'s1',owner:'1',name:'A',...from},{id:'s2',owner:'1',name:'B',...to}],bus_routes:[route]};
 const run=vi.fn().mockRejectedValue('系統が変更されています');
 const view=render(<RoadTransportPanel data={data} companyId="1" run={run}/>);
 fireEvent.click(await screen.findByText('系統を編集'));
 view.rerender(<RoadTransportPanel data={{...data,bus_routes:[]}} companyId="1" run={run}/>);
 fireEvent.click(screen.getByText('バス系統の変更を適用'));
 await waitFor(()=>expect(run).toHaveBeenCalledWith('edit_native_pilot_bus',{edit:{route_id:'route',name:'First version',expected_route:route,from,to}}));
 await screen.findByRole('alert');
});
