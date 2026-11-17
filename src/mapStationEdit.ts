import type {Candidate,Candidates} from './RouteCandidatePreview';
/** Keep every existing constraint in order. Evaluate insertion gaps against physical network distance. */
export async function editStationOrder(ordered:string[],station:string,lookup:(ids:string[])=>Promise<Candidates>,length:(path:Candidate)=>number){
 const removing=ordered.includes(station);
 const options=removing?[ordered.filter(id=>id!==station)]:Array.from({length:ordered.length+1},(_,i)=>[...ordered.slice(0,i),station,...ordered.slice(i)]);
 if(options.some(ids=>ids.length<2))throw Error('路線・系統には2駅以上必要です');
 let best:{ids:string[];result:Candidates;index:number;distance:number}|null=null;
 for(const ids of options){
  const result=await lookup(ids);
  if(result.truncated||result.status==='incomplete')throw Error('経路を確認しきれませんでした。元の編集内容を保持しています');
  if(result.status==='no_path'||!result.candidates.length)continue;
  const index=result.shortest_candidate_index??(result.candidates.length===1?0:-1);
  if(index<0||index>=result.candidates.length)throw Error('経路を比較できませんでした');
  const distance=length(result.candidates[index]);
  if(!Number.isFinite(distance))throw Error('経路の長さを確認できませんでした');
  if(!best||distance<best.distance)best={ids,result,index,distance};
 }
 if(!best)throw Error('この駅には接続できません。元の編集内容を保持しています');
 return best;
}
