// @vitest-environment jsdom
import {act,cleanup,fireEvent,renderHook} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {useKeyboardPan} from './useKeyboardPan';
afterEach(()=>{cleanup();vi.useRealTimers();document.body.innerHTML='';});
function setup(){vi.useFakeTimers();const pan=vi.fn();let blocked=false;const hook=renderHook(()=>useKeyboardPan(pan,()=>blocked));return {pan,block:()=>{blocked=true;},unmount:hook.unmount};}
const advance=(ms=64)=>act(()=>vi.advanceTimersByTime(ms));
test('holding moves every frame without OS repeats, release stops, diagonal speed is normalized',()=>{
 const {pan}=setup();fireEvent.keyDown(window,{key:'ArrowRight'});advance();
 expect(pan.mock.calls.length).toBeGreaterThan(1);expect(pan.mock.calls.every(([x,y])=>x>0&&y===0)).toBe(true);
 const before=pan.mock.calls.length;advance();expect(pan.mock.calls.length).toBeGreaterThan(before);
 fireEvent.keyDown(window,{key:'ArrowDown'});advance();const [x,y]=pan.mock.calls.at(-1)!;expect(x).toBe(y);expect(Math.hypot(x,y)).toBeCloseTo(8);
 fireEvent.keyUp(window,{key:'ArrowRight'});advance();expect(pan.mock.calls.at(-1)![0]).toBe(0);
 fireEvent.keyUp(window,{key:'ArrowDown'});pan.mockClear();advance();expect(pan).not.toHaveBeenCalled();
});
test('repeat events do not multiply speed and opposing arrows cancel movement',()=>{
 const {pan}=setup();fireEvent.keyDown(window,{key:'ArrowRight'});advance();pan.mockClear();
 for(let i=0;i<20;i++)fireEvent.keyDown(window,{key:'ArrowRight',repeat:true});advance();
 expect(pan.mock.calls).toHaveLength(4);expect(pan.mock.calls.every(([x])=>x===8)).toBe(true);
 fireEvent.keyDown(window,{key:'ArrowLeft'});pan.mockClear();advance();expect(pan).not.toHaveBeenCalled();
});
test('typing, modifiers, handled arrows and modal dialogs do not pan or prevent native keys',()=>{
 const {pan}=setup();
 for(const tag of ['input','textarea','select','button']){const target=document.createElement(tag);document.body.append(target);const e=new KeyboardEvent('keydown',{key:'ArrowUp',bubbles:true,cancelable:true});fireEvent(target,e);expect(e.defaultPrevented).toBe(false);target.remove();}
 for(const modifier of ['ctrlKey','metaKey','altKey','shiftKey','isComposing'])fireEvent.keyDown(window,{key:'ArrowUp',[modifier]:true});
 const event=new KeyboardEvent('keydown',{key:'ArrowRight',cancelable:true});event.preventDefault();fireEvent(window,event);
 const dialog=document.createElement('div');dialog.setAttribute('role','dialog');document.body.append(dialog);fireEvent.keyDown(window,{key:'ArrowUp'});advance();expect(pan).not.toHaveBeenCalled();
});
test.each(['blur','pointerdown','wheel','zoom','focus','blocked','unmount'])('movement stops on %s without waiting for keyup',reason=>{
 const {pan,block,unmount}=setup();fireEvent.keyDown(window,{key:'ArrowLeft'});advance();expect(pan).toHaveBeenCalled();pan.mockClear();
 if(reason==='focus'){const input=document.createElement('input');document.body.append(input);input.focus();}
 else if(reason==='zoom')fireEvent.keyDown(window,{key:'+'});else if(reason==='blocked')block();else if(reason==='unmount')unmount();else fireEvent(window,new Event(reason));
 advance();expect(pan).not.toHaveBeenCalled();
});

test.each(['wheel','zoom'])('OS repeats cannot resume panning after %s interruption',reason=>{
 const {pan}=setup();fireEvent.keyDown(window,{key:'ArrowRight'});advance();
 if(reason==='wheel')fireEvent.wheel(window,{deltaY:-100});else fireEvent.keyDown(window,{key:'+'});
 pan.mockClear();fireEvent.keyDown(window,{key:'ArrowRight',repeat:true});advance();expect(pan).not.toHaveBeenCalled();
 fireEvent.keyUp(window,{key:'ArrowRight'});fireEvent.keyDown(window,{key:'ArrowRight'});advance();expect(pan).toHaveBeenCalled();
});
