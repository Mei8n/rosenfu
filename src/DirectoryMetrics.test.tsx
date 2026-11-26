// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen,within} from '@testing-library/react';
import {isTauri} from '@tauri-apps/api/core';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();localStorage.clear();});
test('simulation columns belong to services, not routes, and show no invented values',()=>{
 vi.mocked(isTauri).mockReturnValue(false);render(<App/>);
 fireEvent.click(screen.getByRole('tab',{name:'路線'}));
 const routes=within(screen.getByRole('table',{name:'路線一覧'}));
 for(const name of ['乗客','収支','車両数','稼働中'])expect(routes.queryByRole('columnheader',{name})).toBeNull();
 fireEvent.click(screen.getByRole('tab',{name:'系統'}));
 const services=within(screen.getByRole('table',{name:'系統一覧'}));
 for(const name of ['乗客','収支']){expect(services.getByRole('columnheader',{name})).toBeTruthy();expect(services.getByLabelText(`${name}: 未実装`).textContent).toBe('—');}
 expect(services.getByRole('columnheader',{name:'稼働中'})).toBeTruthy();
 expect(services.getByLabelText('稼働中: 情報なし').textContent).toBe('—');
});
