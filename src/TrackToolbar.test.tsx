// @vitest-environment jsdom
import {describe,it,expect,vi} from 'vitest';
import {render,fireEvent,cleanup} from '@testing-library/react';
import {TrackTypeControls} from './TrackTypeControls';
import app from './App.tsx?raw';
describe('compact track toolbar',()=>{
 it('offers three single label choices and preserves selection',()=>{
  const change=vi.fn();const view=render(<TrackTypeControls value="normal" disabled={false} onChange={change}/>);
  expect(view.getAllByRole('button').map(b=>b.textContent)).toEqual(['通常','高速','トラム']);
  fireEvent.click(view.getByText('トラム'));expect(change).toHaveBeenCalledWith('tram');cleanup();
 });
 it('labels only the speed input, not the minus button',()=>{
  expect(app).toContain('<label htmlFor="track-target-speed">');
  expect(app).not.toContain('<label className="target-speed-control">');
  expect(app).toContain('<input id="track-target-speed"');
 });
 it('road follow blocking applies to every type; switching types selects defaults',()=>{
  expect(app).toContain('const roadFollowingBlocked=roadFollowing;');
  expect(app).toContain('setRoadFollowing(value==="tram")');
 });
});
