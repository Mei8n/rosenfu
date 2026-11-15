import './serviceStopDiagram.css';

export type ServiceStopRow={key:string;name:string;stopping:boolean;fixed?:boolean;detail?:string;undecided?:boolean};
/** Occurrence keys, not station IDs, keep repeated station visits independent. */
export function ServiceStopDiagram({rows,label,controlLabel,disabled=false,onChange}:{rows:ServiceStopRow[];label:string;controlLabel:(index:number,row:ServiceStopRow)=>string;disabled?:boolean;onChange:(key:string,stopping:boolean)=>void}){
 return <ol className="service-stop-diagram" aria-label={label}>{rows.map((row,index)=>
  <li key={row.key} className={row.stopping?'stop-row stopping':'stop-row passing'}>
   <label title={[row.fixed?'端点・停車固定':'',row.detail??''].filter(Boolean).join(' / ')}>
    <span className="stop-rail" aria-hidden="true"/>
    <input type="checkbox" className="stop-marker" aria-label={controlLabel(index,row)} checked={row.stopping} disabled={disabled||row.fixed} onChange={event=>onChange(row.key,event.target.checked)}/>
    <span className="stop-name">{row.name}</span><span className="stop-status"><span className="stop-separator"> / </span>{row.undecided?'未選択':row.stopping?'停車':'通過'}</span>
   </label>
  </li>)}</ol>;
}
