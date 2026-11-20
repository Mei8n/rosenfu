import type {SectionInfrastructure} from './sectionInfrastructure';
type InfrastructureLevel=SectionInfrastructure['level'];
/** Resolve only absent legacy editor metadata. Explicit arrays must match anchors. */
export function anchorHeights(count:number,levels:InfrastructureLevel[]|undefined,start:InfrastructureLevel|undefined,end:InfrastructureLevel):InfrastructureLevel[]{
 if(levels){if(levels.length!==count)throw new Error('アンカーの高さと点の数が一致しません');return [...levels];}
 return Array.from({length:count},(_,index)=>index===0?start??end:end);
}
export function changeAnchorHeight(levels:InfrastructureLevel[],index:number,level:InfrastructureLevel):InfrastructureLevel[]{
 if(!Number.isInteger(index)||index<0||index>=levels.length)throw new Error('高さを変更するアンカーがありません');
 return levels.map((value,i)=>i===index?level:value);
}
