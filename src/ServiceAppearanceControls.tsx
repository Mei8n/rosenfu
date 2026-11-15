import {useEffect,useRef,useState} from 'react';
const DEFAULT_COLOR='#3b82f6';
export function ServiceAppearanceControls({color,enabled,onDirty,onSave,onDuplicate}:{color:string|null;enabled:boolean;onDirty:(dirty:boolean)=>void;onSave:(color:string,expectedColor:string|null)=>Promise<void>;onDuplicate:()=>Promise<void>}){
 const [draft,setDraft]=useState(color??DEFAULT_COLOR),[pending,setPending]=useState(false),[error,setError]=useState(''),[conflict,setConflict]=useState(false);
 const observed=useRef(color),busy=useRef(false);
 const changed=draft!==(color??DEFAULT_COLOR);
 function reset(){setDraft(color??DEFAULT_COLOR);observed.current=color;setConflict(false);setError('');onDirty(false);}
 useEffect(()=>{if(observed.current===color)return;const previous=observed.current;observed.current=color;
  if(draft!==(previous??DEFAULT_COLOR)&&draft!==(color??DEFAULT_COLOR)){setConflict(true);setError('保存済みの色が変更されています。入力を取り消して最新の色を確認してください。');}
  else {setDraft(color??DEFAULT_COLOR);setConflict(false);setError('');}
 },[color]);
 useEffect(()=>{onDirty(changed||conflict);},[changed,conflict,onDirty]);
 async function apply(){if(!enabled||busy.current||conflict||(!changed&&color!==null)||!/^#[0-9a-f]{6}$/i.test(draft))return;
  busy.current=true;setPending(true);setError('');try{await onSave(draft,color);}catch(e){setError(String(e));}finally{busy.current=false;setPending(false);}}
 async function duplicate(){if(!enabled||busy.current||changed||conflict)return;busy.current=true;setPending(true);setError('');try{await onDuplicate();}catch(e){setError(String(e));}finally{busy.current=false;setPending(false);}}
 return <section aria-label="系統の色と複製"><fieldset disabled={!enabled||pending}>
  <label>系統色<input type="color" aria-label="系統色" value={draft} onChange={e=>setDraft(e.target.value)}/></label>{color===null&&<span>未設定</span>}
  <button type="button" disabled={conflict||(!changed&&color!==null)} onClick={()=>void apply()}>{color===null?'色を設定':'色を適用'}</button>
  {(changed||conflict)&&<button type="button" className="secondary" onClick={reset}>色の変更を取り消す</button>}
  <button type="button" className="secondary" disabled={changed||conflict} onClick={()=>void duplicate()}>系統を複製</button>
 </fieldset>{error&&<p role="alert">{error}</p>}</section>;
}
