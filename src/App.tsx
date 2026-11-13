import {FontLicenses} from "./FontLicenses";
import {TrackTypeControls,TRACK_TYPE_LABELS,type TrackType} from "./TrackTypeControls";
import {useAutosave} from "./useAutosave";
import {WorkspaceTabs,type WorkspaceTab} from "./WorkspaceTabs";
import {PanelResizer} from "./PanelResizer";
import {CompanySetup} from "./CompanySetup";
import { useSmoothZoom, wheelZoomDelta } from "./useSmoothZoom";
import {useUnsavedCloseGuard} from "./useUnsavedCloseGuard";
import { ErrorNotice } from "./ErrorNotice";
import { routeAppearance } from "./routeAppearance";
import {useReferenceCopy} from "./useReferenceCopy";
import {ReferenceCopyControls} from "./ReferenceCopyControls";
import type {ReferenceBuild} from "./referenceCopy";
import {AlignmentProvenance} from "./AlignmentProvenance";
import type {SourceRecord,AlignmentProvenanceRecord} from "./AlignmentProvenance";
import {ReferenceGraphControls} from "./ReferenceGraphControls";
import {ReferenceGraphLayer} from "./ReferenceGraphLayer";
import {indexGraphDisplay,queryGraphDisplay} from "./referenceGraphDisplay";
import type {ReferenceGraphBundle} from "./referenceGraphBundle";
import {ServicePathCreator} from "./ServicePathCreator";
import type {CreateServicePath} from "./ServicePathCreator";
import {resolveServicePath,referenceKey} from "./servicePaths";
import type {ServicePath} from "./servicePaths";
import { ServiceFromRoutePreview } from "./ServiceFromRoutePreview";
import { useDisplaySettings } from "./useDisplaySettings";
import { StationNamePreview } from "./StationNamePreview";
import { automaticStationName } from "./automaticStationName";
import { ReferenceStationControls } from "./ReferenceStationControls";
import { createReferenceRepository } from "./referenceStations";
import type { ReferencePack } from "./referenceStations";
import { PlatformLengthInput } from "./PlatformLengthInput";
import type { Segment, Membership } from "./RouteCandidatePreview";
import { RouteCandidatePreview } from "./RouteCandidatePreview";
import { MAP_PROVIDERS } from "./mapProviders";
import { useEffect, useMemo, useRef, useState } from "react";
import type { MouseEvent, FormEvent, CSSProperties, PointerEvent as ReactPointerEvent } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { MAP_BOUNDS as BOUNDS, MAP_VIEWBOX, GSI_TILES, project, coordinatesFromClient, DEFAULT_CAMERA, MIN_ZOOM, MAX_ZOOM, cameraAt, panCamera, visibleTiles, positionTile, viewBounds, mapViewport } from "./map";
import { GsiBackground } from "./GsiBackground";
import type { Camera } from "./map";
import type { TileStatus } from "./GsiBackground";
import { circularAlignment } from "./circularAlignment";
import { nearestOnAlignment, stationPolygon, alignmentLength, distanceM, pointOnAlignment } from "./alignment";
type GeoPoint = { latitude: number; longitude: number };
type Alignment = { id: string; owner: string; name: string; points: GeoPoint[]; track_type: TrackType };
type StationPlacement = { station_id: string; alignment_id: string; position: number; length_m: number; width_m: number };

type EngineInfo = { name: string; version: string; company_setup_required?: boolean };
type Station = { id: string; owner: string; name: string; latitude: number; longitude: number };
type Track = { id: string; owner: string; from: string; to: string };
type Route = { id: string; owner: string; name: string; start: string; tracks: string[] };
type OperatingService = { id: string; owner: string; name: string; start: string; tracks: string[]; stops: number[] };
type RoutePath = { route_id: string; segments: Segment[]; member_stations: string[] };
type World = {
  route_paths?: RoutePath[];
  service_paths?: ServicePath[];
  source_records?: SourceRecord[];
  alignment_provenance?: AlignmentProvenanceRecord[];
  alignments?: Alignment[];
  station_placements?: StationPlacement[];
  next_id: string;
  players: { id: string; name: string }[];
  companies: { id: string; owner: string; name: string }[];
  stations: Station[];
  tracks: Track[];
  routes: Route[];
  services: OperatingService[];
};
type StationHistoryEntry = { station: Station; placement?: StationPlacement };
type Status = "loading" | "preview" | "ready" | "error";

const previewWorld: World = {
  next_id: "8", players: [{ id: "1", name: "Preview" }],
  companies: [{ id: "2", owner: "1", name: "サンプル鉄道" }],
  stations: [
    { id: "3", owner: "2", name: "サンプル中央", latitude: 35.685, longitude: 139.735 },
    { id: "4", owner: "2", name: "サンプル東", latitude: 35.66, longitude: 139.79 },
  ], tracks: [{ id: "5", owner: "2", from: "3", to: "4" }], routes: [{ id: "6", owner: "2", name: "サンプル本線", start: "3", tracks: ["5"] }], services: [{ id: "7", owner: "2", name: "サンプル系統", start: "3", tracks: ["5"], stops: [0, 1] }],
};
const detail = (error: unknown) => error instanceof Error ? error.message : String(error);

