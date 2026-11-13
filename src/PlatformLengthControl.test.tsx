// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {PlatformLengthInput} from './PlatformLengthInput';
afterEach(cleanup);
test('only car-equivalent input is shown and stored fractional metres round-trip',()=>{
 const change=vi.fn();const {rerender}=render(<PlatformLengthInput id="length" label="ホーム長 (20m級・両相当)" value="231.5" onChange={change}/>);
 const input=screen.getByRole('spinbutton') as HTMLInputElement;expect(input.value).toBe('11.575');expect(screen.getAllByRole('spinbutton')).toHaveLength(1);
 fireEvent.change(input,{target:{value:'12.125'}});expect(change).toHaveBeenLastCalledWith('242.5');
 rerender(<PlatformLengthInput id="length" label="ホーム長 (20m級・両相当)" value="45" onChange={change}/>);expect(input.value).toBe('2.25');
});
test('existing metre limits remain 1–50 equivalents, including fractional boundary validation',()=>{
 const change=vi.fn();render(<PlatformLengthInput id="length" label="ホーム長" value="200" onChange={change}/>);const input=screen.getByRole('spinbutton') as HTMLInputElement;
 for(const [value,metres,valid] of [['1','20',true],['50','1000',true],['0.999','19.98',false],['50.001','1000.02',false],['','',false]] as const){fireEvent.change(input,{target:{value}});expect(change).toHaveBeenLastCalledWith(metres);expect(input.checkValidity()).toBe(valid);}
});
