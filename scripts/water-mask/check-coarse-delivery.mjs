import {loadConstructionWaterAt} from '../../src/waterModelCatalog.ts';
const commit=process.argv[2];if(!/^[a-f0-9]{40}$/.test(commit??''))throw Error('Pass published commit');
const root=`https://raw.githubusercontent.com/Mei8n/rosenfu/${commit}/data/water-japan-coarse-20261114/`;
const pin={url:root+'catalog.json',bytes:443,sha256:'2ea1581bd7dab2889627f5dfe123dceb7aa905505a92b82d02dae19199de8cda'};
const receipts=[],results=[];
const fetcher=async(...args)=>{const r=await fetch(...args);receipts.push({url:String(args[0]),status:r.status,cors:r.headers.get('access-control-allow-origin')});return r;};
for(const [name,lon,lat,state] of [['Tokyo',139.7671,35.6812,'model_land'],['Fuji',138.73,35.36,'model_land'],['Biwa',136.1,35.25,'model_water'],['Pacific',140,33,'model_water'],['Kanmon',130.95,33.96,'model_water'],['Osaka',135.5,34.7,'model_land'],['Sapporo',141.35,43.06,'model_land'],['Naha',127.68,26.21,'model_land']]){
 const result=await loadConstructionWaterAt(pin,lon,lat,{fetcher});
 if(result.lookup.state!==state||result.lookup.basis!=='coarse'||result.failure!==null)throw Error(name+' classification failed');
 results.push({name,lon,lat,result});
}
const before=receipts.length,outside=await loadConstructionWaterAt(pin,0,0,{fetcher});
if(outside.failure!=='outside_catalog_scope'||outside.lookup.state!=='unknown'||receipts.length!==before+1)throw Error('Unsupported area accepted');
console.log(JSON.stringify({pin,results,outside,receipts,limitation:'Map model examples, not surveyed ground truth or native build E2E.'},null,2));
