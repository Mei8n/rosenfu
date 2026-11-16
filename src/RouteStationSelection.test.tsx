// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,render,screen} from '@testing-library/react';
import {RouteStationSelection} from './RouteStationSelection';
vi.mock('./map',()=>({project:(latitude:number,longitude:number)=>({x:longitude,y:latitude})}));
afterEach(cleanup);
const stations=[{id:'a',name:'始発駅',latitude:1,longitude:2},{id:'b',name:'中間駅',latitude:3,longitude:4},{id:'c',name:'終着駅',latitude:5,longitude:6}];
const camera={} as Parameters<typeof RouteStationSelection>[0]['camera'];
test('explicit station picks show order and roles; new picks change prior endpoint to via',()=>{
 const {rerender}=render(<svg><RouteStationSelection stations={stations} ordered={['a','b']} camera={camera} scale={2}/></svg>);
 expect(screen.getByLabelText('1 始点 始発駅')).toBeTruthy();
 expect(screen.getByLabelText('2 終点 中間駅')).toBeTruthy();
 expect(screen.queryByLabelText(/終着駅/)).toBeNull();
 rerender(<svg><RouteStationSelection stations={stations} ordered={['a','b','c']} camera={camera} scale={2}/></svg>);
 expect(screen.getByLabelText('2 経由 中間駅')).toBeTruthy();
 expect(screen.getByLabelText('3 終点 終着駅')).toBeTruthy();
 expect(screen.getByLabelText('1 始点 始発駅').getAttribute('transform')).toContain('scale(0.5)');
 expect(screen.getByLabelText('路線の選択駅').getAttribute('pointer-events')).toBe('none');
});
test('reset removes all markers and removed stations do not throw or display internal ids',()=>{
 const {rerender,container}=render(<svg><RouteStationSelection stations={stations} ordered={['a','missing']} camera={camera} scale={1}/></svg>);
 expect(container.textContent).not.toContain('missing');
 rerender(<svg><RouteStationSelection stations={stations} ordered={[]} camera={camera} scale={1}/></svg>);
 expect(container.querySelectorAll('.route-selected-station')).toHaveLength(0);
});
