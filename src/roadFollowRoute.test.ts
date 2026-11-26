import {expect,test} from 'vitest';
import {roadFollowRoute} from './roadFollowRoute';
import type {ReferenceGraphBundle} from './referenceGraphBundle';
function fixture(){
 const nodes=[{osmId:'1',longitude:135,latitude:35},{osmId:'2',longitude:135.01,latitude:35},{osmId:'3',longitude:135.01,latitude:35.01},{osmId:'4',longitude:135.02,latitude:35}];
 const edges=[['1','1','2'],['2','2','3'],['3','2','4']].map(([id,from,to])=>({id,osmWayId:id,segmentIndex:0,from,to,mode:'road' as const,layer:0,level:null,bridge:false,tunnel:false}));
 const bundle:ReferenceGraphBundle={graph:{format:'rosenfu-reference-graph',version:1,dataset:{id:'t',revision:'r',source:'t',license:'t',bounds:{west:134,east:136,south:34,north:36}},nodes,edges},roadWays:Object.fromEntries(edges.map(e=>[e.id,{highway:'residential',access:null,motorVehicle:null,structureUnsupported:false}])),graphSha256:'a'.repeat(64),sourceSha256:'b'.repeat(64),source:{source:'t',sourceDate:null,license:'t',attribution:'t'}};
 const pick=(edgeId:string,fraction:number)=>({datasetId:'t',revision:'r',edgeId,fraction});return {bundle,pick};
}
test('traverses a road junction across ways',()=>{const {bundle,pick}=fixture();const path=roadFollowRoute(bundle,pick('1',.5),pick('2',.5));expect(path.points).toEqual([{longitude:135.005,latitude:35},{longitude:135.01,latitude:35},{longitude:135.01,latitude:35.004999999999995}]);expect(path.lengthM).toBeGreaterThan(900);});
test('same edge uses the interval between interior picks, not a detour via endpoints',()=>{const {bundle,pick}=fixture();const result=roadFollowRoute(bundle,pick('1',.2),pick('1',.8));expect(result.points).toHaveLength(2);expect(result.lengthM).toBeLessThan(600);});
test('joins tile-local vertices only with exact equal ground coordinates',()=>{const {bundle,pick}=fixture();bundle.graph.nodes.push({...bundle.graph.nodes[1],osmId:'5'});bundle.graph.edges[1].from='5';expect(roadFollowRoute(bundle,pick('1',.5),pick('2',.5)).points).toHaveLength(3);bundle.graph.edges[1].layer=null;expect(()=>roadFollowRoute(bundle,pick('1',.5),pick('2',.5))).toThrow('つながる道路');});
test('does not follow bridges or connect disconnected road crossings',()=>{const {bundle,pick}=fixture();bundle.graph.edges[1].bridge=true;expect(()=>roadFollowRoute(bundle,pick('1',.5),pick('2',.5))).toThrow('選択位置');});

test('street geometry can be followed against a motor one-way without assuming any tram authority',()=>{
 const {bundle,pick}=fixture();
 const motorRoad={...bundle.roadWays!['1'],oneway:'yes'};
 bundle.roadWays!['1']=motorRoad;
 const route=roadFollowRoute(bundle,pick('1',.85),pick('1',.15));
 expect(route.points).toHaveLength(2);
 expect(route.points[0].longitude).toBeGreaterThan(route.points[1].longitude);
});
test('a geometric crossing without a shared vertex does not connect roads',()=>{
 const {bundle,pick}=fixture();
 bundle.graph.nodes.push({osmId:'5',longitude:135.005,latitude:34.995},{osmId:'6',longitude:135.005,latitude:35.005});
 const edge={...bundle.graph.edges[0],id:'4',osmWayId:'4',from:'5',to:'6'};
 bundle.graph.edges.push(edge);bundle.roadWays!['4']=bundle.roadWays!['1'];
 expect(()=>roadFollowRoute(bundle,pick('1',.2),pick('4',.8))).toThrow('つながる道路');
});
