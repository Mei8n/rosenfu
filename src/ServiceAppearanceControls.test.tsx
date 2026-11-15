// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {ServiceAppearanceControls} from './ServiceAppearanceControls';
afterEach(cleanup);
const props=()=>({color:null as string|null,enabled:true,onDirty:vi.fn(),onSave:vi.fn().mockResolvedValue(undefined),onDuplicate:vi.fn().mockResolvedValue(undefined)});
test('unset colour remains unset until explicit apply and uses compare-and-set value',async()=>{
 const p=props();render(<ServiceAppearanceControls {...p}/>);expect(p.onSave).not.toHaveBeenCalled();expect(p.onDirty).toHaveBeenLastCalledWith(false);
 fireEvent.click(screen.getByRole('button',{name:'色を設定'}));await waitFor(()=>expect(p.onSave).toHaveBeenCalledWith('#3b82f6',null));
});
test('failure retains colour draft and blocks duplication until applied or discarded',async()=>{
 const p={...props(),color:'#112233',onSave:vi.fn().mockRejectedValue(Error('save rejected'))};render(<ServiceAppearanceControls {...p}/>);
 fireEvent.change(screen.getByLabelText('系統色'),{target:{value:'#aabbcc'}});expect((screen.getByRole('button',{name:'系統を複製'}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.click(screen.getByRole('button',{name:'色を適用'}));await screen.findByText(/save rejected/);expect((screen.getByLabelText('系統色') as HTMLInputElement).value).toBe('#aabbcc');
 fireEvent.click(screen.getByRole('button',{name:'色の変更を取り消す'}));expect((screen.getByLabelText('系統色') as HTMLInputElement).value).toBe('#112233');
 fireEvent.click(screen.getByRole('button',{name:'系統を複製'}));await waitFor(()=>expect(p.onDuplicate).toHaveBeenCalledOnce());
});
test('saved matching colour acknowledges own edit, while external different colour preserves conflict',async()=>{
 const p={...props(),color:'#112233'};const v=render(<ServiceAppearanceControls {...p}/>);fireEvent.change(screen.getByLabelText('系統色'),{target:{value:'#aabbcc'}});
 v.rerender(<ServiceAppearanceControls {...p} color="#AABBCC"/>);expect(screen.queryByRole('alert')).toBeNull();expect((screen.getByRole('button',{name:'色を適用'}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.change(screen.getByLabelText('系統色'),{target:{value:'#ff0000'}});v.rerender(<ServiceAppearanceControls {...p} color="#00ff00"/>);
 expect(screen.getByRole('alert').textContent).toContain('保存済みの色が変更');expect((screen.getByLabelText('系統色') as HTMLInputElement).value).toBe('#ff0000');
 fireEvent.click(screen.getByRole('button',{name:'色の変更を取り消す'}));expect((screen.getByLabelText('系統色') as HTMLInputElement).value).toBe('#00ff00');
});
test('pending duplication cannot be repeated or change colour and errors remain visible',async()=>{
 let resolve!:()=>void;const p={...props(),onDuplicate:vi.fn(()=>new Promise<void>(r=>{resolve=r;}))};render(<ServiceAppearanceControls {...p}/>);
 fireEvent.click(screen.getByRole('button',{name:'系統を複製'}));fireEvent.click(screen.getByRole('button',{name:'系統を複製'}));expect(p.onDuplicate).toHaveBeenCalledOnce();
 expect((screen.getByLabelText('系統色') as HTMLInputElement).closest('fieldset')?.disabled).toBe(true);await act(async()=>resolve());
});

test('comparison ignores hex case but compare-and-set retains raw persisted colour',async()=>{
 const p={...props(),color:'#Ab12eF'};const v=render(<ServiceAppearanceControls {...p}/>);
 expect((screen.getByLabelText('系統色') as HTMLInputElement).value).toBe('#ab12ef');
 expect(p.onDirty).toHaveBeenLastCalledWith(false);
 fireEvent.change(screen.getByLabelText('系統色'),{target:{value:'#abcdef'}});
 fireEvent.click(screen.getByRole('button',{name:'色を適用'}));
 await waitFor(()=>expect(p.onSave).toHaveBeenCalledWith('#abcdef','#Ab12eF'));
 v.rerender(<ServiceAppearanceControls {...p} color="#ABCDEF"/>);
 expect(p.onDirty).toHaveBeenLastCalledWith(false);
 expect(screen.queryByRole('alert')).toBeNull();
 expect((screen.getByRole('button',{name:'系統を複製'}) as HTMLButtonElement).disabled).toBe(false);
});

test('reference swatches, picker and HEX share one draft and apply by Enter',async()=>{
 const p={...props(),color:'#112233'};const view=render(<ServiceAppearanceControls {...p}/>);
 expect(screen.getAllByRole('button',{name:/系統色候補/})).toHaveLength(16);
 fireEvent.click(screen.getByRole('button',{name:'系統色候補 青 #1f6fd1'}));
 expect((screen.getByLabelText('系統色 HEX') as HTMLInputElement).value).toBe('#1f6fd1');
 expect((screen.getByLabelText('系統色') as HTMLInputElement).value).toBe('#1f6fd1');
 fireEvent.change(screen.getByLabelText('系統色 HEX'),{target:{value:'#AB12EF'}});
 expect((screen.getByLabelText('系統色') as HTMLInputElement).value).toBe('#ab12ef');
 fireEvent.keyDown(screen.getByLabelText('系統色 HEX'),{key:'Enter'});
 await waitFor(()=>expect(p.onSave).toHaveBeenCalledWith('#ab12ef','#112233'));
 view.rerender(<ServiceAppearanceControls {...p} color="#AB12EF"/>);
 expect(screen.queryByRole('alert')).toBeNull();
 expect((screen.getByRole('button',{name:'系統を複製'}) as HTMLButtonElement).disabled).toBe(false);
 fireEvent.click(screen.getByRole('button',{name:'系統を複製'}));
 await waitFor(()=>expect(p.onDuplicate).toHaveBeenCalledOnce());
});

test('invalid HEX cannot apply and Escape restores persisted picker and text',()=>{
 const p={...props(),color:'#112233'};render(<ServiceAppearanceControls {...p}/>);
 fireEvent.change(screen.getByLabelText('系統色 HEX'),{target:{value:'#12xx'}});
 expect(screen.getByLabelText('系統色 HEX').getAttribute('aria-invalid')).toBe('true');
 expect((screen.getByRole('button',{name:'色を適用'}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.keyDown(screen.getByLabelText('系統色 HEX'),{key:'Enter'});expect(p.onSave).not.toHaveBeenCalled();
 expect((screen.getByRole('button',{name:'系統を複製'}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.keyDown(screen.getByLabelText('系統色 HEX'),{key:'Escape'});
 expect((screen.getByLabelText('系統色 HEX') as HTMLInputElement).value).toBe('#112233');
 expect((screen.getByLabelText('系統色') as HTMLInputElement).value).toBe('#112233');
 fireEvent.change(screen.getByLabelText('系統色'),{target:{value:'#d7263d'}});
 expect((screen.getByLabelText('系統色 HEX') as HTMLInputElement).value).toBe('#d7263d');
 expect(screen.getByRole('button',{name:'系統色候補 赤 #d7263d'}).getAttribute('aria-pressed')).toBe('true');
});

test('HEX draft survives external changes and conflict prevents keyboard apply',()=>{
 const p={...props(),color:'#112233'};const view=render(<ServiceAppearanceControls {...p}/>);
 fireEvent.change(screen.getByLabelText('系統色 HEX'),{target:{value:'#aabbcc'}});
 view.rerender(<ServiceAppearanceControls {...p} color="#334455"/>);
 expect(screen.getByRole('alert').textContent).toContain('保存済みの色');
 fireEvent.keyDown(screen.getByLabelText('系統色 HEX'),{key:'Enter'});expect(p.onSave).not.toHaveBeenCalled();
 fireEvent.keyDown(screen.getByLabelText('系統色 HEX'),{key:'Escape'});
 expect((screen.getByLabelText('系統色 HEX') as HTMLInputElement).value).toBe('#334455');
});
