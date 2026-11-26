// @vitest-environment jsdom
import {act,cleanup,renderHook} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {useRoadDrawing} from './useRoadDrawing';
class ControlledWorker {static latest:ControlledWorker;onmessage:any;onerror:any;sent:any[]=[];terminated=false;constructor(){ControlledWorker.latest=this;}postMessage(value:any){this.sent.push(value);}terminate(){this.terminated=true;}reply(id:number,result:any){this.onmessage({data:{id,result}});}}
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
const tiles:any[]=[],bundle={} as any;
const query=(longitude:number)=>({point:{latitude:35,longitude},radiusM:20});
function setup(){vi.stubGlobal('Worker',ControlledWorker);return renderHook(({enabled})=>useRoadDrawing(bundle,enabled,tiles),{initialProps:{enabled:true}});}
test('keeps only the newest pending pointer and rejects stale preview results',()=>{
 const h=setup(),w=ControlledWorker.latest;
 act(()=>{h.result.current.hover(query(1));h.result.current.hover(query(2));h.result.current.hover(query(3));});
 expect(w.sent.filter(m=>m.kind==='query')).toHaveLength(1);
 const first=w.sent.at(-1);act(()=>w.reply(first.id,{kind:'point',point:query(1).point}));expect(h.result.current.preview).toBeNull();
 expect(w.sent.at(-1).query.point.longitude).toBe(3);act(()=>w.reply(w.sent.at(-1).id,{kind:'point',point:query(3).point}));expect(h.result.current.preview).toEqual({kind:'point',point:query(3).point});
 act(()=>h.result.current.hover(query(4)));expect(h.result.current.preview).toEqual({kind:'point',point:query(3).point});
 act(()=>h.result.current.cancel());act(()=>w.reply(w.sent.at(-1).id,{kind:'point',point:query(4).point}));expect(h.result.current.preview).toBeNull();
});
test('cancelled clicks cannot place geometry later and teardown terminates the worker',async()=>{const h=setup(),w=ControlledWorker.latest;let result:any;act(()=>{void h.result.current.click(query(1)).then(value=>result=value);});act(()=>h.result.current.cancel());await act(async()=>{});expect(result).toBeNull();act(()=>w.reply(w.sent.at(-1).id,{kind:'point',point:query(1).point}));expect(result).toBeNull();h.rerender({enabled:false});expect(w.terminated).toBe(true);});

test('a retired click releases the editor lock without overlapping worker jobs',async()=>{
 const h=setup(),w=ControlledWorker.latest;
 const oldResult=vi.fn(),nextResult=vi.fn();
 act(()=>{void h.result.current.click(query(1)).then(oldResult);});
 const old=w.sent.at(-1);
 act(()=>h.result.current.cancel());
 expect(h.result.current.clickPending()).toBe(false);
 act(()=>{
  void h.result.current.click(query(2)).then(nextResult);
  h.result.current.hover(query(3));
 });
 expect(h.result.current.clickPending()).toBe(true);
 expect(w.sent.filter(m=>m.kind==='query')).toHaveLength(1);
 await act(async()=>w.reply(old.id,{kind:'point',point:query(1).point}));
 expect(oldResult).toHaveBeenCalledExactlyOnceWith(null);
 expect(nextResult).not.toHaveBeenCalled();
 expect(h.result.current.preview).toBeNull();
 const next=w.sent.at(-1);
 expect(next.query).toEqual(query(2));
 expect(w.sent.filter(m=>m.kind==='query')).toHaveLength(2);
 const expected={kind:'point',point:query(2).point};
 await act(async()=>w.reply(next.id,expected));
 expect(nextResult).toHaveBeenCalledExactlyOnceWith(expected);
 expect(h.result.current.clickPending()).toBe(false);
 expect(h.result.current.preview).toEqual(expected);
});
