import { expect, test } from "vitest";
import { MAP_BOUNDS, MAP_VIEWBOX, DEFAULT_CAMERA, MIN_ZOOM, MAX_ZOOM, MAX_TILES, cameraAt, panCamera, zoomCamera, viewBounds, project, unproject, coordinatesFromClient, visibleTiles, positionTile, mapViewport, zoomCameraAt } from "./map";

test("camera center and geographic roundtrips remain accurate across Japan and all supported zooms", () => {
  for (const [latitude,longitude] of [[35.67504,139.75],[43.06,141.35],[26.21,127.68],[34.69,135.50]]) {
    for(let zoom=MIN_ZOOM; zoom<=MAX_ZOOM; zoom++) {
      const camera = cameraAt(latitude,longitude,zoom);
      const center = project(camera.latitude,camera.longitude,camera);
      expect(center.x).toBeCloseTo(MAP_VIEWBOX.width/2,8);
      expect(center.y).toBeCloseTo(MAP_VIEWBOX.height/2,8);
      const point = project(camera.latitude+0.0001,camera.longitude+0.0001,camera);
      const restored = unproject(point.x,point.y,camera);
      expect(restored.latitude).toBeCloseTo(camera.latitude+0.0001,9);
      expect(restored.longitude).toBeCloseTo(camera.longitude+0.0001,9);
    }
  }
});

test("pan and zoom keep the whole viewport inside viewing limits without changing coordinate data", () => {
  const station = Object.freeze({ latitude:35.681236, longitude:139.767125 });
  let camera = DEFAULT_CAMERA;
  const before = project(station.latitude,station.longitude,camera);
  camera = panCamera(camera,300,-180);
  expect(project(station.latitude,station.longitude,camera).x).toBeCloseTo(before.x-300,8);
  expect(project(station.latitude,station.longitude,camera).y).toBeCloseTo(before.y+180,8);
  expect(station).toEqual({ latitude:35.681236, longitude:139.767125 });
  expect(zoomCamera(DEFAULT_CAMERA,-100).zoom).toBe(MIN_ZOOM);
  expect(zoomCamera(cameraAt(35,139,MAX_ZOOM),1).zoom).toBe(MAX_ZOOM);
  for(const latitude of [MAP_BOUNDS.south,MAP_BOUNDS.north]) for(const longitude of [MAP_BOUNDS.west,MAP_BOUNDS.east]) {
    const edge = panCamera(cameraAt(latitude,longitude), longitude===MAP_BOUNDS.west?-1e9:1e9, latitude===MAP_BOUNDS.north?-1e9:1e9);
    const bounds = viewBounds(edge);
    expect(bounds.west).toBeGreaterThanOrEqual(MAP_BOUNDS.west-1e-9);
    expect(bounds.east).toBeLessThanOrEqual(MAP_BOUNDS.east+1e-9);
    expect(bounds.south).toBeGreaterThanOrEqual(MAP_BOUNDS.south-1e-9);
    expect(bounds.north).toBeLessThanOrEqual(MAP_BOUNDS.north+1e-9);
  }
});

test("client conversion fills wide and tall viewports and rejects coordinates outside them", () => {
  const camera = cameraAt(43.06,141.35,15);
  for(const rect of [{left:10,top:20,width:1000,height:500},{left:10,top:20,width:400,height:1000}]) {
    const result = coordinatesFromClient(rect.left+rect.width/2,rect.top+rect.height/2,rect,camera);
    expect(result?.latitude).toBeCloseTo(camera.latitude,9);
    expect(result?.longitude).toBeCloseTo(camera.longitude,9);
    expect(coordinatesFromClient(rect.left,rect.top,rect,camera)).not.toBeNull();
    expect(coordinatesFromClient(rect.left-1,rect.top,rect,camera)).toBeNull();
    expect(coordinatesFromClient(rect.left+rect.width+1,rect.top,rect,camera)).toBeNull();
  }
  expect(coordinatesFromClient(0,0,{left:0,top:0,width:0,height:0})).toBeNull();
  expect(coordinatesFromClient(NaN,0,{left:0,top:0,width:100,height:100})).toBeNull();
});

