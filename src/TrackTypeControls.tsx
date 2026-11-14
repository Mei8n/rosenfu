export type TrackType = 'normal'|'high_speed'|'tram';
export const defaultDesignSpeed=(type:TrackType):number=>({normal:120,high_speed:300,tram:50}[type]);
export const TRACK_TYPE_LABELS:Record<TrackType,string>={normal:'通常',high_speed:'高速',tram:'トラム'};
export function TrackTypeControls({value,roadFollowing,disabled,onChange,onRoadFollowing}:{value:TrackType;roadFollowing:boolean;disabled:boolean;onChange:(value:TrackType)=>void;onRoadFollowing:(value:boolean)=>void}){
 return <fieldset disabled={disabled} className="track-type-controls" aria-label="線路の種類"><div className="segmented track-type-options" role="group" aria-label="線路種別">
 {Object.entries(TRACK_TYPE_LABELS).map(([type,label])=><button key={type} type="button" className="secondary" aria-pressed={value===type} onClick={()=>onChange(type as TrackType)}>{label}</button>)}</div>
 {value==='tram'&&<label><input type="checkbox" checked={roadFollowing} onChange={event=>onRoadFollowing(event.target.checked)}/>道路に合わせる</label>}
 </fieldset>;
}
