import {expect,test} from 'vitest';
import {constructionRoadAttachment,constructionRoadBounds,savedRoadSnapshot} from './constructionRoads';
import {parseRoadResponse} from './automaticRoadData';
const bounds={west:139.74,south:35.66,east:139.76,north:35.68};
async function fixture(){return parseRoadResponse({osm3s:{timestamp_osm_base:'2026-11-14T21:20:41Z'},elements:[{type:'way',id:'1',nodes:['1','2'],geometry:[{lon:139.75,lat:35.66},{lon:139.75,lat:35.68}],tags:{highway:'service'}}]},bounds,0,{source:'https://download.geofabrik.de/asia/japan/kanto-261114.osm.pbf',sourceSha256:'29967275fa5925a5dc224708e41869800471745d53db004b25104c6eddca52dc'});}
test('maps source evidence and coverage without expanding it to full way extents',async()=>{
 const source=await fixture(),saved=savedRoadSnapshot(source);
 expect(saved.coverage_verified).toBe(true);expect(saved.road_coverage).toEqual([bounds]);expect(saved.source_sha256).toBe(source.bundle.sourceSha256);expect(saved.roads).toEqual(source.roads);
 const unverified={...source,coverageVerified:false};expect(savedRoadSnapshot(unverified).coverage_verified).toBe(false);
 source.bundle.source.source='https://unverified.example/source';expect(savedRoadSnapshot(source).coverage_verified).toBe(false);
});
test('missing roads do not block and previously saved containing coverage is reused',async()=>{
 expect(constructionRoadAttachment(undefined,null).roads).toEqual([]);
 const source=await fixture(),saved=savedRoadSnapshot(source);
 expect(constructionRoadAttachment(source,{road_snapshots:[saved]}).roads).toEqual([]);
 const nested={...source,bounds:{west:139.745,south:35.665,east:139.755,north:35.675}};
 expect(constructionRoadAttachment(nested,{road_snapshots:[saved]}).roads).toEqual([]);
 expect(constructionRoadAttachment(source,null).roads).toHaveLength(1);
});
test('version, geometry and save size conflicts preserve old facts without blocking construction',async()=>{
 const source=await fixture(),saved=savedRoadSnapshot(source);
 expect(constructionRoadAttachment(source,{road_snapshots:[{...saved,revision:'old'}]}).reason).toContain('版');
 const conflict={...saved,road_coverage:[],roads:saved.roads.map(r=>({...r,layer:'1'}))};
 expect(constructionRoadAttachment(source,{road_snapshots:[conflict]}).reason).toContain('矛盾');
 const huge={road_snapshots:[],other:'x'.repeat(15*1024*1024)};
 expect(constructionRoadAttachment(source,huge).reason).toContain('保存容量');
});
test('construction bounds use all paths and never truncate long plans',()=>{
 expect(constructionRoadBounds([],bounds)).toBe(bounds);
 const actual=constructionRoadBounds([{line:[{longitude:139,latitude:35},{longitude:140,latitude:35}]}],bounds);
 expect(actual.west).toBeLessThan(139);expect(actual.east).toBeGreaterThan(140);expect(actual.south).toBeLessThan(actual.north);
});
test('saves tile identities independently across changed viewports without invented OSM dates',async()=>{
 const original=await fixture();
 const tile=(key:string)=>({...original,coverageVerified:false,bundle:{...original.bundle,graph:{...original.bundle.graph,dataset:{...original.bundle.graph.dataset,id:`ofm-tiles/${key}`,revision:'tile-version-42'}},source:{...original.bundle.source,source:`https://tiles.openfreemap.org/planet/version/14/${key}.pbf`,sourceDate:null}},roads:original.roads.map(r=>({...r,id:`ofm:${key}:1`}))});
 const a=tile('1/2'),b=tile('2/2');
 const first=constructionRoadAttachment({...original,tileSnapshots:[a]},null);
 expect(first.roads).toHaveLength(1);expect(first.roads[0].revision).toBe('tile-version-42');expect(first.roads[0].coverage_verified).toBe(false);
 const second=constructionRoadAttachment({...original,tileSnapshots:[a,b]},{road_snapshots:first.roads});
 expect(second.roads).toHaveLength(1);expect(second.roads[0].source_url).toBe(b.bundle.source.source);
 const changed={...a,bundle:{...a.bundle,sourceSha256:'b'.repeat(64)}};
 const conflict=constructionRoadAttachment({...original,tileSnapshots:[changed,b]},{road_snapshots:first.roads});
 expect(conflict.roads).toHaveLength(1);expect(conflict.reason).toContain('版');
});
test('rejects empty and nested tile snapshots instead of claiming coverage',async()=>{
 const source=await fixture();
 expect(constructionRoadAttachment({...source,tileSnapshots:[]},null).reason).toContain('不正');
 expect(constructionRoadAttachment({...source,tileSnapshots:[{...source,tileSnapshots:[]}]},null).roads).toEqual([]);
});
