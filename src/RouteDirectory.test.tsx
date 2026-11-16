// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {RouteDirectory,routeLength} from './RouteDirectory';
import {alignmentLength} from './alignment';
afterEach(cleanup);
const route={id:'r',name:'本線',start:'a',tracks:[]};
const points=[{latitude:35,longitude:139},{latitude:35.01,longitude:139},{latitude:35.01,longitude:139.01}];
const world={routes:[route],tracks:[],alignments:[{id:'l',points}],station_placements:[{station_id:'a',alignment_id:'l',position:.1},{station_id:'b',alignment_id:'l',position:.9}],route_paths:[{route_id:'r',segments:[{kind:'alignment' as const,alignment_id:'l',from:'a',to:'b'}]}]};
test('route distance follows physical geometry and missing geometry is unknown',()=>{
 expect(routeLength(route,world)).toBeCloseTo(alignmentLength(points)*.8);
 expect(routeLength(route,{...world,alignments:[]})).toBeNull();
 expect(routeLength(route,{...world,route_paths:[]})).toBeNull();
});
test('directory exposes real distance, no fabricated simulation metrics and edit/new actions',()=>{
 const choose=vi.fn();render(<RouteDirectory world={world} selected={null} enabled onChoose={choose}/>);
 expect(screen.getByRole('table',{name:'路線一覧'}).textContent).toContain('km');
 for(const label of ['乗客','収支','車両数'])expect(screen.queryByRole('columnheader',{name:label})).toBeNull();
 fireEvent.click(screen.getByRole('button',{name:'編集: 本線'}));expect(choose).toHaveBeenLastCalledWith('r');
 fireEvent.click(screen.getByRole('button',{name:'新規路線'}));expect(choose).toHaveBeenLastCalledWith(null);
});

test('rename retains draft on failure and submits original ID/name without changing route selection',async()=>{
 const rename=vi.fn().mockRejectedValueOnce(Error('競合')).mockResolvedValueOnce(undefined),choose=vi.fn();
 render(<RouteDirectory world={world} selected="r" enabled onChoose={choose} onRename={rename}/>);
 fireEvent.click(screen.getByRole('button',{name:'路線名を変更: 本線'}));
 fireEvent.change(screen.getByRole('textbox',{name:'変更後の路線名'}),{target:{value:'新本線'}});
 fireEvent.click(screen.getByRole('button',{name:'保存'}));await screen.findByRole('alert');
 expect((screen.getByRole('textbox',{name:'変更後の路線名'}) as HTMLInputElement).value).toBe('新本線');
 expect(rename).toHaveBeenLastCalledWith('r','新本線','本線');expect(choose).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole('button',{name:'保存'}));await waitFor(()=>expect(screen.queryByRole('textbox')).toBeNull());
 expect(rename).toHaveBeenCalledTimes(2);
});
test('rename cancel and blank draft never write',()=>{
 const rename=vi.fn(),dirty=vi.fn();render(<RouteDirectory world={world} selected="r" enabled onChoose={()=>{}} onRename={rename} onDirty={dirty}/>);
 fireEvent.click(screen.getByRole('button',{name:'路線名を変更: 本線'}));fireEvent.change(screen.getByRole('textbox'),{target:{value:'  '}});
 expect((screen.getByRole('button',{name:'保存'}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.click(screen.getByRole('button',{name:'取消'}));expect(rename).not.toHaveBeenCalled();expect(screen.queryByRole('textbox')).toBeNull();expect(dirty).toHaveBeenLastCalledWith(false);expect(dirty).toHaveBeenCalledWith(true);
});

test('route search is normalized and never exposes internal identifiers',()=>{
 const onChoose=vi.fn();
 const {container}=render(<RouteDirectory world={{routes:[{id:'private-route-id',name:'Express',start:'station-secret'}]}} enabled selected={null} onChoose={onChoose}/>);
 fireEvent.change(screen.getByLabelText('路線を探す'),{target:{value:'ＥＸＰＲＥＳＳ'}});
 fireEvent.click(screen.getByRole('button',{name:'編集: Express'}));expect(onChoose).toHaveBeenCalledWith('private-route-id');
 expect(container.textContent).not.toContain('private-route-id');
 fireEvent.change(screen.getByLabelText('路線を探す'),{target:{value:'なし'}});
 expect(screen.getByText('一致する路線がありません')).toBeTruthy();
});
