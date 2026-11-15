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
export type ServiceSymbolShape='circle'|'square'|'diamond';
const validSymbol=(symbol:string)=>Array.from(symbol).length<=8&&!/[\u0000-\u001f\u007f-\u009f]/.test(symbol);
export const initialServiceSymbol=(name:string)=>{const first=new Intl.Segmenter('ja',{granularity:'grapheme'}).segment(name.trim())[Symbol.iterator]().next().value?.segment??'';return validSymbol(first)?first:'';};
export function ServiceBadge({color,shape='circle',symbol}:{color:string;shape?:ServiceSymbolShape;symbol:string}){
 return <span aria-hidden="true" className={'service-badge '+shape} style={{backgroundColor:color}}><span>{symbol}</span></span>;
}
export function ServiceAppearanceControls({color,serviceName='',appearance,enabled,onDirty,onSave,onSaveAppearance,onDuplicate}:{color:string|null;serviceName?:string;appearance?:{shape?:ServiceSymbolShape;symbol?:string|null};enabled:boolean;onDirty:(dirty:boolean)=>void;onSave?:(color:string,expectedColor:string|null)=>Promise<void>;onSaveAppearance?:(color:string,shape:ServiceSymbolShape,symbol:string)=>Promise<void>;onDuplicate:()=>Promise<void>}){
 const savedShape=appearance?.shape??'circle',savedSymbol=appearance?.symbol??initialServiceSymbol(serviceName);
 const [draft,setDraft]=useState(displayColor(color)),[shape,setShape]=useState<ServiceSymbolShape>(savedShape),[symbol,setSymbol]=useState(savedSymbol),[pending,setPending]=useState(false),[error,setError]=useState(''),[conflict,setConflict]=useState(false);
 const signature=JSON.stringify([color,savedShape,appearance?.symbol??null]);
 const observed=useRef({signature,color:displayColor(color),shape:savedShape,symbol:savedSymbol,hasSymbol:appearance?.symbol!=null}),busy=useRef(false);
 const [symbolTouched,setSymbolTouched]=useState(false);
 const effectiveSymbol=appearance?.symbol==null&&!symbolTouched?savedSymbol:symbol;
 const changed=draft!==displayColor(color)||shape!==savedShape||((symbolTouched||appearance?.symbol!=null)&&symbol!==savedSymbol)||(symbolTouched&&appearance?.symbol==null);
 const valid=/^#[0-9a-f]{6}$/i.test(draft)&&validSymbol(effectiveSymbol);
 function reset(){setDraft(displayColor(color));setShape(savedShape);setSymbol(savedSymbol);setSymbolTouched(false);observed.current={signature,color:displayColor(color),shape:savedShape,symbol:savedSymbol,hasSymbol:appearance?.symbol!=null};setConflict(false);setError('');onDirty(false);}
 useEffect(()=>{if(observed.current.signature===signature)return;const previous=observed.current;
  const wasDirty=draft!==previous.color||shape!==previous.shape||symbol!==previous.symbol||(symbolTouched&&!previous.hasSymbol);
  observed.current={signature,color:displayColor(color),shape:savedShape,symbol:savedSymbol,hasSymbol:appearance?.symbol!=null};
  if(wasDirty&&changed){setConflict(true);setError('保存済みの色が変更されています（記号・形を含む）。入力を取り消して最新の設定を確認してください。');}
  else reset();
 },[signature]);
 useEffect(()=>{if(appearance?.symbol==null&&!symbolTouched){setSymbol(initialServiceSymbol(serviceName));observed.current.symbol=initialServiceSymbol(serviceName);}},[serviceName]);
 useEffect(()=>{onDirty(changed||conflict);},[changed,conflict,onDirty]);
 async function apply(){if(!enabled||busy.current||conflict||(!changed&&color!==null)||!valid)return;
  busy.current=true;setPending(true);setError('');try{if(onSaveAppearance)await onSaveAppearance(draft,shape,effectiveSymbol);else if(onSave)await onSave(draft,color);else throw Error("表示設定を保存できません。");}catch(e){setError(String(e));}finally{busy.current=false;setPending(false);}}
 async function duplicate(){if(!enabled||busy.current||changed||conflict)return;busy.current=true;setPending(true);setError('');try{await onDuplicate();}catch(e){setError(String(e));}finally{busy.current=false;setPending(false);}}
 return <section aria-label="系統の色と複製"><fieldset disabled={!enabled||pending}>
  <div className="service-symbol-fields"><ServiceBadge color={/^#[0-9a-f]{6}$/i.test(draft)?draft:displayColor(color)} shape={shape} symbol={effectiveSymbol}/>
  <label>記号<input aria-label="系統記号" value={effectiveSymbol} autoComplete="off" onChange={e=>{setSymbolTouched(true);setSymbol(e.target.value);}}/></label>
  <div role="group" aria-label="系統記号の形">{([['circle','丸','●'],['square','四角','■'],['diamond','ひし形','◆']] as const).map(([value,label,glyph])=><button type="button" key={value} aria-label={'系統の形 '+label} aria-pressed={shape===value} onClick={()=>setShape(value)}>{glyph}</button>)}</div></div>
  <div className="service-color-fields"><label>系統色<input type="color" aria-label="系統色" value={valid?draft:displayColor(color)} onChange={e=>setDraft(e.target.value.toLowerCase())}/></label>
  <label className="service-color-hex">HEX<input type="text" aria-label="系統色 HEX" value={draft} maxLength={7} spellCheck={false} autoComplete="off" aria-invalid={!valid}
   onChange={e=>setDraft(e.target.value.toLowerCase())} onKeyDown={e=>{if(e.key==='Enter'){e.preventDefault();e.stopPropagation();void apply();}else if(e.key==='Escape'){e.preventDefault();e.stopPropagation();reset();}}}/></label>{color===null&&<span>未設定</span>}</div>
  <div className="service-color-presets" role="group" aria-label="系統色の候補">{PRESET_COLORS.map(([name,value])=><button key={value} type="button" className="service-color-swatch" style={{backgroundColor:value}} title={name+' '+value} aria-label={'系統色候補 '+name+' '+value} aria-pressed={draft===value} onClick={()=>setDraft(value)}/>)}</div>
  <button type="button" disabled={conflict||!valid||(!changed&&color!==null)} onClick={()=>void apply()}>{color===null?'色を設定':'色を適用'}</button>
  {(changed||conflict)&&<button type="button" className="secondary" onClick={reset}>色の変更を取り消す</button>}
  <button type="button" className="secondary" disabled={changed||conflict} onClick={()=>void duplicate()}>系統を複製</button>
 </fieldset>{error&&<p role="alert">{error}</p>}</section>;
}
