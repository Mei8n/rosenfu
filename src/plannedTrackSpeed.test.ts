import {expect,test} from 'vitest';
import {circularAlignment,provisionalCurveSpeed} from './circularAlignment';
import {plannedTrackSpeed} from './plannedTrackSpeed';
test('hover speed follows the segment and is capped by design speed',()=>{
 const anchors=[{latitude:35,longitude:139},{latitude:35,longitude:139.001},{latitude:35.0004,longitude:139.0014}];
 const g=circularAlignment(anchors);
 expect(plannedTrackSpeed(g,{latitude:35,longitude:139.0005},120)).toBe(120);
 expect(plannedTrackSpeed(g,g.arcs[0].midpoint,120)).toBe(Math.min(120,provisionalCurveSpeed(g.arcs[0].radiusM)!));
 expect(plannedTrackSpeed(g,g.arcs[0].midpoint,10)).toBe(10);
 expect(plannedTrackSpeed({...g,error:'invalid'},anchors[0],120)).toBeNull();
});

test('relaxed curve limit still respects the target speed',()=>{
 const anchors=[{latitude:35,longitude:139},{latitude:35,longitude:139.001},{latitude:35.0004,longitude:139.0014}];
 const g=circularAlignment(anchors);
 const geometry={...g,arcs:g.arcs.map(arc=>({...arc,radiusM:1000}))};
 expect(plannedTrackSpeed(geometry,g.arcs[0].midpoint,300)).toBe(139);
 expect(plannedTrackSpeed(geometry,g.arcs[0].midpoint,120)).toBe(120);
});
