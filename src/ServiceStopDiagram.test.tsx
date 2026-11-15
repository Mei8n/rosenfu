// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen,within} from '@testing-library/react';
import {ServiceStopDiagram} from './ServiceStopDiagram';
afterEach(cleanup);
const rows=[{key:'start',name:'起点',stopping:true,fixed:true},{key:'1:x',name:'中間',stopping:true},{key:'2:x',name:'中間',stopping:false},{key:'3:end',name:'終点',stopping:true,fixed:true}];
test('vertical diagram distinguishes stops and passing visits without changing occurrence identity',()=>{
 const change=vi.fn();const view=render(<ServiceStopDiagram label="快速の停車駅" rows={rows} controlLabel={i=>`停車 ${i+1}`} onChange={change}/>);
 expect(view.container.querySelectorAll('.stopping')).toHaveLength(3);
 expect(view.container.querySelectorAll('.passing')).toHaveLength(1);
 expect((screen.getByLabelText('停車 1') as HTMLInputElement).disabled).toBe(true);
 fireEvent.click(screen.getByLabelText('停車 3'));expect(change).toHaveBeenCalledWith('2:x',true);
 fireEvent.click(screen.getByLabelText('停車 2'));expect(change).toHaveBeenLastCalledWith('1:x',false);
 expect(within(screen.getByRole('list')).getAllByRole('listitem')).toHaveLength(4);
});
test('two service diagrams on the same path have independent stop patterns',()=>{
 const local=vi.fn(),rapid=vi.fn();render(<><ServiceStopDiagram label="普通" rows={rows.map(r=>({...r,stopping:true}))} controlLabel={i=>`普通 ${i}`} onChange={local}/><ServiceStopDiagram label="快速" rows={rows} controlLabel={i=>`快速 ${i}`} onChange={rapid}/></>);
 fireEvent.click(screen.getByLabelText('快速 2'));expect(rapid).toHaveBeenCalledWith('2:x',true);expect(local).not.toHaveBeenCalled();
 expect((screen.getByLabelText('普通 2') as HTMLInputElement).checked).toBe(true);
});
test('unselected stop policy stays explicit and membership detail is available on hover',()=>{
 render(<ServiceStopDiagram label="候補" rows={[{key:'a',name:'途中',stopping:false,undecided:true,detail:'路線所属外'}]} controlLabel={()=>'停車候補'} disabled onChange={vi.fn()}/>);
 expect(screen.getByText('未選択')).toBeTruthy();expect(screen.getByTitle('路線所属外')).toBeTruthy();
 expect((screen.getByLabelText('停車候補') as HTMLInputElement).disabled).toBe(true);
});