test("tile enumeration contains only visible approved pale tiles with bounded count and exact coverage", () => {
  for(const [lat,lon] of [[20,122],[46,154],[35.67504,139.75],[43.06,141.35]]) for(let zoom=12;zoom<=MAX_ZOOM;zoom++) {
    const tiles = visibleTiles(cameraAt(lat,lon,zoom));
    expect(tiles.length).toBeGreaterThan(0); expect(tiles.length).toBeLessThanOrEqual(MAX_TILES);
    expect(new Set(tiles.map(tile=>tile.id)).size).toBe(tiles.length);
    for(const tile of tiles) {
      expect(tile.url).toMatch(/^https:\/\/cyberjapandata\.gsi\.go\.jp\/xyz\/pale\/(12|13|14|15|16|17)\/\d+\/\d+\.png$/);
      expect(tile.width).toBe(tile.height);
      expect(tile.x).toBeLessThan(MAP_VIEWBOX.width); expect(tile.x+tile.width).toBeGreaterThan(0);
      expect(tile.y).toBeLessThan(MAP_VIEWBOX.height); expect(tile.y+tile.height).toBeGreaterThan(0);
    }
    for(const x of [0,MAP_VIEWBOX.width/2,MAP_VIEWBOX.width-0.01]) for(const y of [0,MAP_VIEWBOX.height/2,MAP_VIEWBOX.height-0.01]) {
      expect(tiles.some(tile=>tile.x<=x && tile.x+tile.width>x && tile.y<=y && tile.y+tile.height>y)).toBe(true);
    }
  }
});

test("previous tiles remain georeferenced while waiting for a debounced camera request", () => {
  const tile = visibleTiles(DEFAULT_CAMERA)[0];
  const camera = panCamera(DEFAULT_CAMERA,100,50);
  const moved = positionTile(tile,camera);
  expect(moved.id).toBe(tile.id); expect(moved.url).toBe(tile.url);
  expect(moved.x).toBeCloseTo(tile.x-100,8); expect(moved.y).toBeCloseTo(tile.y-50,8);
  const zoomed = positionTile(tile,zoomCamera(DEFAULT_CAMERA,1));
  expect(zoomed.width).toBe(tile.width*2);
  expect(zoomed.x).toBeCloseTo((tile.x-MAP_VIEWBOX.width/2)*2+MAP_VIEWBOX.width/2,8);
});

test("invalid cameras and geographic inputs fail instead of producing requests", () => {
  for(const args of [[NaN,139,12],[19,139,12],[47,139,12],[35,121,12],[35,155,12],[35,139,MIN_ZOOM-1],[35,139,MAX_ZOOM+1]]) expect(()=>cameraAt(...args as [number,number,number])).toThrow();
  expect(()=>project(NaN,139)).toThrow(); expect(()=>project(90,139)).toThrow();
  expect(()=>project(35,181)).toThrow(); expect(()=>unproject(-1,0)).toThrow();
});

test("cropped full-window rendering and coordinate conversion share an isotropic scale", () => {
  for (const size of [{width:1884,height:934},{width:400,height:1000},{width:960,height:558}]) {
    const viewport=mapViewport(size.width,size.height);
    expect(viewport.width*viewport.scale).toBeCloseTo(size.width,8);
    expect(viewport.height*viewport.scale).toBeCloseTo(size.height,8);
    const rect={left:17,top:31,...size};
    for (const [fx,fy] of [[0,0],[.01,.5],[.5,.5],[.99,.5],[1,1]]) {
      const point=coordinatesFromClient(rect.left+fx*size.width,rect.top+fy*size.height,rect,DEFAULT_CAMERA)!;
      expect(point).not.toBeNull();
      const projected=project(point.latitude,point.longitude,DEFAULT_CAMERA);
      expect((projected.x-viewport.x)*viewport.scale).toBeCloseTo(fx*size.width,6);
      expect((projected.y-viewport.y)*viewport.scale).toBeCloseTo(fy*size.height,6);
    }
    const bounds=viewBounds(DEFAULT_CAMERA,size);
    const nw=coordinatesFromClient(rect.left,rect.top,rect,DEFAULT_CAMERA)!;
    expect(bounds.north).toBeCloseTo(nw.latitude,8);
    expect(bounds.west).toBeCloseTo(nw.longitude,8);
  }
});

