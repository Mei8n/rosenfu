/** Raster providers are display-only; world geometry is provider independent. */
export const MAP_PROVIDERS = {
 osm: { title: "OpenStreetMap", tileUrl: (id: string) => "https://tile.openstreetmap.org/"+id+".png", attribution: "© OpenStreetMap contributors", attributionUrl: "https://www.openstreetmap.org/copyright" },
 gsi: { title: "地理院タイル", tileUrl: (id: string) => "https://cyberjapandata.gsi.go.jp/xyz/pale/"+id+".png", attribution: "出典: 国土地理院 地理院タイル（淡色地図）", attributionUrl: "https://maps.gsi.go.jp/development/ichiran.html" }
} as const;
export type MapProvider = keyof typeof MAP_PROVIDERS;
