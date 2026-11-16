import type {GeoPoint} from './constructionPlans';
/** Only a completed validation can reject a placement. Transport failures and
 * pending validation are not evidence that the geometry is unbuildable. */
export function constructionValidationError(value:unknown):string|null{
 if(!value||typeof value!=='object'||!('validationError' in value))return null;
 const error=value.validationError;
 return typeof error==='string'&&error.length>0?error:null;
}
export function matchingPlacementError(error:string|null,active:boolean,preview:GeoPoint[],candidate:GeoPoint[]):string|null{
 if(!error||!active||preview.length!==candidate.length)return null;
 return preview.every((point,index)=>point.latitude===candidate[index].latitude&&point.longitude===candidate[index].longitude)?error:null;
}
