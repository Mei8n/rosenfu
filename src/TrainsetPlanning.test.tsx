// @vitest-environment jsdom
import {act} from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {TrainsetPlanning} from './TrainsetPlanning';

vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
const plan={cycleMs:380_000,runningMs:300_000,dwellAndClearanceMs:80_000,headwayMs:120_000,trainsets:4};
const panel=(token='one')=><TrainsetPlanning serviceId="12" configurationToken={token} currentHeadwayMs={120_000} enabled/>;

test('both explicit inputs ask native theory without issuing a timetable mutation',async()=>{
 vi.mocked(invoke).mockResolvedValueOnce(plan).mockResolvedValueOnce({...plan,headwayMs:126_667,trainsets:3});
 render(panel());
 fireEvent.click(screen.getByText('編成数と運転間隔の試算'));
 expect(invoke).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole('button',{name:'編成数・間隔を試算'}));
 expect(await screen.findByText('4編成')).toBeTruthy();
 expect(invoke).toHaveBeenNthCalledWith(1,'preview_nominal_trainsets',{serviceId:'12',configurationToken:'one',
  request:{mode:'timedHeadway',headwayMs:120_000}});
 fireEvent.change(screen.getByLabelText('試算の基準'),{target:{value:'trainsetCount'}});
 fireEvent.change(screen.getByLabelText('試算する編成数'),{target:{value:'3'}});
 fireEvent.click(screen.getByRole('button',{name:'編成数・間隔を試算'}));
 expect(await screen.findByText('2分6.667秒')).toBeTruthy();
 expect(invoke).toHaveBeenNthCalledWith(2,'preview_nominal_trainsets',{serviceId:'12',configurationToken:'one',
  request:{mode:'trainsetCount',trainsets:3}});
 expect(invoke).toHaveBeenCalledTimes(2);
});

test('invalid input and a replaced configuration cannot publish results',async()=>{
 let finish!:(value:typeof plan)=>void;
 vi.mocked(invoke).mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
 const view=render(panel());
 fireEvent.click(screen.getByText('編成数と運転間隔の試算'));
 fireEvent.change(screen.getByLabelText('試算する運転間隔（分）'),{target:{value:'0'}});
 fireEvent.click(screen.getByRole('button',{name:'編成数・間隔を試算'}));
 expect(await screen.findByRole('alert')).toBeTruthy();
 expect(invoke).not.toHaveBeenCalled();
 fireEvent.change(screen.getByLabelText('試算する運転間隔（分）'),{target:{value:'2'}});
 fireEvent.click(screen.getByRole('button',{name:'編成数・間隔を試算'}));
 view.rerender(panel('two'));
 await act(async()=>finish(plan));
 expect(screen.queryByText('4編成')).toBeNull();
 expect(invoke).toHaveBeenCalledTimes(1);
});
