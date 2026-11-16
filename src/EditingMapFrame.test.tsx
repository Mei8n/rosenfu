// @vitest-environment jsdom
import { afterEach, expect, test } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import css from './EditingMapFrame.css?raw';
import { EditingMapFrame, editingMapMode } from './EditingMapFrame';
const base = { enabled: true, tab: 'construction', tool: 'tracks', mapMode: 'draw' as const, pickingStations: false };
afterEach(cleanup);
test('only active map editing interactions get an edge', () => {
  expect(editingMapMode(base)?.key).toBe('track');
  expect(editingMapMode({...base, tool:'stations', mapMode:'place'})?.key).toBe('station');
  expect(editingMapMode({...base, tool:'delete', mapMode:'move'})?.key).toBe('delete');
  expect(editingMapMode({...base, tool:'select', mapMode:'move'})?.key).toBe('select');
  for (const tab of ['routes','services']) expect(editingMapMode({...base,tab,pickingStations:true})?.key).toBe(tab==='routes'?'route':'service');
  for (const change of [{enabled:false},{mapMode:'move' as const},{tool:'inactive'},{tab:'save'},{tab:'routes'},{tab:'stations'}]) expect(editingMapMode({...base,...change})).toBeNull();
});
test('switch, exit and re-enter update the same non-focusable live region', () => {
  const {container,rerender}=render(<EditingMapFrame {...base}/>);
  const status=screen.getByRole('status');
  expect(status.textContent).toBe('線路敷設モード');
  rerender(<EditingMapFrame {...base} tool="delete"/>);
  expect(status.textContent).toBe('撤去モード');
  rerender(<EditingMapFrame {...base} mapMode="move"/>);
  expect(container.querySelector('[data-mode]')).toBeNull();
  expect(status.textContent).toBe('');
  rerender(<EditingMapFrame {...base}/>);
  expect(status.textContent).toBe('線路敷設モード');
  expect(container.querySelector('[tabindex],button')).toBeNull();
});
test('frame never captures pointer input, animates, or changes map layout', () => {
  expect(css).toContain('pointer-events: none');
  expect(css).toContain('position: absolute');
  expect(css).toContain('forced-colors: active');
  expect(css).not.toMatch(/\b(animation|transition|filter)\s*:/);
});
