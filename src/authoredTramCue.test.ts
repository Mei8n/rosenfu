import {expect,test,vi} from 'vitest';
import {createRoadDrawingEngine} from './roadDrawingEngine';
import * as seamRouter from './ofmDerivedTilePath';
import type {ReferenceGraphBundle} from './referenceGraphBundle';
import type {TileRoadLine} from './ofmBoundaryContinuations';

const seam=14553/16384*360-180,lat=35.684;
const shared={source:{source:'fixture',sourceDate:null,license:'fixture',attribution:'fixture'},sourceSha256:'a'.repeat(64),graphSha256:'b'.repeat(64)};
const road=()=>({highway:'residential',access:null,motorVehicle:null,structureUnsupported:false});
const props={mode:'road' as const,layer:0,level:null,bridge:false,tunnel:false,segmentIndex:0};
const bounds={west:seam-.02,east:seam+.02,south:lat-.02,north:lat+.02};
test('authored Tram manual road guide retains fractional points and joins only explicit source node',async()=>{
 const source='https://example.invalid/reviewed-reference';
 const a={longitude:seam-.001,latitude:lat},b={longitude:seam,latitude:lat},end={longitude:seam,latitude:lat+.001};
 const bundle=(connected:boolean):ReferenceGraphBundle=>({
  ...shared,
  graph:{format:'rosenfu-reference-graph',version:1,dataset:{id:'reviewed-manual-fixture',revision:'test',source,license:'fixture',bounds},
   nodes:[{...a,osmId:'1'},{...b,osmId:'2'},{...b,osmId:'3'},{...end,osmId:'4'}],
   edges:[{...props,id:'left',osmWayId:'101',from:'1',to:'2'},{...props,id:'north',osmWayId:'102',from:connected?'2':'3',to:'4'}]},
  roadWays:{'101':road(),'102':road()}
 });
 const at={longitude:seam-.0007,latitude:lat};
 const to={longitude:seam,latitude:lat+.0007};
 const worker=createRoadDrawingEngine(bundle(true));
 const q={point:to,start:at,radiusM:20,authoredTramCue:true};
 const valid=await worker(q);
 expect(valid.kind).toBe('route');
 if(valid.kind!=='route')return;
 expect(valid.point.latitude).toBeCloseTo(to.latitude,9);
 expect(valid.line[0]).toEqual(at);
 expect(valid.line.at(-1)).toEqual(valid.point);
 expect((await createRoadDrawingEngine(bundle(false))(q)).kind).toBe('error');
 const far={longitude:at.longitude,latitude:at.latitude+.00025};
 expect((await worker({point:far,radiusM:20,authoredTramCue:true})).kind).toBe('free');
});

test('authored Tram keeps an already observed unambiguous OFM seam before any native junction request',async()=>{
 const source='https://tiles.openfreemap.org/planet',revision='fixed';
 const make=(x:number,idNum:number,offset=0):TileRoadLine=>({
  id:`ofm:${revision}:14:${x}:6451:${idNum}:0`,revision,z:14,x,y:6451,class:'minor',
  coordinates:[[seam-.0005,lat+offset],[seam+.0005,lat+offset]],oneway:x===14552?1:'yes',
 });
 const left=make(14552,0),right=make(14553,0);
 const bundle=(lines:TileRoadLine[]):ReferenceGraphBundle=>({
  ...shared,graph:{format:'rosenfu-reference-graph',version:1,dataset:{id:`ofm-tiles/${revision}`,revision,source,license:'fixture',bounds},
   nodes:lines.flatMap(l=>l.coordinates.map(([longitude,latitude],i)=>({osmId:`${l.id}:${i}`,longitude,latitude}))),
   edges:lines.map(l=>({...props,id:`${l.id}/0`,osmWayId:l.id,from:`${l.id}:0`,to:`${l.id}:1`}))},
  roadWays:Object.fromEntries(lines.map(l=>[l.id,road()]))
 });
 // Choose portions unambiguously owned in each tile. Non-overlap geometry
 // outside the marked region won't make a cross-cell connection.
 const west={longitude:seam-.00043,latitude:lat},east={longitude:seam+.00043,latitude:lat};
 const isolated=(line:TileRoadLine,side:'left'|'right')=>({
   ...line,coordinates:side==='left'?[[seam-.0005,lat],[seam+.0002,lat]] as const:[[seam-.0002,lat],[seam+.0005,lat]] as const
 });
 const paths=[isolated(left,'left'),isolated(right,'right')];
 const remote=make(14558,0,.01);
 const spy=vi.spyOn(seamRouter,'buildTileRoadPath');
 const worker=createRoadDrawingEngine(bundle([...paths,remote]),[...paths,remote]);
 const query={point:east,start:west,radiusM:20,authoredTramCue:true};
 expect(await worker(query)).toEqual({kind:'seam',point:east});
 expect(spy).not.toHaveBeenCalled();
 const valid=await worker({...query,resolveAuthoredSeam:true});
 expect(spy.mock.calls[0][0].map(line=>line.id)).toEqual(paths.map(line=>line.id));
 spy.mockRestore();
 expect(valid.kind).toBe('route');
 if(valid.kind!=='route')return;
 expect(valid.line[0]).toEqual(west);
 expect(valid.line.at(-1)).toEqual(valid.point);
 expect((await worker({point:west,start:east,radiusM:20,authoredTramCue:true,resolveAuthoredSeam:true})).kind).toBe('route');
 const ambiguous=[...paths,isolated(make(14553,1),'right')];
 expect((await createRoadDrawingEngine(bundle(ambiguous),ambiguous)({point:east,start:west,radiusM:20,authoredTramCue:true,resolveAuthoredSeam:true})).kind).toBe('error');
});


test('authored Tram refuses a 257-atom same-fragment span even when endpoint difference is under 256',async()=>{
 const source='https://tiles.openfreemap.org/planet',revision='limit';
 const id='ofm:limit:14:14552:6451:0:0';
 const coordinates=Array.from({length:258},(_,i)=>[seam-.013+i*.00005,lat] as const);
 const nodes=coordinates.map(([longitude,latitude],i)=>({osmId:id+':'+i,longitude,latitude}));
 const line:TileRoadLine={id,revision,z:14,x:14552,y:6451,class:'minor',coordinates};
 const bundle:ReferenceGraphBundle={
  ...shared,graph:{format:'rosenfu-reference-graph',version:1,dataset:{id:'ofm-tiles/limit',revision,source,license:'fixture',bounds},nodes,
   edges:nodes.slice(1).map((to,segmentIndex)=>({...props,segmentIndex,id:id+'/'+segmentIndex,osmWayId:id,from:nodes[segmentIndex].osmId,to:to.osmId}))},
  roadWays:{[id]:road()}
 };
 const worker=createRoadDrawingEngine(bundle,[line]);
 const start={longitude:coordinates[0][0]+.0000375,latitude:lat};
 const bad={longitude:coordinates[256][0]+.0000125,latitude:lat};
 const good={longitude:coordinates[256][0],latitude:lat};
 const cue={start,radiusM:20,authoredTramCue:true,resolveAuthoredSeam:true};
 expect((await worker({...cue,point:bad})).kind).toBe('error');
 expect((await worker({...cue,point:good})).kind).toBe('route');
});
