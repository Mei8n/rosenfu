import type {KeyboardEvent} from 'react';
export const WORKSPACE_TABS=[['construction','建設'],['routes','路線'],['stations','駅'],['services','系統'],['finance','財務'],['city','都市'],['saves','セーブデータ'],['settings','設定']] as const;
export type WorkspaceTab=typeof WORKSPACE_TABS[number][0];
export function WorkspaceTabs({value,onChange}:{value:WorkspaceTab;onChange:(tab:WorkspaceTab)=>void}){
 function key(event:KeyboardEvent<HTMLButtonElement>,index:number){let next=index;if(event.key==='ArrowRight')next=(index+1)%WORKSPACE_TABS.length;else if(event.key==='ArrowLeft')next=(index+WORKSPACE_TABS.length-1)%WORKSPACE_TABS.length;else if(event.key==='Home')next=0;else if(event.key==='End')next=WORKSPACE_TABS.length-1;else return;event.preventDefault();onChange(WORKSPACE_TABS[next][0]);document.getElementById('workspace-tab-'+WORKSPACE_TABS[next][0])?.focus();}
 return <nav className="workspace-tabs" role="tablist" aria-label="ゲームメニュー">{WORKSPACE_TABS.map(([id,label],index)=><button key={id} id={'workspace-tab-'+id} type="button" role="tab" aria-selected={id===value} aria-controls={'workspace-panel-'+id} aria-label={label} tabIndex={id===value?0:-1} onClick={()=>onChange(id)} onKeyDown={e=>key(e,index)}>{id==='settings'?<><span aria-hidden="true">⚙ </span>{label}</>:label}</button>)}</nav>;
}
