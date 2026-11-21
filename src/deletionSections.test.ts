import {describe,expect,it} from 'vitest';
import type {GeoPoint} from './alignment';
import {anchorIndices,deletionSections} from './deletionSections';

const point=(n:number):GeoPoint=>({latitude:n,longitude:n*2});
function previous(anchors:GeoPoint[],line:GeoPoint[]):number[]|undefined{
 let cursor=-1;
 const indices=anchors.map(anchor=>{
  const index=line.findIndex((p,i)=>i>cursor&&p.latitude===anchor.latitude&&p.longitude===anchor.longitude);
  cursor=index;
  return index;
 });
 return indices.length>=2&&indices[0]===0&&indices.at(-1)===line.length-1&&indices.every((n,i)=>n>=0&&(i===0||n>indices[i-1]))?indices:undefined;
}

describe('anchor index walk',()=>{
 it('matches ordered exact points and keeps deletion section boundaries',()=>{
  const line=[0,1,2,3,4].map(point);
  const anchor_indices=anchorIndices([point(0),point(2),point(4)],line);
  expect(anchor_indices).toEqual([0,2,4]);
  expect(deletionSections({points:line,anchor_indices})).toEqual([
   {index:0,points:line.slice(0,3)},{index:1,points:line.slice(2)},
  ]);
 });

 it('preserves repeated and closed-path first-match behavior',()=>{
  expect(anchorIndices([0,1,0].map(point),[0,1,0].map(point))).toEqual([0,1,2]);
  expect(anchorIndices([0,1,0].map(point),[0,1,0,0].map(point))).toBeUndefined();
  expect(anchorIndices([0,1,1,2].map(point),[0,1,1,2].map(point))).toEqual([0,1,2,3]);
  expect(anchorIndices([0,3,2].map(point),[0,1,2].map(point))).toBeUndefined();
  expect(anchorIndices([1,2].map(point),[0,1,2].map(point))).toBeUndefined();
  expect(anchorIndices([0,2].map(point),[0,2,3].map(point))).toBeUndefined();
 });

 it('agrees with the previous search on generated duplicate/missing coordinates',()=>{
  let state=138329;
  const next=(bound:number)=>{
   state=(Math.imul(state,1664525)+1013904223)>>>0;
   return ((state>>>8)%bound);
  };
  for(let i=0;i<300;i++){
   const line=Array.from({length:next(20)},()=>point(next(8)));
   const anchors=i%2===0
    ? line.filter((_,index)=>index===0||index===line.length-1||next(3)===0)
    : Array.from({length:next(8)},()=>point(next(9)));
   expect(anchorIndices(anchors,line)).toEqual(previous(anchors,line));
  }
 });

 it('reads each line entry at most once for a many-anchor preview',()=>{
  const n=4096;
  const raw=Array.from({length:n},(_,i)=>point(i));
  const indices=[...Array.from({length:256},(_,i)=>i*16),n-1];
  const anchors=indices.map(i=>raw[i]);
  let reads=0;
  const line=new Proxy(raw,{
   get(target,index,receiver){
    if(typeof index==='string'&&/^\d+$/.test(index))reads++;
    return Reflect.get(target,index,receiver);
   },
  });
  expect(anchorIndices(anchors,line)).toEqual(indices);
  expect(reads).toBe(n);
  reads=0;
  expect(previous(anchors,line)).toEqual(indices);
  expect(reads).toBeGreaterThan(500000);
 });
});
