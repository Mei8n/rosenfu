// @vitest-environment jsdom
import {render,screen,fireEvent,waitFor,cleanup} from '@testing-library/react';
import {describe,it,expect,vi,afterEach} from 'vitest';
import {RoadTransportPanel,type RoadTransportData} from './RoadTransportPanel';
afterEach(cleanup);
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
