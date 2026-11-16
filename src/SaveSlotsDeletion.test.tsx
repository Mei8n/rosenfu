// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen,within,waitFor} from '@testing-library/react';
import {SaveSlots} from './SaveSlots';
afterEach(cleanup);
test('deletion requires explicit confirmation, retains failed dialog and supports restore',async()=>{
 const onDelete=vi.fn().mockRejectedValueOnce(Error('disk busy')).mockResolvedValue(undefined),onRestore=vi.fn().mockResolvedValue(undefined);
 const props={slots:[{id:'one',name:'私の世界',loadable:true}],deletedSlots:[{id:'two',name:'以前の世界',loadable:true}],current:'one',loading:false,error:'',busy:false,canSave:true,onRefresh:vi.fn(),onLoad:vi.fn(),onSaveAs:vi.fn(),onNew:vi.fn(),onDelete,onRestore};
 render(<SaveSlots {...props}/>);
 fireEvent.click(screen.getByRole('button',{name:'私の世界を削除'}));expect(onDelete).not.toHaveBeenCalled();
 const dialog=within(screen.getByRole('dialog'));expect(dialog.getByText(/現在の編集中の内容は画面に残り/)).toBeTruthy();
 fireEvent.click(dialog.getByRole('button',{name:'キャンセル'}));expect(onDelete).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole('button',{name:'私の世界を削除'}));fireEvent.click(screen.getByRole('button',{name:'削除する'}));
 await screen.findByText('disk busy');expect(screen.getByRole('dialog')).toBeTruthy();
 fireEvent.click(screen.getByRole('button',{name:'削除する'}));await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());expect(onDelete).toHaveBeenLastCalledWith('one');
 fireEvent.click(screen.getByText(/削除済みデータ/));fireEvent.click(screen.getByRole('button',{name:'復元'}));await waitFor(()=>expect(onRestore).toHaveBeenCalledWith('two'));
});
