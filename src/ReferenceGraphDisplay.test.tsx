// @vitest-environment jsdom
import React from 'react';
import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {indexGraphDisplay,queryGraphDisplay,GRAPH_DISPLAY_LIMIT} from './referenceGraphDisplay';
import {ReferenceGraphLayer} from './ReferenceGraphLayer';
import {ReferenceGraphControls} from './ReferenceGraphControls';
import {readReferenceGraphBundle} from './referenceGraphBundle';
import {syntheticReferenceGraph} from './fixtures/referenceGraph';
import {DEFAULT_CAMERA} from './map';
import {App} from './App';
import {invoke} from '@tauri-apps/api/core';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:()=>false}));
vi.mock('./referenceGraphBundle',()=>({readReferenceGraphBundle:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
const graph=syntheticReferenceGraph();const bundle={graph,graphSha256:'a'.repeat(64),sourceSha256:'b'.repeat(64),source:{source:'local <script>text</script>',sourceDate:null,license:'ODbL-1.0',attribution:'© OpenStreetMap contributors'}};
test('display caps dense data, hides low zoom, includes crossing segments, and uses exactly two passive paths',()=>{
 const index=indexGraphDisplay(graph);const first=index[0];const center={west:(first.bounds.west+first.bounds.east)/2-.000001,east:(first.bounds.west+first.bounds.east)/2+.000001,south:(first.bounds.south+first.bounds.north)/2-.000001,north:(first.bounds.south+first.bounds.north)/2+.000001};
 expect(queryGraphDisplay(index,center,14,true).edges).toContain(first);expect(queryGraphDisplay(index,center,13,true).edges).toHaveLength(0);expect(queryGraphDisplay(index,center,14,false).edges).toHaveLength(0);
 const dense=Array.from({length:40000},()=>first);const result=queryGraphDisplay(dense,center,14,true);expect(result.total).toBe(40000);expect(result.edges).toHaveLength(GRAPH_DISPLAY_LIMIT);
 const click=vi.fn();const view=render(<svg onClick={click}><ReferenceGraphLayer edges={index} camera={DEFAULT_CAMERA}/></svg>);const g=view.container.querySelector('g')!;expect(g.getAttribute('pointer-events')).toBe('none');expect(g.querySelectorAll('path')).toHaveLength(2);expect(g.querySelector('path')?.getAttribute('d')).not.toMatch(/[CQ]/);fireEvent.click(g);expect(click).toHaveBeenCalledOnce();
});
test('invalid replacement preserves pack, unmount and clear discard delayed successful reads',async()=>{
 const onBundle=vi.fn(),props={bundle,visible:true,onBundle,onVisible:vi.fn(),total:4,shown:4,zoom:14};const view=render(<ReferenceGraphControls {...props}/>);const files=[new File(['x'],'graph.json')];
 vi.mocked(readReferenceGraphBundle).mockRejectedValueOnce(Error('hash mismatch'));fireEvent.change(screen.getByLabelText('参考線バンドル3ファイル'),{target:{files}});await screen.findByText(/hash mismatch/);expect(onBundle).not.toHaveBeenCalled();expect(view.container.querySelector('script')).toBeNull();expect(screen.getByText(/local <script>text/)).toBeTruthy();
 let finish!:(value:any)=>void;vi.mocked(readReferenceGraphBundle).mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));fireEvent.change(screen.getByLabelText('参考線バンドル3ファイル'),{target:{files}});fireEvent.click(screen.getByRole('button',{name:'参考線を外す'}));finish(bundle);await waitFor(()=>expect(screen.queryByText('参考線バンドルを検証中…')).toBeNull());expect(onBundle.mock.calls).toEqual([[null]]);
 fireEvent.change(screen.getByLabelText('参考線バンドル3ファイル'),{target:{files}});view.unmount();finish(bundle);await Promise.resolve();expect(onBundle.mock.calls).toEqual([[null]]);
});

test('App reference import stays out of World commands and obeys zoom and visibility controls',async()=>{
 const bounds=graph.dataset.bounds;
 const nearby={...graph,dataset:{...graph.dataset,bounds:{west:bounds.west-135+DEFAULT_CAMERA.longitude,east:bounds.east-135+DEFAULT_CAMERA.longitude,south:bounds.south-35+DEFAULT_CAMERA.latitude,north:bounds.north-35+DEFAULT_CAMERA.latitude}},nodes:graph.nodes.map(n=>({...n,longitude:n.longitude-135+DEFAULT_CAMERA.longitude,latitude:n.latitude-35+DEFAULT_CAMERA.latitude}))};vi.mocked(readReferenceGraphBundle).mockResolvedValue({...bundle,graph:nearby});const view=render(<App/>);fireEvent.change(screen.getByLabelText('参考線バンドル3ファイル'),{target:{files:[new File(['x'],'graph.json')]}});await screen.findByText(/synthetic-test/);
 expect(view.container.querySelector('.reference-graph-rail')?.getAttribute('d')).toBe('');fireEvent.click(screen.getByRole('button',{name:'拡大'}));fireEvent.click(screen.getByRole('button',{name:'拡大'}));await waitFor(()=>expect(view.container.querySelector('.reference-graph-rail')?.getAttribute('d')).toMatch(/^M/));expect(view.container.querySelector('.reference-graph-road')?.getAttribute('d')).toMatch(/^M/);
 const map=view.container.querySelector('svg')!;fireEvent.click(map,{clientX:200,clientY:200});expect(invoke).not.toHaveBeenCalled();fireEvent.click(screen.getByLabelText('参考線を表示'));expect(view.container.querySelector('.reference-graph-rail')?.getAttribute('d')).toBe('');fireEvent.click(screen.getByRole('button',{name:'参考線を外す'}));expect(screen.queryByText(/synthetic-test/)).toBeNull();
});

test('large external metadata is bounded in DOM with explicit truncation, not a hidden full text attribute',()=>{
 const long='x'.repeat(100000);const large={...bundle,source:{source:long,sourceDate:long,license:long,attribution:long},graph:{...graph,dataset:{...graph.dataset,id:long,revision:long}}};const view=render(<ReferenceGraphControls bundle={large} visible onBundle={vi.fn()} onVisible={vi.fn()} total={0} shown={0} zoom={14}/>);expect(view.container.textContent!.length).toBeLessThan(3000);expect(screen.getAllByText(/表示を200文字で省略/).length).toBeGreaterThan(0);expect(view.container.innerHTML).not.toContain('x'.repeat(201));expect(large.source.sourceDate).toHaveLength(100000);
});
