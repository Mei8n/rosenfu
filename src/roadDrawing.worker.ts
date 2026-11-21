import type {TileRoadLine} from './ofmBoundaryContinuations';
import {createRoadDrawingEngine,type RoadDrawingQuery} from './roadDrawingEngine';
import type {ReferenceGraphBundle} from './referenceGraphBundle';
export type RoadDrawingRequest={kind:'load';bundle:ReferenceGraphBundle;tileLines:readonly TileRoadLine[]}|{kind:'query';id:number;query:RoadDrawingQuery};
let engine:ReturnType<typeof createRoadDrawingEngine>|undefined;
self.onmessage=async(event:MessageEvent<RoadDrawingRequest>)=>{const request=event.data;if(request.kind==='load'){try{engine=createRoadDrawingEngine(request.bundle,request.tileLines);}catch{engine=undefined;}return;}const result=engine?await engine(request.query):{kind:'error',error:'道路データを利用できません'};self.postMessage({id:request.id,result});};
