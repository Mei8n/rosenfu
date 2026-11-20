// @vitest-environment jsdom
import {afterEach,expect,test} from 'vitest';
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {JunctionPoints} from './JunctionPoints';
import {DEFAULT_CAMERA} from './map';
afterEach(cleanup);
test('only explicit three-port joins display a selectable point, not crossings or ordinary joins',()=>{
 const point={latitude:35.67504,longitude:139.75},lines=['1','2','3'].map(id=>({id,points:[point,{...point,longitude:139.76}]}));
 const a={alignment_id:'1',endpoint:'start' as const},b={alignment_id:'2',endpoint:'start' as const},c={alignment_id:'3',endpoint:'start' as const};
 const {rerender}=render(<svg><JunctionPoints lines={lines} connections={[]} camera={DEFAULT_CAMERA} selectable/></svg>);expect(screen.queryByRole('button')).toBeNull();
 rerender(<svg><JunctionPoints lines={lines} connections={[{a,b}]} camera={DEFAULT_CAMERA} selectable/></svg>);expect(screen.queryByRole('button')).toBeNull();
 rerender(<svg><JunctionPoints lines={lines} connections={[{a,b},{a,b:c}]} camera={DEFAULT_CAMERA} selectable/></svg>);const button=screen.getByRole('button',{name:'分岐ポイント'});fireEvent.keyDown(button,{key:'Enter'});expect(button.getAttribute('aria-pressed')).toBe('true');
});
