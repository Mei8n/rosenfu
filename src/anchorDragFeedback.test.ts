import {test,expect} from 'vitest';
import {anchorDragFeedback} from './anchorDragFeedback';
import {anchorDragPreview} from './anchorDragPreview';
import {circularAlignment} from './circularAlignment';
const p=(x:number,y=0)=>({latitude:35+y/111000,longitude:139+x/91000});
test('moved middle labels both adjacent intervals separately, capped by design speed',()=>{
 const anchors=[p(0),p(200),p(600)],target=p(200,30);
 const line=anchorDragPreview(circularAlignment(anchors).points,anchors,1,target).line;
 const edited={version:1 as const,anchors:[anchors[0],target,anchors[2]],line};
 const labels=anchorDragFeedback(edited,1,300);
 expect(labels.map(label=>label.interval)).toEqual([0,1]);
 expect(labels[0].speed).not.toBe(labels[1].speed);
 expect(labels.every(label=>label.speed>0&&label.speed<=300)).toBe(true);
 expect(anchorDragFeedback(edited,1,20).map(label=>label.speed)).toEqual([20,20]);
 expect(anchorDragFeedback(edited,0,300)).toHaveLength(1);
 expect(anchorDragFeedback(edited,2,300)).toHaveLength(1);
});
test('invalid preview or missing anchors cannot show an optimistic speed',()=>{
 const anchors=[p(0),p(200),p(400)];
 expect(anchorDragFeedback({version:1,anchors,line:[p(0),p(0)]},1,120)).toEqual([]);
 expect(anchorDragFeedback({version:1,anchors,line:[p(0),p(400)]},1,120)).toEqual([]);
});
