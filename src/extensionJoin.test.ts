import {expect,test} from 'vitest';
import {extensionJoinError} from './extensionJoin';
const previous={latitude:35,longitude:139},anchor={latitude:35,longitude:139.01};
test('extension join matches five degree boundary and rejects backward and perpendicular paths',()=>{
 for(const angle of [0,4.99,5.01,90,180]){
 const rad=angle*Math.PI/180,next={latitude:35+0.01*Math.sin(rad),longitude:139.01+0.01*Math.cos(rad)/Math.cos(35*Math.PI/180)};
 expect(Boolean(extensionJoinError([anchor,next],previous))).toBe(angle>5);
 }
 expect(extensionJoinError([anchor,previous])).toBeNull();
 expect(extensionJoinError([anchor],previous)).toBeNull();
});