test("OSM density follows screen pixels without changing camera or geography",()=>{
 const low=visibleTiles(DEFAULT_CAMERA,"osm");
 const high=visibleTiles(DEFAULT_CAMERA,"osm",{width:1920,height:1080});
 expect(low.every(t=>t.id.startsWith("12/"))).toBe(true);
 expect(high.every(t=>t.id.startsWith("13/") && t.width===128)).toBe(true);
 for(const tile of high){
   expect(tile.url).toBe("https://tile.openstreetmap.org/"+tile.id+".png");
   const positioned=positionTile(tile,DEFAULT_CAMERA);
   expect(positioned.x).toBeCloseTo(tile.x,7); expect(positioned.y).toBeCloseTo(tile.y,7);
 }
});

test("high DPI and extreme screens retain full cropped coverage within a request budget",()=>{
 for(const size of [{width:1920,height:1080},{width:3840,height:2160},{width:400,height:1000}]) for(const ratio of [1,1.5,2,4]) {
   const tiles=visibleTiles(DEFAULT_CAMERA,"osm",size,ratio);
   const viewport=mapViewport(size.width,size.height);
   expect(tiles.length).toBeLessThanOrEqual(MAX_TILES);
   for(const fx of [0,.5,.99999]) for(const fy of [0,.5,.99999]) {
     const x=viewport.x+fx*viewport.width,y=viewport.y+fy*viewport.height;
     expect(tiles.some(t=>t.x<=x && t.x+t.width>x && t.y<=y && t.y+t.height>y)).toBe(true);
   }
   for(const t of tiles){
     expect(t.x).toBeLessThan(viewport.x+viewport.width);expect(t.x+t.width).toBeGreaterThan(viewport.x);
     expect(t.y).toBeLessThan(viewport.y+viewport.height);expect(t.y+t.height).toBeGreaterThan(viewport.y);
   }
 }
});

test("provider detail ceilings and invalid DPI are respected",()=>{
 const camera=cameraAt(35,139,16);
 expect(visibleTiles(camera,"gsi",{width:1920,height:1080},2).every(t=>Number(t.id.split("/")[0])<=17)).toBe(true);
 expect(visibleTiles(camera,"osm",{width:1920,height:1080},2).every(t=>Number(t.id.split("/")[0])<=19)).toBe(true);
 for(const ratio of [0,-1,NaN,Infinity]) expect(()=>visibleTiles(camera,"osm",MAP_VIEWBOX,ratio)).toThrow();
});


test("fractional zoom preserves cursor geography and uses integer tile URLs",()=>{
 for(const size of [{width:1920,height:1080},{width:400,height:1000}])for(const anchor of [{x:250,y:200},{x:750,y:400}]){
  const start=cameraAt(35.675,139.75,13.25),point=unproject(anchor.x,anchor.y,start);
  for(const z of [12,12.125,14.9,15.01,16,18.5,20]){
   const moved=zoomCameraAt(start,z,anchor),pixel=project(point.latitude,point.longitude,moved);
   expect(pixel.x).toBeCloseTo(anchor.x,7);expect(pixel.y).toBeCloseTo(anchor.y,7);
   const tiles=visibleTiles(moved,"osm",size,2);
   expect(tiles.length).toBeLessThanOrEqual(MAX_TILES);
   expect(tiles.every(t=>/^\d+\/\d+\/\d+$/.test(t.id))).toBe(true);
   for(const t of tiles){const positioned=positionTile(t,moved);expect(positioned.x).toBeCloseTo(t.x,7);expect(positioned.y).toBeCloseTo(t.y,7);}
  }
 }
});

