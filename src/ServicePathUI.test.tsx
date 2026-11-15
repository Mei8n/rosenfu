// @vitest-environment jsdom
import React from 'react';
import {cleanup,fireEvent,render,screen,within,waitFor} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
import {resolveServicePath} from './servicePaths';
import {ServicePathCreator} from './ServicePathCreator';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
const segments=[{kind:'alignment' as const,alignment_id:'9',from:'3',to:'5'}];
const legs=[{key:'1',segment:segments[0]}];
const world={next_id:'10',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:['3','4','5'].map((id,i)=>({id,owner:'2',name:['起点','途中','終点'][i],latitude:35.67,longitude:139.7+i*.01})),tracks:[],routes:[{id:'6',owner:'2',name:'全線',start:'3',tracks:[]}],route_paths:[{route_id:'6',segments,member_stations:['3','5']}],services:[],alignments:[{id:'9',owner:'2',name:'線形',points:[{latitude:35.67,longitude:139.7},{latitude:35.67,longitude:139.72}]}],station_placements:['3','4','5'].map((station_id,i)=>({station_id,alignment_id:'9',position:i/2,length_m:100,width_m:12}))};
const preview={legs,occurrences:resolveServicePath('3',legs,world),requires_stop_selection:true};
const saved={...world,services:[{id:'10',owner:'2',name:'独立系統',start:'3',tracks:[],stops:[]}],service_paths:[{service_id:'10',next_leg_key:'2',legs,stops:[preview.occurrences[0].reference,preview.occurrences[2].reference]}]};
async function start(snapshot:unknown=world){vi.mocked(isTauri).mockReturnValue(true);vi.mocked(invoke).mockImplementation(async c=>c==='engine_info'?{name:'Rosenfu',version:'path-test'}:c==='preview_service_path'?preview:snapshot);const view=render(<App/>);await screen.findByText(/path-test/);fireEvent.click(screen.getByRole('tab',{name:'系統'}));return view;}
test('Alignment draft requires explicit stop policy; failed create retains inputs and successful new path is protected',async()=>{
 const view=await start();const region=within(screen.getByRole('region',{name:'線形対応の新規系統'}));fireEvent.change(region.getByLabelText('コピーする路線'),{target:{value:'6'}});fireEvent.click(region.getByRole('button',{name:'選択区間の停車候補を確認'}));await region.findByLabelText('新形式停車 2');
 expect((region.getByRole('button',{name:'新形式系統を適用'}) as HTMLButtonElement).disabled).toBe(true);fireEvent.change(region.getByLabelText('新形式の系統名'),{target:{value:'独立系統'}});fireEvent.click(region.getByLabelText('停車駅を個別指定'));fireEvent.click(region.getByLabelText('新形式停車 2'));expect((region.getByLabelText('新形式停車 1') as HTMLInputElement).disabled).toBe(true);fireEvent.click(region.getByLabelText('所属外を含む停車設定を確認しました'));
 vi.mocked(invoke).mockRejectedValueOnce(Error('create rejected'));fireEvent.click(region.getByRole('button',{name:'新形式系統を適用'}));await region.findByText(/create rejected/);expect((region.getByLabelText('新形式の系統名') as HTMLInputElement).value).toBe('独立系統');await waitFor(()=>expect((region.getByRole('button',{name:'新形式系統を適用'}) as HTMLButtonElement).disabled).toBe(false));vi.mocked(invoke).mockResolvedValueOnce(saved);fireEvent.click(region.getByRole('button',{name:'新形式系統を適用'}));await screen.findByText(/新形式の系統を適用しました/);
 expect(invoke).toHaveBeenCalledWith('create_service_path',{sourceRouteId:'6',name:'独立系統',start:'3',segments,stops:saved.service_paths[0].stops});fireEvent.click(screen.getByRole('button',{name:'系統編集: 独立系統'}));expect(screen.getByRole('region',{name:'保存済み新形式系統'}).textContent).toContain('途中 / 通過');expect(view.container.querySelector('#service-name')).toBeNull();expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='save_service')).toBe(false);
});
test('nonstop Alignment station deletion is guarded by saved service path',async()=>{await start({...saved,routes:[],route_paths:[]});fireEvent.click(screen.getByRole('tab',{name:'駅'}));fireEvent.click(screen.getByRole('button',{name:/^途中 /}));expect((screen.getByRole('button',{name:'選択駅を削除'}) as HTMLButtonElement).disabled).toBe(true);expect(screen.getByText(/削除不可: 系統: 独立系統/)).toBeTruthy();});
test('world changes discard delayed preview but retain draft name; wrong leg keys reject',async()=>{
 let finish!:(value:unknown)=>void;vi.mocked(invoke).mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));const props={routes:world.routes,routePaths:world.route_paths,stations:world.stations,world,enabled:true,canStart:true,onDirty:vi.fn(),onSave:vi.fn()};const view=render(<ServicePathCreator {...props}/>);fireEvent.change(screen.getByLabelText('コピーする路線'),{target:{value:'6'}});fireEvent.change(screen.getByLabelText('新形式の系統名'),{target:{value:'保持'}});fireEvent.click(screen.getByRole('button',{name:'選択区間の停車候補を確認'}));view.rerender(<ServicePathCreator {...props} world={{...world}}/>);finish(preview);await waitFor(()=>expect(screen.queryByText('処理中…')).toBeNull());expect(screen.queryByLabelText('新形式停車 1')).toBeNull();expect((screen.getByLabelText('新形式の系統名') as HTMLInputElement).value).toBe('保持');vi.mocked(invoke).mockResolvedValueOnce({...preview,legs:[{...legs[0],key:'2'}]});fireEvent.click(screen.getByRole('button',{name:'選択区間の停車候補を確認'}));await screen.findByText(/形式が一致しません/);
});
test('stable references keep repeated visits distinct and resolve reverse Alignment intermediate stations',()=>{const reverse={key:'2',segment:{...segments[0],from:'5',to:'3'}};const result=resolveServicePath('3',[...legs,reverse],world);expect(result.map(o=>o.station_id)).toEqual(['3','4','5','4','3']);expect(result[1].reference).not.toEqual(result[3].reference);});

