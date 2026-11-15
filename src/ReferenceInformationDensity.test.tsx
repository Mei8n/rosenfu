// @vitest-environment jsdom
import React from 'react';
import {cleanup,render,screen} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {AutomaticRailControls} from './AutomaticRailControls';
import {ReferenceGraphControls} from './ReferenceGraphControls';
import {syntheticReferenceGraph} from './fixtures/referenceGraph';
import type {ReferenceCopyState} from './useReferenceCopy';
afterEach(cleanup);
test('rail controls keep actions and limits without implementation instructions',()=>{
 const copy={active:false,busy:false,plan:{},message:'',picks:[],redo:[],begin:vi.fn()} as unknown as ReferenceCopyState;
 const {container}=render(<AutomaticRailControls copy={copy} status="unknown" retry={vi.fn()} enabled canStart zoom={14} truncated={false}/>);
 expect(screen.getByText('線路データ未確認')).toBeTruthy();
 expect(screen.getByRole('button',{name:'実在線路の追従を開始'})).toBeTruthy();
 expect(container.textContent).not.toContain('元graph');expect(container.textContent).not.toContain('12px');
 const details=container.querySelector('details')!;expect(details.hasAttribute('open')).toBe(false);
 expect(details.textContent).toContain('橋・地下の高さも自動設定しません');
 expect(details.textContent).toContain('TrainsWorld / noritetsu / OpenStreetMap');
});
test('reference data details are collapsed while truncation remains visible',()=>{
 const graph=syntheticReferenceGraph();
 const bundle={graph,graphSha256:'a'.repeat(64),sourceSha256:'b'.repeat(64),source:{source:'example',sourceDate:null,license:'ODbL-1.0',attribution:'© OpenStreetMap contributors'}};
 const {container}=render(<ReferenceGraphControls bundle={bundle} visible onBundle={vi.fn()} onVisible={vi.fn()} total={100} shown={10} zoom={14}/>);
 expect(screen.getByRole('status').textContent).toContain('一部省略中');
 expect(screen.getByText(`${graph.dataset.id} / ${graph.dataset.revision}`).closest('details')?.hasAttribute('open')).toBe(false);
 expect(screen.getByText('出典・対応範囲').closest('details')?.hasAttribute('open')).toBe(false);
 expect(screen.getByRole('button',{name:'参考線を外す'})).toBeTruthy();
});
