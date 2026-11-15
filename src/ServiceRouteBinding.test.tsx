// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {ServiceRouteBindingEditor} from './ServiceRouteBinding';
afterEach(cleanup);
const routes=[{id:'r',name:'本線'}];
test('unknown is not inferred; explicit route assignment saves only on apply',async()=>{
 const save=vi.fn().mockResolvedValue(undefined),dirty=vi.fn();render(<ServiceRouteBindingEditor routes={routes} enabled onDirty={dirty} onSave={save}/>);
 expect((screen.getByLabelText('関連する路線') as HTMLSelectElement).value).toBe('');expect(save).not.toHaveBeenCalled();
 fireEvent.change(screen.getByLabelText('関連する路線'),{target:{value:'route:r'}});expect(save).not.toHaveBeenCalled();expect(dirty).toHaveBeenLastCalledWith(true);
 fireEvent.click(screen.getByRole('button',{name:'関連付けを適用'}));await waitFor(()=>expect(save).toHaveBeenCalledWith({kind:'route',route_id:'r'}));
});
test('independent choice is explicit, failed assignment retains draft, cancel restores prior binding',async()=>{
 const save=vi.fn().mockRejectedValue(Error('経路が路線に含まれません'));render(<ServiceRouteBindingEditor binding={{kind:'independent'}} routes={routes} enabled onDirty={()=>{}} onSave={save}/>);
 fireEvent.change(screen.getByLabelText('関連する路線'),{target:{value:'route:r'}});fireEvent.click(screen.getByRole('button',{name:'関連付けを適用'}));await screen.findByRole('alert');
 expect((screen.getByLabelText('関連する路線') as HTMLSelectElement).value).toBe('route:r');fireEvent.click(screen.getByRole('button',{name:'関連付けの変更を破棄'}));
 expect((screen.getByLabelText('関連する路線') as HTMLSelectElement).value).toBe('independent');expect(save).toHaveBeenCalledTimes(1);
});
