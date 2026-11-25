// @vitest-environment jsdom
import {afterEach,expect,test} from 'vitest';
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {JunctionPoints} from './JunctionPoints';
import {DEFAULT_CAMERA} from './map';
afterEach(cleanup);
test('only explicit three-port joins display a selectable point, not crossings or ordinary joins',()=>{
 const point={latitude:35.67504,longitude:139.75},lines=['1','2','3'].map(id=>({id,points:[point,{...point,longitude:139.76}]}));
 const a={alignment_id:'1',endpoint:'start' as const},b={alignment_id:'2',endpoint:'start' as const},c={alignment_id:'3',endpoint:'start' as const};
 const {rerender}=render(<svg><JunctionPoints lines={lines} connections={[]} camera={DEFAULT_CAMERA} selectable/></svg>);expect(screen.queryByRole('button')).toBeNull();
 rerender(<svg><JunctionPoints lines={lines} connections={[{a,b}]} camera={DEFAULT_CAMERA} selectable/></svg>);expect(screen.queryByRole('button')).toBeNull();
 rerender(<svg><JunctionPoints lines={lines} connections={[{a,b},{a,b:c}]} camera={DEFAULT_CAMERA} selectable/></svg>);const button=screen.getByRole('button',{name:'分岐ポイント'});fireEvent.keyDown(button,{key:'Enter'});expect(button.getAttribute('aria-pressed')).toBe('true');
});

test('visible junctions identify connected line names without internal IDs and pan to other groups',()=>{
 const rows=(prefix:string,longitude:number,names:string[])=>names.map((name,i)=>({id:prefix+i,name,points:[{latitude:35.67504,longitude},{latitude:35.67504,longitude:longitude+.01}]}));
 const lines=[...rows('tokyo-private-',139.75,['幹線','本線','支線']),...rows('far-private-',140.04,['東線','西線','貨物線'])];
 const port=(line:typeof lines[number])=>({alignment_id:line.id,endpoint:'start' as const});
 const connections=[{a:port(lines[0]),b:port(lines[1])},{a:port(lines[0]),b:port(lines[2])},{a:port(lines[3]),b:port(lines[4])},{a:port(lines[3]),b:port(lines[5])}];
 const app=(longitude:number)=><svg><JunctionPoints lines={lines} connections={connections} camera={{...DEFAULT_CAMERA,longitude}} selectable/></svg>;
 const {rerender,container}=render(app(139.75));
 const first=screen.getByRole('button',{name:/分岐ポイント：.*幹線/});
 expect(screen.getAllByRole('button')).toHaveLength(1);
 fireEvent.keyDown(first,{key:' '});
 expect(screen.getByText(/分岐ポイント：.*支線/)).toBeTruthy();
 expect(container.textContent).not.toMatch(/private-/);
 rerender(app(140.04));
 expect(screen.getByRole('button',{name:/分岐ポイント：.*貨物線/})).toBeTruthy();
 expect(screen.getAllByRole('button')).toHaveLength(1);
 rerender(app(139.75));
 expect(screen.getByRole('button',{name:/分岐ポイント：.*幹線/}).getAttribute('aria-pressed')).toBe('true');
});

test('does not rebuild the endpoint index for another view of the same network',()=>{
 let idReads=0;
 const connections:Parameters<typeof JunctionPoints>[0]['connections']=[];
 const lines=Array.from({length:240},(_,group)=>{
  const port=(n:number)=>({alignment_id:`g${group}-${n}`,endpoint:'start' as const});
  connections.push({a:port(0),b:port(1)},{a:port(0),b:port(2)});
  const point={latitude:35.67504,longitude:group<2?139.75:141.5};
  return [0,1,2].map(n=>({get id(){idReads++;return port(n).alignment_id;},points:[point]}));
 }).flat();
 const app=(longitude:number)=><svg><JunctionPoints lines={lines} connections={connections} camera={{...DEFAULT_CAMERA,longitude}} selectable/></svg>;
 const {rerender}=render(app(139.75));
 expect(screen.getAllByRole('button')).toHaveLength(2);expect(idReads).toBe(720);
 rerender(app(141.5));expect(screen.getAllByRole('button')).toHaveLength(238);expect(idReads).toBe(720);
});
