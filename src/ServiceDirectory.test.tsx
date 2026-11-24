// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,within} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {OperationProvider,type OperationView} from './OperationsPanel';
import {ServiceDirectory} from './ServiceDirectory';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();vi.useRealTimers();});
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
 for(const label of ['乗客','収支'])expect(screen.getByLabelText(label+': 未実装').textContent).toBe('—');
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

test('first appearance wins and a new committed snapshot updates the badge',()=>{
 const props={services,selected:null,enabled:true,onChoose:vi.fn(),onCreate:vi.fn()};
 const first={service_id:'internal-a',color:'#ffffff',symbol:'先'};
 const second={service_id:'internal-a',color:'#000000',symbol:'後'};
 const {rerender}=render(<ServiceDirectory {...props} appearances={[first,second]}/>);
 const badge=()=>screen.getByRole('button',{name:'系統編集: 各駅停車'}).querySelector('.service-badge')?.textContent;
 expect(badge()).toBe('先');
 rerender(<ServiceDirectory {...props} appearances={[second,first]}/>);
 expect(badge()).toBe('後');
});


test('directory shows committed rail activity by service, including zero, without confusing buses or missing setup',async()=>{
 const timetable={first_departure_ms:0,last_departure_ms:360000,headway_ms:60000};
 const vehicle={max_speed_kmh:80,acceleration_mps2:1,braking_mps2:1,length_m:50};
 const config=(service_id:string)=>({service_id,timetable,vehicle,dwell_ms:20000});
 const active=(service_id:string,scheduled_ms:number)=>({key:{service_id,scheduled_ms},leg:0,delay_ms:0,state:{state:'waiting' as const,ready_ms:scheduled_ms}});
 const view:OperationView={now_ms:0,paused:true,speed:1,invalidated:false,catching_up:false,timetable_edit_token:'clock',configuration_token:'world',
  services:[config('internal-a'),config('internal-b')],trains:[active('internal-a',0),active('internal-a',1000),active('bus:local',0)]};
 const rows=[...services,{id:'unconfigured',name:'未設定の系統'}];
 vi.mocked(invoke).mockResolvedValueOnce(view).mockResolvedValue({...view,invalidated:true});
 const app=(epoch:number)=><OperationProvider available blocked={false} epoch={epoch} onChange={()=>{}}>
   <ServiceDirectory services={rows} selected={null} enabled onChoose={()=>{}} onCreate={()=>{}}/>
 </OperationProvider>;
 const {rerender}=render(app(1));
 const row=(name:string)=>within(screen.getByRole('button',{name}).closest('tr')!);
 await row('系統編集: 各駅停車').findByLabelText('稼働中: 2編成');
 expect(row('系統編集: Express').getByLabelText('稼働中: 0編成')).toBeTruthy();
 expect(row('系統編集: 未設定の系統').getByLabelText('稼働中: 運行未設定').textContent).toBe('—');
 expect(invoke).toHaveBeenCalledOnce();
 await act(async()=>rerender(app(2)));
 expect(screen.queryByLabelText('稼働中: 2編成')).toBeNull();
 expect(row('系統編集: 各駅停車').getByLabelText('稼働中: 情報なし').textContent).toBe('—');
});
