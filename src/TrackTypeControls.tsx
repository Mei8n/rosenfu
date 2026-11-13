export type TrackType = 'normal'|'high_speed'|'tram';
export const TRACK_TYPE_LABELS:Record<TrackType,string>={normal:'通常',high_speed:'高速',tram:'トラム'};
export function TrackTypeControls({value,roadFollowing,disabled,onChange,onRoadFollowing}:{value:TrackType;roadFollowing:boolean;disabled:boolean;onChange:(value:TrackType)=>void;onRoadFollowing:(value:boolean)=>void}){
 return <fieldset disabled={disabled} className="track-type-controls"><label htmlFor="construction-track-type">線路の種類</label><select id="construction-track-type" value={value} onChange={event=>onChange(event.target.value as TrackType)}>{Object.entries(TRACK_TYPE_LABELS).map(([type,label])=><option key={type} value={type}>{label}</option>)}</select>
 {value==='tram'&&<><label><input type="checkbox" checked={roadFollowing} onChange={event=>onRoadFollowing(event.target.checked)}/>道路に合わせる</label>{roadFollowing?<p role="status">検証済みローカル道路バンドルで「限定道路追従」を開始します。同一wayのみ対応。OFFにすると自由に敷設できます。</p>:<p className="field-hint">自由敷設。併用軌道・専用軌道の区間区別は未実装です。</p>}</>}
 <p className="field-hint">種類は建設後も保存されます。種類別の速度・曲線・勾配・費用の規格差はまだ未実装です。</p></fieldset>;
}