test("close editing magnification overzooms native tiles without unsupported requests or gaps",()=>{
 expect(MAX_ZOOM).toBe(20);
 for(const provider of ["osm","gsi"] as const) for(const zoom of [17,18,19,19.5,20]) for(const size of [{width:1920,height:1080},{width:400,height:1000}]) {
  const camera=cameraAt(35.675,139.75,zoom),tiles=visibleTiles(camera,provider,size,2);
  const ceiling=provider==="osm"?19:17;
  expect(tiles.length).toBeGreaterThan(0);expect(tiles.length).toBeLessThanOrEqual(MAX_TILES);
  expect(tiles.every(t=>Number(t.id.split("/")[0])<=ceiling)).toBe(true);
  const viewport=mapViewport(size.width,size.height);
  for(const fx of [0,.5,.99999])for(const fy of [0,.5,.99999]){
   const x=viewport.x+fx*viewport.width,y=viewport.y+fy*viewport.height;
   expect(tiles.some(t=>t.x<=x&&t.x+t.width>x&&t.y<=y&&t.y+t.height>y)).toBe(true);
  }
  for(const tile of tiles){const moved=positionTile(tile,camera);expect(moved.x).toBeCloseTo(tile.x,6);expect(moved.y).toBeCloseTo(tile.y,6);}
 }
});

test("track and platform geographic dimensions enlarge uniformly without altering world data",()=>{
 // A small polygon represents a platform and its edge a rail: both use project in App.
 const points=Object.freeze([{latitude:35.675,longitude:139.75},{latitude:35.6751,longitude:139.75},{latitude:35.6751,longitude:139.75001}]);
 const projected=(zoom:number)=>points.map(p=>project(p.latitude,p.longitude,cameraAt(35.675,139.75,zoom)));
 const before=projected(16),after=projected(MAX_ZOOM),factor=2**(MAX_ZOOM-16);
 for(let i=0;i<points.length;i++){
  expect(after[i].x-500).toBeCloseTo((before[i].x-500)*factor,6);
  expect(after[i].y-300).toBeCloseTo((before[i].y-300)*factor,6);
 }
 expect(factor).toBe(16);
});

test("Japan overview fits the supported region without world wrapping or missing tiles",()=>{
 const camera=zoomCamera(DEFAULT_CAMERA,-100),bounds=viewBounds(camera);
 expect(camera.zoom).toBe(MIN_ZOOM);
 expect(bounds.west).toBeLessThanOrEqual(MAP_BOUNDS.west);
 expect(bounds.east).toBeGreaterThanOrEqual(MAP_BOUNDS.east);
 expect(bounds.south).toBeLessThanOrEqual(MAP_BOUNDS.south);
 expect(bounds.north).toBeGreaterThanOrEqual(MAP_BOUNDS.north);
 expect(bounds.east).toBeLessThanOrEqual(180);expect(bounds.west).toBeGreaterThanOrEqual(-180);
 for(const provider of ["osm","gsi"] as const){
  const tiles=visibleTiles(camera,provider);
  expect(tiles.length).toBeLessThanOrEqual(MAX_TILES);
  for(const x of [0,500,999])for(const y of [0,300,599])expect(tiles.some(t=>t.x<=x&&t.x+t.width>x&&t.y<=y&&t.y+t.height>y)).toBe(true);
 }
 expect(()=>panCamera(camera,1e9,-1e9)).not.toThrow();
 expect(DEFAULT_CAMERA.zoom).toBe(12);
});
