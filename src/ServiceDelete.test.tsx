// @vitest-environment jsdom
import { afterEach, expect, test, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { ServiceDelete } from './ServiceDelete';
afterEach(cleanup);
test('explicit confirmation cancels without deletion, then deletes only once',async()=>{
 let finish:()=>void=()=>{};const remove=vi.fn(()=>new Promise<void>(resolve=>finish=resolve));
 render(<ServiceDelete name="快速" enabled hasEdits={false} onDelete={remove}/>);
 fireEvent.click(screen.getByText('系統を削除'));expect(remove).not.toHaveBeenCalled();
 fireEvent.click(screen.getByText('キャンセル'));expect(screen.queryByRole('dialog')).toBeNull();
 fireEvent.click(screen.getByText('系統を削除'));fireEvent.click(screen.getByText('削除する'));fireEvent.click(screen.getByText('削除する'));
 expect(remove).toHaveBeenCalledTimes(1);finish();await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());
});
test('unapplied changes and read-only/busy state block deletion',()=>{
 const remove=vi.fn();const view=render(<ServiceDelete name="快速" enabled hasEdits onDelete={remove}/>);
 expect(screen.getByText('系統を削除')).toHaveProperty('disabled',true);
 view.rerender(<ServiceDelete name="快速" enabled={false} hasEdits={false} onDelete={remove}/>);
 expect(screen.getByText('系統を削除')).toHaveProperty('disabled',true);expect(remove).not.toHaveBeenCalled();
});
test('failed request keeps confirmation and supports retry',async()=>{
 const remove=vi.fn().mockRejectedValueOnce(Error('変更されています')).mockResolvedValue(undefined);
 render(<ServiceDelete name="快速" enabled hasEdits={false} onDelete={remove}/>);
 fireEvent.click(screen.getByText('系統を削除'));fireEvent.click(screen.getByText('削除する'));
 await screen.findByRole('alert');expect(screen.getByRole('dialog')).toBeTruthy();
 fireEvent.click(screen.getByText('削除する'));await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());expect(remove).toHaveBeenCalledTimes(2);
});
