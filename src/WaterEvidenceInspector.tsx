import {useEffect,useId,useRef,useState} from 'react';
import {loadWaterEvidenceComposite} from './waterEvidenceDelivery';
import {diagnoseWaterEvidence} from './waterEvidenceDiagnostics';
import type {WaterEvidenceComposite} from './waterEvidenceComposite';
import './waterEvidenceInspector.css';
const ROOT='https://raw.githubusercontent.com/Mei8n/rosenfu/5b647a06d15605eceb2f9235bec6bc7e59ec0bb1/data/water-inspection-trial-20261114/';
const CELLS=[
 {key:'fuji',name:'富士山',pin:'e549783de74d7d10b8432413cb355b383b1ea7579c0f40cb87a40adc42e2107e'},
 {key:'kanmon',name:'関門',pin:'8057fde2778c8af3208c82f7306218382665aebbc8f2b672186de77b660a322e'},
 {key:'takamatsu',name:'高松',pin:'24a9470b2ea3f286b67953df96d51ee6b61554db5ccbce4c579b87767d4e53b4'},
] as const;
type State={phase:'idle'|'loading'|'error';message?:string}|{phase:'loaded';evidence:WaterEvidenceComposite;name:string};
/** Standalone inspection only: no IPC, world state, route, snapshot or build callback. */
export function WaterEvidenceInspector(){
 const [selected,setSelected]=useState<string>('fuji'),[state,setState]=useState<State>({phase:'idle'});
 const active=useRef<AbortController|undefined>(undefined),generation=useRef(0);
 const cancel=()=>{generation.current++;active.current?.abort();active.current=undefined;setState({phase:'idle'});};
 useEffect(()=>()=>{generation.current++;active.current?.abort();},[]);
 const load=async()=>{
  const cell=CELLS.find(c=>c.key===selected);if(!cell)return;
  active.current?.abort();const controller=new AbortController(),id=++generation.current;active.current=controller;setState({phase:'loading'});
  try{const evidence=await loadWaterEvidenceComposite({directoryUrl:ROOT+cell.pin+'/',compositeSha256:cell.pin},{signal:controller.signal});
   if(id===generation.current&&!controller.signal.aborted)setState({phase:'loaded',evidence,name:cell.name});
  }catch{if(id===generation.current&&!controller.signal.aborted)setState({phase:'error',message:'取得・検証に失敗しました。水域なしとは判定しません。再取得できます。'});}
  finally{if(id===generation.current)active.current=undefined;}
 };
 return <section className="water-inspector" aria-label="試験水域の検査">
  <h3>試験水域の検査</h3>
  <p className="water-inspector-warning">公開3地点のみ・網羅性未確認。建設判定ではありません。</p>
  <p>現在の計画経路との交差や、着工可否は判定しません。</p>
  <div className="water-inspector-controls">
   <label>試験地点 <select value={selected} onChange={e=>{cancel();setSelected(e.target.value);}}>{CELLS.map(c=><option key={c.key} value={c.key}>{c.name}</option>)}</select></label>
   <button type="button" onClick={()=>void load()} disabled={state.phase==='loading'}>検査データを取得</button>
   {state.phase==='loading'&&<button type="button" onClick={cancel}>取得を中止</button>}
  </div>
  {state.phase==='idle'&&<p role="status">未取得。表示対象を選んで取得してください。</p>}
  {state.phase==='loading'&&<p role="status">取得・ハッシュ検証中…</p>}
  {state.phase==='error'&&<p role="alert">{state.message}</p>}
  {state.phase==='loaded'&&<EvidenceView evidence={state.evidence} name={state.name}/>}
 </section>;
}
function EvidenceView({evidence,name}:{evidence:WaterEvidenceComposite;name:string}){
 const clipId='water-clip-'+useId().replace(/[^a-zA-Z0-9_-]/g,'');
 const diagnostic=diagnoseWaterEvidence(evidence),bounds=evidence.parts[0].pack.bounds;
 const width=640,height=640*(bounds.north-bounds.south)/((bounds.east-bounds.west)*Math.cos((bounds.north+bounds.south)*Math.PI/360));
 const point=(longitude:number,latitude:number)=>`${(longitude-bounds.west)/(bounds.east-bounds.west)*width},${(bounds.north-latitude)/(bounds.north-bounds.south)*height}`;
 const lines=evidence.parts.flatMap(part=>part.pack.observations.filter(o=>o.kind==='waterway-line').map(o=>({source:part.key,observation:o})));
 const missing=diagnostic.parts.reduce((n,p)=>n+p.waterwayLinesWithoutWidthTag,0),present=diagnostic.parts.reduce((n,p)=>n+p.waterwayLinesWithUnverifiedWidthTag,0);
 return <div>
  <p role="status">{name}：取得済み・網羅性未確認。着工の根拠には使えません。</p>
  <p>描画用の面 {evidence.observedAreas.length}個 / 水路等の線レコード {lines.length}件（原典間の重複を含み得ます）</p>
  <p>幅タグ未記入 {missing}件 / 幅タグあり・未検証 {present}件</p>
  <svg className="water-inspector-map" viewBox={`0 0 ${width} ${height}`} role="img" aria-label={`${name}の原典水域面と水路等の線。建設判定用地図ではありません。`}>
   <defs><clipPath id={clipId}><rect width={width} height={height}/></clipPath></defs>
   <rect className="water-inspector-frame" width={width} height={height}/>
   <g clipPath={`url(#${clipId})`}>
   {evidence.observedAreas.map(({sourceKey,polygon},i)=><path key={`${sourceKey}-${i}`} className="water-inspector-area" fillRule="evenodd" d={polygon.rings.map(r=>'M'+r.map(p=>point(p.longitude,p.latitude)).join('L')+'Z').join(' ')}><title>{sourceKey}：原典の水域面</title></path>)}
   {lines.map(({source,observation:o})=>o.geometry.type==='LineString'&&<polyline key={`${source}-${o.id}`} className="water-inspector-line" points={o.geometry.coordinates.map(([x,y])=>point(x,y)).join(' ')}><title>{`${source} / ${o.id} / 幅タグ: ${o.tags.width?.trim()?o.tags.width:'未記入'}（未検証）`}</title></polyline>)}
   </g>
  </svg>
  <p className="water-inspector-legend">青：原典の水域面 / 橙破線：水路等の線（表示の太さは実幅ではありません）。白い部分も陸地認定ではありません。背景地図・経路は表示していません。</p>
  <p>取得範囲：経度 {bounds.west}–{bounds.east}、緯度 {bounds.south}–{bounds.north}</p>
  <p>© OpenStreetMap contributors — <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noreferrer">帰属・利用条件</a> / <a href="https://opendatacommons.org/licenses/odbl/1-0/" target="_blank" rel="noreferrer">ODbL 1.0</a></p>
  <details><summary>原典の版・欠落情報を確認</summary>
   {diagnostic.messages.map(message=><p key={message}>{message}</p>)}
   {diagnostic.parts.map((part,i)=><div key={part.key} className="water-inspector-source"><h4>{part.key}</h4><p>版表示：{part.revision} / 水域面レコード {part.waterAreaRecords}件 / 線レコード {part.waterwayLineRecords}件</p>
    <p><a href={String(evidence.parts[i].pack.source.url)} target="_blank" rel="noreferrer">原典</a> / {String(evidence.parts[i].pack.source.attribution)} / {String(evidence.parts[i].pack.source.license)}</p>
    {part.sourceIssues.length>0&&<ul>{part.sourceIssues.map((issue,j)=><li key={j}>位置不明の原典問題：{issue.id} — {issue.reason}（この表示範囲内とは限りません）</li>)}</ul>}
   </div>)}
  </details>
 </div>;
}
