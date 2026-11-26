import type {GeoPoint} from './alignment';

/** Resolve only a canonical end of an alignment in the current world index. */
export function resolveSeamEndpoint<T extends {points:GeoPoint[]}>(alignments:ReadonlyMap<string,T>,key:string):{key:string;alignment:T;endpoint:'start'|'end';point:GeoPoint}|undefined{
 const separator=key.lastIndexOf(':');
 if(separator<0)return undefined;
 const endpoint=key.slice(separator+1);
 if(endpoint!=='start'&&endpoint!=='end')return undefined;
 const alignment=alignments.get(key.slice(0,separator));
 if(!alignment)return undefined;
 return {key,alignment,endpoint,point:endpoint==='start'?alignment.points[0]:alignment.points.at(-1)!};
}
