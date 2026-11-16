// @vitest-environment jsdom
import {act,cleanup,renderHook,waitFor} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {useReferenceCopy} from './useReferenceCopy';
import type {ReferenceGraphBundle} from './referenceGraphBundle';
afterEach(cleanup);
function setup(){
 const bundle:ReferenceGraphBundle={graph:{format:'rosenfu-reference-graph',version:1,dataset:{id:'test',revision:'r1',source:'test',license:'test',bounds:{west:134.9,east:135.1,south:34.9,north:35.1}},nodes:[{osmId:'1',latitude:35,longitude:135},{osmId:'2',latitude:35,longitude:135.01}],edges:[{id:'1',osmWayId:'1',segmentIndex:0,from:'1',to:'2',mode:'road',layer:0,level:null,bridge:false,tunnel:false}]},graphSha256:'a'.repeat(64),sourceSha256:'b'.repeat(64),source:{source:'test',sourceDate:null,license:'test',attribution:'test'},roadWays:{'1':{highway:'residential',access:null,motorVehicle:null,structureUnsupported:false}}};
 const props={bundle,worldVersion:0,enabled:true,canStart:true,tool:'tracks',name:'road',roadFollow:true,directRoad:true,onBuild:vi.fn(async()=>{})};
 return {...renderHook(p=>useReferenceCopy(p),{initialProps:props}),props};
}
const a={latitude:35,longitude:135.001},b={latitude:35,longitude:135.009};
test('direct road starts without a second start button, previews on hover, and stages on the second map click',async()=>{
 const h=setup();expect(h.result.current.active).toBe(false);
 await act(()=>h.result.current.click(a));expect(h.result.current.picks).toHaveLength(1);expect(h.result.current.candidates).toEqual([]);
 await act(()=>h.result.current.hover(b));expect(h.result.current.hoverPlan?.points).toHaveLength(2);expect(h.props.onBuild).not.toHaveBeenCalled();
 await act(()=>h.result.current.click(b));await waitFor(()=>expect(h.props.onBuild).toHaveBeenCalledTimes(1));expect(h.result.current.picks).toEqual([]);expect(h.result.current.active).toBe(false);
});
test('failed staging retains endpoints, and ordinary enabled state updates do not reset them',async()=>{
 const h=setup();h.props.onBuild.mockRejectedValueOnce(Error('failure'));
 await act(()=>h.result.current.click(a));await act(()=>h.result.current.click(b));await waitFor(()=>expect(h.result.current.message).toContain('failure'));
 expect(h.result.current.picks).toHaveLength(2);h.rerender({...h.props,enabled:false});h.rerender(h.props);expect(h.result.current.picks).toHaveLength(2);
 await act(()=>h.result.current.build());expect(h.props.onBuild).toHaveBeenCalledTimes(2);
});
test('missing candidate does not invent a road or stage a straight line',async()=>{
 const h=setup();await act(()=>h.result.current.click({latitude:35.05,longitude:135}));expect(h.result.current.picks).toEqual([]);expect(h.props.onBuild).not.toHaveBeenCalled();
});

test('clear exits editing without silently restarting and a later click starts a fresh road',async()=>{
 const h=setup();await act(()=>h.result.current.click(a));expect(h.result.current.active).toBe(true);
 act(()=>h.result.current.clear());expect(h.result.current.active).toBe(false);expect(h.result.current.picks).toEqual([]);
 h.rerender({...h.props,enabled:false});h.rerender(h.props);expect(h.result.current.active).toBe(false);
 await act(()=>h.result.current.click(b));expect(h.result.current.active).toBe(true);expect(h.result.current.picks).toHaveLength(1);
 expect(h.props.onBuild).not.toHaveBeenCalled();
});
