import {useEffect,useRef,useState} from 'react';
const PRESET_COLORS=[
 ['赤','#d7263d'],['青','#1f6fd1'],['緑','#1a9a4a'],['橙','#f08a00'],
 ['紫','#8a3fb8'],['黄','#f2c200'],['茶','#8f5a2a'],['青緑','#0e9a9a'],
 ['灰','#74797e'],['黄緑','#9ccb19'],['桃','#f59dc6'],['赤紫','#e62aa6'],
 ['水色','#5cc3f0'],['オリーブ','#6f7a0f'],['珊瑚','#ee7d63'],['薄紫','#a9a0e8'],
] as const;
const DEFAULT_COLOR='#3b82f6';
// Normalize presentation/comparison only; retain raw persisted colour for CAS.
const displayColor=(color:string|null)=>(color??DEFAULT_COLOR).toLowerCase();
export function ServiceAppearanceControls({color,enabled,onDirty,onSave,onDuplicate}:{color:string|null;enabled:boolean;onDirty:(dirty:boolean)=>void;onSave:(color:string,expectedColor:string|null)=>Promise<void>;onDuplicate:()=>Promise<void>}){
 const [draft,setDraft]=useState(displayColor(color)),[pending,setPending]=useState(false),[error,setError]=useState(''),[conflict,setConflict]=useState(false);
 const observed=useRef(color),busy=useRef(false);
 const changed=draft!==displayColor(color);
 const valid=/^#[0-9a-f]{6}$/i.test(draft);
 function reset(){setDraft(displayColor(color));observed.current=color;setConflict(false);setError('');onDirty(false);}
 useEffect(()=>{if(observed.current===color)return;const previous=observed.current;observed.current=color;
  if(draft!==displayColor(previous)&&draft!==displayColor(color)){setConflict(true);setError('保存済みの色が変更されています。入力を取り消して最新の色を確認してください。');}
  else {setDraft(displayColor(color));setConflict(false);setError('');}
 },[color]);
 useEffect(()=>{onDirty(changed||conflict);},[changed,conflict,onDirty]);
 async function apply(){if(!enabled||busy.current||conflict||(!changed&&color!==null)||!/^#[0-9a-f]{6}$/i.test(draft))return;
  busy.current=true;setPending(true);setError('');try{await onSave(draft,color);}catch(e){setError(String(e));}finally{busy.current=false;setPending(false);}}
 async function duplicate(){if(!enabled||busy.current||changed||conflict)return;busy.current=true;setPending(true);setError('');try{await onDuplicate();}catch(e){setError(String(e));}finally{busy.current=false;setPending(false);}}
 return <section aria-label="系統の色と複製"><fieldset disabled={!enabled||pending}>
  <div className="service-color-fields"><label>系統色<input type="color" aria-label="系統色" value={valid?draft:displayColor(color)} onChange={e=>setDraft(e.target.value.toLowerCase())}/></label>
  <label className="service-color-hex">HEX<input type="text" aria-label="系統色 HEX" value={draft} maxLength={7} spellCheck={false} autoComplete="off" aria-invalid={!valid}
   onChange={e=>setDraft(e.target.value.toLowerCase())} onKeyDown={e=>{if(e.key==='Enter'){e.preventDefault();e.stopPropagation();void apply();}else if(e.key==='Escape'){e.preventDefault();e.stopPropagation();reset();}}}/></label>{color===null&&<span>未設定</span>}</div>
  <div className="service-color-presets" role="group" aria-label="系統色の候補">{PRESET_COLORS.map(([name,value])=><button key={value} type="button" className="service-color-swatch" style={{backgroundColor:value}} title={name+' '+value} aria-label={'系統色候補 '+name+' '+value} aria-pressed={draft===value} onClick={()=>setDraft(value)}/>)}</div>
  <button type="button" disabled={conflict||!valid||(!changed&&color!==null)} onClick={()=>void apply()}>{color===null?'色を設定':'色を適用'}</button>
  {(changed||conflict)&&<button type="button" className="secondary" onClick={reset}>色の変更を取り消す</button>}
  <button type="button" className="secondary" disabled={changed||conflict} onClick={()=>void duplicate()}>系統を複製</button>
 </fieldset>{error&&<p role="alert">{error}</p>}</section>;
}
