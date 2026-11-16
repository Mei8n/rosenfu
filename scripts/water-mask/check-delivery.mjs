// Node 24: actual HTTPS -> pinned catalog -> pinned raster -> production decoder.
import {fetchWaterFile,fetchFineWaterMask} from '../../src/waterMaskModel.ts';
const commit=process.argv[2];
if(!/^[0-9a-f]{40}$/.test(commit ?? ''))throw Error('Pass the published 40-digit commit');
const root=`https://raw.githubusercontent.com/Mei8n/rosenfu/${commit}/data/water-mask-trial-20261114/`;
const receipts=[];
const observedFetch=async (...args)=>{
 const response=await fetch(...args);
 receipts.push({url:String(args[0]),status:response.status,range:response.headers.get('content-range'),cors:response.headers.get('access-control-allow-origin')});
 return response;
};
const signal=AbortSignal.timeout(60000);
const raw=await fetchWaterFile(root+'catalog.json',1579,'07ec91cdbfeda5f0f2a8157fc303bf9824b5da084bcdc924c45f00ec92dc5bab',signal,observedFetch);
const catalog=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(raw));
if(catalog.format!=='rosenfu-observed-water-masks'||catalog.version!==1||catalog.entries.length!==3)throw Error('Wrong catalog');
const results=[];
for(const entry of catalog.entries){
 if(!['fuji','kanmon','takamatsu'].includes(entry.key)||entry.payload.file!==entry.key+'/water.bin'||entry.spec.coverage!=='observations_only')throw Error('Unexpected trial entry');
 const mask=await fetchFineWaterMask(entry.spec,root+entry.payload.file,entry.payload.bytes,entry.payload.sha256,signal,observedFetch);
 const s=mask.spec,rad=Math.PI/180;let water=0,unknown=0;
 for(let row=0;row<s.height;row++)for(let col=0;col<s.width;col++){
  const lon=s.originLongitude+(s.x0+(col+.5)*s.cellM)/(6371008.8*Math.cos(s.originLatitude*rad))/rad;
  const lat=s.originLatitude+(s.y0+(row+.5)*s.cellM)/6371008.8/rad;
  const state=mask.at(lon,lat);if(state==='model_water')water++;else if(state===null)unknown++;else throw Error('Unexpected land');
 }
 if(water!==entry.water_cells||unknown!==entry.unknown_cells)throw Error('Cell count mismatch');
 results.push({key:entry.key,revision:s.revision,water,unknown,land:0});
}
console.log(JSON.stringify({root,receipts,results,limitation:'Three observation windows only; Node HTTPS, not WebView or construction E2E.'},null,2));
