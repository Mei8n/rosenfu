import {invoke} from '@tauri-apps/api/core';
import {vi} from 'vitest';
/** A background read/preview must never consume a mutation's fixture response. */
export function replyToCommandOnce(target:string,value:unknown){
 const previous=vi.mocked(invoke).getMockImplementation();let used=false;
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(!used&&command===target){used=true;return value as never;}
  if(previous)return previous(command,args);
  throw Error(`Unexpected command: ${command}`);
 });
}
