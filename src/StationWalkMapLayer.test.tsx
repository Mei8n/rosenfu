// @vitest-environment jsdom
import {afterEach,expect,test} from 'vitest';
import {cleanup,render,screen,within} from '@testing-library/react';
import {MAX_VISIBLE_STATION_PASSAGES,selectedStationPassages,StationWalkMapLayer} from './StationWalkMapLayer';

afterEach(cleanup);
const camera={latitude:35,longitude:139,zoom:16};
const a={id:'21',name:'小川町',latitude:35,longitude:139};
const b={id:'22',name:'淡路町',latitude:35,longitude:139.003};
const c={id:'23',name:'新御茶ノ水',latitude:35,longitude:139.005};
const stations=new Map([a,b,c].map(s=>[s.id,s]));
const viewport={x:0,y:0,width:1000,height:600,scale:1};
const scope='駅間の徒歩連絡（概略表示・実際の徒歩経路ではありません）';

test('combines a saved pair without making directions or durations symmetric',()=>{
 const links=[{from:a.id,to:b.id,seconds:180},{from:b.id,to:a.id,seconds:245},{from:b.id,to:c.id,seconds:60}];
 const passages=selectedStationPassages(a.id,links,stations);
 const {container}=render(<svg><StationWalkMapLayer selected={a} camera={camera} viewport={viewport} passages={passages}/></svg>);
 const layer=screen.getByLabelText(scope);
 expect(layer.getAttribute('pointer-events')).toBe('none');
 expect(screen.getByLabelText('小川町 から 淡路町（3分） / 淡路町 から 小川町（4分5秒）')).toBeTruthy();
 expect(screen.queryByText(/新御茶ノ水/)).toBeNull();
 expect(within(layer).getByText('3分')).toBeTruthy();
 expect(within(layer).getByText('4分5秒')).toBeTruthy();
 const rotations=[...container.querySelectorAll('.station-walk-arrow')].map(path=>path.getAttribute('transform'));
 expect(rotations).toEqual(['rotate(0)','rotate(-180)']);
});

test('shows a single inbound direction and removes the display when the saved link is removed',()=>{
 const {rerender,container}=render(<svg><StationWalkMapLayer selected={a} camera={camera} viewport={viewport}
  passages={selectedStationPassages(a.id,[{from:b.id,to:a.id,seconds:240}],stations)}/></svg>);
 expect(screen.getByLabelText('淡路町 から 小川町（4分）')).toBeTruthy();
 expect(container.querySelectorAll('.station-walk-arrow')).toHaveLength(1);
 rerender(<svg><StationWalkMapLayer selected={a} camera={camera} viewport={viewport} passages={[]}/></svg>);
 expect(screen.queryByLabelText(scope)).toBeNull();
});

test('skips off-screen and visually tiny links before bounding visible display',()=>{
 const offscreen={id:'far',name:'枠外',latitude:35,longitude:139.007};
 const tiny={id:'tiny',name:'短すぎ',latitude:35,longitude:139.00001};
 const visible=Array.from({length:18},(_,i)=>({id:'v'+i,name:'候補'+i,latitude:35,longitude:139.001+i*.0001}));
 const list=[offscreen,tiny,...visible];
 const {container}=render(<svg><StationWalkMapLayer selected={a} camera={camera} viewport={{x:480,y:320,width:130,height:110,scale:1}}
   passages={selectedStationPassages(a.id,list.map(v=>({from:a.id,to:v.id,seconds:120})),new Map(list.map(v=>[v.id,v])))}/></svg>);
 expect(container.querySelectorAll('.station-walk-passage')).toHaveLength(0);
 expect(screen.queryByText(/短すぎ/)).toBeNull();
 const bounded=render(<svg><StationWalkMapLayer selected={a} camera={camera} viewport={viewport}
  passages={selectedStationPassages(a.id,[tiny,...visible].map(v=>({from:a.id,to:v.id,seconds:120})),new Map([tiny,...visible].map(v=>[v.id,v])))}/></svg>);
 expect(bounded.container.querySelectorAll('.station-walk-passage')).toHaveLength(MAX_VISIBLE_STATION_PASSAGES);
 expect(screen.queryByText(/短すぎ/)).toBeNull();
});