test('native JSON key ordering does not change preview meaning',async()=>{
 const reordered={requires_stop_selection:true,occurrences:preview.occurrences.map(o=>({reference:o.reference.kind==='start'?{kind:'start'}:{station_id:o.reference.station_id,leg_key:o.reference.leg_key,kind:'on_leg'},station_id:o.station_id})),legs:[{segment:{to:'5',from:'3',alignment_id:'9',kind:'alignment'},key:'1'}]};
 vi.mocked(invoke).mockResolvedValue(reordered);render(<ServicePathCreator routes={world.routes} routePaths={world.route_paths} stations={world.stations} world={world} enabled canStart onDirty={vi.fn()} onSave={vi.fn()}/>);fireEvent.change(screen.getByLabelText('コピーする路線'),{target:{value:'6'}});fireEvent.click(screen.getByRole('button',{name:'選択区間の停車候補を確認'}));await screen.findByLabelText('新形式停車 3');expect(screen.queryByRole('alert')).toBeNull();
});
test('saved selection and unsaved new path drafts protect each other',async()=>{
 const view=await start(saved);const panel=within(view.container.querySelector('#workspace-panel-services') as HTMLElement),region=within(screen.getByRole('region',{name:'線形対応の新規系統'}));const routeSelect=region.getByLabelText('コピーする路線');
 fireEvent.click(panel.getByRole('button',{name:'系統編集: 独立系統'}));expect(routeSelect.closest('fieldset')!.disabled).toBe(true);
 fireEvent.click(panel.getByRole('button',{name:'新規系統'}));expect(routeSelect.closest('fieldset')!.disabled).toBe(false);
 fireEvent.change(routeSelect,{target:{value:'6'}});fireEvent.change(region.getByLabelText('新形式の系統名'),{target:{value:'新下書き'}});
 fireEvent.click(panel.getByRole('button',{name:'系統編集: 独立系統'}));expect((region.getByLabelText('新形式の系統名') as HTMLInputElement).value).toBe('新下書き');expect(panel.queryByRole('region',{name:'保存済み新形式系統'})).toBeNull();
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='save_service'||c==='create_service_path')).toBe(false);
 fireEvent.click(region.getByRole('button',{name:'新形式下書きを破棄'}));fireEvent.click(panel.getByRole('button',{name:'系統編集: 独立系統'}));expect(panel.getByRole('region',{name:'保存済み新形式系統'})).toBeTruthy();
});

