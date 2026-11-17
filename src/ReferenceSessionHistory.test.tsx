// @vitest-environment jsdom
import {act,cleanup,renderHook} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {useReferenceCopy,sameReferenceSession,type ReferenceSessionSnapshot} from './useReferenceCopy';
import {syntheticReferenceGraph} from './fixtures/referenceGraph';
afterEach(cleanup);
function setup(){const graph=syntheticReferenceGraph();graph.edges[1].osmWayId='101';graph.edges[1].segmentIndex=1;graph.nodes[2].latitude=35.0005;
 const props={bundle:{graph,graphSha256:'a'.repeat(64),sourceSha256:'b'.repeat(64),source:{source:'synthetic',sourceDate:null,license:'ODbL-1.0',attribution:'© OpenStreetMap contributors'}},worldVersion:0,enabled:true,canStart:true,tool:'tracks',name:'参考線',onBuild:vi.fn(async()=>{})};
 return {...renderHook(p=>useReferenceCopy(p),{initialProps:props}),props};}
async function first(h:ReturnType<typeof setup>){act(()=>h.result.current.begin());await act(()=>h.result.current.click({latitude:35,longitude:134.9995}));act(()=>h.result.current.choose(h.result.current.candidates.findIndex(c=>c.edge.id==='101')));}
test('restores exact picks/redo with a matching session and resets confirmation/candidates',async()=>{
 const h=setup();await first(h);const one=h.result.current.sessionSnapshot;
 await act(()=>h.result.current.click({latitude:35.00025,longitude:135.0005}));act(()=>h.result.current.choose(h.result.current.candidates.findIndex(c=>c.edge.id==='102')));
 const two=h.result.current.sessionSnapshot;act(()=>h.result.current.setConfirmed(true));
 let accepted=false;act(()=>{accepted=h.result.current.restoreSession(one,two);});expect(accepted).toBe(true);expect(sameReferenceSession(h.result.current.sessionSnapshot,one)).toBe(true);expect(h.result.current.confirmed).toBe(false);expect(h.result.current.candidates).toEqual([]);
 act(()=>h.result.current.undo());const undone=h.result.current.sessionSnapshot;expect(undone.redo).toHaveLength(1);
 act(()=>{accepted=h.result.current.restoreSession(two,undone);});expect(accepted).toBe(true);expect(h.result.current.picks).toEqual(two.picks);
 act(()=>{accepted=h.result.current.restoreSession(undone,two);});expect(accepted).toBe(true);expect(h.result.current.redo).toEqual(undone.redo);
});
test('stale expected session, replaced bundle and invalid picks leave session unchanged',async()=>{
 const h=setup();const empty=h.result.current.sessionSnapshot;await first(h);const current=h.result.current.sessionSnapshot;
 const invalids:ReferenceSessionSnapshot[]=[{...current,picks:[{...current.picks[0],fraction:NaN}]},{...current,picks:[{...current.picks[0],edgeId:'missing'}]},{...current,picks:[{...current.picks[0],revision:'other'}]},{...current,mode:'road'},{...current,active:false},{...current,picks:[...current.picks,...current.picks,...current.picks]}];
 for(const target of invalids){let ok=true;act(()=>{ok=h.result.current.restoreSession(target,current);});expect(ok).toBe(false);expect(h.result.current.sessionSnapshot).toBe(current);}
 act(()=>expect(h.result.current.restoreSession(empty,empty)).toBe(false));
 h.rerender({...h.props,bundle:{...h.props.bundle}});act(()=>expect(h.result.current.restoreSession(current,h.result.current.sessionSnapshot)).toBe(false));expect(h.result.current.active).toBe(false);
});
test('pending query rejects restore without cancelling the query; disabled rejects too',async()=>{
 const h=setup();await first(h);const current=h.result.current.sessionSnapshot;let request:Promise<void>|undefined;
 act(()=>{request=h.result.current.click({latitude:35,longitude:134.9995});expect(h.result.current.restoreSession(current,current)).toBe(false);});
 await act(async()=>request);expect(h.result.current.candidates.length).toBeGreaterThan(0);
 h.rerender({...h.props,enabled:false});act(()=>expect(h.result.current.restoreSession(current,h.result.current.sessionSnapshot)).toBe(false));
});
