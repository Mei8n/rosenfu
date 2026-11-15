import {invoke} from '@tauri-apps/api/core';
import type {EnvironmentSnapshot} from './environmentData';
/** The snapshot used by preview is sent unchanged; the backend recomputes before committing. */
export function submitEnvironmentConstruction<T>(alignments:unknown[],stations:unknown[],environment:EnvironmentSnapshot|null):Promise<T>{
 if(alignments.length&&!environment)return Promise.reject(Error('道路・水域データ未取得です。'));
 return invoke<T>(alignments.length?'build_plan_with_environment':'build_plan',alignments.length?{alignments,stations,environment}:{alignments,stations});
}