test('saved new-format stops edit blocks selection and saves through atomic command',async()=>{
 await start(saved);const panel=within(screen.getByRole('tabpanel',{name:'系統'}));fireEvent.click(panel.getByRole('button',{name:'系統編集: 独立系統'}));const editor=within(panel.getByRole('region',{name:'保存済み新形式系統'}));
 fireEvent.change(editor.getByLabelText('保存済み系統名'),{target:{value:'  更新系統  '}});fireEvent.click(editor.getByLabelText('保存済み停車 2'));fireEvent.click(panel.getByRole('button',{name:'新規系統'}));expect(panel.getByRole('button',{name:'系統下書きを破棄して切替'})).toBeTruthy();
 fireEvent.click(editor.getByRole('button',{name:'保存済み系統の変更を取り消す'}));expect((editor.getByLabelText('保存済み系統名') as HTMLInputElement).value).toBe('独立系統');
 fireEvent.change(editor.getByLabelText('保存済み系統名'),{target:{value:'  更新系統  '}});fireEvent.click(editor.getByLabelText('保存済み停車 2'));
 vi.mocked(invoke).mockImplementation(async(command)=>command==='update_service_path'?{...saved,services:[{...saved.services[0],name:'更新系統'}],service_paths:[{...saved.service_paths[0],stops:preview.occurrences.map(o=>o.reference)}]}:saved);
 fireEvent.click(editor.getByRole('button',{name:'保存済み系統の変更を適用'}));await screen.findByText(/系統名と停車設定を適用しました/);
 expect(screen.queryByText(/保存済み系統が変更されています/)).toBeNull();
 expect(invoke).toHaveBeenCalledWith('update_service_path',{serviceId:'10',name:'更新系統',stops:preview.occurrences.map(o=>o.reference)});
 fireEvent.click(editor.getByLabelText('保存済み停車 2'));
 expect((editor.getByRole('button',{name:'保存済み系統の変更を適用'}) as HTMLButtonElement).disabled).toBe(false);
 fireEvent.click(editor.getByRole('button',{name:'保存済み系統の変更を取り消す'}));
 expect((editor.getByLabelText('保存済み停車 2') as HTMLInputElement).checked).toBe(true);
 expect(screen.queryByText(/保存済み系統が変更されています/)).toBeNull();

});
test('partial reverse alignment section sends preview and creation coordinates consistently',async()=>{
 const save=vi.fn().mockResolvedValue(undefined);vi.mocked(invoke).mockImplementation(async(_command,args:any)=>{const ls=args.segments.map((segment:any,i:number)=>({key:String(i+1),segment}));return {legs:ls,occurrences:resolveServicePath(args.start,ls,world),requires_stop_selection:true};});
 render(<ServicePathCreator routes={world.routes} routePaths={world.route_paths} stations={world.stations} world={world} enabled canStart onDirty={vi.fn()} onSave={save}/>);
 fireEvent.change(screen.getByLabelText('コピーする路線'),{target:{value:'6'}});fireEvent.change(screen.getByLabelText('新形式区間の出発'),{target:{value:'2'}});fireEvent.change(screen.getByLabelText('新形式区間の終点'),{target:{value:'1'}});fireEvent.click(screen.getByRole('button',{name:'選択区間の停車候補を確認'}));await screen.findByLabelText('新形式停車 2');
 fireEvent.change(screen.getByLabelText('新形式の系統名'),{target:{value:'区間運行'}});fireEvent.click(screen.getByLabelText('全駅停車（所属外を含む）'));fireEvent.click(screen.getByLabelText('所属外を含む停車設定を確認しました'));fireEvent.click(screen.getByRole('button',{name:'新形式系統を適用'}));await waitFor(()=>expect(save).toHaveBeenCalledTimes(1));expect(save.mock.calls[0][0]).toMatchObject({start:'5',segments:[{...segments[0],from:'5',to:'4'}]});
});

