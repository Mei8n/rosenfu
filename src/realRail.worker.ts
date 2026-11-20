import {loadRailProvider,type RailRequest,type RailResponse} from './realRailProvider';
let provider:Awaited<ReturnType<typeof loadRailProvider>>|null=null;
self.onmessage=async(event:MessageEvent<RailRequest>)=>{
 const request=event.data;let response:RailResponse;
 try {
  if(request.kind==='load'){
   const controller=new AbortController(),timeout=setTimeout(()=>controller.abort(),45000);
   try{provider=await loadRailProvider(controller.signal);response={id:request.id,kind:'loaded',source:provider.source};}finally{clearTimeout(timeout);}
  }else{
   if(!provider)throw Error('線路網が未取得です');
   if(request.kind==='view'){const view=provider.network.visible(request.bounds);response={id:request.id,kind:'view',view:{truncated:view.truncated,chains:view.chains.map(c=>({id:c.id,points:c.points}))}};}
   else if(request.kind==='route'){response={id:request.id,kind:'pick',pick:request.end,route:provider.network.route(request.start,request.end)};}
   else {const pick=provider.network.nearest(request.point,request.radius,request.visibleIds);response={id:request.id,kind:'pick',pick,route:pick&&request.start?provider.network.route(request.start,pick):null};}
  }
 }catch(error){response={id:request.id,kind:'error',message:error instanceof Error?error.message:String(error)};}
 self.postMessage(response);
};
