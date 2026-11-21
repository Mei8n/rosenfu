// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {ServiceDirectory} from './ServiceDirectory';
afterEach(cleanup);
const services=[{id:'internal-a',name:'各駅停車'},{id:'internal-b',name:'Express'}];
test('search preserves selection and exposes no internal identifiers or invented metrics',()=>{
 const onChoose=vi.fn(),onCreate=vi.fn();
 const {container}=render(<ServiceDirectory services={services} selected="internal-a" enabled onChoose={onChoose} onCreate={onCreate}/>);
 expect(screen.getByRole('button',{name:'系統編集: 各駅停車'}).getAttribute('aria-pressed')).toBe('true');
 fireEvent.click(screen.getByRole('button',{name:'系統編集: 各駅停車'}));expect(onChoose).toHaveBeenCalledWith(null);
 fireEvent.change(screen.getByLabelText('系統を探す'),{target:{value:'ＥＸＰＲＥＳＳ'}});
 expect(screen.queryByRole('button',{name:'系統編集: 各駅停車'})).toBeNull();
 fireEvent.click(screen.getByRole('button',{name:'系統編集: Express'}));expect(onChoose).toHaveBeenLastCalledWith('internal-b');
 expect(container.textContent).not.toContain('internal-');
 for(const label of ['乗客','収支','車両数'])expect(screen.getByLabelText(label+': 未実装').textContent).toBe('—');
 fireEvent.click(screen.getByRole('button',{name:'新規系統'}));expect(onCreate).toHaveBeenCalledOnce();
});
test('disabled actions and empty search do not mutate selected service',()=>{
 const onChoose=vi.fn(),onCreate=vi.fn();
 render(<ServiceDirectory services={services} selected="internal-a" enabled={false} onChoose={onChoose} onCreate={onCreate}/>);
 fireEvent.click(screen.getByRole('button',{name:'新規系統'}));
 fireEvent.click(screen.getByRole('button',{name:'系統編集: 各駅停車'}));
 expect(onChoose).not.toHaveBeenCalled();expect(onCreate).not.toHaveBeenCalled();
 fireEvent.change(screen.getByLabelText('系統を探す'),{target:{value:'unknown'}});
 expect(screen.getByText('一致する系統がありません')).toBeTruthy();
});
