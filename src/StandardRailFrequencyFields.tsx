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
  return /^[1-9]\d{0,2}$/.test(input)&&value<=240?value:null;
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
  <label>基本の間隔（分）<input type="number" min="1" max="240" step="1" required value={draft.baseMinutes}
   onChange={e=>onChange({...draft,baseMinutes:e.target.value})}/></label>
  <p>時間帯ごとに変えるときだけ入力します。空欄は基本の間隔です。</p>
  <div className="operation-times">
   {frequencyBands.map(([kind,label])=><label key={kind}>{label}の間隔（分）
    <input type="number" min="1" max="240" step="1" value={draft.overrides[kind]??''}
     onChange={e=>onChange({...draft,overrides:{...draft.overrides,[kind]:e.target.value}})}/>
   </label>)}
  </div>
 </fieldset>;
}
