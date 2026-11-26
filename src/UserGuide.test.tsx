// @vitest-environment jsdom
import React from 'react';
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {afterEach,expect,test} from 'vitest';
import {UserGuide} from './UserGuide';
afterEach(cleanup);
test('offline guide has discoverable expandable topics without hiding operational warnings',()=>{
 const {container}=render(<UserGuide/>);
 const guide=container.querySelector('details')!;
 expect(guide.open).toBe(false);
 fireEvent.click(screen.getByText('操作ガイド'));
 expect(guide.open).toBe(true);
 expect(screen.getByText('地図・線路・計画')).toBeTruthy();
 expect(screen.getByText('駅・編集・取り消し')).toBeTruthy();
 expect(screen.getByText('参考データ')).toBeTruthy();
 expect(container.textContent).toContain('接続先への自動探索');
 expect(container.textContent).toContain('建設費・維持費は未計算');
});


test('new players can find ordered construction and dispatch steps locally',()=>{
 render(<UserGuide/>);
 fireEvent.click(screen.getByText('操作ガイド'));
 fireEvent.click(screen.getByText('まず2駅で列車を走らせる'));
 const steps=[...screen.getByRole('list',{name:'最初の運行手順'}).querySelectorAll('li')];
 expect(steps).toHaveLength(6);
 expect(steps[1].textContent).toContain('計画を着工');
 expect(steps[3].textContent).toContain('路線から作成');
 expect(steps[5].textContent).toContain('再開');
});
