// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen,within} from '@testing-library/react';
import {StationWalkLinks} from './StationWalkLinks';
import {UNDO_WORLD_COMMANDS,worldEditKey} from './editHistory';

afterEach(cleanup);
const a={id:'21',owner:'1',name:'小川町',latitude:35,longitude:139};
const b={id:'22',owner:'1',name:'淡路町',latitude:35,longitude:139.005};
const c={id:'23',owner:'1',name:'遠い駅',latitude:35,longitude:139.032};
const foreign={...b,id:'24',owner:'2',name:'他社駅'};
const platform={...b,id:'25',name:'同じ駅の別ホーム'};
const props={station:a,stations:[a,b,c,foreign,platform],groups:[{station_id:a.id,members:[a.id,platform.id]}],enabled:true};

test('shows same-owner nearby distinct stations, but never creates an implicit link',()=>{
 const onEdit=vi.fn();
 render(<StationWalkLinks {...props} links={[]} onEdit={onEdit}/>);
 fireEvent.click(screen.getByText('駅間の徒歩連絡'));
 const select=screen.getByLabelText('連絡先の駅');
 expect(within(select).getByRole('option',{name:/淡路町/})).toBeTruthy();
 expect(within(select).queryByRole('option',{name:/遠い駅|他社駅|別ホーム/})).toBeNull();
 fireEvent.change(select,{target:{value:b.id}});
 expect(onEdit).not.toHaveBeenCalled();
 fireEvent.change(screen.getByLabelText('徒歩連絡の時間（分）'),{target:{value:'3'}});
 fireEvent.click(screen.getByLabelText('両方向に登録'));
 fireEvent.click(screen.getByRole('button',{name:'徒歩連絡を登録'}));
 expect(onEdit).toHaveBeenCalledExactlyOnceWith({from:a.id,to:b.id,seconds:180,bothDirections:false});
});

test('lists real directions, removes only the selected one, and includes edits in generic undo',()=>{
 const onEdit=vi.fn(),links=[{from:b.id,to:a.id,seconds:240}];
 render(<StationWalkLinks {...props} links={links} onEdit={onEdit}/>);
 fireEvent.click(screen.getByText('駅間の徒歩連絡'));
 expect(screen.getByText(/淡路町 → 小川町/)).toBeTruthy();
 fireEvent.change(screen.getByLabelText('連絡先の駅'),{target:{value:b.id}});
 fireEvent.change(screen.getByLabelText('徒歩連絡の時間（分）'),{target:{value:'3'}});
 const register=screen.getByRole<HTMLButtonElement>('button',{name:'徒歩連絡を登録'});
 expect(register.disabled).toBe(true);
 fireEvent.click(screen.getByRole('button',{name:'淡路町 から 小川町 の徒歩連絡を外す'}));
 expect(onEdit).toHaveBeenCalledExactlyOnceWith({from:b.id,to:a.id,seconds:null,bothDirections:false});
 expect(UNDO_WORLD_COMMANDS.has('edit_station_walk_link')).toBe(true);
 expect(worldEditKey({stations:[a,b],station_walk_links:links})).not.toBe(worldEditKey({stations:[a,b]}));
});
