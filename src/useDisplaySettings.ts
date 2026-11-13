import {useRef,useState} from "react";
import {DEFAULT_DISPLAY_SETTINGS,DISPLAY_SETTINGS_KEY,parseDisplaySettings} from "./displaySettings";
import type {DisplaySettings} from "./displaySettings";
export function useDisplaySettings(){
 const [state,setState]=useState(()=>{try{const raw=localStorage.getItem(DISPLAY_SETTINGS_KEY);return {settings:raw===null?{...DEFAULT_DISPLAY_SETTINGS}:parseDisplaySettings(raw),status:raw===null?"表示設定は変更時にこの端末へ自動保存します。":"この端末の表示設定を復元しました。",error:false};}catch(cause){return {settings:{...DEFAULT_DISPLAY_SETTINGS},status:"表示設定の復元に失敗。既定値を使用中: "+String(cause),error:true};}});
 const current=useRef(state.settings);
 function update(patch:Partial<DisplaySettings>){const next={...current.current,...patch};try{parseDisplaySettings(JSON.stringify({version:1,settings:next}));}catch(cause){setState({settings:current.current,status:"表示設定の値を適用できません: "+String(cause),error:true});return;}
  current.current=next;try{localStorage.setItem(DISPLAY_SETTINGS_KEY,JSON.stringify({version:1,settings:next}));setState({settings:next,status:"表示設定をこの端末に保存しました。",error:false});}catch(cause){setState({settings:next,status:"表示設定を保存できません。このセッションのみ反映（次回は以前の設定の可能性）: "+String(cause),error:true});}
 }
 function reset(){current.current={...DEFAULT_DISPLAY_SETTINGS};try{localStorage.removeItem(DISPLAY_SETTINGS_KEY);setState({settings:{...DEFAULT_DISPLAY_SETTINGS},status:"表示設定を既定値へ戻し、端末の保存設定を削除しました。",error:false});}catch(cause){setState({settings:{...DEFAULT_DISPLAY_SETTINGS},status:"既定値に戻しましたが保存設定の削除に失敗。次回は以前の設定の可能性: "+String(cause),error:true});}}
 return {...state,update,reset};
}
