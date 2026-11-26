// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,render,screen} from '@testing-library/react';
import {RouteStationSelection} from './RouteStationSelection';
vi.mock('./map',()=>({project:(latitude:number,longitude:number,camera:{longitude?:number})=>({x:longitude-(camera.longitude??0),y:latitude})}));
afterEach(cleanup);
const stations=[{id:'a',name:'始発駅',latitude:1,longitude:2},{id:'b',name:'中間駅',latitude:3,longitude:4},{id:'c',name:'終着駅',latitude:5,longitude:6}];
const stationsById=new Map(stations.map(station=>[station.id,station]));
const camera={} as Parameters<typeof RouteStationSelection>[0]['camera'];
test('explicit station picks show order and roles; new picks change prior endpoint to via',()=>{
 const {rerender}=render(<svg><RouteStationSelection stationsById={stationsById} ordered={['a','b']} camera={camera} scale={2}/></svg>);
 expect(screen.getByLabelText('1 始点 始発駅')).toBeTruthy();
 expect(screen.getByLabelText('2 終点 中間駅')).toBeTruthy();
 expect(screen.queryByLabelText(/終着駅/)).toBeNull();
 rerender(<svg><RouteStationSelection stationsById={stationsById} ordered={['a','b','c']} camera={camera} scale={2}/></svg>);
 expect(screen.getByLabelText('2 経由 中間駅')).toBeTruthy();
 expect(screen.getByLabelText('3 終点 終着駅')).toBeTruthy();
 expect(screen.getByLabelText('1 始点 始発駅').getAttribute('transform')).toContain('scale(0.5)');
 expect(screen.getByLabelText('路線の選択駅').getAttribute('pointer-events')).toBe('none');
});
test('reset removes all markers and removed stations do not throw or display internal ids',()=>{
 const {rerender,container}=render(<svg><RouteStationSelection stationsById={stationsById} ordered={['a','missing']} camera={camera} scale={1}/></svg>);
 expect(container.textContent).not.toContain('missing');
 rerender(<svg><RouteStationSelection stationsById={stationsById} ordered={[]} camera={camera} scale={1}/></svg>);
 expect(container.querySelectorAll('.route-selected-station')).toHaveLength(0);
});


test('panning only looks up ordered IDs, retaining separate same-name platforms and their exact markers',()=>{
 const first={id:'group-primary',name:'中央',latitude:1,longitude:2};
 const second={id:'group-platform',name:'中央',latitude:1,longitude:3};
 const linked={id:'linked-separate',name:'中央連絡駅',latitude:1,longitude:4};
 const indexed=new Map([first,second,linked,...Array.from({length:2000},(_,i)=>({id:'other-'+i,name:'遠隔',latitude:10,longitude:10}))].map(station=>[station.id,station]));
 const get=vi.spyOn(indexed,'get');
 const entries=vi.spyOn(indexed,Symbol.iterator);
 const ordered=[second.id,linked.id,first.id];
 const renderLayer=(longitude:number)=><svg><RouteStationSelection stationsById={indexed} ordered={ordered} camera={{latitude:0,zoom:12,longitude}} scale={1}/></svg>;
 const {rerender}=render(renderLayer(0));
 const labels=['1 始点 中央','2 経由 中央連絡駅','3 終点 中央'];
 const before=labels.map(label=>screen.getByLabelText(label));
 expect(before.map(node=>node.getAttribute('transform'))).toEqual(['translate(3 1) scale(1)','translate(4 1) scale(1)','translate(2 1) scale(1)']);
 get.mockClear();
 rerender(renderLayer(10));
 labels.forEach((label,i)=>expect(screen.getByLabelText(label)).toBe(before[i]));
 expect(before.map(node=>node.getAttribute('transform'))).toEqual(['translate(-7 1) scale(1)','translate(-6 1) scale(1)','translate(-8 1) scale(1)']);
 expect(get.mock.calls).toEqual(ordered.map(id=>[id]));
 expect(entries).not.toHaveBeenCalled();
});
