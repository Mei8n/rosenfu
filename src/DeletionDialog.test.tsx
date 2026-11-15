// @vitest-environment jsdom
import {afterEach,beforeEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {DeletionDialog} from './DeletionDialog';
beforeEach(()=>{
 HTMLDialogElement.prototype.showModal=function(){this.open=true;};
 HTMLDialogElement.prototype.close=function(){this.open=false;};
});
afterEach(cleanup);
const props={label:'区間',busy:false,blocked:'',error:'',onCancel:vi.fn(),onDelete:vi.fn()};
test('section confirmation focuses delete and Enter invokes it once without map shortcuts',()=>{
 const onDelete=vi.fn(),key=vi.fn();
 render(<DeletionDialog {...props} defaultDelete onDelete={onDelete}/>);
 const button=screen.getByText('削除する');
 expect(document.activeElement).toBe(button);
 window.addEventListener('keydown',key);
 fireEvent.keyDown(button,{key:'Enter'});
 fireEvent.keyDown(button,{key:'Enter',repeat:true});
 fireEvent.keyDown(button,{key:'Enter',isComposing:true});
 fireEvent.keyDown(button,{key:'Enter',keyCode:229});
 expect(onDelete).toHaveBeenCalledOnce();expect(key).not.toHaveBeenCalled();
 window.removeEventListener('keydown',key);
});
test('Escape remains cancellation and prior focus is restored',()=>{
 const prior=document.createElement('button');document.body.append(prior);prior.focus();
 const onCancel=vi.fn(),onDelete=vi.fn();
 const {unmount}=render(<DeletionDialog {...props} defaultDelete onCancel={onCancel} onDelete={onDelete}/>);
 fireEvent(screen.getByRole('dialog'),new Event('cancel',{cancelable:true}));
 expect(onCancel).toHaveBeenCalledOnce();expect(onDelete).not.toHaveBeenCalled();
 unmount();expect(document.activeElement).toBe(prior);prior.remove();
});
test('other targets and blocked sections retain cancel focus; busy blocks Enter and Escape',()=>{
 const onDelete=vi.fn(),onCancel=vi.fn();
  render(<DeletionDialog {...props} onDelete={onDelete}/>);
 expect(document.activeElement).toBe(screen.getByText('キャンセル'));
 cleanup();
 render(<DeletionDialog {...props} defaultDelete blocked="使用中" onDelete={onDelete}/>);
 expect(document.activeElement).toBe(screen.getByText('キャンセル'));
 fireEvent.keyDown(screen.getByText('削除する'),{key:'Enter'});
 expect(onDelete).not.toHaveBeenCalled();
 cleanup();
 const view=render(<DeletionDialog {...props} defaultDelete onDelete={onDelete} onCancel={onCancel}/>);
 view.rerender(<DeletionDialog {...props} defaultDelete busy onDelete={onDelete} onCancel={onCancel}/>);
 fireEvent.keyDown(screen.getByText('削除する'),{key:'Enter'});
 fireEvent(screen.getByRole('dialog'),new Event('cancel',{cancelable:true}));
 expect(onDelete).not.toHaveBeenCalled();expect(onCancel).not.toHaveBeenCalled();
});
