import {expect,it} from 'vitest';
import {cancelBusInteraction,clickBusControl,emptyBusInteraction,insertBusControl,moveBusControl,redoBusDraft,removeBusControl,requestBusFinish,settleBusValidation,undoBusDraft,type DraftControl,type OrdinaryBusInteraction} from './ordinaryBusInteraction';

const scope={};
const from=(id:string,n:number):DraftControl=>({id,anchor:{
 raw:{feature:3,line:2,vertex:n},point:{latitude:35.7,longitude:139.6+n*.001},
 tile:{x:14552,y:6452,z:14,decoderRule:'ofm_transportation_z14_v1',revision:'r1',rawSha256:'a'.repeat(64)},
}});
const [a,b,c,d]=[from('a',1),from('b',2),from('c',3),from('d',4)];
const list=(s:OrdinaryBusInteraction<string>)=>s.frame.body.controls.map(c=>c.id);
const accept=(s:OrdinaryBusInteraction<string>,proof:string)=>settleBusValidation(s,scope,s.check!.id,{verified:proof});
const initial=()=>accept(clickBusControl(clickBusControl(emptyBusInteraction<string>(scope),a),b),'ab');

it('an ordinary click extends the active end or switches to the other handle without committing',()=>{
 let s=initial();
 expect(list(s)).toEqual(['a','b']);
 s=clickBusControl(s,from('same-source-different-id',1));
 expect(s.frame.body.active).toBe('start');expect(s.finish).toBeUndefined();
 s=accept(clickBusControl(s,c),'cab');
 expect(list(s)).toEqual(['c','a','b']);
 expect(requestBusFinish(s).finish?.verified).toBe('cab');
 expect(clickBusControl(s,from('same-as-start',3)).finish?.verified).toBe('cab');
});

it('double click requests only the immediate pending validated result without adding a duplicate',()=>{
 const old=initial(), prior=clickBusControl(old,c);
 const next=clickBusControl(prior,from('second-event-id',3),2);
 expect(next.check?.body.controls.map(c=>c.id)).toEqual(['a','b','c']);
 expect(next.finish).toBeUndefined();
 expect(settleBusValidation(next,scope,prior.check!.id-1,{verified:'old'})).toBe(next);
 expect(settleBusValidation(next,{other:'world'},prior.check!.id,{verified:'old'})).toBe(next);
 const valid=accept(next,'abc');
 expect(valid.finish?.verified).toBe('abc');
 expect(list(valid)).toEqual(['a','b','c']);
});

it('refuses the latest leg without enabling a previous preview to commit',()=>{
 const old=initial(), next=requestBusFinish(clickBusControl(old,c));
 const bad=settleBusValidation(next,scope,next.check!.id,{reason:'接続できません',leg:1});
 expect(bad.refusal?.leg).toBe(1);
 expect(bad.refusal?.attempt.controls.map(c=>c.id)).toEqual(['a','b','c']);
 expect(list(bad)).toEqual(['a','b']);
 expect(requestBusFinish(bad).finish).toBeUndefined();
 expect(undoBusDraft(bad).refusal).toBeUndefined();
});

it('moves, inserts and removes stable IDs in local history without rerouting accepted undo/redo frames',()=>{
 let s=accept(insertBusControl(initial(),a.id,c),'acb');
 s=accept(moveBusControl(s,c.id,d.anchor),'moved');
 expect(list(s)).toEqual(['a','c','b']);
 expect(s.frame.body.controls[1].anchor).toBe(d.anchor);
 s=accept(removeBusControl(s,c.id),'removed');
 expect(list(s)).toEqual(['a','b']);
 s=undoBusDraft(s);expect(s.frame.verified).toBe('moved');
 expect(s.frame.body.controls[1].id).toBe(c.id);
 s=undoBusDraft(s);expect(s.frame.verified).toBe('acb');
 s=redoBusDraft(s);expect(s.frame.verified).toBe('moved');
});

it('undo/cancel discards async commit intent; bounded drafts reject over-cap before a query',()=>{
 const before=initial(), pending=requestBusFinish(clickBusControl(before,c));
 const undo=undoBusDraft(pending);
 expect(settleBusValidation(undo,scope,pending.check!.id,{verified:'abc'})).toBe(undo);
 expect(undo.finish).toBeUndefined();
 const cancel=cancelBusInteraction(pending);
 expect(list(cancel)).toEqual([]);
 expect(settleBusValidation(cancel,scope,pending.check!.id,{verified:'abc'})).toBe(cancel);
 const limited=clickBusControl({...before,limit:2},c);
 expect(limited.check).toBeUndefined();expect(limited.refusal?.reason).toContain('多すぎます');
});
