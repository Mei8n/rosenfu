import {useId,useState} from 'react';
import {LEVEL_LABELS,type SectionInfrastructure} from './sectionInfrastructure';
import {HEIGHT_COST_FACTORS} from './heightCostModel';
import './HeightControls.css';
type Level=SectionInfrastructure['level'];
const OPTIONS:[Level,string][]=[['elevated_3','+3'],['elevated_2','+2'],['elevated','+1'],['ground','0'],['underground','−1'],['underground_2','−2'],['underground_3','−3']];
export function HeightControls({value,disabled,onChange}:{value:Level|undefined;disabled:boolean;onChange:(level:Level)=>void}){
 const id=useId(),[hover,setHover]=useState<Level|null>(null);
 return <div className="height-controls" role="group" aria-label="敷設する線路の高さ"><span>高さ</span><div className="height-options">{OPTIONS.map(([level,label])=>{const factors=HEIGHT_COST_FACTORS[level],tip=id+level;return <span key={level} className="height-option" onMouseEnter={()=>setHover(level)} onMouseLeave={()=>setHover(null)}><button type="button" className={'height-choice height-'+level} aria-label={LEVEL_LABELS[level]} aria-pressed={level===value} aria-describedby={hover===level?tip:undefined} disabled={disabled} onFocus={()=>setHover(level)} onBlur={()=>setHover(null)} onClick={()=>onChange(level)}>{label}</button>{hover===level&&<span role="tooltip" id={tip} className="height-tooltip"><strong>{LEVEL_LABELS[level]}</strong><span>建設費 {factors.construction}倍・1 kmあたり 未実装</span><span>維持費 {factors.maintenance}倍</span></span>}</span>;})}</div></div>;
}
