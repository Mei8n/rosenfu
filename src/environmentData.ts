import type {GraphBounds,GraphPoint} from './referenceGraph';
export type RoadWay={coverage?:GraphBounds[];id:string;points:GraphPoint[];layer:string|null;level:string|null;bridge:string|null;tunnel:string|null};
export type WaterPolygon={coverage?:GraphBounds[];rings:GraphPoint[][]};
export type EnvironmentSnapshot={revision:string;road_coverage:GraphBounds[];roads:RoadWay[];water_coverage:GraphBounds[];water:WaterPolygon[]};
export type AutomaticCrossing={road_id:string;point:GraphPoint};
export type EnvironmentAssessment={crossings:AutomaticCrossing[];roads_ready:boolean;water_ready:boolean;unknown_road_height:boolean;ground_water:boolean;longest_water_m:number;water_limit_exceeded:boolean};
export function assessmentMessage(a:EnvironmentAssessment):string{
 if(!a.roads_ready)return '道路データ未取得：着工できません。';
 if(!a.water_ready)return '水域データ未取得：着工できません。';
 if(a.unknown_road_height)return '交差する道路の高さが不明：着工できません。';
 if(a.ground_water)return '地上線は水面を通過できません。';
 if(a.water_limit_exceeded)return '連続する水面横断が30 kmを超えています。';
 return `踏切 ${a.crossings.length}か所（自動設置）・費用未実装`;
}
export const assessmentBuildable=(a:EnvironmentAssessment)=>a.roads_ready&&a.water_ready&&!a.unknown_road_height&&!a.ground_water&&!a.water_limit_exceeded;
