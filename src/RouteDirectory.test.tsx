// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {RouteDirectory,routeLength} from './RouteDirectory';
import {alignmentLength} from './alignment';
afterEach(cleanup);
const route={id:'r',name:'本線',start:'a',tracks:[]};
const points=[{latitude:35,longitude:139},{latitude:35.01,longitude:139},{latitude:35.01,longitude:139.01}];
const world={routes:[route],tracks:[],alignments:[{id:'l',points}],station_placements:[{station_id:'a',alignment_id:'l',position:.1},{station_id:'b',alignment_id:'l',position:.9}],route_paths:[{route_id:'r',segments:[{kind:'alignment' as const,alignment_id:'l',from:'a',to:'b'}]}]};
test('route distance follows physical geometry and missing geometry is unknown',()=>{
 expect(routeLength(route,world)).toBeCloseTo(alignmentLength(points)*.8);
 expect(routeLength(route,{...world,alignments:[]})).toBeNull();
 expect(routeLength({...route,tracks:['t']},{...world,route_paths:[],station_placements:[],tracks:[{id:'t',from:'a',to:'b'}]})).toBeNull();
});
test('directory exposes real distance, no fabricated simulation metrics and edit/new actions',()=>{
 const choose=vi.fn();render(<RouteDirectory world={world} selected={null} enabled onChoose={choose}/>);
 expect(screen.getByRole('table',{name:'路線一覧'}).textContent).toContain('km');
 for(const label of ['乗客','収支','車両数'])expect(screen.getByLabelText(`${label}: 未実装`).textContent).toBe('—');
 fireEvent.click(screen.getByRole('button',{name:'編集: 本線'}));expect(choose).toHaveBeenLastCalledWith('r');
 fireEvent.click(screen.getByRole('button',{name:'新規路線'}));expect(choose).toHaveBeenLastCalledWith(null);
});
