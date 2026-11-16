import {readFileSync} from 'node:fs';
import {describe,it,expect,vi} from 'vitest';
import {loadConstructionWaterAt} from './waterModelCatalog';
const dir=new URL('../data/water-japan-coarse-20261114/',import.meta.url),root='https://example.org/japan/';
const pin={url:root+'catalog.json',bytes:443,sha256:'2ea1581bd7dab2889627f5dfe123dceb7aa905505a92b82d02dae19199de8cda'};
const fetcher=()=>vi.fn(async input=>{
 const name=String(input).slice(root.length);
 if(!['catalog.json','japan-coarse.water'].includes(name))throw Error('Unexpected request');
 return new Response(new Uint8Array(readFileSync(new URL(name,dir))),{status:200});
});
describe('pinned Japan-region coarse data',()=>{
 it('classifies example land and water cells through the production catalog loader',async()=>{
  for(const [lon,lat,state] of [[139.7671,35.6812,'model_land'],[138.73,35.36,'model_land'],[136.1,35.25,'model_water'],[140,33,'model_water'],[130.95,33.96,'model_water'],[135.5,34.7,'model_land'],[141.35,43.06,'model_land'],[127.68,26.21,'model_land']]){
   const f=fetcher(),r=await loadConstructionWaterAt(pin,lon,lat,{fetcher:f});
   expect(r.lookup).toMatchObject({state,basis:'coarse'});expect(r.failure).toBeNull();expect(f).toHaveBeenCalledTimes(2);
  }
 });
 it('does not interpret unsupported zeroes outside the generated region as land',async()=>{
  for(const [lon,lat] of [[0,0],[154,30],[130,46],[121.99,30]]){
   const f=fetcher(),r=await loadConstructionWaterAt(pin,lon,lat,{fetcher:f});
   expect(r.failure).toBe('outside_catalog_scope');expect(r.lookup.state).toBe('unknown');expect(f).toHaveBeenCalledTimes(1);
  }
 });
 it('keeps a missing registered coarse object unavailable instead of granting absence fallback',async()=>{
  const good=fetcher(),f=vi.fn(async(input,options)=>String(input).endsWith('.water')?new Response(null,{status:404}):good(input,options));
  const r=await loadConstructionWaterAt(pin,139.7671,35.6812,{fetcher:f});
  expect(r.failure).toBe('coarse_unavailable');expect(r.lookup.state).toBe('unknown');expect(f).toHaveBeenCalledTimes(3);
 });
});
