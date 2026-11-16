import {createRoadFollowRouter} from './roadFollowRoute';
import type {ReferenceGraphBundle} from './referenceGraphBundle';
import type {ReferenceGraphPick} from './referenceGraphPath';
export type RoadRouteRequest={kind:'load';bundle:ReferenceGraphBundle;generation:number}|{kind:'route';generation:number;sequence:number;start:ReferenceGraphPick;end:ReferenceGraphPick};
let route:ReturnType<typeof createRoadFollowRouter>|undefined,generation=-1;
self.onmessage=(event:MessageEvent<RoadRouteRequest>)=>{
 const request=event.data;
 if(request.kind==='load'){
  generation=request.generation;route=undefined;
  try{route=createRoadFollowRouter(request.bundle);self.postMessage({kind:'loaded',generation});}
  catch(error){self.postMessage({kind:'load-error',generation,error:String(error)});}
  return;
 }
 if(request.generation!==generation||!route)return;
 try{self.postMessage({kind:'route',generation,sequence:request.sequence,result:route(request.start,request.end)});}
 catch(error){self.postMessage({kind:'route',generation,sequence:request.sequence,error:String(error)});}
};
