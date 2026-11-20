import {test,expect} from 'vitest';
import {nearestJunctionSource,junctionDirection,junctionPrevious} from './junctionSelection';
import {DEFAULT_CAMERA,unproject} from './map';
const a=unproject(100,300),b=unproject(900,300),line={id:'3',owner:'2',points:[a,b]};
test('arbitrary interior click keeps its fraction rather than jumping to an anchor',()=>{
 const hit=nearestJunctionSource([line],unproject(350,303),DEFAULT_CAMERA,1,'2','ground')!;
 expect(hit.source.segmentIndex).toBe(0);expect(hit.source.fraction).toBeCloseTo(.3125);expect(hit.point.longitude).toBeCloseTo(unproject(350,300).longitude);expect(hit.source.expectedAlignment).toBe(line);
});
test('foreign ownership, different height, ramps and outer endpoints are not selectable',()=>{
 for(const invalid of [{...line,owner:'9'},{...line,infrastructure:[{level:'elevated'}]},{...line,vertical_profile:{start:0}}])expect(nearestJunctionSource([invalid],unproject(350,300),DEFAULT_CAMERA,1,'2','ground')).toBeNull();
 expect(nearestJunctionSource([line],a,DEFAULT_CAMERA,1,'2','ground')).toBeNull();
});
test('cursor direction determines tangent without moving the source',()=>{
 const hit=nearestJunctionSource([line],unproject(350,300),DEFAULT_CAMERA,1,'2','ground')!;
 const forward=junctionDirection(hit.source,hit.point,b),reverse=junctionDirection(hit.source,hit.point,a);
 expect(forward.reverse).toBe(false);expect(reverse.reverse).toBe(true);expect(junctionPrevious(forward,hit.point).longitude).toBeLessThan(hit.point.longitude);expect(junctionPrevious(reverse,hit.point).longitude).toBeGreaterThan(hit.point.longitude);
});
