import {useState} from 'react';

const maxBandMinutes=240;
const maxTrainsets=65_535;
const minuteMs=60_000;

// Standard setup input only. The native recurring timetable supplies the
// actual hours and all dispatch; this component does not make a schedule.
export const frequencyBands=[
 ['earlyMorning','早朝'],
 ['morningRush','朝ラッシュ'],
 ['midday','昼'],
 ['eveningRush','夕ラッシュ'],
 ['lateNight','深夜'],
] as const;

export type FrequencyBand=typeof frequencyBands[number][0];
export type FrequencyDraft={
 baseMinutes:string;
 overrides:Partial<Record<FrequencyBand,string>>;
};
export type FrequencyRequest={
 baseMinutes:number;
 overrides:Partial<Record<FrequencyBand,number>>;
};

export function parseFrequencyDraft(draft:FrequencyDraft):FrequencyRequest|null{
 const decoded=(input:string)=>{
  const value=Number(input);
  return /^[1-9]\d{0,2}$/.test(input)&&value<=maxBandMinutes?value:null;
 };
 const baseMinutes=decoded(draft.baseMinutes);
 if(baseMinutes===null)return null;
 const overrides:FrequencyRequest['overrides']={};
 for(const [kind] of frequencyBands){
  const draftMinutes=draft.overrides[kind];
  if(draftMinutes===undefined||draftMinutes==='')continue;
  const minutes=decoded(draftMinutes);
  if(minutes===null)return null;
  overrides[kind]=minutes;
 }
 return {baseMinutes,overrides};
}

export function StandardRailFrequencyFields({draft,onChange,disabled=false}:{draft:FrequencyDraft;onChange:(draft:FrequencyDraft)=>void;disabled?:boolean}){
 return <fieldset disabled={disabled}>
  <legend>運転間隔（何分おき）</legend>
  <label>基本の間隔（分）<input type="number" min="1" max={maxBandMinutes} step="1" required value={draft.baseMinutes}
   onChange={e=>onChange({...draft,baseMinutes:e.target.value})}/></label>
  <p>時間帯ごとに変えるときだけ入力します。空欄は基本の間隔です。</p>
  <div className="operation-times">
   {frequencyBands.map(([kind,label])=><label key={kind}>{label}の間隔（分）
    <input type="number" min="1" max={maxBandMinutes} step="1" value={draft.overrides[kind]??''}
     onChange={e=>onChange({...draft,overrides:{...draft.overrides,[kind]:e.target.value}})}/>
   </label>)}
  </div>
 </fieldset>;
}

type FleetTarget='base'|FrequencyBand;

/** Convert a verified no-contention cycle to the existing whole-minute
 * frequency input. This never allocates vehicles or modifies a timetable. */
export function TimeBandFleetPlanner({cycleMs,disabled,onAdopt}:{
 cycleMs:number|null|undefined;disabled:boolean;onAdopt:(target:FleetTarget,minutes:number)=>void;
}){
 const [target,setTarget]=useState<FleetTarget>('base');
 const [count,setCount]=useState('');
 if(cycleMs==null||!Number.isSafeInteger(cycleMs)||cycleMs<=0)return null;
 const trainsets=Number(count);
 const valid=/^[0-9]+$/.test(count)&&Number.isSafeInteger(trainsets)&&trainsets>=1&&trainsets<=maxTrainsets;
 const mins=valid?Math.ceil(cycleMs/(trainsets*minuteMs)):null;
 return <details>
  <summary>編成数から時間帯の間隔を決める</summary>
  <p>競合がない往復時間から、1分単位で切り上げます。編成数は試算だけに使い、運行中の上限にはしません。</p>
  <fieldset disabled={disabled}>
   <legend>試算条件</legend>
   <label>間隔に反映する時間帯<select value={target} onChange={e=>setTarget(e.target.value as FleetTarget)}>
    <option value="base">基本の間隔</option>
    {frequencyBands.map(([id,name])=><option key={id} value={id}>{name}</option>)}
   </select></label>
   <label>試算用の編成数<input inputMode="numeric" value={count} maxLength={5}
    onKeyDown={e=>{if(e.key==='Enter')e.preventDefault();}} onChange={e=>setCount(e.target.value)}/></label>
   {mins!==null?<p>{mins<=maxBandMinutes?`試算結果：${mins}分おき`:'試算した間隔が240分を超えます。この設定には反映できません。'}</p>:
    count&&<p>1〜65535の整数を入力してください。</p>}
   <button type="button" disabled={mins===null||mins>maxBandMinutes} onClick={()=>{if(mins!==null&&mins<=maxBandMinutes)onAdopt(target,mins);}}>試算した間隔を入力欄に反映</button>
  </fieldset>
 </details>;
}
