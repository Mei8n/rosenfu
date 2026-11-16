import {expect,test} from 'vitest';
import {constructionValidationError,matchingPlacementError} from './constructionPlacementGuard';
const line=[{latitude:35,longitude:139},{latitude:35.1,longitude:139.1}];
test('only structured completed validation failures reject placement',()=>{
 expect(constructionValidationError({validationError:'水域に敷設できません'})).toBe('水域に敷設できません');
 for(const value of [null,undefined,{error:'network error'},{validationError:null},{validationError:''}])expect(constructionValidationError(value)).toBeNull();
});
test('rejects the matching displayed candidate without rejecting a moved click',()=>{
 expect(matchingPlacementError('invalid',true,line,line.map(p=>({...p})))).toBe('invalid');
 expect(matchingPlacementError('invalid',false,line,line)).toBeNull();
 expect(matchingPlacementError('invalid',true,line,[line[0],{...line[1],latitude:35.2}])).toBeNull();
 expect(matchingPlacementError('invalid',true,line,[line[0]])).toBeNull();
 expect(matchingPlacementError(null,true,line,line)).toBeNull();
});
