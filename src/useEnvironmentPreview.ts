import {useEffect,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import type {EnvironmentSnapshot,EnvironmentAssessment,AutomaticCrossing} from './environmentData';
export type EnvironmentPreview={assessments:EnvironmentAssessment[];crossings:AutomaticCrossing[];buildable:boolean};
export function useEnvironmentPreview(alignments:unknown[],stations:unknown[],environment:EnvironmentSnapshot|null,world:unknown,enabled:boolean):{preview?:EnvironmentPreview;error?:string}{
 const key=JSON.stringify({alignments,stations,environment});
 const [result,setResult]=useState<{key:string;world:unknown;preview?:EnvironmentPreview;error?:string}>();
 useEffect(()=>{let active=true;if(!enabled||!environment||!alignments.length)return;
  const timer=setTimeout(()=>{const request=JSON.parse(key);void invoke<EnvironmentPreview>('preview_environment',request).then(preview=>{if(active&&preview&&Array.isArray(preview.assessments))setResult({key,world,preview});},error=>{if(active)setResult({key,world,error:String(error)});});},200);
  return ()=>{active=false;clearTimeout(timer);};
 },[key,world,enabled,!!environment,alignments.length]);
 return result?.key===key&&result.world===world?result:{};
}
