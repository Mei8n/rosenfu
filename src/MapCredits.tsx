import {MAP_PROVIDERS, type MapProvider} from "./mapProviders";
/** Essential provider attribution stays visible, including standard OSM tile policy. */
export function MapCredits({background,reference,graphVisible,derivedOsmVisible=false,openFreeMapVisible=false}:{background:MapProvider|"sample";reference:{source:string;license:string}|null;graphVisible:boolean;derivedOsmVisible?:boolean;openFreeMapVisible?:boolean}) {
 const provider=background==="sample"?null:MAP_PROVIDERS[background];
 const referenceOsm=!!reference||graphVisible||derivedOsmVisible||openFreeMapVisible;
 return <div className="map-credits" aria-label="地図の出典">
  <div className="map-credits-visible">
   {openFreeMapVisible&&<><a href="https://openfreemap.org/" target="_blank" rel="noopener noreferrer">OpenFreeMap</a><a href="https://openmaptiles.org/" target="_blank" rel="noopener noreferrer">© OpenMapTiles</a></>}
   {provider&&<a href={provider.attributionUrl} target="_blank" rel="noopener noreferrer">{provider.attribution}</a>}
   {referenceOsm&&background!=="osm"&&<a href={MAP_PROVIDERS.osm.attributionUrl} target="_blank" rel="noopener noreferrer">{MAP_PROVIDERS.osm.attribution}</a>}
  </div>
  <details className="map-credits-details"><summary aria-label="地図の出典と詳細" title="地図の出典と詳細">i</summary>
   <div className="map-credits-content">
    {background==="sample"?<p>背景は独自作成の模式図です。外部タイルは使用していません。</p>:<><p>{provider!.title}のオンライン背景地図です。架空の鉄道設備を重畳表示。背景地図とは別データです。</p><p>アプリによるタイルのオフライン保存・同梱は行いません。</p></>}
    {reference&&<p>参考駅: {reference.source} / {reference.license}（背景・ゲーム駅とは別）</p>}
    {derivedOsmVisible&&<p>表示中の計画・建設済み線路にOpenStreetMap由来の形状を含みます。</p>}
    {graphVisible&&<p>参考線: OpenStreetMap / セッション限定・網羅性未保証</p>}
    {referenceOsm&&<a href={MAP_PROVIDERS.osm.attributionUrl} target="_blank" rel="noopener noreferrer">参考データの利用条件（ODbL）</a>}
   </div>
  </details>
 </div>;
}
