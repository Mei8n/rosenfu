// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen} from '@testing-library/react';
import {ErrorNotice,ERROR_NOTICE_MS} from './ErrorNotice';
afterEach(()=>{cleanup();vi.useRealTimers();});
test('error expires after five seconds, not before',()=>{vi.useFakeTimers();const dismiss=vi.fn();render(<ErrorNotice message="失敗" revision={1} onDismiss={dismiss}/>);act(()=>vi.advanceTimersByTime(ERROR_NOTICE_MS-1));expect(dismiss).not.toHaveBeenCalled();act(()=>vi.advanceTimersByTime(1));expect(dismiss).toHaveBeenCalledOnce();});
test('a new or repeated notification restarts the timer and old timer cannot dismiss it',()=>{vi.useFakeTimers();const dismiss=vi.fn();const {rerender}=render(<ErrorNotice message="失敗" revision={1} onDismiss={dismiss}/>);act(()=>vi.advanceTimersByTime(4000));rerender(<ErrorNotice message="別の失敗" revision={2} onDismiss={dismiss}/>);act(()=>vi.advanceTimersByTime(4000));expect(dismiss).not.toHaveBeenCalled();rerender(<ErrorNotice message="別の失敗" revision={3} onDismiss={dismiss}/>);act(()=>vi.advanceTimersByTime(4999));expect(dismiss).not.toHaveBeenCalled();act(()=>vi.advanceTimersByTime(1));expect(dismiss).toHaveBeenCalledOnce();});
test('cross dismisses immediately and unmount cleans the pending timer',()=>{vi.useFakeTimers();const dismiss=vi.fn();const {unmount}=render(<ErrorNotice message="失敗" revision={1} onDismiss={dismiss}/>);fireEvent.click(screen.getByRole('button',{name:'エラー通知を閉じる'}));expect(dismiss).toHaveBeenCalledOnce();unmount();act(()=>vi.advanceTimersByTime(10000));expect(dismiss).toHaveBeenCalledOnce();});
