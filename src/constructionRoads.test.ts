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
 const tile=(key:string)=>({...original,coverageVerified:false,bundle:{...original.bundle,graph:{...original.bundle.graph,dataset:{...original.bundle.graph.dataset,id:`ofm-tiles/${key}`,revision:'tile_version_42'}},source:{...original.bundle.source,source:`https://tiles.openfreemap.org/planet/tile_version_42/14/${key}.pbf`,sourceDate:null}},roads:original.roads.map(r=>({...r,id:`ofm:${key}:1`}))});
 const a=tile('1/2'),b=tile('2/2');
 const first=constructionRoadAttachment({...original,tileSnapshots:[a]},null);
 expect(first.roads).toHaveLength(1);expect(first.roads[0].revision).toBe('tile_version_42');expect(first.roads[0].coverage_verified).toBe(false);
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
test('classification conflicts and invalid metadata preserve previous roads without blocking normal build',async()=>{
 const original=await fixture();const source={...original,roads:original.roads.map(r=>({...r,classification:{class:'path',subclass:'steps',access:null}}))};
 expect(savedRoadSnapshot(source).roads[0].classification?.subclass).toBe('steps');
 const changed={...source,roads:source.roads.map(r=>({...r,classification:{...r.classification,class:'minor'}}))};
 expect(constructionRoadAttachment(changed,{road_snapshots:[{...savedRoadSnapshot(source),road_coverage:[]}]}).reason).toContain('矛盾');
 const invalid={...source,roads:source.roads.map(r=>({...r,classification:{...r.classification,subclass:'あ'.repeat(40)}}))};
 expect(constructionRoadAttachment(invalid,null).reason).toContain('未対応');
});
test('keeps raw one-way evidence and treats absent/null as the same unknown state',async()=>{
 const original=await fixture();const classification={class:'service',subclass:null,access:null};
 const source={...original,roads:original.roads.map(r=>({...r,classification}))};
 const saved={...savedRoadSnapshot(source),road_coverage:[]};
 const unknown={...source,roads:source.roads.map(r=>({...r,classification:{...classification,oneway:null}}))};
 expect(constructionRoadAttachment(unknown,{road_snapshots:[saved]}).reason).not.toContain('矛盾');
 const known={...source,roads:source.roads.map(r=>({...r,classification:{...classification,oneway:'-1'}}))};
 expect(savedRoadSnapshot(known).roads[0].classification?.oneway).toBe('-1');
 expect(constructionRoadAttachment(known,{road_snapshots:[saved]}).reason).toContain('矛盾');
});

test('does not mix later tile releases with previously saved immutable road evidence',async()=>{
 const base=await fixture();
 const make=(rev:string)=>({...base,coverageVerified:false,bundle:{...base.bundle,graph:{...base.bundle.graph,dataset:{...base.bundle.graph.dataset,id:`ofm-tiles/${rev}`,revision:rev}},source:{...base.bundle.source,source:`https://tiles.openfreemap.org/planet/${rev}/14/14552/6451.pbf`,sourceDate:null}}});
 const old=make('old'),next=make('new'),saved=savedRoadSnapshot(old);
 const result=constructionRoadAttachment({...next,tileSnapshots:[next]},{road_snapshots:[saved]});
 expect(result.roads).toEqual([]);expect(result.reason).toContain('提供版');expect(saved.revision).toBe('old');
});
