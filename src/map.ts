import { MAP_PROVIDERS } from "./mapProviders";
import type { MapProvider } from "./mapProviders";
/** Display-only Web Mercator camera. Geographic world data never moves with the camera. */
export const MAP_BOUNDS = { west: 122, east: 154, south: 20, north: 46 } as const;
export const MAP_VIEWBOX = { width: 1000, height: 600 } as const;
export const MIN_ZOOM = 4;
// Editing magnification is independent of the raster providers’ native detail ceilings.
export const MAX_ZOOM = 20;
export const MAX_TILES = 128;
const MAX_TILE_ZOOM = { osm: 19, gsi: 17 } as const;
export const GSI_ATTRIBUTION_URL = "https://maps.gsi.go.jp/development/ichiran.html";
export type Camera = { latitude: number; longitude: number; zoom: number };
export const DEFAULT_CAMERA: Camera = { latitude: 35.67504, longitude: 139.75, zoom: 12 };
const mercatorY = (latitude: number) => (1 - Math.asinh(Math.tan(latitude * Math.PI / 180)) / Math.PI) / 2;
const worldX = (longitude: number) => (longitude + 180) / 360;
const latitudeAt = (y: number) => Math.atan(Math.sinh(Math.PI * (1 - 2 * y))) * 180 / Math.PI;
const clamp = (value: number, minimum: number, maximum: number) => Math.min(maximum, Math.max(minimum, value));
const scaleFor = (zoom: number) => 256 * 2 ** zoom;
function checkCamera(camera: Camera) {
  if (![camera.latitude, camera.longitude, camera.zoom].every(Number.isFinite) || camera.zoom < MIN_ZOOM || camera.zoom > MAX_ZOOM || camera.latitude < MAP_BOUNDS.south || camera.latitude > MAP_BOUNDS.north || camera.longitude < MAP_BOUNDS.west || camera.longitude > MAP_BOUNDS.east) throw new RangeError("Camera outside supported viewing rectangle or zoom range");
}
/** At regional zoom keep the viewport inside Japan; at overview zoom center the
 * viewing rectangle, while keeping the map inside the finite Mercator world. */
export function cameraAt(latitude: number, longitude: number, zoom = DEFAULT_CAMERA.zoom): Camera {
  checkCamera({ latitude, longitude, zoom });
  const scale = scaleFor(zoom);
  const fit = (value:number, low:number, high:number, half:number) =>
    clamp(low + half <= high - half ? clamp(value, low + half, high - half) : (low + high) / 2, half, 1 - half);
  const x = fit(worldX(longitude), worldX(MAP_BOUNDS.west), worldX(MAP_BOUNDS.east), MAP_VIEWBOX.width / (2 * scale));
  const y = fit(mercatorY(latitude), mercatorY(MAP_BOUNDS.north), mercatorY(MAP_BOUNDS.south), MAP_VIEWBOX.height / (2 * scale));
  return { latitude: latitudeAt(y), longitude: x * 360 - 180, zoom };
}
export function project(latitude: number, longitude: number, camera = DEFAULT_CAMERA) {
  checkCamera(camera);
  if (!Number.isFinite(latitude) || !Number.isFinite(longitude) || Math.abs(latitude) > 85.05112878 || Math.abs(longitude) > 180) throw new RangeError("Coordinate outside Web Mercator range");
  const scale = scaleFor(camera.zoom);
  return { x: (worldX(longitude) - worldX(camera.longitude)) * scale + MAP_VIEWBOX.width / 2, y: (mercatorY(latitude) - mercatorY(camera.latitude)) * scale + MAP_VIEWBOX.height / 2 };
}
export function unproject(x: number, y: number, camera = DEFAULT_CAMERA) {
  checkCamera(camera);
  if (![x,y].every(Number.isFinite) || x < 0 || y < 0 || x > MAP_VIEWBOX.width || y > MAP_VIEWBOX.height) throw new RangeError("Point outside map viewbox");
  const scale = scaleFor(camera.zoom);
  return { latitude: latitudeAt(mercatorY(camera.latitude) + (y - MAP_VIEWBOX.height / 2) / scale), longitude: (worldX(camera.longitude) + (x - MAP_VIEWBOX.width / 2) / scale) * 360 - 180 };
}
export function panCamera(camera: Camera, dx: number, dy: number): Camera {
  checkCamera(camera);
  if (![dx,dy].every(Number.isFinite)) throw new RangeError("Invalid pan");
  const scale = scaleFor(camera.zoom);
  return cameraAt(clamp(latitudeAt(mercatorY(camera.latitude) + dy / scale), MAP_BOUNDS.south, MAP_BOUNDS.north), clamp(camera.longitude + dx / scale * 360, MAP_BOUNDS.west, MAP_BOUNDS.east), camera.zoom);
}
export function zoomCamera(camera: Camera, delta: number): Camera {
  if (!Number.isFinite(delta)) throw new RangeError("Invalid zoom delta");
  return cameraAt(camera.latitude, camera.longitude, clamp(camera.zoom + delta, MIN_ZOOM, MAX_ZOOM));
}
/** Keep a logical screen point over the same geographic position while zooming.
 * At geographic limits cameraAt may clamp, which intentionally takes precedence.
 */
