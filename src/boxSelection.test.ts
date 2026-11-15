import {expect,test} from 'vitest';
import {selectAlignmentSections} from './boxSelection';
import {DEFAULT_CAMERA,unproject} from './map';
const points=[[100,300],[500,300],[900,300]].map(([x,y])=>unproject(x,y));
const line={id:'3',owner:'2',points,anchor_indices:[0,1,2]};
test('crossing rectangle selects only intersected anchor intervals in either drag direction',()=>{
 for(const [start,end] of [[{x:200,y:280},{x:300,y:320}],[{x:300,y:320},{x:200,y:280}]])expect(selectAlignmentSections([line],'2',{start,end},DEFAULT_CAMERA)).toEqual([{alignmentId:'3',sectionIndices:[0]}]);
});
test('all crossed sections group once, foreign lines and nonintersections excluded',()=>{
 expect(selectAlignmentSections([line,{...line,id:'4',owner:'other'}],'2',{start:{x:200,y:290},end:{x:800,y:310}},DEFAULT_CAMERA)).toEqual([{alignmentId:'3',sectionIndices:[0,1]}]);
 expect(selectAlignmentSections([line],'2',{start:{x:200,y:200},end:{x:800,y:250}},DEFAULT_CAMERA)).toEqual([]);
});
