declare const process:{env:Record<string,string|undefined>};
import {it,expect} from 'vitest';
import {createRoadRepository} from './automaticRoadData';
import {createWaterRepository} from './automaticWaterData';
it.skipIf(process.env.ROSENFU_LIVE!=='1')('checks small live OSM and GSI reference samples',async()=>{
 const bounds={west:139.745,south:35.675,east:139.75,north:35.68};
 const roads=await createRoadRepository().load(bounds).catch(error=>({error:String(error)}));
 const water=await createWaterRepository().load(bounds);expect(water.coverage.length).toBeGreaterThan(0);
 console.log({roads,waterPolygons:water.water.length,tiles:water.coverage.length});
},120000);
