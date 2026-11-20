// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {SaveSlots} from './SaveSlots';
afterEach(cleanup);
test('renaming requires explicit submit and preserves failed input for retry',async()=>{
 const onRename=vi.fn().mockRejectedValueOnce(Error('保存先が変更されています')).mockResolvedValue(undefined);
 render(<SaveSlots slots={[{id:'slot-abcdef',name:'元の名前',loadable:true},{id:'legacy',name:'以前のワールド',loadable:false}]} current="slot-abcdef" loading={false} error="" busy={false} canSave onRefresh={()=>{}} onLoad={()=>{}} onSaveAs={()=>{}} onNew={()=>{}} onRename={onRename} onDelete={vi.fn()}/>);
 expect(screen.queryByRole('button',{name:'以前のワールドの名前を変更'})).toBeNull();
 fireEvent.click(screen.getByRole('button',{name:'元の名前の名前を変更'}));
 const input=screen.getByRole('textbox',{name:'セーブデータ名'});
 expect(input.getAttribute('autocomplete')).toBe('off');
 expect(document.activeElement).toBe(input);
 fireEvent.click(screen.getByRole('button',{name:'元の名前を削除'}));
 expect(screen.getAllByRole('dialog')).toHaveLength(1);
 expect(screen.queryByRole('dialog',{name:'セーブデータの削除'})).toBeNull();
 fireEvent.change(input,{target:{value:'  東京 🚆  '}});
 expect(onRename).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole('button',{name:'変更する'}));
 await screen.findByRole('alert');expect((input as HTMLInputElement).value).toBe('  東京 🚆  ');
 expect(onRename).toHaveBeenLastCalledWith('slot-abcdef','元の名前','東京 🚆');
 fireEvent.click(screen.getByRole('button',{name:'変更する'}));
 await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());
 fireEvent.click(screen.getByRole('button',{name:'元の名前の名前を変更'}));
 fireEvent.change(screen.getByRole('textbox'),{target:{value:' '}});
 expect((screen.getByRole('button',{name:'変更する'}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.click(screen.getByRole('button',{name:'キャンセル'}));expect(onRename).toHaveBeenCalledTimes(2);
});
test('pending rename disables cancellation and duplicate submission',async()=>{
 let finish!:()=>void;const onRename=vi.fn(()=>new Promise<void>(resolve=>{finish=resolve;}));
 render(<SaveSlots slots={[{id:'slot-abcdef',name:'Before',loadable:true}]} current={null} loading={false} error="" busy={false} canSave onRefresh={()=>{}} onLoad={()=>{}} onSaveAs={()=>{}} onNew={()=>{}} onRename={onRename} onDelete={vi.fn()}/>);
 fireEvent.click(screen.getByRole('button',{name:'Beforeの名前を変更'}));
 fireEvent.change(screen.getByRole('textbox'),{target:{value:'After'}});
 fireEvent.click(screen.getByRole('button',{name:'変更する'}));
 expect((screen.getByRole('button',{name:'キャンセル'}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.click(screen.getByRole('button',{name:'変更する'}));expect(onRename).toHaveBeenCalledTimes(1);
 finish();await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());
});