export function App() {
  const svgRef = useRef<SVGSVGElement | null>(null);
  const cameraRef = useRef<Camera>(DEFAULT_CAMERA);
  const [pixelRatio, setPixelRatio] = useState(() => window.devicePixelRatio || 1);
  const [mapSize, setMapSize] = useState<{width:number;height:number}>(MAP_VIEWBOX);
  const gestureRef = useRef<{ pointerId: number; startX: number; startY: number; camera: Camera; scale: number; dragged: boolean; mayPlace: boolean } | null>(null);
  const suppressMapClick = useRef(false);
  const suppressSelectionClick = useRef(false);
  const [mapMode, setMapMode] = useState<"move" | "place" | "draw">("move");
  const [dragging, setDragging] = useState(false);
  const [camera, setCamera] = useState<Camera>(DEFAULT_CAMERA);
  const [requestedCamera, setRequestedCamera] = useState<Camera>(DEFAULT_CAMERA);
  const [centerLatitude, setCenterLatitude] = useState(DEFAULT_CAMERA.latitude.toFixed(5));
  const [centerLongitude, setCenterLongitude] = useState(DEFAULT_CAMERA.longitude.toFixed(5));
  const [cameraError, setCameraError] = useState("");
  const displaySettings=useDisplaySettings();
  const background=displaySettings.settings.background;
  const setBackground=(value:"sample"|"gsi"|"osm")=>displaySettings.update({background:value});
  const provider = MAP_PROVIDERS[background === "sample" ? "osm" : background];
  const requestedTiles = useMemo(() => background === "sample" ? [] : visibleTiles(requestedCamera,background,mapSize,pixelRatio), [requestedCamera,background,mapSize,pixelRatio]);
  const displayTiles = useMemo(() => requestedTiles.map((tile) => positionTile(tile, camera)), [requestedTiles, camera]);
  const bounds = viewBounds(camera, mapSize);
  const [graphBundle,setGraphBundle]=useState<ReferenceGraphBundle|null>(null);
  const [graphVisible,setGraphVisible]=useState(true);
  const [pendingGraphChange,setPendingGraphChange]=useState<{bundle:ReferenceGraphBundle|null}|null>(null);
  const [candidateDirty,setCandidateDirty]=useState(false);
  const graphIndex=useMemo(()=>graphBundle?indexGraphDisplay(graphBundle.graph):[],[graphBundle]);
  const graphDisplay=useMemo(()=>queryGraphDisplay(graphIndex,viewBounds(camera,mapSize),camera.zoom,graphVisible),[graphIndex,camera,mapSize,graphVisible]);
  const [referencePack,setReferencePack]=useState<ReferencePack|null>(null);
  const [referenceVisible,setReferenceVisible]=useState(true);
  const referenceRepository=useMemo(()=>referencePack?createReferenceRepository(referencePack):null,[referencePack]);
  const referenceResult=referenceRepository?.query(bounds) ?? {stations:[],total:0};
  const cameraPending = camera !== requestedCamera;
  // Anchor the fictional offline scene to the same camera as railway geometry.
  const sampleScale = 2 ** (camera.zoom - DEFAULT_CAMERA.zoom);
  const sampleOrigin = project(DEFAULT_CAMERA.latitude, DEFAULT_CAMERA.longitude, camera);
  const sampleTransform = "translate(" + (sampleOrigin.x - MAP_VIEWBOX.width / 2 * sampleScale) + " " + (sampleOrigin.y - MAP_VIEWBOX.height / 2 * sampleScale) + ") scale(" + sampleScale + ")";
  useEffect(() => {
    const timer = window.setTimeout(() => setRequestedCamera(camera), 250);
    return () => window.clearTimeout(timer);
  }, [camera]);
  useEffect(() => {
    const svg = svgRef.current;
    if (!svg || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => {
      const {width,height} = entry.contentRect;
      if(width > 0 && height > 0) setMapSize({width,height});
    });
    observer.observe(svg);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    const update = () => setPixelRatio(window.devicePixelRatio || 1);
    const query = window.matchMedia?.(`(resolution: ${pixelRatio}dppx)`);
    query?.addEventListener("change", update);
    window.addEventListener("resize", update);
    return () => {
      query?.removeEventListener("change", update);
      window.removeEventListener("resize", update);
    };
  }, [pixelRatio]);
  const smoothZoom=useSmoothZoom(cameraRef,renderCamera);
  function renderCamera(next: Camera) {
    cameraRef.current = next; setCamera(next); setCenterLatitude(next.latitude.toFixed(5)); setCenterLongitude(next.longitude.toFixed(5)); setCameraError("");
  }
  function changeCamera(next: Camera) { smoothZoom.cancel(); renderCamera(next); }
  function zoomFromControls(delta: number) { smoothZoom.zoomBy(delta, { x: MAP_VIEWBOX.width / 2, y: MAP_VIEWBOX.height / 2 }); }
  useEffect(()=>{smoothZoom.cancel();},[background]);
  function moveToCoordinates(event: FormEvent) {
    event.preventDefault();
    if (!centerLatitude.trim() || !centerLongitude.trim()) { setCameraError("中心の緯度・経度を入力してください。"); return; }
    try { changeCamera(cameraAt(Number(centerLatitude), Number(centerLongitude), camera.zoom)); }
    catch { setCameraError("中心座標は北緯20〜46度・東経122〜154度で指定してください。"); }
  }

  const [tileStatus, setTileStatus] = useState<TileStatus>({ loaded: 0, failed: 0, total: GSI_TILES.length });
  const [status, setStatus] = useState<Status>("loading");
  const [info, setInfo] = useState<EngineInfo | null>(null);
  const [world, setWorld] = useState<World | null>(null);
  const [attempt, setAttempt] = useState(0);
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const [error, setErrorMessage] = useState("");
  const [errorRevision,setErrorRevision]=useState(0);
  function setError(message:string){setErrorMessage(message);setErrorRevision(value=>value+1);}
  const [notice, setNotice] = useState("");
  const [latitude, setLatitude] = useState("35.67500");
  const [longitude, setLongitude] = useState("139.75000");
  const [selected, setSelected] = useState<string | null>(null);
  const [editStationName, setEditStationName] = useState("");
  const stationNameInput=useRef<HTMLInputElement>(null);
  const stationNameEditButton=useRef<HTMLButtonElement>(null);
  const [pendingStation, setPendingStation] = useState<{ id: string } | null>(null);
  const [pendingDelete, setPendingDelete] = useState<{ kind: "station" | "track"; id: string } | null>(null);
  const [stationFrom, setStationFrom] = useState("");
  const [stationTo, setStationTo] = useState("");
  const [dirty, setDirty] = useState(false);
  const [confirmLoad, setConfirmLoad] = useState(false);
  const [loadSource,setLoadSource]=useState<"manual"|"auto">("manual");
  const [worldEpoch,setWorldEpoch]=useState(0);
  const [restoredAutosaveEpoch,setRestoredAutosaveEpoch]=useState(-1);
  const [mainTab,setMainTab]=useState<WorkspaceTab>("construction");
  const [companySetupRequired,setCompanySetupRequired]=useState(false);
  const [activeTool, setActiveTool] = useState<"stations" | "tracks" | "routes" | "services" | "company" | "station-management" | "inactive">("tracks");
  function chooseMainTab(tab:WorkspaceTab){setError("");setMainTab(tab);setPanelCollapsed(false);setActiveTool(tab==="construction"?"tracks":tab==="stations"?"station-management":tab==="settings"?"company":tab==="routes"?"routes":tab==="services"?"services":"inactive");changeMapMode(tab==="construction"?"draw":"move");}
  const [panelCollapsed, setPanelCollapsed] = useState(false);
  // Internal compatibility label only; physical tracks are not user-named routes.
  const alignmentName = "新しい線路";
  const [draftPoints, setDraftPoints] = useState<GeoPoint[]>([]);
  type ParkedPlan = { id: number; name: string; points: GeoPoint[]; redo: GeoPoint[]; line: GeoPoint[]; curved: boolean; trackType: TrackType; roadFollowing: boolean };
  const [parkedPlans, setParkedPlans] = useState<ParkedPlan[]>([]);
  const [activePlanId, setActivePlanId] = useState(1);
  const [planName,setPlanName]=useState("");
  const [trackType,setTrackType]=useState<TrackType>("normal");
  const [roadFollowing,setRoadFollowing]=useState(false);
  const roadFollowingBlocked=trackType==="tram"&&roadFollowing;
  function chooseTrackType(value:TrackType){setTrackType(value);setRoadFollowing(value==="tram");setHoverPoint(null);}
  const nextPlanId = useRef(2);
  const [stationUndo,setStationUndo]=useState<StationHistoryEntry[]>([]);
  const [stationRedo,setStationRedo]=useState<StationHistoryEntry[]>([]);
  function clearStationHistory(){setStationUndo([]);setStationRedo([]);}
  function recordStation(snapshot:World){const station=snapshot.stations.find(item=>!world?.stations.some(old=>old.id===item.id));if(station){setStationUndo(history=>[...history,{station,placement:snapshot.station_placements?.find(p=>p.station_id===station.id)}].slice(-50));setStationRedo([]);}}
  const [redoPoints, setRedoPoints] = useState<GeoPoint[]>([]);
  const [hoverPoint, setHoverPoint] = useState<GeoPoint | null>(null);
  const {theme,lineWidth,contrast,panelWidth,textSize}=displaySettings.settings;
  const setTheme=(value:"light"|"dark")=>displaySettings.update({theme:value});
  const setLineWidth=(value:number)=>displaySettings.update({lineWidth:value});
  const setContrast=(value:number)=>displaySettings.update({contrast:value});
  const setPanelWidth=(value:number)=>displaySettings.update({panelWidth:value});
  const setTextSize=(value:number)=>displaySettings.update({textSize:value});
  function undoPoint() { if (!draftPoints.length) return; setRedoPoints(redo => [...redo, draftPoints[draftPoints.length-1]]); setDraftPoints(points => points.slice(0,-1)); setHoverPoint(null); }
  function redoPoint() { if (!redoPoints.length) return; setDraftPoints(points => [...points, redoPoints[redoPoints.length-1]]); setRedoPoints(points => points.slice(0,-1)); setHoverPoint(null); }
  const [curved, setCurved] = useState(true);
  const [alignmentId, setAlignmentId] = useState("");
  const [platformLength, setPlatformLength] = useState("200");
  const platformWidth = "12";
  const [editLength, setEditLength] = useState("200");
  const [editWidth, setEditWidth] = useState("12");
  const [stationPosition, setStationPosition] = useState<number | null>(null);
  const alignments = world?.alignments ?? [];
  const detailedStations = camera.zoom >= 15;
  const stationPolygons=useMemo(()=>new Map((world?.station_placements??[]).map(placement=>{
    const alignment=world?.alignments?.find(a=>a.id===placement.alignment_id);
    return [placement.station_id,alignment?stationPolygon(alignment.points,placement.position,placement.length_m,placement.width_m):null];
  })),[world]);
  const routeVisual = useMemo(() => routeAppearance(world), [world]);
  const selectedAlignment = alignments.find((a) => a.id === alignmentId);
  const previewHalf = selectedAlignment ? Number(platformLength) / alignmentLength(selectedAlignment.points) / 2 : 0;
  const previewCenter = selectedAlignment && stationPosition !== null ? pointOnAlignment(selectedAlignment.points, Math.max(previewHalf,Math.min(1-previewHalf,stationPosition))) : null;
  const stationPreview = selectedAlignment && stationPosition !== null ? stationPolygon(selectedAlignment.points, stationPosition, Number(platformLength), Number(platformWidth)) : [];
  const draftGeometry = useMemo(() => circularAlignment(draftPoints), [draftPoints]);
  const previewGeometry = useMemo(() => circularAlignment(hoverPoint && draftPoints.length && mapMode === "draw" ? [...draftPoints, hoverPoint] : draftPoints), [draftPoints, hoverPoint, mapMode]);
  const draftLine = curved ? draftGeometry.points : draftPoints;
  const previewLine = curved ? previewGeometry.points : hoverPoint && draftPoints.length && mapMode === "draw" ? [...draftPoints, hoverPoint] : draftPoints;
  const draftInvalid = curved ? Boolean(draftGeometry.error) : draftPoints.some((point,index) => index > 0 && distanceM(draftPoints[index-1], point) < .1);
  const previewError = curved ? previewGeometry.error : null;
  const circleGuide = curved && mapMode === "draw" ? previewGeometry.guide : null;
  const guideCenter = circleGuide ? project(circleGuide.center.latitude,circleGuide.center.longitude,camera) : null;
  const guideEdge = circleGuide ? project(circleGuide.edge.latitude,circleGuide.edge.longitude,camera) : null;
  function parkCurrentPlan(): ParkedPlan | null {
    return draftPoints.length || redoPoints.length ? {id:activePlanId,name:planName,points:draftPoints,redo:redoPoints,line:draftLine,curved,trackType,roadFollowing} : null;
  }
  function newIndependentPlan() {
    if (!enabled || referenceCopy.active) return;
    const current=parkCurrentPlan();
    setParkedPlans(plans=>current?[...plans,current]:plans);
    setActivePlanId(nextPlanId.current++);setPlanName("");setDraftPoints([]);setRedoPoints([]);setHoverPoint(null);changeMapMode("draw");
  }
  function selectIndependentPlan(id:number) {
    if(!enabled || referenceCopy.active)return;
    const target=parkedPlans.find(plan=>plan.id===id);if(!target)return;
    const current=parkCurrentPlan();
    setParkedPlans(plans=>[...plans.filter(plan=>plan.id!==id),...(current?[current]:[])]);
    setActivePlanId(target.id);setPlanName(target.name);setDraftPoints(target.points);setRedoPoints(target.redo);setCurved(target.curved);setTrackType(target.trackType);setRoadFollowing(target.roadFollowing);setHoverPoint(null);changeMapMode("move");
  }
  const [planHover, setPlanHover] = useState<{id:number;x:number;y:number} | null>(null);
  useEffect(() => { setPlanHover(null); }, [draftPoints, parkedPlans, activeTool, camera, world]);
  const hoveredPlan = planHover?.id === activePlanId ? {name:planName,line:draftLine,points:draftPoints,curved} : parkedPlans.find(plan=>plan.id===planHover?.id);
  const hoveredGeometry = hoveredPlan?.curved ? circularAlignment(hoveredPlan.points) : null;
  function hoverPlan(event: ReactPointerEvent<SVGPathElement>, id:number) {
    if (gestureRef.current || event.pointerType === "touch") return;
    const rect=svgRef.current?.getBoundingClientRect();
    if(rect)setPlanHover({id,x:event.clientX-rect.left,y:event.clientY-rect.top});
  }
  const viewport = mapViewport(mapSize.width,mapSize.height);
  const drawPath = (points: GeoPoint[]) => points.map((point, index) => { const p = project(point.latitude, point.longitude, camera); return (index ? "L" : "M") + p.x + "," + p.y; }).join(" ");
  const [companyName, setCompanyName] = useState("");
  const [routeId, setRouteId] = useState<string | null>(null);
  const [routeName, setRouteName] = useState("新路線");
  const [routeStart, setRouteStart] = useState("");
  const [routeTracks, setRouteTracks] = useState<string[]>([]);
  const [nextTrack, setNextTrack] = useState("");
  const [routeDirty, setRouteDirty] = useState(false);
  const [routeHint, setRouteHint] = useState("");
  const [pendingRouteChoice, setPendingRouteChoice] = useState<{ id: string | null } | null>(null);
  const [serviceId, setServiceId] = useState<string | null>(null);
  const [serviceName, setServiceName] = useState("新系統");
  const [serviceStart, setServiceStart] = useState("");
  const [serviceTracks, setServiceTracks] = useState<string[]>([]);
  const [serviceStops, setServiceStops] = useState<number[]>([]);
  const [serviceNextTrack, setServiceNextTrack] = useState("");
  const [serviceDirty, setServiceDirty] = useState(false);
  const [pathServiceDirty,setPathServiceDirty]=useState(false);
  const [pathDraftEpoch,setPathDraftEpoch]=useState(0);
  const [serviceHint, setServiceHint] = useState("");
  const [pendingServiceChoice, setPendingServiceChoice] = useState<{ id: string | null } | null>(null);
  const enabled = status === "ready" && !busy && !companySetupRequired;
  const stationsById = useMemo(()=>new Map(world?.stations.map((station) => [station.id, station])),[world]);
  const validEndpoints = stationFrom !== stationTo && stationsById.has(stationFrom) && stationsById.has(stationTo);

  useEffect(() => {
    const svg = svgRef.current;
    if (!svg) return;
    function wheel(event: WheelEvent) {
      if (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey || !Number.isFinite(event.deltaY) || event.deltaY === 0 || Math.abs(event.deltaX) > Math.abs(event.deltaY)) return;
      event.preventDefault();
      if (gestureRef.current) return;
      const rect=svg!.getBoundingClientRect();
      const location=coordinatesFromClient(event.clientX,event.clientY,rect,cameraRef.current);
      if(!location)return;
      smoothZoom.zoomBy(wheelZoomDelta(event.deltaY,event.deltaMode,rect.height),project(location.latitude,location.longitude,cameraRef.current));
    }
    svg.addEventListener("wheel", wheel, { passive: false });
    return () => svg.removeEventListener("wheel", wheel);
  }, []);
  useEffect(() => { smoothZoom.cancel(); changeMapMode(activeTool === "tracks" ? "draw" : "move"); setHoverPoint(null); setPanelCollapsed(false); }, [activeTool]);
  const selectedServicePath=world?.service_paths?.find(path=>path.service_id===serviceId);
  const selectedServiceOccurrences=(()=>{if(!world||!selectedServicePath)return [];try{return resolveServicePath(serviceStart,selectedServicePath.legs,world);}catch{return [];}})();
  const selectedRoutePath = world?.route_paths?.find(path => path.route_id === routeId);
  const companyDirty = !!world && companyName !== (world.companies[0]?.name ?? "");
  const selectedStation = world?.stations.find((station) => station.id === selected);
  const stationDirty = !!selectedStation && editStationName !== selectedStation.name;
  const selectedPlacement = world?.station_placements?.find(p => p.station_id === selected);
  const platformDirty = !!selectedPlacement && (Number(editLength) !== selectedPlacement.length_m || Number(editWidth) !== selectedPlacement.width_m);
  const stationSettingsDirty = stationDirty || platformDirty;
  const copyCanStart=!(routeDirty||serviceDirty||pathServiceDirty||companyDirty||stationSettingsDirty||draftPoints.length||redoPoints.length||parkedPlans.length);
  const referenceCopy=useReferenceCopy({bundle:graphBundle,worldVersion:world,enabled,canStart:copyCanStart,tool:activeTool,name:alignmentName,onBuild:buildReferenceAlignment,roadFollow:roadFollowingBlocked});
  const graphNodesById=useMemo(()=>new Map(graphBundle?.graph.nodes.map(node=>[node.osmId,node])??[]),[graphBundle]);
  useEffect(()=>{referenceCopy.clearHover();},[camera]);
  const saveBlocked = candidateDirty || routeDirty || serviceDirty || pathServiceDirty || companyDirty || stationSettingsDirty;
  const hasPlans = draftPoints.length > 0 || redoPoints.length > 0 || parkedPlans.length > 0 || referenceCopy.active;
  const unapplied = saveBlocked || hasPlans;
  const autosave=useAutosave({ready:status==="ready",dirty:dirty&&!companySetupRequired,world,epoch:worldEpoch,restoredEpoch:restoredAutosaveEpoch,busyRef,setBusy});
  const closeGuard = useUnsavedCloseGuard(dirty || unapplied);
  function stationReferences(id: string) {
    const links = world?.tracks.filter((track) => track.from === id || track.to === id) ?? [];
    const routes = world?.routes.filter((route) => route.start === id || world?.route_paths?.some(path => path.route_id === route.id && (path.member_stations.includes(id) || path.segments.some(segment => segment.kind === "alignment" ? segment.from === id || segment.to === id : links.some(link => link.id === segment.track_id)))) || route.tracks.some((track) => links.some((link) => link.id === track))) ?? [];
    const services = world?.services.filter((service) => service.start === id || world?.service_paths?.some(path=>{if(path.service_id!==service.id)return false;try{return resolveServicePath(service.start,path.legs,world).some(o=>o.station_id===id);}catch{return true;}}) || service.tracks.some((track) => links.some((link) => link.id === track))) ?? [];
    return [links.length ? "接続線路 " + links.length + "本" : "", ...routes.map((route) => "路線: " + route.name), ...services.map((service) => "系統: " + service.name)].filter(Boolean).join(" / ");
  }
  function trackReferences(id: string) {
    return [...(world?.routes.filter((route) => (route.tracks.includes(id) || world?.route_paths?.some(path => path.route_id === route.id && path.segments.some(segment => segment.kind === "track" && segment.track_id === id)))).map((route) => "路線: " + route.name) ?? []), ...(world?.services.filter((service) => (service.tracks.includes(id)||world?.service_paths?.some(path=>path.service_id===service.id&&path.legs.some(leg=>leg.segment.kind==="track"&&leg.segment.track_id===id)))).map((service) => "系統: " + service.name) ?? [])].join(" / ");
  }
  function selectStation(id: string) {
    if (busyRef.current) return;
    if (stationSettingsDirty && id !== selected) { setPendingStation({ id }); return; }
    if (id === selected) { setMainTab("stations"); setActiveTool("station-management"); setPanelCollapsed(false); return; }
    setMainTab("stations"); setActiveTool("station-management");
    setSelected(id); setEditStationName(stationsById.get(id)?.name ?? ""); setPendingStation(null); setPendingDelete(null);
    const placement = world?.station_placements?.find((entry) => entry.station_id === id);
    setEditLength(String(placement?.length_m ?? 200)); setEditWidth(String(placement?.width_m ?? 12)); setPanelCollapsed(false);
  }
  function beginStationNameEdit(){
    if(!enabled || busyRef.current || !selectedStation)return;
    stationNameInput.current?.focus();stationNameInput.current?.select();
  }
  function renameStation(event: FormEvent) {
    event.preventDefault();
    if (!enabled || !selectedStation || !stationDirty || !editStationName.trim()) return;
    void perform(async () => {
      const snapshot = await invoke<World>("rename_station", { stationId: selectedStation.id, name: editStationName.trim() });
      setWorld(snapshot); setEditStationName(snapshot.stations.find((station) => station.id === selectedStation.id)?.name ?? "");
      setDirty(true); setConfirmLoad(false); setNotice("駅名を変更しました。保存でファイルに反映します。駅設置履歴は終了しました。");clearStationHistory();
    });
  }
  function deleteEquipment() {
    if (!enabled || !pendingDelete || unapplied) return;
    const { kind, id } = pendingDelete;
    if (kind === "station" ? stationReferences(id) : trackReferences(id)) return;
    void perform(async () => {
      const snapshot = await invoke<World>(kind === "station" ? "delete_station" : "delete_track", kind === "station" ? { stationId: id } : { trackId: id });
      setWorld(snapshot); setDirty(true); setConfirmLoad(false); setPendingDelete(null);
      if (kind === "station") {
        clearStationHistory();
        if (selected === id) { setSelected(null); setEditStationName(""); setPendingStation(null); }
        if (stationFrom === id) setStationFrom("");
        if (stationTo === id) setStationTo("");
        if (routeStart === id) resetRoute();
        if (serviceStart === id) resetService();
      }
      if (nextTrack === id) setNextTrack("");
      if (serviceNextTrack === id) setServiceNextTrack("");
      setNotice("設備を削除しました。保存でファイルに反映します。");
    });
  }
  const routeStations = routeStart ? [routeStart] : [];
  let routeEnd = routeStart;
  let routeValid = stationsById.has(routeStart);
  for (const id of routeTracks) {
    const track = world?.tracks.find((entry) => entry.id === id);
    if (!track || (track.from !== routeEnd && track.to !== routeEnd)) { routeValid = false; break; }
    routeEnd = track.from === routeEnd ? track.to : track.from;
    routeStations.push(routeEnd);
  }
  const nextTracks = routeValid ? world?.tracks.filter((track) => track.from === routeEnd || track.to === routeEnd) ?? [] : [];
  const serviceStations = serviceStart ? [serviceStart] : [];
  let serviceEnd = serviceStart;
  let serviceValid = stationsById.has(serviceStart);
  for (const id of serviceTracks) {
    const track = world?.tracks.find((entry) => entry.id === id);
    if (!track || (track.from !== serviceEnd && track.to !== serviceEnd)) { serviceValid = false; break; }
    serviceEnd = track.from === serviceEnd ? track.to : track.from;
    serviceStations.push(serviceEnd);
  }
  const serviceNextTracks = serviceValid ? world?.tracks.filter((track) => track.from === serviceEnd || track.to === serviceEnd) ?? [] : [];
  useEffect(() => {
    let active = true;
    setError("");
    if (!isTauri()) {
      setWorld(previewWorld); setCompanyName(previewWorld.companies[0].name);
      setStatus("preview");
      return;
    }
    setStatus("loading");
    Promise.all([invoke<EngineInfo>("engine_info"), invoke<World>("world_snapshot")]).then(
      ([engine, snapshot]) => {
        if (active) { setInfo(engine); setCompanySetupRequired(engine.company_setup_required===true); setWorld(snapshot); setCompanyName(snapshot.companies[0]?.name ?? ""); setStatus("ready"); }
      },
      (failure) => {
        if (active) { setError(detail(failure)); setStatus("error"); }
      },
    );
    return () => { active = false; };
  }, [attempt]);

  async function perform(operation: () => Promise<void>) {
    if (status !== "ready" || busyRef.current) return;
    busyRef.current = true;
    setBusy(true); setError(""); setNotice("");
    try { await operation(); }
    catch (failure) { setError(detail(failure)); }
    finally { busyRef.current = false; setBusy(false); }
  }

  async function addStation(lat: number, lon: number) {
    if (!enabled) return;
    if (!Number.isFinite(lat) || !Number.isFinite(lon) || lat < BOUNDS.south || lat > BOUNDS.north || lon < BOUNDS.west || lon > BOUNDS.east) {
      setError("編集範囲内（北緯20〜46度・東経122〜154度）の緯度・経度を指定してください。"); return;
    }
    const name=automaticStationName({latitude:lat,longitude:lon},referenceRepository,world?.stations.map(s=>s.name)??[]).name;
    await perform(async () => {
      const snapshot = await invoke<World>("add_station", { name, latitude: lat, longitude: lon });
      recordStation(snapshot);
      setWorld(snapshot); setDirty(true); setConfirmLoad(false);
      setNotice(`${name.trim()}を配置しました。`);
    });
  }
  function submitTrack(event: FormEvent) {
    event.preventDefault();
    if (!enabled || !validEndpoints) return;
    void perform(async () => {
      const snapshot = await invoke<World>("add_track", { from: stationFrom, to: stationTo });
      setWorld(snapshot); setDirty(true); setConfirmLoad(false);
      setNotice("線路を接続しました。");
    });
  }
  function cancelMapGesture() {
    const gesture = gestureRef.current;
    gestureRef.current = null; suppressMapClick.current = true; suppressSelectionClick.current = true; setDragging(false);
    const svg = svgRef.current;
    if (gesture && svg?.hasPointerCapture?.(gesture.pointerId)) svg.releasePointerCapture(gesture.pointerId);
  }
  function changeMapMode(mode: "move" | "place" | "draw") {
    setError("");
    const wasGesturing = gestureRef.current !== null;
    cancelMapGesture(); setMapMode(mode); setHoverPoint(null); setPlanHover(null);
    if (!wasGesturing) { suppressMapClick.current = false; suppressSelectionClick.current = false; }
  }
  function pointerDown(event: ReactPointerEvent<SVGSVGElement>) {
    smoothZoom.cancel();
    setPlanHover(null);referenceCopy.clearHover();
    if (event.button !== 0 || event.isPrimary === false || gestureRef.current) return;
    suppressMapClick.current = true;
    const rect = event.currentTarget.getBoundingClientRect();
    if (!coordinatesFromClient(event.clientX, event.clientY, rect, cameraRef.current)) return;
    const { scale } = mapViewport(rect.width, rect.height);
    const existingEquipment = event.target instanceof Element && !!event.target.closest(".station-marker, .track-connection");
    gestureRef.current = { pointerId: event.pointerId, startX: event.clientX, startY: event.clientY, camera: cameraRef.current, scale, dragged: false, mayPlace: ((mapMode === "place" && activeTool === "stations") || (mapMode === "draw" && activeTool === "tracks")) && !existingEquipment };
    suppressMapClick.current = false;
    suppressSelectionClick.current = false;
  }
  function pointerMove(event: ReactPointerEvent<SVGSVGElement>) {
    if(enabled&&referenceCopy.active&&referenceCopy.roadFollow&&mapMode==="draw"&&!gestureRef.current){const p=coordinatesFromClient(event.clientX,event.clientY,event.currentTarget.getBoundingClientRect(),camera);if(p)void referenceCopy.hover(p);}

    if (enabled && !roadFollowingBlocked && !referenceCopy.active && mapMode === "draw" && activeTool === "tracks" && !gestureRef.current) setHoverPoint(coordinatesFromClient(event.clientX,event.clientY,event.currentTarget.getBoundingClientRect(),camera));
    const gesture = gestureRef.current;
    if (!gesture || gesture.pointerId !== event.pointerId) return;
    const dx = event.clientX - gesture.startX, dy = event.clientY - gesture.startY;
    if (!gesture.dragged && Math.hypot(dx, dy) < 5) return;
    if (!gesture.dragged && event.type === "pointermove") event.currentTarget.setPointerCapture?.(event.pointerId);
    gesture.dragged = true; suppressMapClick.current = true; suppressSelectionClick.current = true; setDragging(true);
    changeCamera(panCamera(gesture.camera, -dx / gesture.scale, -dy / gesture.scale));
  }
  function pointerEnd(event: ReactPointerEvent<SVGSVGElement>, canceled = false) {
    const gesture = gestureRef.current;
    if (!gesture || gesture.pointerId !== event.pointerId) return;
    if (!canceled) pointerMove(event);
    suppressMapClick.current = canceled || gesture.dragged || !gesture.mayPlace;
    suppressSelectionClick.current = canceled || gesture.dragged;
    gestureRef.current = null; setDragging(false);
    if (event.currentTarget.hasPointerCapture?.(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
  }
  async function buildReferenceAlignment(build:ReferenceBuild){
    if(!enabled||busyRef.current||!copyCanStart)throw Error("別の下書きまたは処理中です。");
    busyRef.current=true;setBusy(true);try{const snapshot=await invoke<World>("build_alignment_from_reference",{...build,trackType});setWorld(snapshot);setDirty(true);setConfirmLoad(false);setNotice("参考形状と出典をワールドへ建設しました。ファイルへの保存は別途行ってください。");}finally{busyRef.current=false;setBusy(false);}
  }
  function mapClick(event: MouseEvent<SVGSVGElement>) {
    smoothZoom.cancel();
    if (!enabled || suppressMapClick.current || mapMode === "move") return;
    const rect = event.currentTarget.getBoundingClientRect();
    const coordinates = coordinatesFromClient(event.clientX, event.clientY, rect, camera);
    if (!coordinates) return;
    if (activeTool === "tracks" && mapMode === "draw") {
      if(referenceCopy.active){void referenceCopy.click(coordinates);return;}
      if(roadFollowingBlocked)return;
      // The second click of a double-click ends the draft without adding
      // another endpoint or constructing it. The first click already placed it.
      if (event.detail >= 2) {
        if (draftPoints.length >= 2) { changeMapMode("move"); setHoverPoint(null); }
        return;
      }
      if (draftPoints.length && distanceM(draftPoints[draftPoints.length - 1], coordinates) < .1) return;
      if (draftPoints.length >= 128) { setError("線形点は128点までです。一度建設を確定してください。"); return; }
      setDraftPoints((points) => [...points, coordinates]); setRedoPoints([]); setHoverPoint(null); return;
    }
    if (activeTool !== "stations" || mapMode !== "place") return;
    if (alignments.length) {
      const candidates = (selectedAlignment ? [selectedAlignment] : alignments).map((a) => ({ alignment: a, nearest: nearestOnAlignment(a.points, coordinates) })).sort((a,b) => a.nearest.distanceM - b.nearest.distanceM);
      const nearest = candidates[0];
      if (!nearest || nearest.nearest.distanceM > 150) { setError("線路の近くを選んでください。駅は建設済みの線路に沿って配置します。"); return; }
      setError(""); setAlignmentId(nearest.alignment.id); setStationPosition(nearest.nearest.position); return;
    }
    setError("先に線路タブから線路を建設してください。");
  }
  function commitAlignment() {
    if (!enabled || roadFollowingBlocked || draftLine.length < 2 || draftInvalid || !alignmentName.trim()) return;
    void perform(async () => {
      const snapshot = await invoke<World>("build_alignment", { name: alignmentName.trim(), points: draftLine, trackType });
      const created = snapshot.alignments?.find((a) => !alignments.some((old) => old.id === a.id));
      setWorld(snapshot); setDirty(true); setConfirmLoad(false); setPlanName(""); setDraftPoints([]); setRedoPoints([]); setHoverPoint(null);
      if (created) setAlignmentId(created.id);
      changeMapMode("move"); setNotice("線路を建設しました。次は駅タブからホームを配置できます。");
    });
  }
  function commitPlatform(event: FormEvent) {
    event.preventDefault();
    if (!enabled || !selectedAlignment || stationPosition === null || stationPreview.length < 4) return;
    const name=automaticStationName(previewCenter,referenceRepository,world?.stations.map(s=>s.name)??[]).name;
    void perform(async () => {
      const snapshot = await invoke<World>("place_station_on_alignment", { name, alignmentId: selectedAlignment.id, position: stationPosition, lengthM: Number(platformLength), widthM: Number(platformWidth) });
      recordStation(snapshot);
      setWorld(snapshot); setDirty(true); setConfirmLoad(false); setStationPosition(null);
      setNotice(name.trim() + "を線路に沿って配置しました。");
    });
  }
  function resizePlatform(event: FormEvent) {
    event.preventDefault();
    if (!enabled || !selected || !platformDirty || !Number.isFinite(Number(editLength)) || Number(editLength) < 20 || Number(editLength) > 1000) return;
    void perform(async () => {
      const snapshot = await invoke<World>("update_station_placement", { stationId: selected, lengthM: Number(editLength), widthM: Number(editWidth) });
      const placement = snapshot.station_placements?.find(p => p.station_id === selected);
      if (placement) { setEditLength(String(placement.length_m)); setEditWidth(String(placement.width_m)); }
      setWorld(snapshot); setDirty(true); setConfirmLoad(false); setNotice("ホーム寸法を変更しました。保存でファイルに反映します。");clearStationHistory();
    });
  }
  function stationHistoryBlock(){
    const placement=world?.station_placements?.find(p=>p.station_id===selected);
    if(pendingDelete || pendingStation || confirmLoad)return "表示中の確認を完了または取り消してください。";
    if(unapplied || stationPosition!==null || (placement && (Number(editLength)!==placement.length_m || Number(editWidth)!==placement.width_m)))return "編集中の下書き・配置プレビューを適用または解除してください。";
    return "";
  }
  function undoStation(){
    if(!enabled || busyRef.current)return;
    const entry=stationUndo.at(-1);if(!entry){setNotice("取り消せる駅設置履歴はありません。保存済み・読込済みの駅は対象外です。");return;}
    const blocked=stationHistoryBlock();if(blocked){setError(blocked);return;}
    const current=world?.stations.find(station=>station.id===entry.station.id);
    const currentPlacement=world?.station_placements?.find(p=>p.station_id===entry.station.id);
    if(JSON.stringify(current)!==JSON.stringify(entry.station)||JSON.stringify(currentPlacement)!==JSON.stringify(entry.placement)){setError("対象駅が設置後に変更されています。駅設置Undoは実行できません。");return;}
    const references=stationReferences(entry.station.id);if(references){setError("駅設置Undo不可: "+references);return;}
    void perform(async()=>{
      const snapshot=await invoke<World>("delete_station",{stationId:entry.station.id});
      setWorld(snapshot);setDirty(true);setConfirmLoad(false);setStationUndo(history=>history.slice(0,-1));setStationRedo(history=>[...history,entry]);
      if(selected===entry.station.id){setSelected(null);setEditStationName("");setPendingStation(null);}
      if(stationFrom===entry.station.id)setStationFrom("");if(stationTo===entry.station.id)setStationTo("");
      if(routeStart===entry.station.id)resetRoute();if(serviceStart===entry.station.id)resetService();setPendingDelete(null);
      setNotice("直前の駅設置を取り消しました。やり直すと新しい駅IDになります。");
    });
  }
  function redoStation(){
    if(!enabled || busyRef.current)return;
    const entry=stationRedo.at(-1);if(!entry){setNotice("やり直せる駅設置履歴はありません。");return;}
    const blocked=stationHistoryBlock();if(blocked){setError(blocked);return;}
    if(entry.placement&&!alignments.some(a=>a.id===entry.placement!.alignment_id)){setError("元の建設線形がありません。駅設置Redoは実行できません。");return;}
    void perform(async()=>{
      const p=entry.placement;
      const snapshot=await invoke<World>(p?"place_station_on_alignment":"add_station",p?{name:entry.station.name,alignmentId:p.alignment_id,position:p.position,lengthM:p.length_m,widthM:p.width_m}:{name:entry.station.name,latitude:entry.station.latitude,longitude:entry.station.longitude});
      const created=snapshot.stations.find(station=>!world?.stations.some(old=>old.id===station.id));
      setWorld(snapshot);setDirty(true);setConfirmLoad(false);
      if(!created){clearStationHistory();setError("再設置の結果を識別できません。重複防止のため履歴を終了しました。ワールドを確認してください。");return;}
      setStationRedo(history=>history.slice(0,-1));setStationUndo(history=>[...history,{station:created,placement:snapshot.station_placements?.find(p=>p.station_id===created.id)}].slice(-50));
      setNotice("駅を再設置しました。駅IDは新規発行です。保存でファイルに反映します。");
    });
  }
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (event.target instanceof HTMLElement && (event.target.isContentEditable || /INPUT|SELECT|TEXTAREA/.test(event.target.tagName))) return;
      if (!event.ctrlKey && !event.metaKey && !event.altKey && !event.isComposing && (event.key === "+" || event.key === "-" || event.code === "NumpadAdd" || event.code === "NumpadSubtract")) {
        event.preventDefault();
        zoomFromControls(event.key === "+" || event.code === "NumpadAdd" ? 1 : -1);
        return;
      }
      if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase()==="z") {
        event.preventDefault();if(event.repeat || busyRef.current)return;
        if(referenceCopy.active && activeTool==="tracks"){if(event.shiftKey)referenceCopy.redoPick();else referenceCopy.undo();}
        else if(activeTool==="tracks" && (draftPoints.length || redoPoints.length)){if(event.shiftKey)redoPoint();else undoPoint();}
        else if(event.shiftKey)redoStation();else undoStation();return;
      }
      if (event.key === "Escape") { changeMapMode("move"); setStationPosition(null); }
    };
    window.addEventListener("keydown", key); return () => window.removeEventListener("keydown", key);
  });
  function submitStation(event: FormEvent) {
    event.preventDefault();
    if (!latitude.trim() || !longitude.trim()) { setError("緯度・経度を入力してください。"); return; }
    void addStation(Number(latitude), Number(longitude));
  }
  function resetRoute(route?: Route) {
    setRouteId(route?.id ?? null); setRouteName(route?.name ?? "新路線");
    setRouteStart(route?.start ?? ""); setRouteTracks(route ? [...route.tracks] : []);
    setNextTrack(""); setRouteDirty(false); setRouteHint(""); setPendingRouteChoice(null);
  }
  function chooseRoute(id: string | null) {
    if (!enabled) return;
    if (routeDirty) { setPendingRouteChoice({ id }); return; }
    resetRoute(world?.routes.find((route) => route.id === id));
  }
  function startCompany(event:FormEvent){
    event.preventDefault();if(status!=="ready"||busy||!companyName.trim())return;
    void perform(async()=>{const snapshot=await invoke<World>("rename_company",{name:companyName.trim()});setWorld(snapshot);setCompanyName(snapshot.companies[0]?.name??"");setCompanySetupRequired(false);setDirty(true);setNotice("会社を設定しました。建設を始められます。");});
  }
  function submitCompany(event: FormEvent) {
    event.preventDefault();
    if (!enabled || !companyName.trim()) return;
    void perform(async () => {
      const snapshot = await invoke<World>("rename_company", { name: companyName.trim() });
      setWorld(snapshot); setCompanyName(snapshot.companies[0]?.name ?? ""); setDirty(true); setConfirmLoad(false);
      setNotice("会社名を変更しました。");
    });
  }
  function submitRoute(event: FormEvent) {
    event.preventDefault();
    if (!enabled || selectedRoutePath || !routeName.trim() || !routeValid || !routeTracks.length) return;
    void perform(async () => {
      const snapshot = await invoke<World>("save_route", { routeId, name: routeName.trim(), start: routeStart, tracks: routeTracks });
      const applied = routeId ? snapshot.routes.find((route) => route.id === routeId)
        : snapshot.routes.find((route) => !world?.routes.some((existing) => existing.id === route.id));
      setWorld(snapshot); setDirty(true); setConfirmLoad(false); resetRoute(applied);
      setNotice("路線をワールドに適用しました。ファイルへの保存は「保存」で行います。");
    });
  }
  async function applyPlannedRoute(name: string, membership: Membership) {
    if (!enabled || routeDirty) throw new Error("先に路線下書きを適用または破棄してください。");
    if (busyRef.current) throw new Error("処理中です。");
    busyRef.current=true;setBusy(true);
    try {
      const snapshot=await invoke<World>("save_planned_route",{routeId:null,name,start:membership.path.stations[0],segments:membership.path.segments,excludedStations:membership.excluded_stations});
      const created=snapshot.routes.find(route=>!world?.routes.some(old=>old.id===route.id));
      setWorld(snapshot);setDirty(true);setConfirmLoad(false);resetRoute(created);setNotice("路線と所属駅を適用しました。ファイルへの保存は「保存」で行います。");
    } finally {busyRef.current=false;setBusy(false);}
  }
  function appendTrack() {
    if (!enabled || !nextTracks.some((track) => track.id === nextTrack)) return;
    setRouteTracks((tracks) => [...tracks, nextTrack]); setNextTrack(""); setRouteDirty(true); setRouteHint("");
  }
  function resetService(service?: OperatingService) {
    setServiceId(service?.id ?? null); setServiceName(service?.name ?? "新系統");
    setServiceStart(service?.start ?? ""); setServiceTracks(service ? [...service.tracks] : []);
    setServiceStops(service ? [...service.stops] : []); setServiceNextTrack("");
    setServiceDirty(false); setServiceHint(""); setPendingServiceChoice(null);
  }
  function chooseService(id: string | null) {
    if (!enabled) return;
    if(pathServiceDirty){setServiceHint("先に新形式下書きを破棄または適用してください。");return;}
    if (serviceDirty) { setPendingServiceChoice({ id }); return; }
    resetService(world?.services.find((service) => service.id === id));
  }
  function updateServicePath(start: string, tracks: string[]) {
    if (!enabled) return;
    setServiceStart(start); setServiceTracks(tracks); setServiceNextTrack("");
    setServiceStops(start ? Array.from({ length: tracks.length + 1 }, (_, index) => index) : []);
    setServiceDirty(true); setServiceHint("経路を変更したため、停車設定を全駅停車へ戻しました。");
  }
  function appendServiceTrack() {
    if (!enabled || !serviceNextTracks.some((track) => track.id === serviceNextTrack)) return;
    updateServicePath(serviceStart, [...serviceTracks, serviceNextTrack]);
  }
  function toggleServiceStop(index: number, checked: boolean) {
    if (!enabled || index === 0 || index === serviceTracks.length) return;
    setServiceStops((stops) => checked ? [...new Set([...stops, index])].sort((a,b) => a-b) : stops.filter((stop) => stop !== index));
    setServiceDirty(true);
  }
  function submitService(event: FormEvent) {
    event.preventDefault();
    if (!enabled || selectedServicePath || pathServiceDirty || !serviceName.trim() || !serviceValid || !serviceTracks.length) return;
    void perform(async () => {
      const snapshot = await invoke<World>("save_service", { serviceId, name: serviceName.trim(), start: serviceStart, tracks: serviceTracks, stops: serviceStops });
      const applied = serviceId ? snapshot.services.find((service) => service.id === serviceId)
        : snapshot.services.find((service) => !world?.services.some((existing) => existing.id === service.id));
      setWorld(snapshot); setDirty(true); setConfirmLoad(false); resetService(applied);
      setNotice("運行系統をワールドに適用しました。ファイルへの保存は「保存」で行います。");
    });
  }
  async function createPathService(draft:CreateServicePath){
    if(!enabled||busyRef.current||serviceDirty||serviceId!==null)throw Error("別の系統を編集中です。");
    busyRef.current=true;setBusy(true);try{const snapshot=await invoke<World>("create_service_path",draft);setWorld(snapshot);setDirty(true);setPathServiceDirty(false);setNotice("新形式の系統を適用しました。ファイルへは保存してください。");}finally{busyRef.current=false;setBusy(false);}
  }
  function save() {
    if (saveBlocked) return;
    void perform(async () => {
      await invoke<{ saved: boolean }>("save_world");
      setDirty(false);clearStationHistory(); setNotice(hasPlans?"建設済みワールドを保存しました。未着工計画は保存対象外です。画面上の計画は残っています。":"ワールドを保存しました。");
    });
  }
  function requestLoad(source:"manual"|"auto"){setLoadSource(source);if(dirty||unapplied)setConfirmLoad(true);else load(source);}
  function load(source: "manual"|"auto" = loadSource) {
    setConfirmLoad(false);
    void perform(async () => {
      const snapshot = await invoke<World>(source==="auto"?"load_autosave":"load_world");
      setRestoredAutosaveEpoch(source==="auto"?worldEpoch+1:-1);
      setWorldEpoch(value=>value+1);
      referenceCopy.clear();setCandidateDirty(false);setPendingGraphChange(null);
      setCompanySetupRequired(false);
      clearStationHistory();
      setPathDraftEpoch(value=>value+1);setPathServiceDirty(false);setWorld(snapshot); setDirty(source==="auto"); setParkedPlans([]);setPlanName("");setTrackType("normal");setRoadFollowing(false);setActivePlanId(1);nextPlanId.current=2;setDraftPoints([]); setRedoPoints([]); setHoverPoint(null); setStationPosition(null); setAlignmentId(""); setSelected(null); setEditStationName(""); setPendingStation(null); setPendingDelete(null); setStationFrom(""); setStationTo(""); setCompanyName(snapshot.companies[0]?.name ?? ""); resetRoute(); resetService();
      setNotice(source==="auto"?"自動保存から復元しました。手動保存データは上書きしていません。必要なら保存してください。":"保存したワールドを読み込みました。");
    });
  }

  return (
    <main className="workbench" data-theme={theme} style={{"--rail-width":lineWidth,"--map-contrast":contrast/100,"--panel-width":panelWidth+"px","--ui-font":textSize+"px"} as CSSProperties}>
      <header className="app-header">
        <div className="session-status"><span className={`connection-dot ${status}`} aria-hidden="true" />{status === "ready" ? `${info?.version} / エンジン接続済み` : status === "preview" ? "ブラウザプレビュー" : status === "loading" ? "接続中" : "接続エラー"}</div>
      </header>
      <div className="toolbar">
        <div><span className="eyebrow">CONSTRUCTION LAB</span><h2>線路を描き、街をつなぐ。</h2></div>
        <div className="save-controls"><span className="save-state">{unapplied ? "未適用の下書きあり" : dirty ? "未保存の変更あり" : "ローカルワールド"}</span><button className="secondary" disabled={!enabled} onClick={() => requestLoad("manual")}>読込</button><button disabled={!enabled || saveBlocked} onClick={save}>保存</button></div>
      </div>
      {status === "preview" && <p className="banner">ブラウザでの画面確認です。表示はサンプルデータで、駅の配置・線路接続・保存・読込はデスクトップ版で利用できます。</p>}
      {status === "loading" && <p className="banner" role="status">共通エンジンからワールドを読み込んでいます。</p>}
      {status === "error" && <div className="banner error"><p>共通エンジンに接続できませんでした。</p><button onClick={() => setAttempt((x) => x + 1)}>再試行</button></div>}
      {closeGuard.closeError && <div className="banner" role="alert">{closeGuard.closeError}</div>}
      {autosave.permission==="blocked" && <div className="banner" role="status">以前の自動保存を保護するため、自動保存を保留しています。<button className="secondary" onClick={()=>chooseMainTab("saves")}>自動保存の保留を確認</button></div>}
      {autosave.message.includes("失敗") && <div className="banner" role="status">オートセーブの確認または保存に失敗しています。<button className="secondary" onClick={()=>chooseMainTab("saves")}>自動保存の状態を確認</button></div>}
      {closeGuard.confirmClose && <div className="banner load-confirm" role="alert"><p>閉じると未着工計画（参考コピー・道路追従を含む）は消えます。未保存の変更も破棄して終了しますか。計画は再起動後に復元できません。</p><button className="secondary" onClick={closeGuard.cancelClose}>編集に戻る</button><button className="danger" onClick={()=>void closeGuard.discardAndClose()}>破棄して終了</button></div>}
      {pendingGraphChange && <div className="banner load-confirm" role="alert"><p>参考線データを変更すると、現在の参考コピー・道路追従の未着工計画を破棄します。変更しますか。</p><button className="secondary" onClick={()=>setPendingGraphChange(null)}>計画に戻る</button><button disabled={!enabled} onClick={()=>{referenceCopy.clear();setGraphBundle(pendingGraphChange.bundle);setPendingGraphChange(null);}}>参考計画を破棄して変更</button></div>}
      {confirmLoad && <div className="banner load-confirm" role="alert"><p>読込すると、未着工計画（参考コピー・道路追従を含む）、未保存の建設内容と未適用の下書きが破棄され、保存時点の状態に置き換わります。</p><button className="secondary" disabled={busy} onClick={() => setConfirmLoad(false)}>戻る</button><button disabled={busy} onClick={()=>load()}>読込を続ける</button></div>}
      {saveBlocked && <p className="draft-warning">駅設定・会社・路線・系統の下書きを適用、または破棄してから手動保存してください。自動保存も適用済みの内容だけが対象です。</p>}
      {hasPlans && <p className="draft-warning">未着工計画は保存されません。建設済みワールドは保存でき、画面上の計画はそのまま残ります。</p>}
      {unapplied && (activeTool === "stations" || activeTool === "tracks") && <p className="draft-warning">設備削除は未適用の下書きを適用・破棄してから行えます。</p>}
      {pendingDelete && <div className="banner load-confirm" role="alert"><p>{pendingDelete.kind === "station" ? "駅「" + (stationsById.get(pendingDelete.id)?.name ?? pendingDelete.id) + "」" : "線路 ID " + pendingDelete.id}を削除しますか。連鎖削除は行いません。適用後は保存が必要です。</p><button className="secondary" disabled={busy} onClick={() => setPendingDelete(null)}>削除をやめる</button><button className="danger" disabled={!enabled || unapplied || !!(pendingDelete.kind === "station" ? stationReferences(pendingDelete.id) : trackReferences(pendingDelete.id))} onClick={deleteEquipment}>削除を確定</button></div>}
      {error && !companySetupRequired && <ErrorNotice message={error} revision={errorRevision} onDismiss={()=>setError("")}/>}
      {displaySettings.error && <div className="display-settings-warning" role="status">{displaySettings.status}</div>}
      <div className="workspace">
        <section className="map-panel" aria-labelledby="map-title">
          <div className="panel-heading"><div><h3 id="map-title">{background === "sample" ? "駅配置サンプル" : provider.title}</h3><p>{background === "sample" ? "自作サンプル背景 / 実地図ではありません" : provider.title + " / オンライン"}</p></div><span className="north-label" aria-label="上が北">N ↑</span></div>
          <details className="map-options"><summary>地図・表示設定</summary><div className="display-settings"><p>背景を含む表示設定は端末内に自動保存。World保存・共有とは別です。初回はOSM（オンライン）。選択は次回も復元します。サンプル選択中は画像通信しません。サンプルは模式図で、オフライン実地図ではありません。</p><p>{displaySettings.status}</p><button type="button" className="secondary" onClick={displaySettings.reset}>表示設定を既定に戻す</button><span>配色</span><button className="secondary" onClick={() => setTheme(theme === "light" ? "dark" : "light")}>{theme === "light" ? "ダークに切替" : "ライトに切替"}</button>
  <label>線の太さ<input aria-label="線の太さ" type="range" min="2" max="8" value={lineWidth} onChange={e=>setLineWidth(Number(e.target.value))}/></label>
  <label>地図コントラスト<input aria-label="地図コントラスト" type="range" min="70" max="140" value={contrast} onChange={e=>setContrast(Number(e.target.value))}/></label>
  <label>パネル幅<input aria-label="パネル幅" type="range" min="260" max="400" value={panelWidth} onChange={e=>setPanelWidth(Number(e.target.value))}/></label>
  <label>文字サイズ<input aria-label="文字サイズ" type="range" min="11" max="16" value={textSize} onChange={e=>setTextSize(Number(e.target.value))}/></label></div><div className="background-controls">
            <span id="background-label">背景</span>
            <div className="background-choices" role="group" aria-labelledby="background-label">
              {([["osm", "OpenStreetMap（オンライン）"], ["gsi", "地理院地図（オンライン）"], ["sample", "自作サンプル（オフライン）"]] as const).map(([source, label]) =>
                <button key={source} type="button" className="secondary" aria-pressed={background === source} onClick={() => {
                  if (background === source) return;
                  setBackground(source); setRequestedCamera(camera); setTileStatus({ loaded: 0, failed: 0, total: visibleTiles(camera).length });
                }}>{label}</button>)}
            </div>
            <span>ZL {Number(camera.zoom.toFixed(2))} / Web Mercator</span><span>駅表示：ZL15以上はホーム帯、ZL15未満は点（寸法は不変）</span>
          </div>
          <div className="camera-controls" aria-label="地図表示操作">
            <div className="map-mode-controls" role="group" aria-label="地図の操作モード"><button className="secondary" aria-pressed={mapMode === "move"} onClick={() => { changeMapMode("move"); }}>移動</button><button className="secondary" aria-pressed={mapMode === "place"} disabled={!enabled || activeTool !== "stations"} onClick={() => { changeMapMode("place"); }}>駅配置</button><span>ドラッグで移動 / 縦ホイールでカーソル位置をズーム</span></div>
            <div className="camera-buttons"><button className="secondary" aria-label="西へ移動" onClick={() => changeCamera(panCamera(camera, -300, 0))}>←</button><button className="secondary" aria-label="北へ移動" onClick={() => changeCamera(panCamera(camera, 0, -180))}>↑</button><button className="secondary" aria-label="南へ移動" onClick={() => changeCamera(panCamera(camera, 0, 180))}>↓</button><button className="secondary" aria-label="東へ移動" onClick={() => changeCamera(panCamera(camera, 300, 0))}>→</button><button className="secondary" aria-label="縮小" disabled={camera.zoom <= MIN_ZOOM} onClick={() => zoomFromControls(-1)}>−</button><button className="secondary" aria-label="拡大" disabled={camera.zoom >= MAX_ZOOM} onClick={() => zoomFromControls(1)}>＋</button><button className="secondary" onClick={() => changeCamera(DEFAULT_CAMERA)}>東京へ戻る</button></div>
            <form className="camera-coordinate-form" onSubmit={moveToCoordinates}><label htmlFor="camera-latitude">中心緯度</label><input id="camera-latitude" type="number" step="any" min={BOUNDS.south} max={BOUNDS.north} value={centerLatitude} onChange={(event) => setCenterLatitude(event.target.value)} /><label htmlFor="camera-longitude">中心経度</label><input id="camera-longitude" type="number" step="any" min={BOUNDS.west} max={BOUNDS.east} value={centerLongitude} onChange={(event) => setCenterLongitude(event.target.value)} /><button className="secondary" type="submit">座標へ移動</button></form>
            <p>閲覧・建設範囲: 北緯20〜46度 / 東経122〜154度。領土境界を示すものではありません。範囲端では表示を矩形内に制限します。</p>
            {cameraError && <p role="alert">{cameraError}</p>}
          </div>
          <ReferenceGraphControls bundle={graphBundle} visible={graphVisible} onBundle={bundle=>{if(referenceCopy.active)setPendingGraphChange({bundle});else setGraphBundle(bundle);}} onVisible={setGraphVisible} total={graphDisplay.total} shown={graphDisplay.edges.length} zoom={camera.zoom}/>
          <ReferenceStationControls pack={referencePack} visible={referenceVisible} onPack={setReferencePack} onVisible={setReferenceVisible} total={referenceResult.total} shown={referenceResult.stations.length}/>
          </details>
          {background !== "sample" && <div className={`tile-status ${tileStatus.failed ? "tile-error" : ""}`}>
            <p role="status">{cameraPending ? "表示移動中。新しいタイルの取得を待機しています。" : tileStatus.loaded + tileStatus.failed < tileStatus.total ? `${provider.title}を読み込み中（${tileStatus.loaded} / ${tileStatus.total}）` : tileStatus.failed ? "地図の一部を表示できません。" : `${provider.title}を表示中（${tileStatus.total}枚）`}</p>
            {tileStatus.failed > 0 && <p role="alert">{tileStatus.failed}枚の読み込みに失敗しました。自動再試行やサンプルへの自動切替は行いません。</p>}
            <button className="secondary" onClick={() => setBackground("sample")}>サンプル背景に戻す</button>
          </div>}
          {activeTool === "tracks" && !referenceCopy.active && !dragging && planHover && hoveredPlan && hoveredPlan.line.length > 1 && <div className="construction-tooltip" role="status" style={{left:Math.max(8,Math.min(mapSize.width-240,planHover.x+18)),top:Math.max(8,Math.min(mapSize.height-145,planHover.y+18))}}><strong>{hoveredPlan.name.trim() || "線路計画"} · {Math.round(alignmentLength(hoveredPlan.line))} m</strong><div>{hoveredGeometry?.error ? "敷設不可："+hoveredGeometry.error : "基本条件OK（地形等は未判定）"}</div><small>最小半径（近似）：{Number.isFinite(hoveredGeometry?.minimumRadiusM ?? Infinity) ? Math.round(hoveredGeometry!.minimumRadiusM)+" m" : "直線"}<br/>制限速度：未算定<br/>費用倍率・建設費・維持費：未計算</small></div>}
            <div className="map-surface">
            <svg ref={svgRef} onPointerDown={pointerDown} onPointerMove={pointerMove} onPointerUp={(event) => pointerEnd(event)} onPointerCancel={(event) => pointerEnd(event, true)} onPointerLeave={() => { referenceCopy.clearHover();setPlanHover(null); setHoverPoint(null); if (gestureRef.current && !gestureRef.current.dragged) cancelMapGesture(); }} onLostPointerCapture={(event) => { if (event.target === event.currentTarget) pointerEnd(event, true); }} role="img" aria-label={background === "sample" ? "駅配置用サンプル地図" : background === "osm" ? "駅配置用OpenStreetMap" : "駅配置用地理院地図"} viewBox={`0 0 ${MAP_VIEWBOX.width} ${MAP_VIEWBOX.height}`} preserveAspectRatio="xMidYMid slice" onClick={mapClick} className={dragging ? "map-dragging" : enabled && mapMode !== "move" ? "placement-enabled" : "map-move-mode"}>
              <defs>
                <clipPath id="map-extent"><rect width={MAP_VIEWBOX.width} height={MAP_VIEWBOX.height} /></clipPath>
                <pattern id="sample-scene" data-sample-scene="true" width="1000" height="600" patternUnits="userSpaceOnUse" patternTransform={sampleTransform}>
                  <rect width="1000" height="600" fill="#e9efea" />
                  <path d="M 0 300 C 166 100 334 100 500 300 S 834 500 1000 300" fill="none" stroke="#bfd8dc" strokeWidth="60" />
                  <path d="M 0 150 H 1000 M 0 450 H 1000 M 200 0 V 600 M 700 0 V 600" fill="none" stroke="#f8faf7" strokeWidth="12" />
                </pattern>
                <pattern id="sample-grid" width="100" height="100" patternUnits="userSpaceOnUse" patternTransform={sampleTransform}><path d="M 100 0 L 0 0 0 100" fill="none" stroke="#cedbd6" strokeWidth="1" /></pattern>
              </defs>
              <g clipPath="url(#map-extent)">
                {background !== "sample" ? <GsiBackground key={background} tiles={displayTiles} camera={camera} width={MAP_VIEWBOX.width} height={MAP_VIEWBOX.height} onStatus={setTileStatus} /> : <g className="sample-background">
                  <rect width={MAP_VIEWBOX.width} height={MAP_VIEWBOX.height} fill="url(#sample-scene)" />
                  <rect width={MAP_VIEWBOX.width} height={MAP_VIEWBOX.height} fill="url(#sample-grid)" />
                </g>}
              <ReferenceGraphLayer edges={graphDisplay.edges} camera={camera}/>
              {alignments.map((alignment) => <g key={alignment.id} className="alignment route-base" data-alignment-id={alignment.id}>
                <title>{alignment.name}</title>
                <path d={drawPath(alignment.points)} className="track-outline" fill="none" />
                <path d={drawPath(alignment.points)} className="alignment-sleepers" />
                <path d={drawPath(alignment.points)} className={alignmentId === alignment.id ? "alignment-path alignment-selected" : "alignment-path"} />
                {routeVisual.alignmentSections.get(alignment.id)?.map((points,index)=><path key={index} d={drawPath(points)} className="alignment-path route-assigned-section" />)}
                <path d={drawPath(alignment.points)} className="alignment-hit" onClick={(event) => { if (mapMode === "move" && !suppressSelectionClick.current) { event.stopPropagation(); setAlignmentId(alignment.id); } }} />
              </g>)}
              {referenceCopy.active&&referenceCopy.roadFollow&&(referenceCopy.candidates.length?referenceCopy.candidates:referenceCopy.hoverCandidates).map(candidate=>{const a=graphNodesById.get(candidate.edge.from),b=graphNodesById.get(candidate.edge.to);return a&&b?<path key={candidate.edge.id} d={drawPath([a,b])} className="road-follow-candidate" stroke="#298746" strokeWidth="4" opacity=".7" fill="none" pointerEvents="none"/>:null;})}
              {referenceCopy.active&&referenceCopy.plan.build&&<path d={drawPath(referenceCopy.plan.build.points)} className="draft-path reference-copy-preview" style={referenceCopy.roadFollow?{stroke:"#298746"}:undefined} pointerEvents="none"/>}
              {parkedPlans.map(plan=><g key={plan.id}><path d={drawPath(plan.line)} className="draft-path parked-plan" data-plan-id={plan.id} pointerEvents="none" /><path d={drawPath(plan.line)} className="plan-hover-hit" data-plan-hover-id={plan.id} fill="none" stroke="transparent" strokeWidth="14" pointerEvents="stroke" onPointerEnter={event=>hoverPlan(event,plan.id)} onPointerMove={event=>hoverPlan(event,plan.id)} onPointerLeave={()=>setPlanHover(null)} /></g>)}
              {!referenceCopy.active && guideCenter && guideEdge && <circle className="curve-circle-guide" cx={guideCenter.x} cy={guideCenter.y} r={Math.hypot(guideEdge.x-guideCenter.x,guideEdge.y-guideCenter.y)} fill="none" stroke={previewError ? "#b34c4c" : "#65716b"} strokeWidth="1" opacity=".6" vectorEffect="non-scaling-stroke" pointerEvents="none" />}
              {!referenceCopy.active&&previewLine.length > 1 && <path d={drawPath(previewLine)} className="draft-path" style={previewError ? {stroke:"#b34c4c"} : undefined} />}
              {!referenceCopy.active && draftLine.length > 1 && <path d={drawPath(draftLine)} className="plan-hover-hit" data-plan-hover-id={activePlanId} fill="none" stroke="transparent" strokeWidth="14" pointerEvents="stroke" onPointerEnter={event=>hoverPlan(event,activePlanId)} onPointerMove={event=>hoverPlan(event,activePlanId)} onPointerLeave={()=>setPlanHover(null)} />}
              {draftPoints.map((point,index) => { const p = project(point.latitude,point.longitude,camera); return <rect key={index} x={p.x-4} y={p.y-4} width="8" height="8" className="draft-anchor" />; })}
              {stationPreview.length >= 4 && (detailedStations ? <path d={drawPath(stationPreview) + "Z"} className="station-platform" opacity=".65" pointerEvents="none" /> : <circle cx={project(previewCenter!.latitude,previewCenter!.longitude,camera).x} cy={project(previewCenter!.latitude,previewCenter!.longitude,camera).y} r="5" className="station-platform station-dot" opacity=".65" pointerEvents="none" />)}
              {world?.tracks.map((track) => {
                const fromPlacement = world?.station_placements?.find((p) => p.station_id === track.from);
                const toPlacement = world?.station_placements?.find((p) => p.station_id === track.to);
                if (fromPlacement && toPlacement && fromPlacement.alignment_id === toPlacement.alignment_id) return null;
                const from = stationsById.get(track.from);
                const to = stationsById.get(track.to);
                if (!from || !to) return null;
                const a = project(from.latitude, from.longitude, camera);
                const b = project(to.latitude, to.longitude, camera);
                const points = { x1: a.x, y1: a.y, x2: b.x, y2: b.y };
                return <g key={track.id} className={"track-connection" + (routeVisual.tracks.has(track.id) ? "" : " route-unassigned")} data-track-id={track.id} onClick={(event) => event.stopPropagation()}>
                  <title>{from.name} — {to.name}</title>
                  <line {...points} className="track-outline" /><line {...points} className="track-line" />
                </g>;
              })}
              {referenceVisible && referenceResult.stations.map(station=>{const p=project(station.latitude,station.longitude,camera);return <g key={station.osmType+"/"+station.osmId} className="reference-station" pointerEvents="none"><title>参考駅: {station.name} ({station.kind})</title><path d={`M${p.x},${p.y-5} l5,5 -5,5 -5,-5 Z`}/><text x={p.x+9} y={p.y-7}>参考: {station.name}</text></g>;})}
              {world?.stations.map((station) => {
                const { x, y } = project(station.latitude, station.longitude, camera);
                const placement = world?.station_placements?.find((p) => p.station_id === station.id);
                const polygon = stationPolygons.get(station.id);
                return <g key={station.id} className={"station-marker" + (selected === station.id ? " selected" : "") + (routeVisual.stations.has(station.id) ? "" : " route-unassigned")} onClick={(event) => { event.stopPropagation(); if (!suppressSelectionClick.current) selectStation(station.id); }}>
                  <title>{station.name}{placement ? ` / ホーム ${placement.length_m}m` : ""}</title>{!detailedStations ? <circle cx={x} cy={y} r="5" className="station-dot" /> : polygon ? <path d={drawPath(polygon) + "Z"} className="station-platform" /> : <rect x={x - 13} y={y - 5} width="26" height="10" className="station-platform" />}<text x={x + 18} y={y + 5}>{station.name}</text>
                </g>;
              })}
              </g>
            </svg>
            <div className="map-instruction">{status !== "ready" ? "ドラッグで地図を移動 / 建設はデスクトップ版" : mapMode === "draw" ? "クリックで線形点を追加 / 終端をダブルクリックで計画終了" : mapMode === "place" ? "線路の近くをクリック → ホームを配置" : "ドラッグで移動 · ホイールで拡大縮小"}</div>
            <div className="map-coordinates">{bounds.south.toFixed(4)}–{bounds.north.toFixed(4)}°N / {bounds.west.toFixed(4)}–{bounds.east.toFixed(4)}°E</div>
          </div>
          <footer className="map-footer">
            {referenceVisible && referencePack && <div className="reference-attribution">参考駅: {referencePack.dataset.source} / {referencePack.dataset.license}（背景・ゲーム駅とは別）<br/><a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener">参考駅データ: © OpenStreetMap contributors</a></div>}
            {graphBundle&&graphVisible&&<span>参考線: © <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener">OpenStreetMap contributors</a> / セッション限定・網羅性未保証</span>}
            {background === "sample" ? <span>背景は独自作成の模式図で、地理的な対応はありません。外部タイルは使用していません。</span> : <div className="map-attribution">
              <a href={provider.attributionUrl} target="_blank" rel="noopener">{provider.attribution}</a>
              <p>架空の鉄道設備を重畳表示。背景地図とは別データです。</p>
              <p>画像はオンライン取得。アプリによるオフライン保存・同梱は行いません。</p>
            </div>}
            <span>線形は試作 / 勾配・分岐・OSMネットワーク自動追従は未対応</span>
          </footer>
        </section>
        <aside className={panelCollapsed ? "side-panel collapsed" : "side-panel"}><div className="inspector-title"><span>RAILWAY WORKSPACE</span><button className="secondary" aria-label={panelCollapsed ? "操作パネルを開く" : "操作パネルを畳む"} onClick={() => setPanelCollapsed(!panelCollapsed)}>{panelCollapsed ? "+" : "−"}</button></div>
          <WorkspaceTabs value={mainTab} onChange={chooseMainTab}/>
          <nav className="editor-tabs construction-tabs" role="tablist" aria-label="建設ツール" hidden={mainTab!=="construction"}>
            {([["tracks","線路"],["stations","駅設置"]] as const).map(([tool,label],index)=><button key={tool} role="tab" id={"tab-"+tool} aria-controls={"panel-"+tool} aria-selected={activeTool===tool} tabIndex={activeTool===tool?0:-1} onClick={()=>{setError("");setActiveTool(tool);changeMapMode(tool==="tracks"?"draw":"move");}} onKeyDown={event=>{if(!["ArrowLeft","ArrowRight","Home","End"].includes(event.key))return;event.preventDefault();const next=event.key==="Home"?"tracks":event.key==="End"?"stations":index===0?"stations":"tracks";setActiveTool(next);document.getElementById("tab-"+next)?.focus();}}>{label}</button>)}
          </nav>
          <section id="workspace-panel-finance" role="tabpanel" aria-labelledby="workspace-tab-finance" hidden={mainTab!=="finance"}><h3>財務</h3><p>未実装です。収支・運賃・経費の画面は今後追加します。</p></section>
          <section id="workspace-panel-city" role="tabpanel" aria-labelledby="workspace-tab-city" hidden={mainTab!=="city"}><h3>都市</h3><p>未実装です。人口・需要・都市発展の画面は今後追加します。</p></section>
          <section id="workspace-panel-saves" role="tabpanel" aria-labelledby="workspace-tab-saves" hidden={mainTab!=="saves"}><h3>セーブデータ</h3><p>この試作は1つのローカル保存先を使います。セーブ一覧・複数スロットは未実装です。</p><button disabled={!enabled||saveBlocked} onClick={save}>ワールドを保存</button><button className="secondary" disabled={!enabled} onClick={()=>requestLoad("manual")}>ワールドを読み込む</button><h4>オートセーブ</h4><label>自動保存間隔<select aria-label="自動保存間隔" value={autosave.minutes} onChange={event=>autosave.setMinutes(Number(event.target.value))}><option value={0}>無効</option><option value={1}>1分</option><option value={5}>5分</option><option value={10}>10分</option></select></label><p>適用済みワールドのみを専用ファイルに自動保存します。未着工計画・未適用編集は対象外。手動保存を上書きしません。最新1件のみ保持します。</p><p role="status">{autosave.message}</p>{autosave.permission==="blocked"&&<button className="secondary" disabled={!enabled} onClick={autosave.allowReplacement}>このワールドで自動保存を開始（以前の自動保存を置き換える）</button>}{autosave.permission==="failed"&&<button className="secondary" disabled={status!=="ready"||busy} onClick={autosave.retryStatus}>自動保存データを再確認</button>}<button className="secondary" disabled={!enabled||!autosave.available} onClick={()=>requestLoad("auto")}>自動保存から復元</button></section>
          <div id="workspace-panel-construction" className="construction-content" role="tabpanel" aria-labelledby="workspace-tab-construction" hidden={mainTab!=="construction"}>
          <section className="station-editor" role="tabpanel" id="panel-stations" aria-labelledby="tab-stations" hidden={activeTool !== "stations"}>
            <div className="section-number">02 / STATION</div><h3 id="editor-title">線路にホームを置く</h3>
            <p className="muted">建設した線路を選び、ホームを置く位置をクリック。カーブにも沿って配置します。</p>
            <form onSubmit={commitPlatform}><fieldset disabled={!enabled || !alignments.length}>
              <label htmlFor="platform-alignment" className="track-to-label">配置先の線路</label><select id="platform-alignment" value={alignmentId} onChange={(event) => { setError(""); setAlignmentId(event.target.value); setStationPosition(null); }}><option value="">クリックした線路を自動選択</option>{alignments.map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}</select>
              <label htmlFor="platform-position" className="track-to-label">線路上の位置 {stationPosition === null ? "未選択" : (stationPosition * 100).toFixed(1) + "%"}</label><input id="platform-position" type="range" min="0" max="100" step="0.1" disabled={!selectedAlignment} value={(stationPosition ?? .5) * 100} onChange={(event) => {setError("");setStationPosition(Number(event.target.value) / 100);}} />
              <div className="coordinate-fields"><PlatformLengthInput id="platform-length" label="ホーム長 (20m級・両相当)" value={platformLength} onChange={setPlatformLength}/></div>
              <button type="button" className="secondary full-width" aria-pressed={mapMode === "place"} onClick={() => changeMapMode(mapMode === "place" ? "move" : "place")}>地図でホーム位置を選ぶ</button>
              <button className="full-width track-submit" type="submit" disabled={!enabled || stationPosition === null || stationPreview.length < 4}>ホームを配置</button>
              <p className="field-hint">{stationPosition === null ? "地図で位置を選ぶとプレビューが表示されます。" : stationPreview.length < 4 ? "この線路に収まるホーム長を指定してください。" : "位置選択済み。寸法を確認して配置してください。"}</p>
            </fieldset></form>
            <StationNamePreview pack={referencePack} repository={referenceRepository} point={stationPreview.length>=4?previewCenter:null} existingNames={world?.stations.map(s=>s.name)??[]}/>
            {!alignments.length && <p className="field-hint">まず「線路」で線路を建設してください。</p>}
            <p className="tool-helper">ホーム長は実寸で保存。編成長による収容判定は今後実装します。</p>
            <details className="legacy-tools"><summary>旧ワールド用：単独駅の座標配置</summary>
            <form onSubmit={submitStation}><fieldset disabled={!enabled}>
              <div className="coordinate-fields"><div><label htmlFor="station-latitude">緯度</label><input id="station-latitude" type="number" min={BOUNDS.south} max={BOUNDS.north} step="any" value={latitude} onChange={(event) => setLatitude(event.target.value)} /></div><div><label htmlFor="station-longitude">経度</label><input id="station-longitude" type="number" min={BOUNDS.west} max={BOUNDS.east} step="any" value={longitude} onChange={(event) => setLongitude(event.target.value)} /></div></div>
              <button className="full-width" type="submit">指定座標に配置</button>
            </fieldset></form></details>
          </section>
          <section className="track-editor" role="tabpanel" id="panel-tracks" aria-labelledby="tab-tracks" hidden={activeTool !== "tracks"}>
            <div className="section-number">01 / CONSTRUCTION</div><h3>線路を描く</h3>
            <p className="muted">駅より先に線路を建設。地図に点を打って経路を決めます。</p>
            <TrackTypeControls value={trackType} roadFollowing={roadFollowing} disabled={!enabled||referenceCopy.active} onChange={chooseTrackType} onRoadFollowing={value=>{setRoadFollowing(value);setHoverPoint(null);}}/>
            <ReferenceCopyControls copy={{...referenceCopy,begin:()=>{referenceCopy.begin();changeMapMode("draw");setHoverPoint(null);}}} enabled={enabled} canStart={copyCanStart} hasBundle={!!graphBundle}/>
            <div className="segmented"><button className="secondary" disabled={!enabled||referenceCopy.active} aria-pressed={curved} onClick={() => setCurved(true)}>曲線</button><button className="secondary" disabled={!enabled||referenceCopy.active} aria-pressed={!curved} onClick={() => setCurved(false)}>直線</button></div>
            <button className="full-width" disabled={!enabled} aria-pressed={mapMode === "draw"} onClick={() => changeMapMode(mapMode === "draw" ? "move" : "draw")}>{mapMode === "draw" ? "描画を中断して移動" : "地図で線路を描く"}</button>
            <button className="secondary full-width" disabled={!enabled || referenceCopy.active || (!draftPoints.length && !redoPoints.length)} onClick={newIndependentPlan}>別の線路計画を追加</button>
            <div role="group" aria-label="未着工の線路計画"><strong>編集中：{planName.trim() || `計画 ${activePlanId}`}</strong>{parkedPlans.map(plan=><button key={plan.id} className="secondary" disabled={!enabled || referenceCopy.active} onClick={()=>selectIndependentPlan(plan.id)}>{plan.name.trim() || `計画 ${plan.id}`} を編集（{Math.round(alignmentLength(plan.line))} m）</button>)}</div>
            <p className="tool-helper">離れた経路も同時に計画できます。着工・消去は編集中の計画だけが対象です。タブ移動では計画は残ります。ゲームを終了すると未着工の計画は失われます。</p>
            <label htmlFor="construction-plan-name">計画名（任意）</label><input id="construction-plan-name" maxLength={80} placeholder={`計画 ${activePlanId}`} disabled={!enabled || referenceCopy.active} value={planName} onChange={event=>setPlanName(event.target.value)}/><p className="field-hint">名前なしでも計画・着工できます。路線名や線路名ではなく、この画面で計画を見分けるための名前です。</p>
            <div className="build-summary"><div><strong>{draftPoints.length}</strong><span>線形点</span></div><div><strong>{alignments.length}</strong><span>建設済み線路</span></div></div>
            <div className="route-actions"><button className="secondary" disabled={!enabled || !draftPoints.length} onClick={undoPoint}>1点戻す</button><button className="secondary" disabled={!enabled || !redoPoints.length} onClick={redoPoint}>やり直す</button><button className="secondary" disabled={!enabled || (!draftPoints.length && !redoPoints.length)} onClick={() => {setPlanName("");setDraftPoints([]);setRedoPoints([]);setHoverPoint(null);}}>下書きを消去</button></div>
            <button className="full-width track-submit" disabled={!enabled || roadFollowingBlocked || referenceCopy.active || draftLine.length < 2 || draftInvalid || !alignmentName.trim()} onClick={commitAlignment}>計画を着工</button><p className="tool-helper">計画は着工まで未確定です。建設費・維持費は未計算。</p>
            <p className="tool-helper">ドラッグは移動。自由描画にはOSM追従なし。参考形状コピーは別モードです。</p>
            <ul className="construction-list">{alignments.map((a) => <li key={a.id}><button className="secondary" aria-pressed={a.id === alignmentId} onClick={() => { setAlignmentId(a.id); changeCamera(cameraAt(a.points[0].latitude,a.points[0].longitude,camera.zoom)); }}>{a.name}（{TRACK_TYPE_LABELS[a.track_type]??"通常"}）<small>選択</small></button><small>{world?.alignment_provenance?.some(p=>p.alignment_id===a.id)?"参考OSM由来":"出典記録なし"}</small></li>)}</ul>
            {selectedAlignment&&<AlignmentProvenance alignment={selectedAlignment} provenance={world?.alignment_provenance?.find(p=>p.alignment_id===selectedAlignment.id)} sourceRecords={world?.source_records??[]}/>}
            <details className="legacy-tools"><summary>運行経路用：駅間接続</summary>
            <div className="track-heading"><h3 id="track-editor-title">駅を線路でつなぐ</h3><span className="count" aria-label="線路数">{world?.tracks.length ?? 0}</span></div>
            <p className="muted">2駅を直線で接続します。線形・距離・速度はまだ再現していません。</p>
            <form onSubmit={submitTrack}><fieldset disabled={!enabled}>
              <label htmlFor="track-from">接続元の駅</label>
              <select id="track-from" value={stationFrom} onChange={(event) => setStationFrom(event.target.value)}><option value="">駅を選択</option>{world?.stations.map((station) => <option key={station.id} value={station.id}>{station.name} (ID {station.id})</option>)}</select>
              <label htmlFor="track-to" className="track-to-label">接続先の駅</label>
              <select id="track-to" value={stationTo} onChange={(event) => setStationTo(event.target.value)}><option value="">駅を選択</option>{world?.stations.map((station) => <option key={station.id} value={station.id}>{station.name} (ID {station.id})</option>)}</select>
              {stationFrom && stationFrom === stationTo && <p className="field-hint">異なる2駅を選択してください。</p>}
              <button className="full-width track-submit" type="submit" disabled={!enabled || !validEndpoints}>線路を接続</button>
            </fieldset></form>
            {world?.tracks.length ? <ul className="track-list">{world.tracks.map((track) => <li key={track.id}><span>{stationsById.get(track.from)?.name ?? track.from} ↔ {stationsById.get(track.to)?.name ?? track.to} (ID {track.id})</span><button className="secondary danger" disabled={!enabled || unapplied || !!trackReferences(track.id)} onClick={() => setPendingDelete({ kind: "track", id: track.id })}>線路 {track.id} を削除</button>{trackReferences(track.id) && <small>削除不可: {trackReferences(track.id)}</small>}</li>)}</ul> : <p className="field-hint">駅を2つ配置すると接続できます。</p>}</details>
          </section>
          </div>
          <section hidden={activeTool!=="stations" && activeTool!=="station-management"} className="station-history"><p className="field-hint">取消対象: {stationUndo.at(-1)?.station.name ?? "なし"} / Undo {stationUndo.length}件・Redo {stationRedo.length}件</p><div className="route-actions"><button type="button" className="secondary" disabled={!enabled || !stationUndo.length} onClick={undoStation}>駅設置を取り消す</button><button type="button" className="secondary" disabled={!enabled || !stationRedo.length} onClick={redoStation}>駅設置をやり直す</button></div><p className="field-hint">Ctrl+Z / Ctrl+Shift+Z：このセッションの新規駅設置のみ（最大50件）。参照付き駅は不可。保存・読込・駅名/寸法変更・手動駅削除で履歴終了。Redoは駅IDを再発行。参照駅は対象外。線路ツールの点下書きがあれば点のUndo/Redoを優先します。</p></section>
          <section id="workspace-panel-stations" role="tabpanel" className="station-directory" aria-labelledby="workspace-tab-stations" hidden={activeTool !== "stations" && activeTool !== "station-management"}><div className="directory-heading"><h3 id="stations-title">配置済みの駅</h3><span className="count">{world?.stations.length ?? 0}</span></div>
            {world?.stations.length ? <ul>{world.stations.map((station) => <li key={station.id}><button className={`station-entry ${selected === station.id ? "selected" : ""}`} onClick={() => selectStation(station.id)}><span>{station.name}</span><small>{station.latitude.toFixed(5)}, {station.longitude.toFixed(5)}</small></button></li>)}</ul> : <p className="empty-state">まだ駅はありません。<br />最初の駅を配置してみましょう。</p>}
          </section>
          <section hidden={activeTool !== "stations" && activeTool !== "station-management"}>
            {selectedStation && <div className="equipment-editor"><h4>駅設定 · ID {selectedStation.id}</h4><p className="field-hint">設置後も駅名・ホーム長を変更できます。有人・無人・委託などの運営形態、維持費・満足度への反映は今後実装予定です。</p><div className="station-detail-name"><strong onDoubleClick={beginStationNameEdit}>{selectedStation.name}</strong><button ref={stationNameEditButton} type="button" className="secondary" disabled={!enabled} aria-label="選択駅の名前を編集" onClick={beginStationNameEdit}>✎ 編集</button></div><p className="field-hint">駅名をダブルクリック、または編集ボタンで入力へ。Enterで適用、Escで取消。</p>
              {world?.station_placements?.some((p) => p.station_id === selectedStation.id) && <form onSubmit={resizePlatform}><fieldset disabled={!enabled}><div className="coordinate-fields"><PlatformLengthInput id="edit-platform-length" label="選択駅のホーム長 (20m級・両相当)" value={editLength} onChange={setEditLength}/></div><div className="route-actions"><button type="submit" className="secondary" disabled={!platformDirty || !editLength || !Number.isFinite(Number(editLength)) || Number(editLength) < 20 || Number(editLength) > 1000}>ホーム寸法を適用</button><button type="button" className="secondary" disabled={!platformDirty} onClick={() => { if(selectedPlacement){setEditLength(String(selectedPlacement.length_m));setEditWidth(String(selectedPlacement.width_m));} }}>ホーム長の下書きを破棄</button></div></fieldset></form>}
              <form onSubmit={renameStation}><fieldset disabled={!enabled}><label htmlFor="edit-station-name">選択駅の名前</label><input ref={stationNameInput} id="edit-station-name" onKeyDown={event=>{if(event.key!=="Escape" || event.nativeEvent.isComposing || event.nativeEvent.keyCode===229)return;event.preventDefault();event.stopPropagation();if(!enabled || busyRef.current)return;setEditStationName(selectedStation.name);stationNameEditButton.current?.focus();}} maxLength={80} value={editStationName} onChange={(event) => setEditStationName(event.target.value)} /><div className="route-actions"><button type="submit" disabled={!enabled || !stationDirty || !editStationName.trim()}>駅名を適用</button><button type="button" className="secondary" disabled={!enabled || !stationDirty} onClick={() => setEditStationName(selectedStation.name)}>駅名下書きを破棄</button></div></fieldset></form>
              <button className="secondary danger" disabled={!enabled || unapplied || !!stationReferences(selectedStation.id)} onClick={() => setPendingDelete({ kind: "station", id: selectedStation.id })}>選択駅を削除</button>
              {stationReferences(selectedStation.id) && <p className="field-hint">削除不可: {stationReferences(selectedStation.id)}</p>}
            </div>}
            {pendingStation && <div className="inline-confirm" role="alert"><p>{platformDirty ? "駅名・ホーム長の下書きを破棄して別駅を選びますか。" : "駅名の下書きを破棄して別駅を選びますか。"}</p><button className="secondary" disabled={busy} onClick={() => setPendingStation(null)}>駅名編集を続ける</button><button disabled={busy} onClick={() => { const placement = world?.station_placements?.find(p=>p.station_id===pendingStation.id); setEditLength(String(placement?.length_m ?? 200));setEditWidth(String(placement?.width_m ?? 12));setMainTab("stations"); setActiveTool("station-management");setSelected(pendingStation.id); setEditStationName(stationsById.get(pendingStation.id)?.name ?? ""); setPendingStation(null); setPendingDelete(null); }}>{platformDirty ? "駅設定下書きを破棄して切替" : "駅名下書きを破棄して切替"}</button></div>}
          </section>
          <section className="route-editor" role="tabpanel" id="workspace-panel-routes" aria-labelledby="workspace-tab-routes" hidden={activeTool !== "routes"}>
            <div className="section-number">03 / ROUTE</div>
            <h3>路線を定義する</h3><p className="muted">線路設備とは独立した、順序付きの経路です。停車駅やダイヤは設定しません。</p>
            <RouteCandidatePreview resetKey={worldEpoch} onDirty={setCandidateDirty} stations={world?.stations ?? []} worldVersion={world} enabled={enabled} canApply={!routeDirty} onSave={applyPlannedRoute} onApply={(start, tracks) => {
              resetRoute(); setRouteStart(start); setRouteTracks(tracks); setRouteDirty(true);
              setRouteHint("候補を新規路線の下書きへ取り込みました。路線名と経路を確認して適用してください。");
            }}/>
            <div className="route-directory"><button className="secondary" disabled={!enabled} onClick={() => chooseRoute(null)}>新規路線</button>
              {world?.routes.map((route) => <button key={route.id} className={`route-choice ${route.id === routeId ? "selected" : ""}`} disabled={!enabled} onClick={() => chooseRoute(route.id)}>編集: {route.name}</button>)}
            </div>
            {pendingRouteChoice && <div className="inline-confirm" role="alert"><p>未適用の路線下書きを破棄して切り替えますか。</p><button className="secondary" disabled={busy} onClick={() => setPendingRouteChoice(null)}>編集を続ける</button><button disabled={busy} onClick={() => resetRoute(world?.routes.find((route) => route.id === pendingRouteChoice.id))}>下書きを破棄して切替</button></div>}
            {selectedRoutePath && <section aria-label="保存済み路線の経路"><h4>{routeName}</h4><p>新方式の路線です。旧手動エディターでは上書きできません。既存の所属駅変更は未対応です。</p><p>所属駅: {selectedRoutePath.member_stations.map(id=>stationsById.get(id)?.name ?? id).join(" → ")}</p><ol>{selectedRoutePath.segments.map((segment,index)=><li key={index}>{segment.kind === "track" ? "駅間接続 " + segment.track_id : "建設線形 " + (alignments.find(a=>a.id===segment.alignment_id)?.name ?? segment.alignment_id) + ": " + (stationsById.get(segment.from)?.name ?? segment.from) + " → " + (stationsById.get(segment.to)?.name ?? segment.to)}</li>)}</ol></section>}
            <form onSubmit={submitRoute}><fieldset disabled={!enabled || !!selectedRoutePath}>
              <label htmlFor="route-name">路線名</label><input id="route-name" value={routeName} maxLength={80} onChange={(event) => { setRouteName(event.target.value); setRouteDirty(true); }} />
              <label htmlFor="route-start" className="track-to-label">路線の起点</label><select id="route-start" value={routeStart} onChange={(event) => { setRouteStart(event.target.value); setRouteTracks([]); setNextTrack(""); setRouteDirty(true); setRouteHint("起点を変更したため、経路をクリアしました。"); }}><option value="">駅を選択</option>{world?.stations.map((station) => <option key={station.id} value={station.id}>{station.name} (ID {station.id})</option>)}</select>
              <label htmlFor="route-next" className="track-to-label">次にたどる線路</label><select id="route-next" value={nextTrack} onChange={(event) => setNextTrack(event.target.value)}><option value="">接続する線路を選択</option>{nextTracks.map((track) => { const next = track.from === routeEnd ? track.to : track.from; return <option key={track.id} value={track.id}>{stationsById.get(next)?.name ?? next} (線路 {track.id})</option>; })}</select>
              <button className="secondary full-width track-submit" type="button" disabled={!enabled || !nextTrack} onClick={appendTrack}>経路に追加</button>
              {routeHint && <p className="field-hint">{routeHint}</p>}
              <ol className="route-walk" aria-label="路線の経由順序">{routeStations.map((id, index) => <li key={`${index}-${id}`}>{stationsById.get(id)?.name ?? id}</li>)}</ol>
              <div className="route-actions"><button type="button" className="secondary" disabled={!enabled || !routeTracks.length} onClick={() => { setRouteTracks((tracks) => tracks.slice(0, -1)); setNextTrack(""); setRouteDirty(true); }}>末尾を戻す</button><button type="button" className="secondary" disabled={!enabled || !routeTracks.length} onClick={() => { setRouteTracks([]); setNextTrack(""); setRouteDirty(true); }}>経路をクリア</button></div>
              <button className="full-width track-submit" type="submit" disabled={!enabled || !routeName.trim() || !routeValid || !routeTracks.length}>路線を適用</button>
              <button className="secondary full-width track-submit" type="button" disabled={!enabled || !routeDirty} onClick={() => chooseRoute(routeId)}>下書きを破棄</button>
              <p className="field-hint">{routeId ? `編集中の路線 ID: ${routeId}` : "新規路線の下書き"} / 適用後に「保存」でファイルへ反映</p>
            </fieldset></form>
          </section>
          <section className="service-editor" role="tabpanel" id="workspace-panel-services" aria-labelledby="workspace-tab-services" hidden={activeTool !== "services"}>
            <div className="section-number">04 / SERVICE</div><h3>運行系統を定義する</h3>
            <p className="muted">路線とは独立した経路と停車位置を設定します。同じ駅への再訪も、経路上の位置ごとに区別します。種別・ダイヤ・列車運行は未実装です。</p>
            <ServicePathCreator key={pathDraftEpoch} routes={world?.routes??[]} routePaths={world?.route_paths??[]} stations={world?.stations??[]} world={world??previewWorld} enabled={enabled} canStart={!serviceDirty&&serviceId===null} onDirty={setPathServiceDirty} onSave={createPathService}/>
            <ServiceFromRoutePreview routes={world?.routes??[]} tracks={world?.tracks??[]} routePaths={world?.route_paths??[]} stations={world?.stations??[]} worldVersion={world} enabled={enabled} canApply={!serviceDirty && !pathServiceDirty && serviceId===null} onApply={draft=>{if(!enabled || busyRef.current || serviceDirty || pathServiceDirty || serviceId!==null)return;resetService();setServiceName("");setServiceStart(draft.start);setServiceTracks(draft.tracks);setServiceStops(draft.stops);setServiceDirty(true);setServiceHint("路線区間を新規下書きへ取り込みました。系統名を入力して停車設定を再確認してください。");}}/>
            <div className="service-directory"><button className="secondary" disabled={!enabled} onClick={() => chooseService(null)}>新規系統</button>{world?.services.map((service) => <button key={service.id} className={`route-choice ${service.id === serviceId ? "selected" : ""}`} disabled={!enabled} onClick={() => chooseService(service.id)}>系統編集: {service.name}</button>)}</div>
            {pendingServiceChoice && <div className="inline-confirm" role="alert"><p>未適用の系統下書きを破棄して切り替えますか。</p><button className="secondary" disabled={busy} onClick={() => setPendingServiceChoice(null)}>系統編集を続ける</button><button disabled={busy} onClick={() => resetService(world?.services.find((service) => service.id === pendingServiceChoice.id))}>系統下書きを破棄して切替</button></div>}
            {serviceHint&&<p role="status">{serviceHint}</p>}
            {selectedServicePath&&<section aria-label="保存済み新形式系統"><h4>新形式系統: {serviceName}</h4><p>経路・停車は安定参照で保存。既存系統の更新は未対応です。旧編集は使用できません。</p><ol>{selectedServiceOccurrences.map((o,i)=><li key={referenceKey(o.reference)}>{i+1}. {stationsById.get(o.station_id)?.name??o.station_id} / {selectedServicePath.stops.some(stop=>referenceKey(stop)===referenceKey(o.reference))?"停車":"通過"}</li>)}</ol><p>区間: {selectedServicePath.legs.map(l=>l.segment.kind==="track"?`接続線路 ${l.segment.track_id}`:`建設線形 ${l.segment.alignment_id} (${l.segment.from} → ${l.segment.to})`).join(" / ")}</p></section>}
            <form onSubmit={submitService}><fieldset disabled={!enabled||!!selectedServicePath||pathServiceDirty}>
              <label htmlFor="service-name">系統名</label><input id="service-name" value={serviceName} maxLength={80} onChange={(event) => { setServiceName(event.target.value); setServiceDirty(true); }} />
              <label htmlFor="service-start" className="track-to-label">系統の起点</label><select id="service-start" value={serviceStart} onChange={(event) => updateServicePath(event.target.value, [])}><option value="">駅を選択</option>{world?.stations.map((station) => <option key={station.id} value={station.id}>{station.name} (ID {station.id})</option>)}</select>
              <label htmlFor="service-next" className="track-to-label">系統で次にたどる線路</label><select id="service-next" value={serviceNextTrack} onChange={(event) => setServiceNextTrack(event.target.value)}><option value="">接続する線路を選択</option>{serviceNextTracks.map((track) => { const next = track.from === serviceEnd ? track.to : track.from; return <option key={track.id} value={track.id}>{stationsById.get(next)?.name ?? next} (線路 {track.id})</option>; })}</select>
              <button className="secondary full-width track-submit" type="button" disabled={!enabled || !serviceNextTrack} onClick={appendServiceTrack}>系統経路に追加</button>
              <ol className="service-stops" aria-label="系統の停車位置">{serviceStations.map((id,index) => <li key={`${index}-${id}`}><label><input type="checkbox" aria-label={`停車 ${index+1}: ${stationsById.get(id)?.name ?? id}`} checked={serviceStops.includes(index)} disabled={!enabled || index === 0 || index === serviceTracks.length} onChange={(event) => toggleServiceStop(index,event.target.checked)} /><span>{index+1}. {stationsById.get(id)?.name ?? id}{index === 0 || index === serviceTracks.length ? "（端点・停車固定）" : serviceStops.includes(index) ? "（停車）" : "（通過）"}</span></label></li>)}</ol>
              <div className="route-actions"><button type="button" className="secondary" disabled={!enabled || !serviceTracks.length} onClick={() => updateServicePath(serviceStart, serviceTracks.slice(0,-1))}>系統末尾を戻す</button><button type="button" className="secondary" disabled={!enabled || !serviceTracks.length} onClick={() => updateServicePath(serviceStart, [])}>系統経路をクリア</button></div>
              <button className="full-width track-submit" type="submit" disabled={!enabled || !serviceName.trim() || !serviceValid || !serviceTracks.length}>系統を適用</button>
              <button className="secondary full-width track-submit" type="button" disabled={!enabled || !serviceDirty} onClick={() => chooseService(serviceId)}>系統下書きを破棄</button>
              <p className="field-hint">{serviceId ? `編集中の系統 ID: ${serviceId}` : "新規系統の下書き"} / 適用後に「保存」でファイルへ反映</p>
            </fieldset></form>
          </section>
          <section className="company-editor" role="tabpanel" id="workspace-panel-settings" aria-labelledby="workspace-tab-settings" hidden={activeTool !== "company"}>
            <h3>設定</h3><h4>会社名</h4><p className="muted">この試作では1プレイヤー・1会社です。会社の追加や子会社には未対応です。</p>
            <form onSubmit={submitCompany}><fieldset disabled={!enabled}><label htmlFor="company-name">会社名</label><input id="company-name" value={companyName} maxLength={80} onChange={(event) => setCompanyName(event.target.value)} /><button className="full-width track-submit" type="submit" disabled={!enabled || !companyName.trim() || !companyDirty}>設定変更</button><button className="secondary full-width track-submit" type="button" disabled={!enabled || !companyDirty} onClick={() => setCompanyName(world?.companies[0]?.name ?? "")}>会社名の入力を戻す</button></fieldset></form>
            <FontLicenses />
          </section>
        </aside>
        {!panelCollapsed&&<PanelResizer value={panelWidth} onChange={setPanelWidth}/>}
      </div>
      {companySetupRequired&&<CompanySetup name={companyName} onName={setCompanyName} busy={busy} error={error} errorRevision={errorRevision} onDismissError={()=>setError("")} onStart={startCompany} onLoad={()=>load("manual")} onRestore={autosave.available?()=>load("auto"):undefined}/>}
      <footer className="app-footer"><p role="status" aria-live="polite">{busy ? "ワールドを更新しています。" : notice || "線路 → 駅 → 運行経路の順に建設"}</p><span>列車種別・ダイヤ・列車運行・旅客流動は未実装</span></footer>
    </main>
  );
}
