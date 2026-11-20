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
