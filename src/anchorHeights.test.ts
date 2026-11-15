import {expect,test} from 'vitest';
import {anchorHeights,changeAnchorHeight} from './anchorHeights';
test('each anchor keeps its own explicit level and edits do not mutate history',()=>{
 const original=anchorHeights(3,['ground','elevated','ground'],undefined,'ground');
 const next=changeAnchorHeight(original,1,'underground');
 expect(original).toEqual(['ground','elevated','ground']);
 expect(next).toEqual(['ground','underground','ground']);
 expect(anchorHeights(3,next,'ground','elevated_3')).toEqual(next);
});
test('absent metadata derives editor defaults while malformed explicit state rejects',()=>{
 expect(anchorHeights(3,undefined,'ground','elevated')).toEqual(['ground','elevated','elevated']);
 expect(()=>anchorHeights(2,['ground'],undefined,'ground')).toThrow('一致');
 expect(()=>changeAnchorHeight(['ground'],1,'elevated')).toThrow('ありません');
});
