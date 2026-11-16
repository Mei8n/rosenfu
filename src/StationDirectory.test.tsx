// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {StationDirectory} from './StationDirectory';
afterEach(cleanup);
const stations=[{id:'1',name:'中央'},{id:'2',name:'中央別ホーム'},{id:'3',name:'東駅'}];
test('one group row preserves selection of a member and shows actual routes without coordinates or fictional figures',()=>{
 const onSelect=vi.fn();const {container}=render(<StationDirectory stations={stations} groups={[{station_id:'1',members:['1','2']}]} selected="2" routes={[{id:'r',name:'本線',start:'2'}]} paths={[{route_id:'r',member_stations:['2']}]} onSelect={onSelect}/>);
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
