/** Coalesces previews for already-prepared, immutable route proof only.
 * Never obtains evidence itself, never mutates the world, and never runs a
 * second IPC request while an older one is in flight. */
export function createOrdinaryBusHover<Q, R>(
 run:(query:Q,signal:AbortSignal)=>Promise<R>,
 show:(result:R|null)=>void,
 delayMs=120,
){
 type Work={key:string;query:Q};
 const limit=12;
 let latest:Work|null=null, revision=0,epoch=0;
 let timer:ReturnType<typeof setTimeout>|undefined;
 let running:AbortController|undefined;
 let completedKey:string|undefined;
 const cache=new Map<string,R>();
 function clearTimer(){if(timer!==undefined){clearTimeout(timer);timer=undefined;}}
 function arm(){
  if(!latest||running||timer!==undefined||completedKey===latest.key)return;
  timer=setTimeout(()=>{
   timer=undefined;
   if(!latest||running)return;
   const work=latest,awaiting=revision,sameEpoch=epoch;
   const controller=new AbortController();running=controller;
   void Promise.resolve().then(()=>run(work.query,controller.signal)).then(result=>{
    if(sameEpoch!==epoch||controller.signal.aborted)return;
    cache.delete(work.key);cache.set(work.key,result);
    if(cache.size>limit)cache.delete(cache.keys().next().value!);
    if(revision===awaiting){completedKey=work.key;show(result);}
   }).catch(()=>{
    // Evidence gone/stale is not a negative path proof. Clear the visual
    // preview but don't retry perpetually for the same hover.
    if(sameEpoch===epoch&&revision===awaiting){completedKey=work.key;show(null);}
   }).finally(()=>{if(running===controller)running=undefined;arm();});
  },delayMs);
 }
 /** Parent passes null whenever current proof is missing/evicted or a click
  * will need actual data acquisition. A key includes exact world token,
  * ordered stable anchor keys and immutable tile digest manifest. */
 function offer(work:Work|null){
  if(latest?.key===work?.key)return;
  revision++;latest=work;completedKey=undefined;clearTimer();
  const recorded=work&&cache.has(work.key)?cache.get(work.key):undefined;
  if(work&&recorded!==undefined){completedKey=work.key;show(recorded);return;}
  show(null);arm();
 }
 function invalidate(){
  epoch++;revision++;latest=null;completedKey=undefined;cache.clear();clearTimer();running?.abort();show(null);
 }
 return {offer,invalidate};
}
