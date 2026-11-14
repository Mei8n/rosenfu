import {describe,it,expect} from 'vitest';
import {anchorDragPreview} from './anchorDragPreview';
const p=(x:number,y=0)=>({latitude:35+y/111000,longitude:139+x/91000});
describe('anchor drag geometric preview',()=>{
 it('curves both adjacent intervals while retaining outer geometry and original input',()=>{
  const anchors=[p(-100),p(0),p(100),p(200),p(300)],line=anchors.map(a=>({...a})),before=JSON.stringify(line),target=p(100,30);
  const result=anchorDragPreview(line,anchors,2,target);
  expect(result.error).toBeNull();expect(result.line[0]).toEqual(anchors[0]);expect(result.line.at(-1)).toEqual(anchors[4]);
  expect(result.line).toContainEqual(target);expect(result.line).toContainEqual(anchors[1]);expect(result.line).toContainEqual(anchors[3]);
  expect(result.line.some(a=>a.longitude>anchors[1].longitude&&a.longitude<target.longitude&&a.latitude>35)).toBe(true);
  expect(result.line.some(a=>a.longitude>target.longitude&&a.longitude<anchors[3].longitude&&a.latitude>35)).toBe(true);
  expect(JSON.stringify(line)).toBe(before);
 });
 it('rejects missing provenance, endpoints, invalid targets without changing input',()=>{
  const line=[p(0),p(100),p(200)];
  for(const result of [anchorDragPreview(line,line,0,p(10)),anchorDragPreview(line,[p(0),p(99),p(200)],1,p(50)),anchorDragPreview(line,line,1,{latitude:NaN,longitude:139})]){expect(result.error).not.toBeNull();expect(result.line).toEqual(line);}
 });
});
