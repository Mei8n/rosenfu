import {expect,test} from 'vitest';
import {plannedTrackHeight} from './plannedTrackHeight';
const points=[{latitude:35,longitude:139},{latitude:35,longitude:139.01},{latitude:35,longitude:139.02}];
test('flat height and changing section endpoints are local to hovered section',()=>{
 const path={points,anchorLevels:['ground','ground','elevated'] as const};
 expect(plannedTrackHeight({...path,anchorLevels:[...path.anchorLevels]},points,{latitude:35,longitude:139.005})).toBe('高さ 0 m');
 expect(plannedTrackHeight({...path,anchorLevels:[...path.anchorLevels]},points,{latitude:35,longitude:139.015})).toBe('区間端点高 0 → +6 m');
});
test('negative heights and defaults are truthful',()=>{
 expect(plannedTrackHeight({points,infrastructure:{track_count:2,level:'underground_3'}},points,points[1])).toBe('高さ -18 m');
 expect(plannedTrackHeight({points,startLevel:'elevated_3'},points,points[0])).toBe('区間端点高 +18 → 0 m');
});
test('unmatched samples or malformed height metadata are not shown as ground',()=>{
 expect(plannedTrackHeight({points},[points[0],points[2]],points[1])).toBe('高さ：未確認');
 expect(plannedTrackHeight({points,anchorLevels:['ground']},points,points[1])).toBe('高さ：未確認');
});
