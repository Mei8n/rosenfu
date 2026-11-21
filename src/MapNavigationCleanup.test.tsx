// @vitest-environment jsdom
import React from 'react';
import {act,cleanup,fireEvent,render,screen} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>false,invoke:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.useRealTimers();});
const scene=()=>document.querySelector('[data-sample-scene]')!.getAttribute('patternTransform');
test('map settings omit navigation and coordinates but retain display settings; held arrows pan until released',()=>{
 vi.useFakeTimers();render(<App/>);
 for(const name of ['西へ移動','北へ移動','南へ移動','東へ移動','拡大','縮小','東京へ戻る','座標へ移動'])expect(screen.queryByRole('button',{name,hidden:true})).toBeNull();
 expect(screen.queryByLabelText('中心緯度')).toBeNull();expect(screen.queryByLabelText('中心経度')).toBeNull();
 expect(screen.getByLabelText('地図コントラスト')).toBeTruthy();expect(screen.getByLabelText('線の太さ')).toBeTruthy();
 const before=scene();fireEvent.keyDown(window,{key:'ArrowRight'});act(()=>vi.advanceTimersByTime(64));expect(scene()).not.toBe(before);
 fireEvent.keyUp(window,{key:'ArrowRight'});const after=scene();act(()=>vi.advanceTimersByTime(64));expect(scene()).toBe(after);
});
