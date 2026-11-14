import {pointOnAlignment} from './alignment';
import {provisionalCurveSpeed} from './circularAlignment';
import {sampledGeometry,type EditedGeometry} from './editedGeometry';
/** Only the intervals adjoining the moved anchor; never a whole-line speed. */
export function anchorDragFeedback(edited:EditedGeometry,index:number,designSpeed:number){
 if(!Number.isInteger(index)||index<0||index>=edited.anchors.length||!Number.isFinite(designSpeed)||designSpeed<=0)return [];
 const geometry=sampledGeometry(edited.line);if(geometry.error)return [];
 const indices=edited.anchors.map(anchor=>edited.line.findIndex(p=>p.latitude===anchor.latitude&&p.longitude===anchor.longitude));
 if(indices.some((n,i)=>n<0||(i>0&&n<=indices[i-1])))return [];
 return [index-1,index].filter(i=>i>=0&&i<indices.length-1).map(i=>{
  const start=indices[i],end=indices[i+1];
  const limits=geometry.arcs.filter(arc=>arc.startPointIndex<end&&arc.endPointIndex>start).map(arc=>provisionalCurveSpeed(arc.radiusM)??0);
  return {interval:i,point:pointOnAlignment(edited.line.slice(start,end+1),.5),speed:Math.min(designSpeed,...limits)};
 });
}
