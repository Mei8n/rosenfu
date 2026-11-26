import {assertNever} from './assertNever';
export const TRACK_TYPES=['normal','high_speed','tram'] as const;
export type TrackType=typeof TRACK_TYPES[number];
export function defaultDesignSpeed(type:TrackType):number {
 switch(type){
  case 'normal':return 120;
  case 'high_speed':return 300;
  case 'tram':return 50;
  default:return assertNever(type);
 }
}
export const TRACK_TYPE_LABELS:Record<TrackType,string>={normal:'通常',high_speed:'高速',tram:'トラム'};
export function TrackTypeControls({value,disabled,onChange}:{value:TrackType;disabled:boolean;onChange:(value:TrackType)=>void}){
 return <fieldset disabled={disabled} className="track-type-controls" aria-label="線路の種類"><div className="segmented track-type-options" role="group" aria-label="線路種別">
 {TRACK_TYPES.map(type=><button key={type} type="button" className="secondary" aria-pressed={value===type} onClick={()=>onChange(type)}>{TRACK_TYPE_LABELS[type]}</button>)}</div></fieldset>;
}