test('saved service route replacement uses fresh references, explicit stops and preserves failed draft',async()=>{
 await start(saved);const panel=within(screen.getByRole('tabpanel',{name:'系統'}));fireEvent.click(panel.getByRole('button',{name:'系統編集: 独立系統'}));const region=within(screen.getByRole('region',{name:'保存済み系統の経路再設定'}));
 const newSegment={...segments[0],from:'5',to:'4'},newLegs=[{key:'2',segment:newSegment}],newOccurrences=resolveServicePath('5',newLegs,world);
 vi.mocked(invoke).mockImplementation(async command=>command==='preview_service_path_replacement'?{legs:newLegs,occurrences:newOccurrences,requires_stop_selection:true}:saved);
 fireEvent.change(region.getByLabelText('コピーする路線'),{target:{value:'6'}});fireEvent.change(region.getByLabelText('新形式区間の出発'),{target:{value:'2'}});fireEvent.change(region.getByLabelText('新形式区間の終点'),{target:{value:'1'}});fireEvent.click(region.getByRole('button',{name:'選択区間の停車候補を確認'}));await region.findByLabelText('新形式停車 2');
 expect((region.getByRole('button',{name:'保存済み系統の経路を置き換える'}) as HTMLButtonElement).disabled).toBe(true);fireEvent.click(region.getByLabelText('全駅停車（所属外を含む）'));fireEvent.click(region.getByLabelText('所属外を含む停車設定を確認しました'));
 vi.mocked(invoke).mockRejectedValueOnce(Error('変更拒否'));fireEvent.click(region.getByRole('button',{name:'保存済み系統の経路を置き換える'}));await region.findByText(/変更拒否/);expect(region.getByLabelText('新形式停車 2')).toBeTruthy();
 const updated={...saved,services:[{...saved.services[0],start:'5'}],service_paths:[{...saved.service_paths[0],next_leg_key:'3',legs:newLegs,stops:newOccurrences.map(o=>o.reference)}]};vi.mocked(invoke).mockResolvedValueOnce(updated);fireEvent.click(region.getByRole('button',{name:'保存済み系統の経路を置き換える'}));await screen.findByText(/同じ系統IDで経路と停車設定を更新しました/);
 expect(invoke).toHaveBeenCalledWith('replace_service_path',{sourceRouteId:'6',serviceId:'10',expectedPath:saved.service_paths[0],name:'独立系統',start:'5',segments:[newSegment],stops:newOccurrences.map(o=>o.reference)});expect(screen.getByRole('button',{name:'系統編集: 独立系統'})).toBeTruthy();expect(screen.queryByText(/保存済み系統が変更されています/)).toBeNull();await waitFor(()=>expect((region.getByLabelText('コピーする路線').closest('fieldset') as HTMLFieldSetElement).disabled).toBe(false));
});

test('inactive creator cannot clear replacement dirty flag and pristine replacement name follows saved rename',async()=>{
 await start(saved);const panel=within(screen.getByRole('tabpanel',{name:'系統'}));fireEvent.click(panel.getByRole('button',{name:'系統編集: 独立系統'}));const replacement=within(screen.getByRole('region',{name:'保存済み系統の経路再設定'})),editor=within(screen.getByRole('region',{name:'保存済み新形式系統'}));
 fireEvent.change(editor.getByLabelText('保存済み系統名'),{target:{value:'改名後'}});vi.mocked(invoke).mockResolvedValueOnce({...saved,services:[{...saved.services[0],name:'改名後'}]});fireEvent.click(editor.getByRole('button',{name:'保存済み系統の変更を適用'}));await screen.findByText(/系統名と停車設定を適用しました/);await waitFor(()=>expect((replacement.getByLabelText('新形式の系統名') as HTMLInputElement).value).toBe('改名後'));
 fireEvent.change(replacement.getByLabelText('コピーする路線'),{target:{value:'6'}});const inactive=within(screen.getByRole('region',{name:'線形対応の新規系統'}));expect((inactive.getByRole('button',{name:'新形式下書きを破棄'}) as HTMLButtonElement).disabled).toBe(true);fireEvent.click(inactive.getByRole('button',{name:'新形式下書きを破棄'}));fireEvent.click(panel.getByRole('button',{name:'新規系統'}));expect(screen.getByRole('region',{name:'保存済み系統の経路再設定'})).toBeTruthy();expect((editor.getByLabelText('保存済み系統名').closest('fieldset') as HTMLFieldSetElement).disabled).toBe(true);
});

test('selected saved service toggles off and protects an unsaved edit',async()=>{
 await start(saved);
 const button=()=>screen.getByRole('button',{name:'系統編集: 独立系統'});
 fireEvent.click(button());fireEvent.click(button());
 expect(screen.queryByRole('region',{name:'保存済み新形式系統'})).toBeNull();
 fireEvent.click(button());fireEvent.change(screen.getByLabelText('保存済み系統名'),{target:{value:'下書き'}});
 fireEvent.click(button());expect(screen.getByRole('region',{name:'保存済み新形式系統'})).toBeTruthy();
 fireEvent.click(screen.getByRole('button',{name:'系統下書きを破棄して切替'}));
 expect(screen.queryByRole('region',{name:'保存済み新形式系統'})).toBeNull();
});
