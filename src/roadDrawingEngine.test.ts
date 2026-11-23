import {expect,test} from 'vitest';
import {createRoadDrawingEngine} from './roadDrawingEngine';
import {buildTileRoadPath} from './ofmDerivedTilePath';
import type {TileRoadLine} from './ofmBoundaryContinuations';
import type {ReferenceGraphBundle} from './referenceGraphBundle';

function corridor(count:number,options:{broken?:boolean;denseJoin?:boolean}={}){
 const tileSize=360/16384,startX=14530,y=6451,latitude=35.684;
 const longitude=(x:number,raw:number)=>(x+raw/4096)*tileSize-180;
 const lines:TileRoadLine[]=Array.from({length:count},(_,i)=>{
  const x=startX+i,offset=options.broken&&i===count-1?.001:0;
  const dense=options.denseJoin&&i===0?Array.from({length:260},(_,j)=>longitude(x,4096-18+j/8)):[];
  return {id:`ofm:test:14:${x}:${y}:0:0`,revision:'test',z:14,x,y,class:'minor',
   coordinates:[longitude(x,i===0?1000:-120),...dense,longitude(x,i===count-1?3096:4216)].map(longitude=>[longitude,latitude+offset] as const)};
 });
 const source='https://tiles.openfreemap.org/planet/test';
 const bundle:ReferenceGraphBundle={graph:{format:'rosenfu-reference-graph',version:1,
  dataset:{id:'ofm-tiles/test',revision:'test',source,license:'ODbL-1.0',bounds:{west:139,east:140,south:35,north:36}},
  nodes:lines.flatMap(l=>l.coordinates.map(([longitude,latitude],i)=>({osmId:`${l.id}:${i}`,longitude,latitude}))),
  edges:lines.flatMap(l=>l.coordinates.slice(1).map((_,i)=>({id:`${l.id}:${i}`,osmWayId:l.id,segmentIndex:i,from:`${l.id}:${i}`,to:`${l.id}:${i+1}`,mode:'road',layer:0,level:null,bridge:false,tunnel:false})))},
  roadWays:Object.fromEntries(lines.map(l=>[l.id,{highway:'residential',access:null,motorVehicle:null,structureUnsupported:false}])),
  graphSha256:'a'.repeat(64),sourceSha256:'b'.repeat(64),source:{source,sourceDate:null,license:'ODbL-1.0',attribution:'test'}};
 const [a,b]=[lines[0].coordinates[0],lines.at(-1)!.coordinates[1]];
 return {lines,draw:createRoadDrawingEngine(bundle,lines),query:{start:{longitude:a[0],latitude:a[1]},point:{longitude:b[0],latitude:b[1]},radiusM:20}};
}

test('keeps 13 linked tiles possible and advises splitting a proven 14-tile path, without returning geometry',async()=>{
 const shorter=corridor(13);
 expect((await shorter.draw(shorter.query)).kind).toBe('route');
 const longer=corridor(14);
 expect(await longer.draw(longer.query)).toEqual({kind:'error',error:'一度に道路に沿って敷設できる区間の上限です。手前の道路上まで敷設し、そこからの延伸を試してください。'});
 expect(await longer.draw({...longer.query,authoredTramCue:true,resolveAuthoredSeam:true})).toEqual(await longer.draw(longer.query));
});

test('does not explain a disconnected road as a limit',async()=>{
 const broken=corridor(14,{broken:true});
 expect(await broken.draw(broken.query)).toEqual({kind:'error',error:'道路のつながりを確認できません。区間を短くして選び直してください'});
});

test('does not suggest splitting and resuming at an individually oversized boundary',async()=>{
 const dense=corridor(2,{denseJoin:true});
 const [a,b]=dense.lines;
 expect(()=>buildTileRoadPath(dense.lines,{fragmentId:a.id,segmentIndex:0,fraction:0},{fragmentId:b.id,segmentIndex:0,fraction:1})).toThrow('join-segment-limit');
 const result={kind:'error',error:'この道路の境界をまたいで敷設できません。別の道路を選ぶか、境界の手前まで敷設してください。'};
 expect(await dense.draw(dense.query)).toEqual(result);
 expect(await dense.draw({...dense.query,authoredTramCue:true,resolveAuthoredSeam:true})).toEqual(result);
});
