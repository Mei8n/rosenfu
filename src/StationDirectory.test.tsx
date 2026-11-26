// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {StationDirectory} from './StationDirectory';
afterEach(cleanup);
const stations=[{id:'1',name:'中央'},{id:'2',name:'中央別ホーム'},{id:'3',name:'東駅'}];
test('one group row preserves selection of a member and shows actual routes without coordinates or fictional figures',()=>{
 const onSelect=vi.fn();const {container}=render(<StationDirectory stations={stations} groups={[{station_id:'1',members:['1','2']}]} selected="2" placements={[{station_id:"1"},{station_id:"2"}]} routes={[{id:'r',name:'本線',start:'2'}]} paths={[{route_id:'r',member_stations:['2']}]} onSelect={onSelect}/>);
 expect(container.querySelectorAll('.station-entry')).toHaveLength(2);
 const button=screen.getByRole('button',{name:/中央 本線/});expect(button.getAttribute('aria-pressed')).toBe('true');fireEvent.click(button);expect(onSelect).toHaveBeenCalledWith('1');
 expect(button.textContent).toContain('2ホーム');expect(screen.queryByText(/乗客|維持費|緯度|Undo/)).toBeNull();
 fireEvent.change(screen.getByLabelText('駅を探す'),{target:{value:'本線'}});expect(container.querySelectorAll('.station-entry')).toHaveLength(1);
 fireEvent.change(screen.getByLabelText('駅を探す'),{target:{value:'存在しない'}});expect(screen.getByText('一致する駅がありません')).toBeTruthy();expect(onSelect).toHaveBeenCalledTimes(1);
 fireEvent.change(screen.getByLabelText('駅を探す'),{target:{value:''}});expect(container.querySelectorAll('.station-entry')).toHaveLength(2);
});
test('empty directory directs creation to construction, not legacy coordinates',()=>{
 render(<StationDirectory stations={[]} routes={[]} selected={null} onSelect={()=>{}}/>);expect(screen.getByText(/「建設」の「駅設置」/)).toBeTruthy();expect(screen.queryByRole('button')).toBeNull();
});

test("legacy standalone station does not invent a physical platform",()=>{
 render(<StationDirectory stations={[stations[2]]} routes={[]} selected={null} onSelect={()=>{}}/>);expect(screen.getByTitle("ホーム情報なし").textContent).toContain("—");
});

test('first matching route path and distinct platform membership stay live across snapshots',()=>{
 const routes=[{id:'r',name:'本線',start:'3'}];
 const placements=[{station_id:'1'},{station_id:'1'}];
 const props={stations,routes,selected:null,onSelect:vi.fn()};
 const {rerender}=render(<StationDirectory {...props} placements={placements} paths={[{route_id:'r',member_stations:['1']},{route_id:'r',member_stations:['3']}]}/>);
 expect(screen.getByRole('button',{name:/中央 本線/})).toBeTruthy();
 expect(screen.getByRole('button',{name:/東駅 路線未所属/})).toBeTruthy();
 expect(screen.getAllByTitle('1ホーム')).toHaveLength(1);
 rerender(<StationDirectory {...props} placements={[{station_id:'3'}]} paths={[{route_id:'r',member_stations:['3']}]}/>);
 expect(screen.getByRole('button',{name:/東駅 本線/})).toBeTruthy();
 expect(screen.getByRole('button',{name:/中央 路線未所属/})).toBeTruthy();
 expect(screen.getByRole('button',{name:/東駅 本線/}).querySelector('[title="1ホーム"]')).toBeTruthy();
});

test('an unrelated parent render skips stable rows; search and replacement paths remain live',()=>{
 const firstName=vi.fn(()=> '北口'), onSelect=vi.fn(), routes=[{id:'r',name:'本線',start:'1'}];
 const stations=[{id:'1',get name(){return firstName();}},{id:'2',name:'東口'}];
 const paths=[{route_id:'r',member_stations:['1']}];
 const props={stations,routes,paths,selected:null,onSelect};
 const screenFor=(hidden:boolean)=> <div hidden={hidden}><StationDirectory {...props}/></div>;
 const {rerender}=render(screenFor(false));
 fireEvent.change(screen.getByLabelText('駅を探す'),{target:{value:'本線'}});
 expect(screen.getByRole('button',{name:/北口 本線/})).toBeTruthy();
 firstName.mockClear();
 rerender(screenFor(true));
 rerender(screenFor(false));
 expect(firstName).not.toHaveBeenCalled();
 expect((screen.getByLabelText('駅を探す') as HTMLInputElement).value).toBe('本線');
 rerender(<div><StationDirectory {...props} selected="2" paths={[{route_id:'r',member_stations:['2']}]}/></div>);
 const east=screen.getByRole('button',{name:/東口 本線/});
 expect(east.getAttribute('aria-pressed')).toBe('true');
 expect(screen.queryByRole('button',{name:/北口/})).toBeNull();
 fireEvent.click(east);expect(onSelect).toHaveBeenCalledExactlyOnceWith('2');
});
