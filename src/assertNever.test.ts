import {expect,test} from 'vitest';
import {assertNever} from './assertNever';

test('an unhandled variant is rejected by TypeScript and fails explicitly at runtime',()=>{
 // This directive must remain used: accepting a string here would disable exhaustiveness checks.
 // @ts-expect-error Unhandled union members cannot be passed to never.
 expect(()=>assertNever('new_variant')).toThrow('未対応の値です: new_variant');
});
