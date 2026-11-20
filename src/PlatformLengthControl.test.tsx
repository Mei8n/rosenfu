// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {PlatformLengthInput} from './PlatformLengthInput';
afterEach(cleanup);
test('only car-equivalent input is shown and stored fractional metres round-trip',()=>{
 const change=vi.fn();const {rerender}=render(<PlatformLengthInput id="length" label="駅の長さ" value="231.5" onChange={change}/>);
 const input=screen.getByRole('spinbutton') as HTMLInputElement;expect(input.value).toBe('11.575');expect(screen.getAllByRole('spinbutton')).toHaveLength(1);
 fireEvent.change(input,{target:{value:'12.125'}});expect(change).toHaveBeenLastCalledWith('242.5');
 rerender(<PlatformLengthInput id="length" label="駅の長さ" value="45" onChange={change}/>);expect(input.value).toBe('2.25');
});
test('new editing limits are 2–20 equivalents, including fractional boundary validation',()=>{
 const change=vi.fn();render(<PlatformLengthInput id="length" label="ホーム長" value="200" onChange={change}/>);const input=screen.getByRole('spinbutton') as HTMLInputElement;
 for(const [value,metres,valid] of [['2','40',true],['20','400',true],['1.999','39.98',false],['20.001','400.02',false],['','',false]] as const){fireEvent.change(input,{target:{value}});expect(change).toHaveBeenLastCalledWith(metres);expect(input.checkValidity()).toBe(valid);}
});
test('compact controls step within limits and preserve legacy values until edited',()=>{
 const change=vi.fn();const {rerender}=render(<PlatformLengthInput id="length" label="駅の長さ" value="20" onChange={change} compact/>);
 expect((screen.getByRole('spinbutton') as HTMLInputElement).value).toBe('1');expect(change).not.toHaveBeenCalled();
 expect((screen.getByRole('button',{name:'駅を1両短くする'}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.click(screen.getByRole('button',{name:'駅を1両長くする'}));expect(change).toHaveBeenLastCalledWith('40');
 rerender(<PlatformLengthInput id="length" label="駅の長さ" value="1000" onChange={change} compact/>);
 expect((screen.getByRole('spinbutton') as HTMLInputElement).value).toBe('50');
 expect((screen.getByRole('button',{name:'駅を1両長くする'}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.click(screen.getByRole('button',{name:'駅を1両短くする'}));expect(change).toHaveBeenLastCalledWith('400');
});