export function zoomCameraAt(camera:Camera,zoom:number,anchor:{x:number;y:number}):Camera {
  checkCamera(camera);
  if(!Number.isFinite(zoom))throw new RangeError("Invalid zoom");
  const point=unproject(anchor.x,anchor.y,camera);
  const nextZoom=clamp(zoom,MIN_ZOOM,MAX_ZOOM),scale=scaleFor(nextZoom);
  const x=worldX(point.longitude)-(anchor.x-MAP_VIEWBOX.width/2)/scale;
  const y=mercatorY(point.latitude)-(anchor.y-MAP_VIEWBOX.height/2)/scale;
  return cameraAt(clamp(latitudeAt(y),MAP_BOUNDS.south,MAP_BOUNDS.north),clamp(x*360-180,MAP_BOUNDS.west,MAP_BOUNDS.east),nextZoom);
}
export function viewBounds(camera = DEFAULT_CAMERA, size: { width: number; height: number } = MAP_VIEWBOX) {
  const visible = mapViewport(size.width, size.height);
  const nw = unproject(visible.x,visible.y,camera), se = unproject(visible.x+visible.width,visible.y+visible.height,camera);
  return { west: nw.longitude, east: se.longitude, north: nw.latitude, south: se.latitude };
}
/** Visible logical area for SVG xMidYMid slice. Fill the window without stretching geography. */
export function mapViewport(width: number, height: number) {
  if (![width,height].every(Number.isFinite) || width <= 0 || height <= 0) throw new RangeError("Invalid viewport size");
  const scale = Math.max(width / MAP_VIEWBOX.width, height / MAP_VIEWBOX.height);
  const visibleWidth = width / scale, visibleHeight = height / scale;
  return { x:(MAP_VIEWBOX.width-visibleWidth)/2, y:(MAP_VIEWBOX.height-visibleHeight)/2, width:visibleWidth, height:visibleHeight, scale };
}
/** Convert client coordinates through the same uniform crop used to render the map. */
export function coordinatesFromClient(clientX: number, clientY: number, rect: { left: number; top: number; width: number; height: number }, camera = DEFAULT_CAMERA) {
  if (![clientX, clientY, rect.left, rect.top, rect.width, rect.height].every(Number.isFinite) || rect.width <= 0 || rect.height <= 0) return null;
  if (clientX < rect.left || clientX > rect.left + rect.width || clientY < rect.top || clientY > rect.top + rect.height) return null;
  const { scale } = mapViewport(rect.width, rect.height);
  const x = (clientX - rect.left - (rect.width - MAP_VIEWBOX.width * scale) / 2) / scale;
  const y = (clientY - rect.top - (rect.height - MAP_VIEWBOX.height * scale) / 2) / scale;
  if (x < 0 || y < 0 || x > MAP_VIEWBOX.width || y > MAP_VIEWBOX.height) return null;
  return unproject(x, y, camera);
}
export type MapTile = { id: string; url: string; x: number; y: number; width: number; height: number };
/** Reposition a previously requested tile while a new camera settles; URL/id remain unchanged. */
export function positionTile(tile: MapTile, camera: Camera): MapTile {
  checkCamera(camera);
  const [zoom,x,y] = tile.id.split("/").map(Number);
  const count = 2 ** zoom, scale = scaleFor(camera.zoom);
  return { ...tile, x: (x / count - worldX(camera.longitude)) * scale + MAP_VIEWBOX.width / 2, y: (y / count - mercatorY(camera.latitude)) * scale + MAP_VIEWBOX.height / 2, width: scale / count, height: scale / count };
}
/** Enumerate only visible tiles at screen resolution, never prefetch or persist them.
 * Camera coordinates stay unchanged; finer raster tiles are drawn at smaller logical sizes.
 * Bound requests on extreme DPI/screens by reducing detail rather than dropping coverage.
 */
export function visibleTiles(camera = DEFAULT_CAMERA, provider: MapProvider = "gsi", size: {width:number;height:number} = MAP_VIEWBOX, pixelRatio = 1): MapTile[] {
  checkCamera(camera);
  if (!Number.isFinite(pixelRatio) || pixelRatio <= 0) throw new RangeError("Invalid pixel ratio");
  const viewport = mapViewport(size.width,size.height);
  const scale = scaleFor(camera.zoom);
  const left = worldX(camera.longitude) * scale - MAP_VIEWBOX.width / 2;
  const top = mercatorY(camera.latitude) * scale - MAP_VIEWBOX.height / 2;
  const extraZoom = Math.max(0, Math.min(2, Math.ceil(Math.log2(viewport.scale * pixelRatio))));
  let tileZoom = Math.min(MAX_TILE_ZOOM[provider], Math.ceil(camera.zoom + extraZoom));
  let width: number, x0: number, x1: number, y0: number, y1: number;
  do {
    width = 256 / 2 ** (tileZoom - camera.zoom);
    x0 = Math.floor((left+viewport.x) / width); x1 = Math.ceil((left+viewport.x+viewport.width) / width);
    y0 = Math.floor((top+viewport.y) / width); y1 = Math.ceil((top+viewport.y+viewport.height) / width);
    if ((x1-x0)*(y1-y0) <= MAX_TILES) break;
    tileZoom--;
  } while (tileZoom >= Math.floor(camera.zoom));
  const count = 2 ** tileZoom;
  const tiles: MapTile[] = [];
  for (let x=x0; x<x1; x++) for (let y=y0; y<y1; y++) {
    if (x<0 || y<0 || x>=count || y>=count) continue;
    const id = `${tileZoom}/${x}/${y}`;
    tiles.push({ id, url: MAP_PROVIDERS[provider].tileUrl(id), x: x*width-left, y: y*width-top, width, height: width });
  }
  return tiles;
}
export const GSI_TILES: readonly MapTile[] = visibleTiles();
