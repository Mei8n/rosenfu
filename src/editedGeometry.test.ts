import {test,expect} from 'vitest';
import {circularAlignment} from './circularAlignment';
import {anchorDragPreview} from './anchorDragPreview';
import {plannedGeometry,sampledGeometry, type EditedGeometry} from './editedGeometry';
import {plannedTrackSpeed} from './plannedTrackSpeed';
const p=(x:number,y=0)=>({latitude:35+y/111000,longitude:139+x/91000});
test('legacy shape is unchanged; edited shape survives roundtrip and continues without regeneration',()=>{
 const anchors=[p(0),p(200),p(400)],original=circularAlignment(anchors);
 expect(plannedGeometry(anchors)).toEqual(original);
 const moved=[anchors[0],p(200,20),anchors[2]],preview=anchorDragPreview(original.points,anchors,1,moved[1]);
 const edited:EditedGeometry={version:1,anchors:moved,line:preview.line};
 const geometry=plannedGeometry(moved,JSON.parse(JSON.stringify(edited)));
 expect(geometry.error).toBeNull();expect(geometry.points).toEqual(preview.line);
 expect(geometry.arcs.length).toBeGreaterThan(1);
 const extended=plannedGeometry([...moved,p(600)],edited);
 expect(extended.error).toBeNull();expect(extended.points.slice(0,geometry.points.length)).toEqual(geometry.points);
 expect(plannedGeometry(moved.slice(0,2),edited).points.at(-1)).toEqual(moved[1]);
 expect(plannedTrackSpeed(geometry,moved[1],30)).toBeLessThanOrEqual(30);
});
test('endpoint movement retains opposite anchor and degenerate and tight shapes cannot build',()=>{
 const anchors=[p(0),p(200),p(400)],line=circularAlignment(anchors).points;
 for(const index of [0,2]){const target=p(index*200,10),preview=anchorDragPreview(line,anchors,index,target);expect(preview.error).toBeNull();expect(preview.line[index===0?0:preview.line.length-1]).toEqual(target);expect(sampledGeometry(preview.line).error).toBeNull();}
 expect(sampledGeometry([p(0),p(0)]).error).not.toBeNull();
 expect(sampledGeometry([p(0),p(1),p(1,1)]).error).not.toBeNull();
 expect(plannedGeometry([p(0),p(200,1),p(400)],{version:1,anchors,line}).error).not.toBeNull();
});
test('resampling a curved plan does not invent a much tighter curve at every chord boundary',()=>{
 const anchors=[p(0),p(200),p(400,100),p(500,300)],old=circularAlignment(anchors);
 expect(old.error).toBeNull();
 const target={...anchors[1],latitude:anchors[1].latitude+1e-7};
 const edited=anchorDragPreview(old.points,anchors,1,target);
 const current=sampledGeometry(edited.line);
 expect(current.error).toBeNull();expect(current.minimumRadiusM).toBeGreaterThan(old.minimumRadiusM*.8);
});
test('edited extension still enforces the backend five-degree join angle',()=>{
 const anchors=[p(0),p(100,100)],geometry:EditedGeometry={version:1,anchors,line:anchors};
 expect(plannedGeometry(anchors,geometry,p(-100)).error).toContain('接合角');
 const straight=[p(0),p(100)];expect(plannedGeometry(straight,{version:1,anchors:straight,line:straight},p(-100)).error).toBeNull();
});
