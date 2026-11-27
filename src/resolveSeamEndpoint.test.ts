import {expect,test,vi} from 'vitest';
import {firstByKey} from './mapLookupIndex';
import {resolveSeamEndpoint} from './resolveSeamEndpoint';
const build=(id:string,latitude:number)=>({id,points:[{latitude,longitude:139},{latitude,longitude:140}]});
test('unset and malformed picks do not touch the current world index',()=>{
 const index=new Map<string,ReturnType<typeof build>>();
 const get=vi.spyOn(index,'get');
 for(const key of ['', 'unset', 'a:Start','a:end ','a:start:','a:ending'])
  expect(resolveSeamEndpoint(index,key)).toBeUndefined();
 expect(get).not.toHaveBeenCalled();
});
test('use exactly the last delimiter, preserve the first matching alignment and never reuse an old world result',()=>{
 const first=build('road:end:3',35), duplicate=build(first.id,36), prefix=build('road:end',37);
 const index=firstByKey([first,duplicate,prefix],a=>a.id);
 const get=vi.spyOn(index,'get');
 const end=resolveSeamEndpoint(index,'road:end:3:end');
 expect(end).toEqual({key:'road:end:3:end',alignment:first,endpoint:'end',point:first.points[1]});
 expect(end?.alignment).toBe(first);
 expect(get).toHaveBeenCalledExactlyOnceWith('road:end:3');
 expect(resolveSeamEndpoint(index,'road:end:3:start')?.point).toBe(first.points[0]);
 expect(resolveSeamEndpoint(index,'unknown:start')).toBeUndefined();
 const replacement=build(first.id,38);
 expect(resolveSeamEndpoint(new Map([[first.id,replacement]]),'road:end:3:end')?.point).toBe(replacement.points[1]);
 expect(resolveSeamEndpoint(new Map(),'road:end:3:end')).toBeUndefined();
});
