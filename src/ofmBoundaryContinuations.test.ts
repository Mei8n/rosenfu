import {describe,it,expect} from 'vitest';
import {findTileBoundaryContinuations as find,continuousSeparation,type TileRoadLine} from './ofmBoundaryContinuations';
const seam=139.76806640625;
function line(id:string,x=14552,offset=0):TileRoadLine{return {id,revision:'same',z:14,x,y:6451,class:'minor',coordinates:[[seam-.0003,35.684+offset],[seam+.0003,35.684+offset]]};}
describe('tile boundary geometric candidates',()=>{
 it('finds unique nonzero overlap without changing geometry',()=>{const a=line('a'),b=line('b',14553);const before=JSON.stringify([a,b]);const r=find([a,b]);expect(r.pairs).toHaveLength(1);expect(r.pairs[0].overlapMeters).toBeCloseTo(20);expect(JSON.stringify([a,b])).toBe(before);});
 it('permits small quantization difference',()=>expect(find([line('a'),line('b',14553,.000004)]).pairs).toHaveLength(1));
 it('rejects nearby parallel alternatives as ambiguous',()=>expect(find([line('a'),line('b',14553),line('c',14553,.000004)]).pairs).toHaveLength(0));
 it('does not join same-tile disjoint multiline fragments',()=>expect(find([line('a'),line('b')]).pairs).toHaveLength(0));
 it('rejects point contact rather than overlap',()=>{const a=line('a'),b=line('b',14553);a.coordinates=[[seam-.0003,35.684],[seam,35.684]];b.coordinates=[[seam,35.684],[seam+.0003,35.684]];expect(find([a,b]).pairs).toHaveLength(0);});
 it('rejects bridges, unknown structures and height mismatch',()=>{for(const change of [{brunnel:'bridge'},{brunnel:'tunnel'},{brunnel:'unknown'},{layer:1},{level:'1'}])expect(find([{...line('a'),...change},{...line('b',14553),...change}]).pairs).toHaveLength(0);});
 it('rejects revision, class and direction mismatch',()=>{for(const change of [{revision:'other'},{class:'service'},{oneway:1}])expect(find([line('a'),{...line('b',14553),...change}]).pairs).toHaveLength(0);});
 it('supports opposite feature digitization',()=>{const b=line('b',14553);b.coordinates=[...b.coordinates].reverse();expect(find([line('a'),b]).pairs).toHaveLength(1);});
 it('rejects reversed one-way geometry',()=>{const a={...line('a'),oneway:1},b={...line('b',14553),oneway:1};b.coordinates=[...b.coordinates].reverse();expect(find([a,b]).pairs).toHaveLength(0);});
 it('does not infer continuity across different access tags',()=>{expect(find([{...line('a'),access:'private'},{...line('b',14553),access:'yes'}]).pairs).toHaveLength(0);expect(find([{...line('a'),access:'no'},line('b',14553)]).pairs).toHaveLength(0);});
 it('rejects tile corners',()=>{const y=35.6929946320988;const a=line('a'),b=line('b',14553);a.coordinates=a.coordinates.map(([x])=>[x,y]);b.coordinates=b.coordinates.map(([x])=>[x,y]);expect(find([a,b]).pairs).toHaveLength(0);});
 it('rejects repeated passage and zero length',()=>{const a=line('a');a.coordinates=[...a.coordinates,...a.coordinates];expect(find([a,line('b',14553)]).pairs).toHaveLength(0);});
 it('rejects duplicate ids and invalid input',()=>{expect(find([line('a'),line('a',14553)]).pairs).toHaveLength(0);expect(find([{...line('a'),x:NaN}]).pairs).toHaveLength(0);});
 it('fails closed on capacity',()=>expect(find(Array.from({length:2001},(_,i)=>line(String(i)))).truncated).toBe(true));
});

it('rejects unknown raw oneway without treating it as absent',()=>{for(const oneway of ['yes','no','unknown',NaN,Infinity,2])expect(find([{...line('a'),oneway},{...line('b',14553),oneway}]).pairs).toHaveLength(0);});

it('bounds a source interior peak missed by endpoint sampling',()=>{expect(continuousSeparation([[0,0],[2,0]],[[0,0],[1,2],[2,0]])).toBeGreaterThan(.8);});
it('can refine a bend without accepting an unproved sample estimate',()=>{expect(continuousSeparation([[0,0],[2,0]],[[0,.3],[1,.4],[2,.3]])).toBeLessThanOrEqual(.8);});
it('exhausted continuous-distance budget fails conservatively',()=>{const budget={remaining:0};expect(continuousSeparation([[0,0],[2,0]],[[0,0],[2,0]],budget)).toBe(Infinity);});
