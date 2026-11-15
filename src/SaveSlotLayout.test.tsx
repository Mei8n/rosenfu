// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,render,screen,within} from '@testing-library/react';
import {SaveSlots} from './SaveSlots';
// @ts-expect-error Node fs is test-only.
import {readFileSync} from 'node:fs';
const css=readFileSync('src/SaveSlots.css','utf8');
afterEach(()=>{cleanup();document.head.querySelectorAll('style[data-save-layout]').forEach(el=>el.remove());});
test('all save actions share a wrapping spaced row, cards show only available saved data',()=>{
 const style=document.createElement('style');style.dataset.saveLayout='true';style.textContent=css;document.head.append(style);
 const actions={onSave:vi.fn(),onSaveAs:vi.fn(),onNew:vi.fn(),onRefresh:vi.fn(),onLoad:vi.fn()};
 const {container}=render(<SaveSlots {...actions} slots={[{id:'one',name:'第一ワールド',loadable:true,updated_at:1700000000},{id:'old',name:'旧保存',loadable:false,error:'非対応の保存形式'}]} current="one" busy={false} canSave loading={false} error=""/>);
 const row=container.querySelector<HTMLElement>('.save-slot-actions')!;
 expect(within(row).getAllByRole('button')).toHaveLength(4);
 expect(within(row).getByRole('button',{name:'ワールドを保存'})).toBeTruthy();
 expect(within(row).getByRole('button',{name:'別のセーブとして保存'})).toBeTruthy();
 expect(getComputedStyle(row).display).toBe('flex');expect(getComputedStyle(row).flexWrap).toBe('wrap');expect(getComputedStyle(row).gap).toBe('8px');
 const cards=container.querySelectorAll('.save-slot-card');expect(cards).toHaveLength(2);
 expect(cards[0].classList.contains('is-current')).toBe(true);expect(cards[1].classList.contains('is-current')).toBe(false);
 expect(cards[0].querySelector('time')?.dateTime).toBe('2023-11-14T22:13:20.000Z');expect(cards[1].querySelector('time')).toBeNull();
 expect((screen.getByRole('button',{name:'旧保存'}) as HTMLButtonElement).disabled).toBe(true);
 expect(screen.queryByText(/乗客|収支|GB|プレビュー/)).toBeNull();
});

test('planned station name shrinks within its column while delete remains on the same row',()=>{
 const style=document.createElement('style');style.dataset.saveLayout='true';style.textContent=readFileSync('src/PlanBreakdown.css','utf8');document.head.append(style);
 const {container}=render(<main className="workbench"><div className="plan-breakdown" style={{width:240}}><ul><li className="planned-station-row"><input aria-label="計画駅名" defaultValue="長い駅名"/><span role="status">線路に収まらないため着工不可</span><button aria-label="計画駅を削除">×</button></li></ul></div></main>);
 const row=container.querySelector('.planned-station-row')!;
 expect(getComputedStyle(row).display).toBe('grid');expect(getComputedStyle(row).gridTemplateColumns).toBe('minmax(0, 1fr) auto');
 const input=screen.getByRole('textbox',{name:'計画駅名'}),button=screen.getByRole('button',{name:'計画駅を削除'});
 expect(getComputedStyle(input).minWidth).toBe('0');expect(getComputedStyle(input).gridRow).toBe('1');expect(getComputedStyle(button).gridRow).toBe('1');
 expect(getComputedStyle(screen.getByRole('status')).gridRow).toBe('2');
 expect(getComputedStyle(button).color).toBe('rgb(181, 49, 49)');expect(getComputedStyle(button).backgroundColor).toBe('rgba(0, 0, 0, 0)');expect(getComputedStyle(button).minWidth).toBe('32px');
});
