import {assertNever} from './assertNever';
import type {WorkspaceTab} from './WorkspaceTabs';

export type EditorTool='stations'|'tracks'|'routes'|'services'|'company'|'station-management'|'delete'|'select'|'inactive';
export type Status='loading'|'preview'|'ready'|'error';
export type MapMode='move'|'place'|'draw';

export function toolForTab(tab:WorkspaceTab):EditorTool {
 switch(tab){
  case 'construction':return 'tracks';
  case 'stations':return 'station-management';
  case 'settings':return 'company';
  case 'routes':return 'routes';
  case 'services':return 'services';
  case 'finance':case 'city':case 'saves':return 'inactive';
  default:return assertNever(tab);
 }
}
export function mapModeForTool(tool:EditorTool):MapMode {
 switch(tool){
  case 'tracks':return 'draw';
  case 'stations':return 'place';
  case 'routes':case 'services':case 'company':case 'station-management':
  case 'delete':case 'select':case 'inactive':return 'move';
  default:return assertNever(tool);
 }
}
/** Starting/resuming a construction path draws even while another panel is selected. */
export function constructionMapMode(tool:EditorTool):MapMode {
 switch(tool){
  case 'stations':return 'place';
  case 'tracks':case 'routes':case 'services':case 'company':case 'station-management':
  case 'delete':case 'select':case 'inactive':return 'draw';
  default:return assertNever(tool);
 }
}
export function mapInstruction(status:Status,mode:MapMode):string {
 switch(status){
  case 'loading':case 'preview':case 'error':return 'ドラッグで地図を移動 / 建設はデスクトップ版';
  case 'ready':break;
  default:return assertNever(status);
 }
 switch(mode){
  case 'draw':return 'クリックで線形点を追加 / 最後の点をもう一度クリックで計画終了';
  case 'place':return '線路にカーソルを重ねてクリックで駅を配置';
  case 'move':return 'ドラッグで移動 · ホイールで拡大縮小';
  default:return assertNever(mode);
 }
}
