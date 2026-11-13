export type DisplaySettings={background:"sample"|"gsi"|"osm";theme:"light"|"dark";lineWidth:number;contrast:number;panelWidth:number;textSize:number};
export const DEFAULT_DISPLAY_SETTINGS:DisplaySettings={background:"osm",theme:"light",lineWidth:3,contrast:100,panelWidth:300,textSize:12};
export const DISPLAY_SETTINGS_KEY="rosenfu.display-settings";
export function parseDisplaySettings(text:string):DisplaySettings {
 if(text.length>4096 || new TextEncoder().encode(text).byteLength>4096)throw Error("表示設定が大きすぎます。");const root:unknown=JSON.parse(text);
 if(!root||typeof root!=="object"||Array.isArray(root))throw Error("表示設定の形式が不正です。");
 const r=root as Record<string,unknown>;if(r.version!==undefined&&r.version!==0&&r.version!==1)throw Error("未対応の表示設定の版です。");
 const raw=r.version===1?r.settings:r.settings??r;
 if(!raw||typeof raw!=="object"||Array.isArray(raw))throw Error("表示設定の内容が不正です。");
 const p=raw as Record<string,unknown>,settings={...DEFAULT_DISPLAY_SETTINGS};
 if(p.theme!==undefined){if(p.theme!=="light"&&p.theme!=="dark")throw Error("配色が不正です。");settings.theme=p.theme;}
 if(p.background!==undefined){if(p.background!=="sample"&&p.background!=="gsi"&&p.background!=="osm")throw Error("背景設定が不正です。");settings.background=p.background;}
 const ranges={lineWidth:[2,8],contrast:[70,140],panelWidth:[260,400],textSize:[11,16]} as const;
 for(const key of Object.keys(ranges) as (keyof typeof ranges)[]){const v=p[key];if(v===undefined)continue;const [min,max]=ranges[key];if(typeof v!=="number"||!Number.isInteger(v)||v<min||v>max)throw Error("表示設定の範囲が不正です: "+key);settings[key]=v;}
 return settings;
}
