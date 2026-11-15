import {PARALLEL_RANGE_M,parallelProjection,offsetTrack,parallelSection} from "./parallelTrack";
import {TrackFollowIcon} from "./TrackFollowIcon";
import {stationGroupViews,footprintsIntersect,type StationGroup} from "./stationGroups";
import {twoEndedJoin} from "./twoEndedJoin";
import {DeletionDialog} from "./DeletionDialog";
import {removePlannedTarget,type PlannedDeleteTarget} from "./plannedDeletion";
import {deletionSections,anchorIndices} from "./deletionSections";
import {destinationJoinError,occupiedEndpoint} from "./destinationJoin";
import type {EndpointConnection} from "./destinationJoin";
import {anchorDragFeedback} from "./anchorDragFeedback";
import {PlanDeleteDialog} from "./PlanDeleteDialog";
import {TrackDeleteDialog} from "./TrackDeleteDialog";
import {anchorDragPreview} from "./anchorDragPreview";
import {plannedGeometry, type EditedGeometry} from "./editedGeometry";
import {plannedTrackSpeed} from "./plannedTrackSpeed";
import {emptyConstructionPlans,compactEmptyPlans,removeConstructionPlan,nextConstructionIds,type ConstructionPlans,type PlannedPath,type PlannedStation,type Extension} from "./constructionPlans";
import {UserGuide} from "./UserGuide";
import {MapCredits} from "./MapCredits";
import {snapReferenceStart} from "./referenceEndpointSnap";
import {extensionJoinError} from "./extensionJoin";
import {stationTarget, stationContainsPoint, type StationLine} from "./stationTarget";
import {FontLicenses} from "./FontLicenses";
import {TrackTypeControls,defaultDesignSpeed,TRACK_TYPE_LABELS,type TrackType} from "./TrackTypeControls";
import {useAutosave} from "./useAutosave";
import {WorkspaceTabs,type WorkspaceTab} from "./WorkspaceTabs";
import {PanelResizer} from "./PanelResizer";
import {CompanySetup} from "./CompanySetup";
import { useSmoothZoom, wheelZoomDelta } from "./useSmoothZoom";
import {SaveReplacementDialog} from "./SaveReplacementDialog";
import {UnsavedCloseDialog} from "./UnsavedCloseDialog";
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
import {ServicePathEditor} from "./ServicePathEditor";
import {ServicePathCreator} from "./ServicePathCreator";
import type {CreateServicePath} from "./ServicePathCreator";
import {resolveServicePath} from "./servicePaths";
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
import { provisionalCurveSpeed, snapStraight } from "./circularAlignment";
import { nearestOnAlignment, stationPolygon, alignmentLength, distanceM, pointOnAlignment } from "./alignment";
type GeoPoint = { latitude: number; longitude: number };
type Alignment = { anchor_indices?:number[]|null; id: string; owner: string; name: string; points: GeoPoint[]; track_type: TrackType; design_speed_kmh?:number|null };
type StationPlacement = { station_id: string; alignment_id: string; position: number; length_m: number; width_m: number };

type EngineInfo = { name: string; version: string; company_setup_required?: boolean };
type Station = { id: string; owner: string; name: string; latitude: number; longitude: number };
type Track = { id: string; owner: string; from: string; to: string };
type Route = { id: string; owner: string; name: string; start: string; tracks: string[] };
type OperatingService = { id: string; owner: string; name: string; start: string; tracks: string[]; stops: number[] };
type RoutePath = { route_id: string; segments: Segment[]; member_stations: string[] };
type World = {
  alignment_connections?: EndpointConnection[];
  construction_plans?: ConstructionPlans | null;
  view_state?: Camera | null;
  route_paths?: RoutePath[];
  service_paths?: ServicePath[];
  source_records?: SourceRecord[];
  alignment_provenance?: AlignmentProvenanceRecord[];
  alignments?: Alignment[];
  station_placements?: StationPlacement[];
  station_groups?: StationGroup[];
  next_id: string;
  players: { id: string; name: string }[];
  companies: { id: string; owner: string; name: string }[];
  stations: Station[];
  tracks: Track[];
  routes: Route[];
  services: OperatingService[];
};
type DeleteTarget=PlannedDeleteTarget|{kind:"built-section";alignmentId:string;sectionIndex:number}|{kind:"station";stationId:string};
type StationHistoryEntry = { station: Station; placement?: StationPlacement; planned?:PlannedStation };
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
  const [saveReplacement,setSaveReplacement]=useState<{token:string;company_name:string}|null>(null);
  const [replacementError,setReplacementError]=useState("");
  const [replacementInvalid,setReplacementInvalid]=useState(false);
  const [savedViewFingerprint,setSavedViewFingerprint]=useState(JSON.stringify(DEFAULT_CAMERA));
  function restoreView(view:Camera|null|undefined){
    const next=view?cameraAt(view.latitude,view.longitude,view.zoom):DEFAULT_CAMERA;
    changeCamera(next);setSavedViewFingerprint(JSON.stringify(next));
  }
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
  const [activeTool, setActiveTool] = useState<"stations" | "tracks" | "routes" | "services" | "company" | "station-management" | "delete" | "inactive">("tracks");
  function chooseMainTab(tab:WorkspaceTab){setError("");setMainTab(tab);setPanelCollapsed(false);setActiveTool(tab==="construction"?"tracks":tab==="stations"?"station-management":tab==="settings"?"company":tab==="routes"?"routes":tab==="services"?"services":"inactive");changeMapMode(tab==="construction"?"draw":"move");}
  const [panelCollapsed, setPanelCollapsed] = useState(false);
  // Internal compatibility label only; physical tracks are not user-named routes.
  const alignmentName = "新しい線路";
  const [draftFinished, setDraftFinished] = useState(false);
  const [draftPoints, setDraftPoints] = useState<GeoPoint[]>([]);
  const [draftEditedGeometry,setDraftEditedGeometry]=useState<EditedGeometry|undefined>();
  type AnchorEdit={kind?:"point";id:number;before:{points:GeoPoint[];geometry?:EditedGeometry;redo:GeoPoint[];destination?:Extension;redoDestination?:Extension};after:{points:GeoPoint[];geometry?:EditedGeometry;redo:GeoPoint[];destination?:Extension;redoDestination?:Extension}};
  const [anchorUndo,setAnchorUndo]=useState<AnchorEdit[]>([]),[anchorRedo,setAnchorRedo]=useState<AnchorEdit[]>([]);
  // Keep session-only edit history with its path while another plan is selected.
  const parkedAnchorHistory=useRef(new Map<number,{undo:AnchorEdit[];redo:AnchorEdit[]}>());
  function parkAnchorHistory(){
    parkedAnchorHistory.current.set(activePathId,{undo:anchorUndo.filter(edit=>edit.id===activePathId),redo:anchorRedo.filter(edit=>edit.id===activePathId)});
  }
  type AnchorGesture={pointerId:number;id:number;index:number;startX:number;startY:number;points:GeoPoint[];line:GeoPoint[];geometry?:EditedGeometry;redo:GeoPoint[];destination?:Extension;redoDestination?:Extension;previous?:GeoPoint;preview:EditedGeometry|null;error:string|null;snapped:boolean};
  const anchorGesture=useRef<AnchorGesture|null>(null);
  const [anchorPreview,setAnchorPreview]=useState<AnchorGesture|null>(null);
  useEffect(()=>{cancelAnchorGesture();},[camera,activeTool,mapSize]);
  const [draftExtension,setDraftExtension]=useState<Extension|null>(null);
  const [draftDestination,setDraftDestination]=useState<Extension|undefined>();
  const [draftRedoDestination,setDraftRedoDestination]=useState<Extension|undefined>();
  const [parkedPaths, setParkedPaths] = useState<PlannedPath[]>([]);
  const [activePathId, setActivePathId] = useState(1);
  const [plans,setPlans]=useState([{id:1,name:""}]);
  const [activePlanId,setActivePlanId]=useState(1);
  const nextPlanId=useRef(2);
  const planName=plans.find(plan=>plan.id===activePlanId)?.name??"";
  function setPlanName(name:string){setPlans(items=>items.map(plan=>plan.id===activePlanId?{...plan,name}:plan));}
  const [trackType,setTrackType]=useState<TrackType>("normal");
  const [designSpeedKmh,setDesignSpeedKmh]=useState(120);
  const [roadFollowing,setRoadFollowing]=useState(false);
  const [referencePanel,setReferencePanel]=useState(false);
  const [parallelMode,setParallelMode]=useState(false);
  const [parallelStart,setParallelStart]=useState<{sourceId:string;offset:number;point:GeoPoint}|null>(null);
  function cancelParallel(){setParallelStart(null);}
  useEffect(()=>{cancelParallel();},[activeTool,activePlanId,world]);
  const roadFollowingBlocked=roadFollowing;
  function chooseTrackType(value:TrackType){cancelParallel();setParallelMode(false);setTrackType(value);setDesignSpeedKmh(defaultDesignSpeed(value));setRoadFollowing(value==="tram");setReferencePanel(value==="tram");setHoverPoint(null);}
  const nextPathId = useRef(2);
  const [plannedStations,setPlannedStations]=useState<PlannedStation[]>([]);
  const nextPlannedStationId=useRef(1);
  const [stationCursor,setStationCursor]=useState<GeoPoint|null>(null);
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
  const pendingPathHistory=useRef<{id:number;redo:boolean}|null>(null);
  function adjacentHistoryPath(redo:boolean){
    return [...parkedPaths].reverse().find(path=>path.planId===activePlanId&&!path.reference&&(
      redo ? path.redo.length>0 || !!parkedAnchorHistory.current.get(path.id)?.redo.length
        : path.points.length>0 || !!parkedAnchorHistory.current.get(path.id)?.undo.length
    ));
  }
  function movePathHistory(redo:boolean){
    const target=adjacentHistoryPath(redo);if(!target)return;
    pendingPathHistory.current={id:target.id,redo};
    selectPath(target.id,false);
  }
  // Selection restores that path's own history first. Apply the key only after
  // React has installed the selected path, never against the previous draft.
  useEffect(()=>{
    const pending=pendingPathHistory.current;
    if(!pending||pending.id!==activePathId)return;
    pendingPathHistory.current=null;
    if(pending.redo)redoPoint();else undoPoint();
  },[activePathId]);
  function undoPoint() { if(anchorUndo.length){undoAnchor(false);return;} if (!draftPoints.length) {movePathHistory(false);return;} if(draftFinished&&draftPoints.length===2){undoLastSegment();return;} setDraftRedoDestination(draftDestination??draftRedoDestination);setDraftDestination(undefined);setRedoPoints(redo => [...redo, draftPoints[draftPoints.length-1]]); setDraftPoints(points => points.slice(0,-1)); setHoverPoint(null); }
  function redoPoint() {
    if(anchorRedo.length){undoAnchor(true);return;} if (!redoPoints.length) {movePathHistory(true);return;}
    const count=draftFinished&&!draftPoints.length&&redoPoints.length>=2?2:1;
    const after={points:[...draftPoints,...redoPoints.slice(-count).reverse()],geometry:draftEditedGeometry,redo:redoPoints.slice(0,-count),destination:redoPoints.length===count?draftRedoDestination:draftDestination,redoDestination:redoPoints.length===count?undefined:draftRedoDestination};
    setAnchorUndo(items=>[...items,{kind:"point" as const,id:activePathId,before:{points:draftPoints,geometry:draftEditedGeometry,redo:redoPoints,destination:draftDestination,redoDestination:draftRedoDestination},after}].slice(-50));
    applyAnchorEdit(activePathId,after);
  }
  const [alignmentId, setAlignmentId] = useState("");
  const [selectedPlanPath,setSelectedPlanPath]=useState<number|null>(null);
  const [trackDeleteConfirm,setTrackDeleteConfirm]=useState(false);
  const [deleteTarget,setDeleteTarget]=useState<DeleteTarget|null>(null);
  const [deleteHover,setDeleteHover]=useState<{target:DeleteTarget;x:number;y:number}|null>(null);
  useEffect(()=>{setDeleteHover(null);},[activeTool,camera,world,draftPoints,parkedPaths,plannedStations]);
  type DeleteHistory={kind:"built";world:World;plans:string;command:string;args:Record<string,unknown>}|{kind:"plan";before:ConstructionPlans;after:string;world:World|null};
  const [trackDeleteUndo,setTrackDeleteUndo]=useState<DeleteHistory|null>(null);
  const [trackDeleteRedo,setTrackDeleteRedo]=useState<{entry:DeleteHistory;world:World|null;plans:string}|null>(null);

  const stationNamesVisible=displaySettings.settings.stationNamesVisible;
  const setStationNamesVisible=(value:boolean)=>displaySettings.update({stationNamesVisible:value});
  const [platformLength, setPlatformLength] = useState("200");
  const platformWidth = "12";
  const [editLength, setEditLength] = useState("200");
  const [editWidth, setEditWidth] = useState("12");
  const alignments = world?.alignments ?? [];
  const detailedStations = camera.zoom >= 13;
  const [stationResizePending,setStationResizePending]=useState(false);
  const [stationResizeFailure,setStationResizeFailure]=useState<string|null>(null);
  const stationPolygons=useMemo(()=>new Map((world?.station_placements??[]).map(placement=>{
    const alignment=world?.alignments?.find(a=>a.id===placement.alignment_id);
    return [placement.station_id,alignment?stationPolygon(alignment.points,placement.position,placement.length_m,placement.width_m):null];
  })),[world]);
  const routeVisual = useMemo(() => routeAppearance(world), [world]);
  const selectedAlignment = alignments.find((a) => a.id === alignmentId);
  function extensionPrevious(extension:Extension|null){
    const alignment=alignments.find(a=>a.id===extension?.alignmentId);
    return alignment&&extension?(extension.endpoint==="start"?alignment.points[1]:alignment.points.at(-2)):undefined;
  }
  const previousPoint=extensionPrevious(draftExtension);
  const [cursorPosition,setCursorPosition]=useState<{x:number;y:number;width:number;height:number}|null>(null);
  const draftGeometry = useMemo(() => plannedGeometry(draftPoints,draftEditedGeometry,previousPoint), [draftPoints,draftEditedGeometry,previousPoint]);
  function fittedDestination(anchors:GeoPoint[],edited:typeof draftEditedGeometry,target:ReturnType<typeof destinationTarget>) {
    const inside=target?.endpoint==="start"?target.alignment.points[1]:target?.alignment.points.at(-2);
    return inside?twoEndedJoin(anchors,edited,previousPoint,inside)??edited:edited;
  }
  const previewAnchors=hoverPoint && !draftFinished && draftPoints.length && mapMode === "draw" ? [...draftPoints, hoverPoint] : draftPoints;
  const previewGeometry = plannedGeometry(previewAnchors,previewAnchors!==draftPoints?fittedDestination(previewAnchors,draftEditedGeometry,hoverPoint&&cursorPosition?destinationTarget(hoverPoint,cursorPosition.width,cursorPosition.height):null):draftEditedGeometry,previousPoint);
  const draftLine = draftGeometry.points;
  const previewLine = previewGeometry.points;
  const draftInvalid = Boolean(draftGeometry.error);
  const previewError = previewGeometry.error;
  // Invalid geometry is diagnostic only. Never commit this fallback chord.
  const candidateLine = hoverPoint && !draftFinished && draftPoints.length ? (previewLine.length > draftLine.length ? previewLine.slice(Math.max(0,draftLine.length-1)) : [draftPoints[draftPoints.length-1],hoverPoint]) : [];
  const drawingCursor=useRef<{point:GeoPoint;width:number;height:number}|null>(null);
  useEffect(()=>{drawingCursor.current=null;setHoverPoint(null);setCursorPosition(null);},[camera]);
  function endpointDistance(point:GeoPoint, end:GeoPoint, width:number,height:number) {
    const a=project(end.latitude,end.longitude,camera),b=project(point.latitude,point.longitude,camera);
    return Math.hypot(a.x-b.x,a.y-b.y)*mapViewport(width,height).scale;
  }
  function snappedEndpoint(point:GeoPoint, width:number,height:number) {
    if(draftFinished || !draftPoints.length)return null;
    const end=draftPoints[draftPoints.length-1];
    return endpointDistance(point,end,width,height)<=8?end:null;
  }
  function drawingPoint(point:GeoPoint,width:number,height:number,disableStraightSnap=false) {
    if(draftFinished||!draftPoints.length)return point;
    const target=destinationTarget(point,width,height);if(target)return target.point;
    if(disableStraightSnap)return point;
    const metresPerPixel=2*Math.PI*6371000*Math.cos(draftPoints[0].latitude*Math.PI/180)/(256*2**camera.zoom*mapViewport(width,height).scale);
    return draftEditedGeometry?snapStraight(draftLine.slice(-2),point,metresPerPixel):snapStraight(draftPoints,point,metresPerPixel,previousPoint);
  }
  const circleGuide = !draftFinished && hoverPoint && mapMode === "draw" ? previewGeometry.guide : null;
  const guideCenter = circleGuide ? project(circleGuide.center.latitude,circleGuide.center.longitude,camera) : null;
  const guideEdge = circleGuide ? project(circleGuide.edge.latitude,circleGuide.edge.longitude,camera) : null;
  function discardPlanStations(id:number){
    setPlannedStations(items=>items.filter(station=>station.pathId!==id));
    setStationUndo(items=>items.filter(entry=>entry.planned?.pathId!==id));
    setStationRedo(items=>items.filter(entry=>entry.planned?.pathId!==id));
  }
  function parkCurrentPlan(): PlannedPath | null {
    return draftPoints.length || redoPoints.length ? {id:activePathId,planId:activePlanId,name:planName,points:draftPoints,redo:redoPoints,line:draftLine,editedGeometry:draftEditedGeometry,designSpeedKmh,trackType,roadFollowing,extension:draftExtension,destination:draftDestination,redoDestination:draftRedoDestination} : null;
  }
  function newPath(discardEmptyRedo=false) {
    // A new drawing replaces a fully undone path; switching plans still preserves its Redo.
    const discard=discardEmptyRedo&&!draftPoints.length;
    const current=discard?null:parkCurrentPlan();
    if(discard)discardPlanStations(activePathId);
    if(current)parkAnchorHistory();
    setParkedPaths(paths=>current?[...paths,current]:paths);
    setDraftFinished(false);setActivePathId(nextPathId.current++);setDraftExtension(null);setDraftDestination(undefined);setDraftRedoDestination(undefined);setDraftPoints([]);setDraftEditedGeometry(undefined);setAnchorUndo([]);setAnchorRedo([]);setRedoPoints([]);setHoverPoint(null);setDraftExtension(null);setDraftDestination(undefined);setDraftRedoDestination(undefined);changeMapMode(activeTool==="stations"?"place":"draw");
  }
  function newIndependentPlan() {
    if (!enabled || referenceCopy.active) return;
    newPath();const id=nextPlanId.current++;setPlans(items=>[...items,{id,name:""}]);setActivePlanId(id);
  }
  function selectPath(id:number,resume=true) {
    if(!enabled || referenceCopy.active)return;
    const target=parkedPaths.find(path=>path.id===id);if(!target||target.reference)return;
    const current=parkCurrentPlan();
    if(current)parkAnchorHistory();
    const history=parkedAnchorHistory.current.get(id);parkedAnchorHistory.current.delete(id);
    setParkedPaths(paths=>[...paths.filter(path=>path.id!==id),...(current?[current]:[])]);
    setAnchorUndo(history?.undo??[]);setAnchorRedo(history?.redo??[]);setActivePlanId(target.planId);setDraftFinished(!resume||!!target.destination);setActivePathId(target.id);setDraftExtension(target.extension);setDraftDestination(target.destination);setDraftRedoDestination(target.redoDestination);setDraftPoints(target.points);setDraftEditedGeometry(target.editedGeometry);setRedoPoints(target.redo);setTrackType(target.trackType);setDesignSpeedKmh(target.designSpeedKmh);setRoadFollowing(target.roadFollowing);setHoverPoint(null);changeMapMode(activeTool==="stations"?"place":"draw");
  }
  function choosePlan(id:number){
    if(id===activePlanId)return;
    const path=parkedPaths.find(path=>path.planId===id&&!path.reference);
    if(path)selectPath(path.id,false);else{newPath();setActivePlanId(id);}
  }
  function stationPlanId(station:PlannedStation){return station.planId??(station.pathId===activePathId?activePlanId:parkedPaths.find(path=>path.id===station.pathId)?.planId);}
  const [deletePlanConfirm,setDeletePlanConfirm]=useState(false);
  function deleteCurrentPlan(){
    const saved=savedPlanFingerprint;restoreConstructionPlans(removeConstructionPlan(constructionPlans,activePlanId));setSavedPlanFingerprint(saved);
    setStationUndo([]);setStationRedo([]);setHoverPoint(null);setDeletePlanConfirm(false);
  }
  function clearCurrentPlan(){
    const ids=new Set([activePathId,...parkedPaths.filter(path=>path.planId===activePlanId).map(path=>path.id)]);
    ids.forEach(id=>parkedAnchorHistory.current.delete(id));
    ids.forEach(discardPlanStations);setPlannedStations(items=>items.filter(station=>stationPlanId(station)!==activePlanId));setStationUndo(items=>items.filter(entry=>!entry.planned||stationPlanId(entry.planned)!==activePlanId));setStationRedo(items=>items.filter(entry=>!entry.planned||stationPlanId(entry.planned)!==activePlanId));setParkedPaths(paths=>paths.filter(path=>path.planId!==activePlanId));
    setDraftFinished(false);setDraftPoints([]);setDraftEditedGeometry(undefined);setAnchorUndo([]);setAnchorRedo([]);setRedoPoints([]);setHoverPoint(null);setDraftExtension(null);setDraftDestination(undefined);setDraftRedoDestination(undefined);
  }
  const [planHover, setPlanHover] = useState<{id:number;x:number;y:number;point:GeoPoint;anchor?:number} | null>(null);
  const [builtHover,setBuiltHover]=useState<{id:string;point:GeoPoint}|null>(null);
  useEffect(() => { setPlanHover(null);setBuiltHover(null); }, [draftPoints, parkedPaths, activeTool, camera, world]);
  function hoverBuilt(event:ReactPointerEvent<SVGPathElement>,id:string){
    if(gestureRef.current||anchorGesture.current||event.pointerType==="touch")return;
    const rect=svgRef.current?.getBoundingClientRect();
    if(rect){const point=coordinatesFromClient(event.clientX,event.clientY,rect,camera);if(point){setPlanHover(null);setBuiltHover({id,point});}}
  }
  const hoveredBuilt=builtHover?alignments.find(line=>line.id===builtHover.id):null;
  const dragDesignSpeed=anchorPreview?.id===activePathId?designSpeedKmh:parkedPaths.find(path=>path.id===anchorPreview?.id)?.designSpeedKmh;
  const dragSpeedLabels=anchorPreview?.preview&&!anchorPreview.error&&dragDesignSpeed!==undefined?anchorDragFeedback(anchorPreview.preview,anchorPreview.index,dragDesignSpeed):[];
  const hoveredPlan = planHover?.id === activePathId ? {name:planName,line:draftLine,points:draftPoints,editedGeometry:draftEditedGeometry,reference:undefined,designSpeedKmh,extension:draftExtension,destination:draftDestination,redoDestination:draftRedoDestination} : parkedPaths.find(plan=>plan.id===planHover?.id);
  const hoveredExtension=hoveredPlan?.extension?alignments.find(a=>a.id===hoveredPlan.extension!.alignmentId):null;
  const hoveredPrevious=hoveredExtension?(hoveredPlan!.extension!.endpoint==="end"?hoveredExtension.points.at(-2):hoveredExtension.points[1]):undefined;
  const hoveredGeometry = hoveredPlan&&!hoveredPlan.reference ? plannedGeometry(hoveredPlan.points,hoveredPlan.editedGeometry,hoveredPrevious) : null;
  const hoveredSpeed=hoveredGeometry&&planHover?plannedTrackSpeed(hoveredGeometry,planHover.point,hoveredPlan!.designSpeedKmh):null;

  function hoverPlan(event: ReactPointerEvent<SVGElement>, id:number, anchor?:number) {
    if (gestureRef.current || anchorGesture.current || event.pointerType === "touch") return;
    setBuiltHover(null);
    const rect=svgRef.current?.getBoundingClientRect();
    if(rect){const points=id===activePathId?draftPoints:parkedPaths.find(path=>path.id===id)?.points;const point=anchor===undefined?coordinatesFromClient(event.clientX,event.clientY,rect,camera):points?.[anchor];if(point)setPlanHover({id,x:event.clientX-rect.left,y:event.clientY-rect.top,point,anchor});}
  }
  const viewport = mapViewport(mapSize.width,mapSize.height);
  // Both focus markers use the same size: the visible rail plus a small
  // screen-space margin. Appearance never changes the snap/hit thresholds.
  const focusRadius=lineWidth/2+3/viewport.scale;
  const focusMarker=(point:GeoPoint,kind:"line"|"anchor",className:string,allowed=true)=>{
    const p=project(point.latitude,point.longitude,camera),square=4/viewport.scale;
    return <g pointerEvents="none" aria-hidden="true"><circle className={"track-focus-marker "+className} data-kind={kind} data-allowed={allowed} cx={p.x} cy={p.y} r={focusRadius} vectorEffect="non-scaling-stroke"/>{kind==="anchor"&&<rect className="focus-anchor-center" x={p.x-square/2} y={p.y-square/2} width={square} height={square} vectorEffect="non-scaling-stroke"/>}</g>;
  };
  const lineFocusMarker=(points:GeoPoint[],target:GeoPoint,className:string)=>{
    const p=project(target.latitude,target.longitude,camera);
    const endpoint=[points[0],points.at(-1)].filter((point):point is GeoPoint=>!!point).map(point=>{const q=project(point.latitude,point.longitude,camera);return {point,distance:Math.hypot(p.x-q.x,p.y-q.y)*viewport.scale};}).filter(item=>item.distance<=8).sort((a,b)=>a.distance-b.distance)[0]?.point;
    return focusMarker(endpoint??nearestOnAlignment(points,target).point,endpoint?"anchor":"line",className);
  };
  const drawPath = (points: GeoPoint[]) => points.map((point, index) => { const p = project(point.latitude, point.longitude, camera); return (index ? "L" : "M") + p.x + "," + p.y; }).join(" ");
  const drawAnchors=(points:GeoPoint[],parked=false,id=activePathId)=>points.map((point,index) => {
    const p=project(point.latitude,point.longitude,camera),size=5/viewport.scale,hit=22/viewport.scale,endpoint=index===0||index===points.length-1;
    return <g key={index}><rect x={p.x-size/2} y={p.y-size/2} width={size} height={size} className={(parked?"parked-anchor":"draft-anchor")+(endpoint?" draft-endpoint":"")} vectorEffect="non-scaling-stroke"/>
      <rect onClick={event=>{if(mapMode==="move"){event.stopPropagation();selectTrackPath(id);}}} onPointerEnter={event=>hoverPlan(event,id,index)} onPointerMove={event=>hoverPlan(event,id,index)} onPointerLeave={()=>setPlanHover(null)} onPointerDown={event=>beginAnchorGesture(event,id,index)} data-anchor-index={index} data-anchor-path={id} x={p.x-hit/2} y={p.y-hit/2} width={hit} height={hit} className="anchor-drag-hit"/></g>;
  });
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
  const enabled = status === "ready" && !busy && !companySetupRequired && !saveReplacement;
  const stationsById = useMemo(()=>new Map(world?.stations.map((station) => [station.id, station])),[world]);
  const validEndpoints = stationFrom !== stationTo && stationsById.has(stationFrom) && stationsById.has(stationTo);

  useEffect(() => {
    const svg = svgRef.current;
    if (!svg) return;
    function wheel(event: WheelEvent) {
      if (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey || !Number.isFinite(event.deltaY) || event.deltaY === 0 || Math.abs(event.deltaX) > Math.abs(event.deltaY)) return;
      event.preventDefault();
      if (gestureRef.current || anchorGesture.current) return;
      const rect=svg!.getBoundingClientRect();
      const location=coordinatesFromClient(event.clientX,event.clientY,rect,cameraRef.current);
      if(!location)return;
      smoothZoom.zoomBy(wheelZoomDelta(event.deltaY,event.deltaMode,rect.height),project(location.latitude,location.longitude,cameraRef.current));
    }
    svg.addEventListener("wheel", wheel, { passive: false });
    return () => svg.removeEventListener("wheel", wheel);
  }, []);
  useEffect(() => { smoothZoom.cancel(); changeMapMode(activeTool === "tracks" ? "draw" : activeTool==="stations"?"place":"move"); setHoverPoint(null); setPanelCollapsed(false); }, [activeTool]);
  const selectedServicePath=world?.service_paths?.find(path=>path.service_id===serviceId);

  const selectedRoutePath = world?.route_paths?.find(path => path.route_id === routeId);
  const companyDirty = !!world && companyName !== (world.companies[0]?.name ?? "");
  const selectedStation = world?.stations.find((station) => station.id === selected);
  const directoryStations=world?.stations.filter(station=>!world.station_groups?.some(group=>group.members.includes(station.id)&&group.station_id!==station.id))??[];
  const stationDirty = !!selectedStation && editStationName !== selectedStation.name;
  const selectedPlacement = world?.station_placements?.find(p => p.station_id === selected);
  const platformDirty = !!selectedPlacement && (Number(editLength) !== selectedPlacement.length_m || Number(editWidth) !== selectedPlacement.width_m);
  const stationSettingsDirty = stationDirty || platformDirty;
  const copyCanStart=!(routeDirty||serviceDirty||pathServiceDirty||companyDirty||stationSettingsDirty);
  const referenceCopyState=useReferenceCopy({bundle:graphBundle,worldVersion:world,enabled,canStart:copyCanStart,tool:activeTool,name:alignmentName,onBuild:buildReferenceAlignment,roadFollow:roadFollowingBlocked});
  const [referenceExtensionKey,setReferenceExtensionKey]=useState("");
  const referenceBuild=referenceCopyState.plan.build;
  const referenceExtensionCandidates=referenceBuild&&graphBundle?alignments.flatMap(alignment=>(["start","end"] as const).flatMap(endpoint=>{
    const point=endpoint==="start"?alignment.points[0]:alignment.points.at(-1),origin=referenceBuild.points[0];
    if(!point||endpointDistance(point,origin,mapSize.width||1000,mapSize.height||600)>8)return [];
    const build=snapReferenceStart(graphBundle,referenceCopyState.picks,point,alignmentName);
    return build?[{key:alignment.id+":"+endpoint,alignment,endpoint,build}]:[];
  })):[];
  const referenceExtension=referenceExtensionCandidates.find(candidate=>candidate.key===referenceExtensionKey);
  const referenceExtensionError=referenceExtensionKey?(!referenceExtension?"延伸対象の端点が参照形状に一致しません。起点を選び直してください。":referenceExtension.alignment.owner!==world?.companies[0]?.id?"他社の線路には延伸できません。":extensionJoinError(referenceExtension.build.points,extensionPrevious({alignmentId:referenceExtension.alignment.id,endpoint:referenceExtension.endpoint}))):null;
  const referenceCopy={...referenceCopyState,plan:referenceExtension?{build:referenceExtension.build,error:referenceCopyState.plan.error}:referenceCopyState.plan};
  useEffect(()=>{setReferenceExtensionKey("");},[referenceCopy.active,graphBundle]);
  const stationLines:StationLine[] = [...alignments,...(referenceCopy.active&&referenceCopy.plan.build?[{id:"reference-plan",pathId:-1,points:referenceCopy.plan.build.points}]:[]),...parkedPaths.map(plan=>({id:"plan:"+plan.id,pathId:plan.id,points:plan.line,invalid:plan.reference?!!extensionJoinError(plan.points,extensionPrevious(plan.extension)):!!plannedGeometry(plan.points,plan.editedGeometry,extensionPrevious(plan.extension)).error})),...(draftLine.length>1?[{id:"plan:"+activePathId,pathId:activePathId,points:draftLine,invalid:draftInvalid||roadFollowingBlocked}]:[])];
  const plannedPlatforms=plannedStations.map(station=>{
    const line=stationLines.find(line=>station.alignmentId?line.pathId===undefined&&line.id===station.alignmentId:line.pathId===station.pathId);
    const nearest=line?nearestOnAlignment(line.points,station.point):null;
    const polygon=line&&nearest?stationPolygon(line.points,nearest.position,station.lengthM,12):[];
    const half=line?station.lengthM/alignmentLength(line.points)/2:0;
    const position=nearest?Math.max(half,Math.min(1-half,nearest.position)):0;
    const center=line&&polygon.length>=4?pointOnAlignment(line.points,position):null;
    return {station,line,position,polygon,valid:!!line&&!line.invalid&&!!center&&distanceM(center,station.point)<1&&polygon.length>=4};
  });
  const stationGroupDisplay=stationGroupViews([
    ...(world?.stations??[]).flatMap(station=>{const placement=world?.station_placements?.find(p=>p.station_id===station.id),polygon=stationPolygons.get(station.id);return placement&&polygon?[{id:station.id,name:station.name,owner:station.owner,lineId:placement.alignment_id,polygon,planned:false}]:[];}),
    ...plannedPlatforms.flatMap(p=>p.line?[{id:p.station.id,name:p.station.name,owner:world?.companies[0]?.id??"",lineId:p.line.id,polygon:p.polygon,planned:true}]:[])
  ],world?.station_groups);
  const stationDisplayGroup=(id:string)=>stationGroupDisplay.find(g=>g.members.some(m=>m.id===id));
  const mergedStationIds=new Set(stationGroupDisplay.filter(g=>g.members.length>1).flatMap(g=>g.members.map(m=>m.id)));
  const occupiedStationPlatforms=[...(world?.station_placements??[]).map(p=>({lineId:p.alignment_id,position:p.position,lengthM:p.length_m})),...plannedPlatforms.flatMap(p=>p.line?[{lineId:p.line.id,position:p.position,lengthM:p.station.lengthM}]:[])];
  const hoveredPlannedStation=stationCursor?plannedPlatforms.find(p=>stationContainsPoint(p.polygon,stationCursor)):undefined;
  const hoveredBuiltStation=stationCursor?world?.stations.find(station=>stationContainsPoint(stationPolygons.get(station.id)??[],stationCursor)):undefined;
  const hoveredStation=hoveredPlannedStation?{name:stationDisplayGroup(hoveredPlannedStation.station.id)?.name??hoveredPlannedStation.station.name,lengthM:hoveredPlannedStation.station.lengthM}:hoveredBuiltStation?{name:hoveredBuiltStation.name,lengthM:world?.station_placements?.find(p=>p.station_id===hoveredBuiltStation.id)?.length_m}:undefined;
  const stationSelection=stationCursor?stationTarget(stationLines,stationCursor,Number(platformLength),camera,mapSize.width>0&&mapSize.height>0?mapViewport(mapSize.width,mapSize.height).scale:1,occupiedStationPlatforms):null;
  const stationJoinPreview=stationSelection?.target&&!stationSelection.error?stationGroupDisplay.find(g=>g.members.some(m=>m.owner===world?.companies[0]?.id&&m.lineId!==stationSelection.target!.line.id&&footprintsIntersect(m.polygon,stationSelection.target!.polygon))):undefined;
  const stationJoinOrigin=stationJoinPreview?(world?.stations.find(s=>s.id===stationJoinPreview.id)??plannedStations.find(s=>s.id===stationJoinPreview.id)?.point):undefined;
  const stationPreview=activeTool==="stations"&&mapMode==="place"&&!dragging&&!stationSelection?.error?stationSelection?.target?.polygon??[]:[];
  const previewCenter=stationSelection?.target?.center??null;
  useEffect(()=>{if(!referenceCopy.active)discardPlanStations(-1);},[referenceCopy.active]);
  const graphNodesById=useMemo(()=>new Map(graphBundle?.graph.nodes.map(node=>[node.osmId,node])??[]),[graphBundle]);
  useEffect(()=>{referenceCopy.clearHover();},[camera]);
  const saveBlocked = !!anchorPreview || candidateDirty || routeDirty || serviceDirty || pathServiceDirty || companyDirty || stationSettingsDirty;
  const hasPlans = plannedStations.length>0 || draftPoints.length > 0 || redoPoints.length > 0 || parkedPaths.length > 0 || referenceCopy.active;
  const constructionPlans=useMemo<ConstructionPlans>(()=>({version:1,plans,parkedPaths,activePathId,activePlanId,draftPoints,draftEditedGeometry,redoPoints,draftExtension,draftDestination,draftRedoDestination,trackType,designSpeedKmh,roadFollowing,plannedStations:plannedStations.filter(station=>station.pathId!==-1),draftFinished}),[plans,parkedPaths,activePathId,activePlanId,draftPoints,draftEditedGeometry,redoPoints,draftExtension,draftDestination,draftRedoDestination,trackType,designSpeedKmh,roadFollowing,plannedStations,draftFinished]);
  const persistedPlans=useMemo(()=>compactEmptyPlans(constructionPlans),[constructionPlans]);
  const planFingerprint=JSON.stringify(constructionPlans);
  const [savedPlanFingerprint,setSavedPlanFingerprint]=useState(()=>JSON.stringify(emptyConstructionPlans()));
  const plansDirty=planFingerprint!==savedPlanFingerprint;
  const unapplied = saveBlocked || referenceCopy.active;
  const viewDirty=JSON.stringify(camera)!==savedViewFingerprint;
  const autosaveSnapshot=useMemo(()=>({world,constructionPlans,camera}),[world,constructionPlans,camera]);
  const autosave=useAutosave({ready:status==="ready",dirty:(dirty||plansDirty||viewDirty)&&!companySetupRequired&&!saveReplacement,world:autosaveSnapshot,constructionPlans:persistedPlans,viewState:camera,epoch:worldEpoch,restoredEpoch:restoredAutosaveEpoch,busyRef,setBusy});
  const closeGuard = useUnsavedCloseGuard(dirty || plansDirty || unapplied || stationResizePending);
  function restoreConstructionPlans(value:ConstructionPlans|null|undefined,compact=false){
    parkedAnchorHistory.current.clear();
    const original=value??emptyConstructionPlans(),restored=compact?compactEmptyPlans(original):original,ids=nextConstructionIds(restored);
    setPlans(restored.plans);setParkedPaths(restored.parkedPaths);setActivePathId(restored.activePathId);setActivePlanId(restored.activePlanId);
    setDraftPoints(restored.draftPoints);setDraftEditedGeometry(restored.draftEditedGeometry);setAnchorUndo([]);setAnchorRedo([]);setRedoPoints(restored.redoPoints);setDraftExtension(restored.draftExtension);setDraftDestination(restored.draftDestination);setDraftRedoDestination(restored.draftRedoDestination);setDraftFinished(restored.draftFinished);
    setTrackType(restored.trackType);setDesignSpeedKmh(restored.designSpeedKmh);setRoadFollowing(restored.roadFollowing);setPlannedStations(restored.plannedStations);
    nextPlanId.current=ids.plan;nextPathId.current=ids.path;nextPlannedStationId.current=ids.station;
    setSavedPlanFingerprint(JSON.stringify(restored));
  }
  function alignmentReferences(id:string){
    if(world?.alignments?.find(a=>a.id===id)?.owner!==world?.companies[0]?.id)return "他社の線路";
    const refs:string[]=[];
    if(world?.alignment_connections?.some(c=>c.a.alignment_id===id||c.b.alignment_id===id))refs.push("端点接続");
    if(world?.station_placements?.some(p=>p.alignment_id===id))refs.push("駅");
    if(world?.route_paths?.some(p=>p.segments.some(s=>s.kind==="alignment"&&s.alignment_id===id)))refs.push("路線");
    if(world?.service_paths?.some(p=>p.legs.some(l=>l.segment.kind==="alignment"&&l.segment.alignment_id===id)))refs.push("系統");
    if(draftExtension?.alignmentId===id||draftDestination?.alignmentId===id||draftRedoDestination?.alignmentId===id||parkedPaths.some(p=>p.extension?.alignmentId===id||p.destination?.alignmentId===id||p.redoDestination?.alignmentId===id)||plannedStations.some(s=>s.alignmentId===id))refs.push("未着工計画");
    return refs.join("・");
  }
  const selectedTrackExists=selectedPlanPath!==null
    ?selectedPlanPath===activePathId?draftPoints.length>0:parkedPaths.some(p=>p.id===selectedPlanPath)
    :!!selectedAlignment;
  const selectedTrackReferences=selectedPlanPath!==null
    ?plannedStations.some(s=>s.pathId===selectedPlanPath)?"計画駅":""
    :alignmentId?alignmentReferences(alignmentId):"";
  function selectTrackPath(id:number){
    if(!enabled||mapMode!=="move"||suppressSelectionClick.current)return;
    setSelectedPlanPath(id);setAlignmentId("");setTrackDeleteConfirm(false);
  }
  function deleteSelectedTrack(){
    if(!enabled||unapplied||!selectedTrackExists||selectedTrackReferences)return;
    if(selectedPlanPath!==null){
      const id=selectedPlanPath,before=constructionPlans;
      setTrackDeleteRedo(null);
      const after:ConstructionPlans=id===activePathId?{...before,draftPoints:[],draftEditedGeometry:undefined,redoPoints:[],draftExtension:null,draftDestination:undefined,draftRedoDestination:undefined,draftFinished:false}:{...before,parkedPaths:before.parkedPaths.filter(p=>p.id!==id)};
      const saved=savedPlanFingerprint;
      restoreConstructionPlans(after);setSavedPlanFingerprint(saved);
      setTrackDeleteUndo({kind:"plan",before,after:JSON.stringify(after),world});
      setSelectedPlanPath(null);setTrackDeleteConfirm(false);return;
    }
    void perform(async()=>{
      const snapshot=await invoke<World>("delete_alignment",{alignmentId,constructionPlans});
      setWorld(snapshot);setDirty(true);setTrackDeleteRedo(null);setTrackDeleteUndo({kind:"built",world:snapshot,plans:planFingerprint,command:"delete_alignment",args:{alignmentId,constructionPlans}});
      setAlignmentId("");setTrackDeleteConfirm(false);
    });
  }
  const canUndoTrackDelete=trackDeleteUndo?.kind==="built"
    ?trackDeleteUndo.world===world&&trackDeleteUndo.plans===planFingerprint
    :trackDeleteUndo?.world===world&&trackDeleteUndo?.after===planFingerprint;
  const canRedoTrackDelete=!!trackDeleteRedo&&trackDeleteRedo.world===world&&trackDeleteRedo.plans===planFingerprint;
  useEffect(()=>{if(trackDeleteRedo&&!canRedoTrackDelete)setTrackDeleteRedo(null);},[trackDeleteRedo,canRedoTrackDelete]);
  function undoTrackDelete(){
    if(!enabled||!trackDeleteUndo||!canUndoTrackDelete)return;
    const entry=trackDeleteUndo;
    if(entry.kind==="plan"){
      const saved=savedPlanFingerprint;restoreConstructionPlans(entry.before);setSavedPlanFingerprint(saved);
      setTrackDeleteRedo({entry,world,plans:JSON.stringify(entry.before)});setTrackDeleteUndo(null);return;
    }
    void perform(async()=>{const snapshot=await invoke<World>("undo_alignment_deletion");setWorld(snapshot);setDirty(true);setTrackDeleteRedo({entry,world:snapshot,plans:planFingerprint});setTrackDeleteUndo(null);});
  }
  function redoTrackDelete(){
    if(!enabled||!trackDeleteRedo||!canRedoTrackDelete)return;
    const entry=trackDeleteRedo.entry;
    if(entry.kind==="plan"){
      const saved=savedPlanFingerprint;restoreConstructionPlans(JSON.parse(entry.after) as ConstructionPlans);setSavedPlanFingerprint(saved);
      setTrackDeleteUndo(entry);setTrackDeleteRedo(null);return;
    }
    void perform(async()=>{
      const snapshot=await invoke<World>(entry.command,entry.args);
      setWorld(snapshot);setDirty(true);setTrackDeleteUndo({...entry,world:snapshot});setTrackDeleteRedo(null);
    });
  }
  function deletionLabel(target:DeleteTarget){
    if(target.kind==="station")return (world?.stations.find(station=>station.id===target.stationId)?.name??"駅")+(world?.station_groups?.some(g=>g.members.includes(target.stationId))?"のホーム":"");
    if(target.kind==="plan-station")return "駅計画";
    if(target.kind==="plan-anchor")return "アンカー";
    if(target.kind==="plan-path")return "計画線路";
    if(target.kind==="plan-section")return "計画線路の区間";
    const line=alignments.find(line=>line.id===target.alignmentId),section=line&&deletionSections(line)[target.sectionIndex];
    return section?`建設済みの線路 ${(alignmentLength(section.points)/1000).toFixed(2)} km`:"建設済みの線路";
  }
  function deletionBlocked(target:DeleteTarget):string{
    if(unapplied)return "編集中の変更を適用または取り消してください";
    // Section safety depends on the removed span, not references to the whole line.
    // The transactional backend validates station overlap and surviving endpoints.
    if(target.kind==="built-section")return world?.alignments?.find(a=>a.id===target.alignmentId)?.owner!==world?.companies[0]?.id?"他社の線路は削除できません":"";
    if(target.kind==="station"){
      if(world?.stations.find(station=>station.id===target.stationId)?.owner!==world?.companies[0]?.id)return "他社の駅は削除できません";
      const refs=stationReferences(target.stationId);return refs?`${refs}が使用中です`:"";
    }
    try{removePlannedTarget(constructionPlans,target,extensionPrevious);return "";}catch(error){return detail(error);}
  }
  function hoverDeletion(event:ReactPointerEvent<SVGElement>,target:DeleteTarget){
    if(gestureRef.current||event.pointerType==="touch")return;
    const rect=svgRef.current?.getBoundingClientRect();if(rect)setDeleteHover({target,x:event.clientX-rect.left,y:event.clientY-rect.top});
  }
  function requestDeletion(event:MouseEvent<SVGElement>,target:DeleteTarget){
    event.stopPropagation();if(!enabled||busyRef.current||suppressSelectionClick.current)return;
    setError("");setDeleteHover(null);
    if(target.kind==="plan-anchor"){
      const blocked=deletionBlocked(target);
      if(blocked){setError(blocked);return;}
      deletePlannedTarget(target);return;
    }
    setDeleteTarget(target);
  }
  function deleteHitProps(target:DeleteTarget,filled=false){
    const hovered=JSON.stringify(deleteHover?.target)===JSON.stringify(target);
    return {className:"deletion-hit"+(hovered?" deletion-hover":"")+(filled?" deletion-filled":""),fill:filled?(hovered?"#db667080":"transparent"):"none",stroke:hovered?"#d45260":"transparent",strokeWidth:(filled?2:14)/viewport.scale,pointerEvents:filled?"all" as const:"stroke" as const,onPointerEnter:(event:ReactPointerEvent<SVGElement>)=>hoverDeletion(event,target),onPointerMove:(event:ReactPointerEvent<SVGElement>)=>hoverDeletion(event,target),onPointerLeave:()=>setDeleteHover(null),onClick:(event:MouseEvent<SVGElement>)=>requestDeletion(event,target)};
  }
  function deletePlannedTarget(target:PlannedDeleteTarget){
    try{
      const before=constructionPlans,after=removePlannedTarget(before,target,extensionPrevious),saved=savedPlanFingerprint;
      setTrackDeleteRedo(null);
      restoreConstructionPlans(after);setSavedPlanFingerprint(saved);setStationUndo([]);setStationRedo([]);
      setTrackDeleteUndo({kind:"plan",before,after:JSON.stringify(after),world});setDeleteTarget(null);setSelectedPlanPath(null);
    }catch(error){setError(detail(error));}
  }
  function confirmDeletion(){
    if(!enabled||!deleteTarget||deletionBlocked(deleteTarget))return;
    const target=deleteTarget;
    if(target.kind!=="built-section"&&target.kind!=="station"){
      deletePlannedTarget(target);return;
    }
    void perform(async()=>{
      const snapshot=await invoke<World>(target.kind==="station"?"delete_station":"delete_alignment_section",target.kind==="station"?{stationId:target.stationId,constructionPlans}:{alignmentId:target.alignmentId,sectionIndex:target.sectionIndex,constructionPlans});
      setWorld(snapshot);setDirty(true);setTrackDeleteRedo(null);setTrackDeleteUndo({kind:"built",world:snapshot,plans:planFingerprint,command:target.kind==="station"?"delete_station":"delete_alignment_section",args:target.kind==="station"?{stationId:target.stationId,constructionPlans}:{alignmentId:target.alignmentId,sectionIndex:target.sectionIndex,constructionPlans}});
      setDeleteTarget(null);setAlignmentId("");setSelected(null);setStationUndo([]);setStationRedo([]);
    });
  }
  function stationReferences(id: string) {
    const links = world?.tracks.filter((track) => track.from === id || track.to === id) ?? [];
    const routes = world?.routes.filter((route) => route.start === id || world?.route_paths?.some(path => path.route_id === route.id && (path.member_stations.includes(id) || path.segments.some(segment => segment.kind !== "track" ? segment.from === id || segment.to === id : links.some(link => link.id === segment.track_id)))) || route.tracks.some((track) => links.some((link) => link.id === track))) ?? [];
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
        if (active) { setInfo(engine); setCompanySetupRequired(engine.company_setup_required===true); setWorld(snapshot); restoreConstructionPlans(snapshot.construction_plans,true);restoreView(snapshot.view_state);setCompanyName(snapshot.companies[0]?.name ?? ""); setStatus("ready"); }
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
    cancelAnchorGesture();cancelMapGesture(); setMapMode(mode);setStationCursor(null); setCursorPosition(null); setHoverPoint(null); setPlanHover(null);setBuiltHover(null);
    if (!wasGesturing) { suppressMapClick.current = false; suppressSelectionClick.current = false; }
  }
  function applyAnchorEdit(id:number,value:{points:GeoPoint[];geometry?:EditedGeometry;redo:GeoPoint[];destination?:Extension;redoDestination?:Extension}){
    if(id===activePathId){setDraftDestination(value.destination);setDraftRedoDestination(value.redoDestination);setDraftPoints(value.points);setDraftEditedGeometry(value.geometry);setRedoPoints(value.redo);setHoverPoint(null);}
    else {parkedAnchorHistory.current.delete(id);setParkedPaths(paths=>paths.map(path=>path.id===id?{...path,destination:value.destination,redoDestination:value.redoDestination,points:value.points,editedGeometry:value.geometry,line:plannedGeometry(value.points,value.geometry,extensionPrevious(path.extension)).points,redo:value.redo}:path));}
  }
  function undoLastSegment(historyEntries=0){
    const entry:AnchorEdit={kind:"point",id:activePathId,
      before:{points:[],geometry:draftEditedGeometry,redo:[...redoPoints,draftPoints[1],draftPoints[0]],redoDestination:draftDestination??draftRedoDestination},
      after:{points:draftPoints,geometry:draftEditedGeometry,redo:redoPoints,destination:draftDestination,redoDestination:draftRedoDestination}};
    applyAnchorEdit(activePathId,entry.before);
    if(historyEntries)setAnchorUndo(items=>items.slice(0,-historyEntries));
    setAnchorRedo(items=>[...items,entry]);
  }
  function undoAnchor(redo:boolean){
    const history=redo?anchorRedo:anchorUndo,entry=history.at(-1);if(!entry)return;
    if(!redo&&entry.kind==="point"&&entry.id===activePathId&&draftFinished&&draftPoints.length===2&&entry.before.points.length===1){
      const start=history.at(-2);
      undoLastSegment(start?.kind==="point"&&start.id===activePathId&&start.before.points.length===0&&start.after.points.length===1?2:1);return;
    }
    applyAnchorEdit(entry.id,redo?entry.after:entry.before);
    if(redo){setAnchorRedo(items=>items.slice(0,-1));setAnchorUndo(items=>[...items,entry]);}else{setAnchorUndo(items=>items.slice(0,-1));setAnchorRedo(items=>[...items,entry]);}
  }
  function cancelAnchorGesture(){
    const gesture=anchorGesture.current;anchorGesture.current=null;setAnchorPreview(null);
    if(gesture){suppressMapClick.current=true;suppressSelectionClick.current=true;if(svgRef.current?.hasPointerCapture?.(gesture.pointerId))svgRef.current.releasePointerCapture(gesture.pointerId);}
  }
  function beginAnchorGesture(event:ReactPointerEvent<SVGRectElement>,id:number,index:number){
    if(parallelMode)return;
    if(!enabled||busyRef.current||activeTool!=="tracks"||referenceCopy.active||event.button!==0||event.isPrimary===false)return;
    const path=id===activePathId?parkCurrentPlan():parkedPaths.find(p=>p.id===id);
    if(!path||path.reference||path.roadFollowing||path.points.length<2||(index===0&&path.extension))return;
    event.stopPropagation();smoothZoom.cancel();cancelMapGesture();
    const gesture:AnchorGesture={pointerId:event.pointerId,id,index,startX:event.clientX,startY:event.clientY,points:path.points,line:path.line,geometry:path.editedGeometry,redo:path.redo,destination:path.destination,redoDestination:path.redoDestination,previous:extensionPrevious(path.extension),preview:null,error:null,snapped:false};
    anchorGesture.current=gesture;setAnchorPreview(gesture);setHoverPoint(null);setPlanHover(null);setBuiltHover(null);
    svgRef.current?.setPointerCapture?.(event.pointerId);
  }
  function moveAnchorGesture(event:ReactPointerEvent<SVGSVGElement>){
    const gesture=anchorGesture.current;if(!gesture||gesture.pointerId!==event.pointerId)return;
    if(!gesture.preview&&Math.hypot(event.clientX-gesture.startX,event.clientY-gesture.startY)<5)return;
    const rect=event.currentTarget.getBoundingClientRect();setCursorPosition({x:event.clientX-rect.left,y:event.clientY-rect.top,width:rect.width,height:rect.height});let target=coordinatesFromClient(event.clientX,event.clientY,rect,camera);
    if(!target)return;
    const anchors=[...alignments.flatMap(line=>[line.points[0],line.points.at(-1)!]),...parkedPaths.filter(p=>p.id!==gesture.id).flatMap(p=>p.points),...(gesture.id!==activePathId?draftPoints:[])];
    const nearest=anchors.map(point=>({point,distance:endpointDistance(target!,point,rect.width,rect.height)})).sort((a,b)=>a.distance-b.distance)[0];
    gesture.snapped=!!nearest&&nearest.distance<=14;if(gesture.snapped)target=nearest.point;
    else {
      const lines=[...alignments.map(line=>line.points),...parkedPaths.filter(path=>path.id!==gesture.id).map(path=>path.line),...(gesture.id!==activePathId?[draftLine]:[])];
      const hit=lines.filter(line=>line.length>1).map(line=>nearestOnAlignment(line,target!).point).map(point=>({point,distance:endpointDistance(target!,point,rect.width,rect.height)})).sort((a,b)=>a.distance-b.distance)[0];
      if(hit&&hit.distance<=6){target=hit.point;gesture.snapped=true;}
    }
    const preview=anchorDragPreview(gesture.line,gesture.points,gesture.index,target);
    const points=gesture.points.map((point,index)=>index===gesture.index?target!:point);
    const geometry:EditedGeometry={version:1,anchors:points,line:preview.line};
    const validation=plannedGeometry(points,geometry,gesture.previous);
    gesture.preview=geometry;gesture.error=preview.error||validation.error;
    // Do not silently create a zero-length anchor interval, even if sampling obscures it.
    if(points.some((point,index)=>index>0&&distanceM(point,points[index-1])<.1))gesture.error="アンカーが重なっています";
    setAnchorPreview({...gesture});
  }
  function endAnchorGesture(event:ReactPointerEvent<SVGSVGElement>,canceled=false){
    const gesture=anchorGesture.current;if(!gesture||gesture.pointerId!==event.pointerId)return;
    if(!canceled)moveAnchorGesture(event);
    const moved=!!gesture.preview;
    if(!canceled&&gesture.preview&&!gesture.error){
      const entry:AnchorEdit={id:gesture.id,before:{points:gesture.points,geometry:gesture.geometry,redo:gesture.redo,destination:gesture.destination,redoDestination:gesture.redoDestination},after:{points:gesture.preview.anchors,geometry:gesture.preview,redo:[],destination:gesture.index===gesture.points.length-1?undefined:gesture.destination}};
      applyAnchorEdit(gesture.id,entry.after);setAnchorUndo(items=>[...items,entry].slice(-50));setAnchorRedo([]);setError("");
    }else if(!canceled&&gesture.error)setError(gesture.error);
    cancelAnchorGesture();
    // A click without dragging retains finish/extend semantics.
    if(!moved&&!canceled){suppressMapClick.current=false;suppressSelectionClick.current=false;}
  }
  function pointerDown(event: ReactPointerEvent<SVGSVGElement>) {
    smoothZoom.cancel();
    setPlanHover(null);setBuiltHover(null);referenceCopy.clearHover();
    if (event.button !== 0 || event.isPrimary === false || gestureRef.current) return;
    suppressMapClick.current = true;
    const rect = event.currentTarget.getBoundingClientRect();
    if (!coordinatesFromClient(event.clientX, event.clientY, rect, cameraRef.current)) return;
    const { scale } = mapViewport(rect.width, rect.height);
    const existingEquipment = activeTool!=="stations" && event.target instanceof Element && !!event.target.closest(".station-marker, .track-connection");
    gestureRef.current = { pointerId: event.pointerId, startX: event.clientX, startY: event.clientY, camera: cameraRef.current, scale, dragged: false, mayPlace: ((mapMode === "place" && activeTool === "stations") || (mapMode === "draw" && activeTool === "tracks")) && !existingEquipment };
    suppressMapClick.current = false;
    suppressSelectionClick.current = false;
  }
  function pointerMove(event: ReactPointerEvent<SVGSVGElement>) {
    if(anchorGesture.current){moveAnchorGesture(event);return;}
    if(enabled&&referenceCopy.active&&referenceCopy.roadFollow&&mapMode==="draw"&&!gestureRef.current){const p=coordinatesFromClient(event.clientX,event.clientY,event.currentTarget.getBoundingClientRect(),camera);if(p)void referenceCopy.hover(p);}

    if(enabled&&activeTool==="stations"&&mapMode==="place"&&!gestureRef.current){const rect=event.currentTarget.getBoundingClientRect();setStationCursor(coordinatesFromClient(event.clientX,event.clientY,rect,camera));setCursorPosition({x:event.clientX-rect.left,y:event.clientY-rect.top,width:rect.width,height:rect.height});}
    if (enabled && !referenceCopy.active && mapMode === "draw" && activeTool === "tracks" && !gestureRef.current) {
      const rect=event.currentTarget.getBoundingClientRect(),point=coordinatesFromClient(event.clientX,event.clientY,rect,camera);
      setCursorPosition(point?{x:event.clientX-rect.left,y:event.clientY-rect.top,width:rect.width,height:rect.height}:null);
      drawingCursor.current=point?{point,width:rect.width,height:rect.height}:null;
      setHoverPoint(parallelMode?point:point && !snappedEndpoint(point,rect.width,rect.height)?drawingPoint(point,rect.width,rect.height,event.ctrlKey):null);
    }
    const gesture = gestureRef.current;
    if (!gesture || gesture.pointerId !== event.pointerId) return;
    const dx = event.clientX - gesture.startX, dy = event.clientY - gesture.startY;
    if (!gesture.dragged && Math.hypot(dx, dy) < 5) return;
    if (!gesture.dragged && event.type === "pointermove") event.currentTarget.setPointerCapture?.(event.pointerId);
    gesture.dragged = true; suppressMapClick.current = true; suppressSelectionClick.current = true; setDragging(true);
    changeCamera(panCamera(gesture.camera, -dx / gesture.scale, -dy / gesture.scale));
  }
  function pointerEnd(event: ReactPointerEvent<SVGSVGElement>, canceled = false) {
    if(anchorGesture.current){endAnchorGesture(event,canceled);return;}
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
    if(referenceExtensionError)throw Error(referenceExtensionError);
    build=referenceExtension?.build??build;
    const stations=plannedPlatforms.filter(item=>item.station.pathId===-1);
    if(stations.some(item=>!item.valid))throw Error("計画駅が線路に収まりません。");
    const id=nextPathId.current++;
    setParkedPaths(paths=>[...paths,{id,planId:activePlanId,name:planName,points:build.points,redo:[],line:build.points,designSpeedKmh:referenceExtension?(referenceExtension.alignment.design_speed_kmh??defaultDesignSpeed(referenceExtension.alignment.track_type??"normal")):designSpeedKmh,trackType:referenceExtension?.alignment.track_type??trackType,roadFollowing:false,extension:referenceExtension?{alignmentId:referenceExtension.alignment.id,endpoint:referenceExtension.endpoint}:null,reference:build}]);
    setPlannedStations(items=>items.map(station=>station.pathId===-1?{...station,pathId:id}:station));
    const moveHistory=(entries:StationHistoryEntry[])=>entries.map(entry=>entry.planned?.pathId===-1?{...entry,planned:{...entry.planned,pathId:id}}:entry);
    setStationUndo(moveHistory);setStationRedo(moveHistory);
    setNotice("実在形状に沿う経路を選択中の計画へ追加しました。『計画を着工』で他の経路・駅とまとめて建設します。");
  }
  function destinationTarget(point:GeoPoint,width:number,height:number) {
    if(!draftExtension||draftFinished||!draftPoints.length)return null;
    return alignments.flatMap(alignment=>[
      {alignment,endpoint:"start" as const,point:alignment.points[0]},
      {alignment,endpoint:"end" as const,point:alignment.points.at(-1)!},
    ]).filter(e=>e.point&&!(e.alignment.id===draftExtension.alignmentId&&e.endpoint===draftExtension.endpoint)).map(e=>({...e,distance:endpointDistance(point,e.point,width,height)}))
      .filter(e=>e.distance<=8).sort((a,b)=>a.distance-b.distance)[0]??null;
  }
  function joinError(line:GeoPoint[],source:Extension|null,destination:Extension|undefined){
    return destinationJoinError(line,source,destination,alignments,world?.alignment_connections??[],world?.companies[0]?.id);
  }
  function startTarget(coordinates:GeoPoint,width:number,height:number) {
        const candidates=[...(draftFinished?[{id:activePathId,points:draftPoints}]:[]),...parkedPaths.filter(path=>path.planId===activePlanId&&!path.reference)]
          .filter(plan=>plan.points.length>=1)
          .map(plan=>({id:plan.id,distance:endpointDistance(coordinates,plan.points[plan.points.length-1],width,height)}))
          .filter(plan=>plan.distance<=8).sort((a,b)=>a.distance-b.distance);

    if(candidates.length)return {kind:"plan" as const,id:candidates[0].id,
      point:candidates[0].id===activePathId?draftPoints.at(-1)!:parkedPaths.find(path=>path.id===candidates[0].id)!.points.at(-1)!};
        const builtEnds=alignments.flatMap(alignment=>[
          {alignment,endpoint:"start" as const,point:alignment.points[0]},
          {alignment,endpoint:"end" as const,point:alignment.points.at(-1)!},
        ]).filter(item=>item.point).map(item=>({...item,distance:endpointDistance(coordinates,item.point,width,height)}))
          .filter(item=>item.distance<=8).sort((a,b)=>a.distance-b.distance);

    return builtEnds.length?{kind:"built" as const,...builtEnds[0]}:null;
  }
  const endpointCandidate=!parallelMode&&enabled&&activeTool==="tracks"&&mapMode==="draw"&&!referenceCopy.active&&!roadFollowingBlocked&&!dragging&&(draftFinished||!draftPoints.length)&&hoverPoint&&cursorPosition
    ?startTarget(hoverPoint,cursorPosition.width,cursorPosition.height):null;
  const endpointCandidateAllowed=endpointCandidate?.kind!=="built"||(endpointCandidate.alignment.owner===world?.companies[0]?.id&&!occupiedEndpoint(world?.alignment_connections??[],{alignmentId:endpointCandidate.alignment.id,endpoint:endpointCandidate.endpoint}));
  const destinationCandidate=enabled&&activeTool==="tracks"&&mapMode==="draw"&&!referenceCopy.active&&!dragging&&hoverPoint&&cursorPosition?destinationTarget(hoverPoint,cursorPosition.width,cursorPosition.height):null;
  const destinationError=destinationCandidate?(previewGeometry.error||joinError(previewLine,draftExtension,{alignmentId:destinationCandidate.alignment.id,endpoint:destinationCandidate.endpoint})):null;
  // Show where the current preview would continue, not the tangent at its
  // already-fixed start. Invalid previews have no reliable outgoing tangent.
  const guideGeometry=hoverPoint?previewGeometry:draftGeometry;
  const guideEndPoint=guideGeometry.points.at(-1);
  const tangentGuideStart=guideEndPoint?project(guideEndPoint.latitude,guideEndPoint.longitude,camera):null;
  // Reach beyond every viewport corner, even if the fixed end is offscreen.
  // Use the observed viewport rather than stale pointer dimensions after resize.
  const guideLength=tangentGuideStart?Math.hypot(
    Math.max(Math.abs(tangentGuideStart.x-viewport.x),Math.abs(tangentGuideStart.x-viewport.x-viewport.width)),
    Math.max(Math.abs(tangentGuideStart.y-viewport.y),Math.abs(tangentGuideStart.y-viewport.y-viewport.height)),
  )+32/viewport.scale:0;
  const tangentGuide=!parallelMode&&activeTool==="tracks"&&mapMode==="draw"&&!referenceCopy.active&&!draftFinished&&!dragging&&!anchorPreview&&guideGeometry.tangent&&tangentGuideStart
    ?{start:tangentGuideStart,end:{x:tangentGuideStart.x+guideGeometry.tangent.x*guideLength,y:tangentGuideStart.y-guideGeometry.tangent.y*guideLength}}:null;

  const parallelSources=alignments.filter(line=>line.owner===world?.companies[0]?.id&&line.points.length>1);
  const parallelCandidate=parallelMode&&hoverPoint?parallelSources.map(source=>({source,projection:parallelProjection(source.points,hoverPoint)})).filter(item=>item.projection&&item.projection.distance<=PARALLEL_RANGE_M).sort((a,b)=>a.projection!.distance-b.projection!.distance)[0]:undefined;
  const parallelSource=parallelSources.find(source=>source.id===parallelStart?.sourceId);
  const parallelOffsetLine=parallelSource&&parallelStart?offsetTrack(parallelSource.points,parallelStart.offset):null;
  const parallelPreview=parallelOffsetLine&&parallelStart&&hoverPoint&&(parallelProjection(parallelSource!.points,hoverPoint)?.distance??Infinity)<=PARALLEL_RANGE_M?parallelSection(parallelOffsetLine,parallelStart.point,hoverPoint):null;
  function placeParallel(point:GeoPoint){
    if(!parallelStart){
      const pick=parallelSources.map(source=>({source,p:parallelProjection(source.points,point)})).filter(item=>item.p&&item.p.distance<=PARALLEL_RANGE_M&&Math.abs(item.p.offset)>=3).sort((a,b)=>a.p!.distance-b.p!.distance)[0];
      if(!pick)return;
      const line=offsetTrack(pick.source.points,pick.p!.offset),start=line&&parallelProjection(line,point)?.point;if(!start)return;
      setParallelStart({sourceId:pick.source.id,offset:pick.p!.offset,point:start});return;
    }
    if(!parallelOffsetLine)return;
    const target=parallelProjection(parallelSource!.points,point);if(!target||target.distance>PARALLEL_RANGE_M)return;
    const geometry=parallelSection(parallelOffsetLine,parallelStart.point,point);if(!geometry)return;
    newPath(true);setDraftPoints(geometry.anchors);setDraftEditedGeometry(geometry);setDraftFinished(true);setRedoPoints([]);setAnchorUndo([]);setAnchorRedo([]);setParallelStart(null);setHoverPoint(null);
  }

  function mapClick(event: MouseEvent<SVGSVGElement>) {
    smoothZoom.cancel();
    if (!enabled || suppressMapClick.current || mapMode === "move") return;
    const rect = event.currentTarget.getBoundingClientRect();
    const rawCoordinates = coordinatesFromClient(event.clientX, event.clientY, rect, camera);
    if (!rawCoordinates) return;
    let coordinates:GeoPoint=rawCoordinates;
    if (activeTool === "tracks" && mapMode === "draw") {
      if(parallelMode){if(event.detail<2)placeParallel(coordinates);return;}
      if(referenceCopy.active){void referenceCopy.click(coordinates);return;}
      if(roadFollowingBlocked)return;
      // A fresh click on the last anchor finishes the path, independent of
      // double-click timing. Ignore trailing clicks of that same gesture.
      if(draftFinished || !draftPoints.length){
        if(event.detail>=2)return;
        const target=startTarget(coordinates,rect.width,rect.height);
        if(target?.kind==="plan"){
          if(target.id===activePathId){if(draftDestination)return;setDraftFinished(false);setHoverPoint(null);}
          else selectPath(target.id);
          return;
        }
        if(target?.kind==="built"){
          if(target.alignment.owner!==world?.companies[0]?.id){setError("他社の線路は延伸できません。");return;}
          if(occupiedEndpoint(world?.alignment_connections??[],{alignmentId:target.alignment.id,endpoint:target.endpoint})){setError("この端点は接続済みです");return;}
          if(draftFinished)newPath(true);
          setDraftDestination(undefined);setDraftRedoDestination(undefined);setDraftExtension({alignmentId:target.alignment.id,endpoint:target.endpoint});
          setTrackType(target.alignment.track_type??"normal");setDesignSpeedKmh(target.alignment.design_speed_kmh??defaultDesignSpeed(target.alignment.track_type??"normal"));setRoadFollowing(false);
          setDraftPoints([target.point]);setRedoPoints([]);setHoverPoint(null);setDraftFinished(false);
          setNotice("建設済み線路の"+(target.endpoint==="start"?"始点":"終点")+"から延伸を計画しています。着工まで既存線路は変更されません。");return;
        }
        setDraftExtension(null);setDraftDestination(undefined);setDraftRedoDestination(undefined);
        if(draftFinished){newPath(true);setDraftPoints([coordinates]);return;}
      }
      if(snappedEndpoint(coordinates,rect.width,rect.height)){
        if(draftPoints.length>=2&&!draftInvalid){setDraftFinished(true);setHoverPoint(null);}
        return;
      }
      const destination=destinationTarget(coordinates,rect.width,rect.height);
      coordinates=destination?.point??drawingPoint(coordinates,rect.width,rect.height,event.ctrlKey);
      let editedForAppend=draftEditedGeometry&&redoPoints.length?(draftPoints.length>=2?{version:1 as const,anchors:draftPoints,line:draftLine}:undefined):draftPoints.length?draftEditedGeometry:undefined;
      if(destination)editedForAppend=fittedDestination([...draftPoints,coordinates],editedForAppend,destination);
      const candidate=plannedGeometry([...draftPoints,coordinates],editedForAppend,previousPoint);if(candidate.error){setHoverPoint(coordinates);return;}
      const targetRef=destination?{alignmentId:destination.alignment.id,endpoint:destination.endpoint}:undefined;
      const targetError=joinError(candidate.points,draftExtension,targetRef);if(targetError){setError(targetError);return;}
      if (draftPoints.length && distanceM(draftPoints[draftPoints.length - 1], coordinates) < .1) return;
      if (draftPoints.length >= 128) { setError("線形点は128点までです。一度建設を確定してください。"); return; }
      setAnchorUndo(items=>[...items,{kind:"point" as const,id:activePathId,before:{points:draftPoints,geometry:draftEditedGeometry,redo:[...redoPoints,coordinates],destination:draftDestination,redoDestination:targetRef??draftRedoDestination},after:{points:[...draftPoints,coordinates],geometry:editedForAppend,redo:[],destination:targetRef}}].slice(-50));setAnchorRedo([]);setDraftDestination(targetRef);setDraftRedoDestination(undefined);if(targetRef)setDraftFinished(true);setDraftEditedGeometry(editedForAppend);setDraftPoints((points) => [...points, coordinates]); setRedoPoints([]); setHoverPoint(null); return;
    }
    if (activeTool !== "stations" || mapMode !== "place") return;
    if(event.detail>=2)return;
    placePlatformAt(coordinates,rect.width,rect.height);
  }

  const currentPaths:PlannedPath[]=[...parkedPaths.filter(path=>path.planId===activePlanId),...(draftPoints.length||redoPoints.length?[{id:activePathId,planId:activePlanId,name:planName,points:draftPoints,redo:redoPoints,line:draftLine,editedGeometry:draftEditedGeometry,designSpeedKmh,trackType,roadFollowing,extension:draftExtension,destination:draftDestination,redoDestination:draftRedoDestination}]:[])];
  const currentStations=plannedPlatforms.filter(item=>stationPlanId(item.station)===activePlanId);
  const planHasContent=currentPaths.length>0||currentStations.length>0;
  // Keep fully undone paths for Redo, but do not submit them as empty alignments.
  const buildPaths=currentPaths.filter(path=>path.points.length>0);
  const planBuildable=(buildPaths.length>0||currentStations.some(item=>item.station.alignmentId))&&currentStations.every(item=>item.station.pathId===undefined||buildPaths.some(path=>path.id===item.station.pathId))&&buildPaths.every(path=>path.line.length>=2&&!(path.reference?extensionJoinError(path.points,extensionPrevious(path.extension)):plannedGeometry(path.points,path.editedGeometry,extensionPrevious(path.extension)).error)&&!joinError(path.line,path.extension,path.destination)&&!(path.roadFollowing));
  function commitAlignment() {
    if (!enabled || !planBuildable || anchorGesture.current) return;
    void perform(async () => {
      const ids=new Set(buildPaths.map(path=>path.id));
      if(currentStations.some(item=>!item.valid))throw Error("計画駅が線路に収まりません。線形を元に戻すか、計画駅を取り消してください。");
      const requests=buildPaths.map(path=>({name:alignmentName,points:path.line,anchorIndices:anchorIndices(path.points,path.line),trackType:path.trackType,designSpeedKmh:path.designSpeedKmh,extension:path.extension??undefined,destination:path.destination,reference:path.reference?{sourceRecord:path.reference.sourceRecord,provenance:path.reference.provenance}:undefined,stations:plannedPlatforms.filter(item=>item.station.pathId===path.id).map(item=>({name:stationDisplayGroup(item.station.id)?.name??item.station.name,position:item.position,lengthM:item.station.lengthM,widthM:12}))}));
      const stations=currentStations.filter(item=>item.station.alignmentId).map(item=>({alignmentId:item.station.alignmentId!,name:stationDisplayGroup(item.station.id)?.name??item.station.name,position:item.position,lengthM:item.station.lengthM,widthM:12}));
      const first=requests[0];
      const batch=stations.length>0||requests.length>1||requests.some(request=>request.extension||request.reference);
      const snapshot=await invoke<World>(batch?"build_plan":first.stations.length?"build_alignment_with_stations":"build_alignment",batch?{alignments:requests,stations}:first.stations.length?first:{name:first.name,points:first.points,anchorIndices:first.anchorIndices,trackType:first.trackType,designSpeedKmh:first.designSpeedKmh});
      const extendedIds=new Set(requests.flatMap(request=>request.extension?[request.extension.alignmentId]:[]));
      const keepHistory=(entry:StationHistoryEntry)=>Boolean(entry.planned)||!entry.placement||!extendedIds.has(entry.placement.alignment_id);
      setStationUndo(history=>history.filter(keepHistory));setStationRedo(history=>history.filter(keepHistory));
      ids.forEach(discardPlanStations);setPlannedStations(items=>items.filter(station=>stationPlanId(station)!==activePlanId));setStationUndo(items=>items.filter(entry=>!entry.planned||stationPlanId(entry.planned)!==activePlanId));setStationRedo(items=>items.filter(entry=>!entry.planned||stationPlanId(entry.planned)!==activePlanId));setParkedPaths(paths=>paths.filter(path=>path.planId!==activePlanId));
      const created=snapshot.alignments?.find(a=>!alignments.some(old=>old.id===a.id));
      setWorld(snapshot);setDirty(true);setConfirmLoad(false);setDraftFinished(false);setDraftPoints([]);setDraftEditedGeometry(undefined);setAnchorUndo([]);setAnchorRedo([]);setRedoPoints([]);setHoverPoint(null);setDraftExtension(null);setDraftDestination(undefined);setDraftRedoDestination(undefined);
      const saved=savedPlanFingerprint;restoreConstructionPlans(removeConstructionPlan(constructionPlans,activePlanId));setSavedPlanFingerprint(saved);
      if(created)setAlignmentId(created.id);
      const builtStations=snapshot.stations.filter(station=>!world?.stations.some(old=>old.id===station.id)).map(station=>({station,placement:snapshot.station_placements?.find(p=>p.station_id===station.id)}));
      setStationUndo(history=>[...history,...builtStations].slice(-50));setStationRedo([]);
      changeMapMode(activeTool==="stations"?"place":"draw");setNotice(requests.length?"線路を建設しました。計画駅も反映しました。":"計画駅を建設しました。");
    });
  }
  function renamePlannedStation(id:string,name:string) {
    const group=stationDisplayGroup(id);if(group?.members.some(m=>!m.planned))return;
    const ids=new Set(group?.members.filter(m=>m.planned).map(m=>m.id)??[id]);
    setPlannedStations(items=>items.map(s=>ids.has(s.id)?{...s,name}:s));
    const rename=(items:StationHistoryEntry[])=>items.map(entry=>entry.planned&&ids.has(entry.planned.id)?{...entry,station:{...entry.station,name},planned:{...entry.planned,name}}:entry);
    setStationUndo(rename);setStationRedo(rename);
  }
  function placePlatformAt(point:GeoPoint,width:number,height:number) {
    if(!enabled||busyRef.current)return;
    const selection=stationTarget(stationLines,point,Number(platformLength),camera,mapViewport(width,height).scale,occupiedStationPlatforms);
    setStationCursor(point);
    if(selection.error||!selection.target){setError(selection.error);return;}
    const {line,position,center}=selection.target;
    const joinedGroup=stationGroupDisplay.find(g=>g.members.some(m=>m.owner===world?.companies[0]?.id&&m.lineId!==line.id&&footprintsIntersect(m.polygon,selection.target!.polygon)));
    const name=joinedGroup?.name??automaticStationName(center,referenceRepository,[...(world?.stations.map(s=>s.name)??[]),...plannedStations.map(s=>s.name)]).name;
    const planId=line.pathId===undefined?activePlanId:line.pathId===activePathId||line.pathId===-1?activePlanId:parkedPaths.find(path=>path.id===line.pathId)?.planId;
    if(planId===undefined){setError("駅の所属計画が見つかりません。");return;}
    const planned:PlannedStation={id:"planned:"+nextPlannedStationId.current++,...(line.pathId===undefined?{alignmentId:line.id}:{pathId:line.pathId}),planId,name,point:center,lengthM:Number(platformLength)};
    setPlannedStations(items=>[...items,planned]);
    setStationUndo(items=>[...items,{station:{id:planned.id,owner:"",name,...center},planned}].slice(-50));setStationRedo([]);
    setError("");setNotice(name+"を計画に追加しました。計画を着工するまで建設されません。");
  }
  const resizeKey=JSON.stringify([selected,editLength,editWidth]);
  const resizeValid=!!editLength&&Number.isFinite(Number(editLength))&&Number(editLength)>=40&&Number(editLength)<=400;
  async function resizePlatform() {
    if (!enabled || busyRef.current || !selected || !platformDirty || !resizeValid) return;
    const stationId=selected,lengthM=Number(editLength),widthM=Number(editWidth),key=resizeKey;
    setStationResizePending(true);
    await perform(async () => {
      try {
        const snapshot=await invoke<World>("update_station_placement",{stationId,lengthM,widthM});
        // The server owns endpoint clamping. Never overwrite a newer typed length.
        setWorld(snapshot);setDirty(true);setConfirmLoad(false);setStationResizeFailure(null);
        setNotice("ホーム寸法を変更しました。保存でファイルに反映します。");clearStationHistory();
      } catch(failure) {setStationResizeFailure(key);throw failure;}
    });
    setStationResizePending(false);
  }
  useEffect(()=>{
    if(!enabled||!platformDirty||!resizeValid||pendingStation||confirmLoad||stationResizeFailure===resizeKey)return;
    const timer=setTimeout(()=>{void resizePlatform();},150);
    return ()=>clearTimeout(timer);
  },[enabled,platformDirty,resizeValid,resizeKey,pendingStation,confirmLoad,stationResizeFailure]);
  function stationHistoryBlock(){
    const placement=world?.station_placements?.find(p=>p.station_id===selected);
    if(pendingDelete || pendingStation || confirmLoad)return "表示中の確認を完了または取り消してください。";
    if(saveBlocked || (placement && (Number(editLength)!==placement.length_m || Number(editWidth)!==placement.width_m)))return "編集中の下書き・配置プレビューを適用または解除してください。";
    return "";
  }
  function undoStation(){
    if(!enabled || busyRef.current)return;
    const entry=stationUndo.at(-1);if(!entry){setNotice("取り消せる駅設置履歴はありません。保存済み・読込済みの駅は対象外です。");return;}
    if(pendingDelete||pendingStation||confirmLoad){setError("表示中の確認を完了または取り消してください。");return;}
    if(entry.planned){setPlannedStations(items=>items.filter(item=>item.id!==entry.planned!.id));setStationUndo(items=>items.slice(0,-1));setStationRedo(items=>[...items,entry]);setNotice("計画駅の設置を取り消しました。");return;}
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
    if(pendingDelete||pendingStation||confirmLoad){setError("表示中の確認を完了または取り消してください。");return;}
    if(entry.planned){
      if(!stationLines.some(line=>entry.planned!.alignmentId?line.pathId===undefined&&line.id===entry.planned!.alignmentId:line.pathId===entry.planned!.pathId)){setError("元の線路計画がありません。");return;}
      setPlannedStations(items=>[...items,entry.planned!]);setStationRedo(items=>items.slice(0,-1));setStationUndo(items=>[...items,entry]);setNotice("計画駅を再設置しました。");return;
    }
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
      if(deleteTarget)return;
      if (event.target instanceof HTMLElement && (event.target.isContentEditable || /INPUT|SELECT|TEXTAREA/.test(event.target.tagName))) return;
      if (!event.ctrlKey && !event.metaKey && !event.altKey && !event.isComposing && (event.key === "+" || event.key === "-" || event.code === "NumpadAdd" || event.code === "NumpadSubtract")) {
        event.preventDefault();
        zoomFromControls(event.key === "+" || event.code === "NumpadAdd" ? 1 : -1);
        return;
      }
      if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase()==="z") {
        event.preventDefault();if(event.repeat || busyRef.current)return;
        if(parallelStart){if(!event.shiftKey)cancelParallel();return;}
        if(anchorGesture.current){cancelAnchorGesture();return;}
        if(activeTool==="delete"){if(event.shiftKey)redoTrackDelete();else undoTrackDelete();return;}
        if(activeTool==="tracks" && (event.shiftKey?anchorRedo.length:anchorUndo.length)){undoAnchor(event.shiftKey);return;}
        if(referenceCopy.active && activeTool==="tracks"){if(event.shiftKey)referenceCopy.redoPick();else referenceCopy.undo();}
        else if(activeTool==="tracks" && (draftPoints.length || redoPoints.length || adjacentHistoryPath(event.shiftKey))){if(event.shiftKey)redoPoint();else undoPoint();}
        else if(event.shiftKey)redoStation();else undoStation();return;
      }
      if (event.key === "Escape" && anchorGesture.current) {event.preventDefault();cancelAnchorGesture();return;}
      if (event.key === "Escape") {cancelParallel(); if(activeTool==="tracks")setActiveTool("inactive"); changeMapMode("move"); setStationCursor(null); }
    };
    window.addEventListener("keydown", key); return () => window.removeEventListener("keydown", key);
  });
  useEffect(() => {
    const refresh=(disableStraightSnap:boolean)=>{
      const cursor=drawingCursor.current;
      if(!cursor||!enabled||referenceCopy.active||mapMode!=="draw"||activeTool!=="tracks"||gestureRef.current||anchorGesture.current)return;
      const {point,width,height}=cursor;
      setHoverPoint(!snappedEndpoint(point,width,height)?drawingPoint(point,width,height,disableStraightSnap):null);
    };
    const modifier=(event:KeyboardEvent)=>{
      if(event.key!=="Control"||event.repeat)return;
      if(event.target instanceof HTMLElement&&(event.target.isContentEditable||/INPUT|SELECT|TEXTAREA/.test(event.target.tagName)))return;
      refresh(event.type==="keydown");
    };
    const blur=()=>{drawingCursor.current=null;setHoverPoint(null);};
    window.addEventListener("keydown",modifier);window.addEventListener("keyup",modifier);window.addEventListener("blur",blur);
    return ()=>{window.removeEventListener("keydown",modifier);window.removeEventListener("keyup",modifier);window.removeEventListener("blur",blur);};
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
  function completeSave() {
    setSavedViewFingerprint(JSON.stringify(camera));setSavedPlanFingerprint(planFingerprint);
    setDirty(false);setStationUndo(history=>history.filter(entry=>entry.planned));setStationRedo(history=>history.filter(entry=>entry.planned));
    setNotice(referenceCopy.active?"ワールドと未着工計画を保存しました。選択中の参照候補は計画に追加するまで保存されません。":"ワールドと未着工計画を保存しました。");
  }
  function save() {
    if (saveBlocked||saveReplacement) return;
    void perform(async () => {
      const result=await invoke<{saved:boolean;replacement_required?:{token:string;company_name:string}}>("save_world",{constructionPlans:persistedPlans,viewState:camera}).catch(error=>{if(detail(error).includes("save file changed since it was observed"))throw Error("保存先が読み込み時から変更されています。編集中の内容は保持しています。未保存のまま読込や終了をせず、保存先を確認してください。");throw error;});
      if(result.replacement_required){setSaveReplacement(result.replacement_required);setReplacementError("");setReplacementInvalid(false);return;}
      if(!result.saved)throw Error("保存結果を確認できませんでした。編集中の内容は保持しています。");
      completeSave();
    });
  }
  async function cancelSaveReplacement(){
    if(!saveReplacement||busyRef.current)return;
    busyRef.current=true;setBusy(true);
    try{await invoke("cancel_manual_save",{token:saveReplacement.token});setSaveReplacement(null);setReplacementError("");}
    catch(error){setReplacementError(detail(error));}
    finally{busyRef.current=false;setBusy(false);}
  }
  async function confirmSaveReplacement(){
    if(!saveReplacement||replacementInvalid||busyRef.current)return;
    busyRef.current=true;setBusy(true);
    try{const result=await invoke<{saved:boolean}>("confirm_manual_save",{token:saveReplacement.token,constructionPlans:persistedPlans,viewState:camera});if(!result.saved)throw Error("保存結果を確認できませんでした。");completeSave();setSaveReplacement(null);setReplacementError("");}
    catch(error){setReplacementInvalid(true);setReplacementError("保存できませんでした。編集中の内容は保持しています。編集へ戻り、保存先を再確認してください。 "+detail(error));}
    finally{busyRef.current=false;setBusy(false);}
  }
  function requestLoad(source:"manual"|"auto"){setLoadSource(source);if(dirty||plansDirty||unapplied)setConfirmLoad(true);else load(source);}
  function load(source: "manual"|"auto" = loadSource) {
    setConfirmLoad(false);
    void perform(async () => {
      const snapshot = await invoke<World>(source==="auto"?"load_autosave":"load_world");
      restoreView(snapshot.view_state);
      setRestoredAutosaveEpoch(source==="auto"?worldEpoch+1:-1);
      setWorldEpoch(value=>value+1);
      referenceCopy.clear();setCandidateDirty(false);setPendingGraphChange(null);
      setCompanySetupRequired(false);
      clearStationHistory();
      setTrackDeleteUndo(null);setSelectedPlanPath(null);setTrackDeleteConfirm(false);setPathDraftEpoch(value=>value+1);setPathServiceDirty(false);setWorld(snapshot); setDirty(source==="auto"); restoreConstructionPlans(snapshot.construction_plans,true);setStationCursor(null); setHoverPoint(null); setStationCursor(null); setAlignmentId(""); setSelected(null); setEditStationName(""); setPendingStation(null); setPendingDelete(null); setStationFrom(""); setStationTo(""); setCompanyName(snapshot.companies[0]?.name ?? ""); resetRoute(); resetService();
      setNotice(source==="auto"?"自動保存から復元しました。手動保存データは上書きしていません。必要なら保存してください。":"保存したワールドを読み込みました。");
    });
  }

  const showPlanHover=!!planHover&&!!hoveredPlan&&!referenceCopy.active&&!dragging&&(activeTool==="tracks"||activeTool==="inactive");
  return (
    <main className="workbench" data-theme={theme} style={{"--rail-width":lineWidth,"--map-contrast":contrast/100,"--panel-width":panelWidth+"px","--ui-font":textSize+"px"} as CSSProperties}>
      <header className="app-header">
        <div className="session-status"><span className={`connection-dot ${status}`} aria-hidden="true" />{status === "ready" ? `${info?.version} / エンジン接続済み` : status === "preview" ? "ブラウザプレビュー" : status === "loading" ? "接続中" : "接続エラー"}</div>
      </header>
      <div className="toolbar">
        <div><span className="eyebrow">CONSTRUCTION LAB</span><h2>線路を描き、街をつなぐ。</h2></div>
        <div className="save-controls"><span className="save-state">{unapplied ? "未適用の下書きあり" : dirty||plansDirty ? "未保存の変更あり" : "ローカルワールド"}</span><button className="secondary" disabled={!enabled} onClick={() => requestLoad("manual")}>読込</button><button disabled={!enabled || saveBlocked} onClick={save}>保存</button></div>
      </div>
      {saveReplacement && <SaveReplacementDialog companyName={saveReplacement.company_name} currentCompanyName={world?.companies[0]?.name??companyName} busy={busy} invalid={replacementInvalid} error={replacementError} onCancel={()=>void cancelSaveReplacement()} onConfirm={()=>void confirmSaveReplacement()}/> }
      {closeGuard.confirmClose && <UnsavedCloseDialog error={closeGuard.closeError} closing={closeGuard.closing} onCancel={closeGuard.cancelClose} onDiscard={()=>void closeGuard.discardAndClose()} />}
      {pendingGraphChange && <div className="banner load-confirm" role="alert"><p>参考線データを変更すると、現在の実在形状に沿う敷設・道路追従の未着工計画を破棄します。変更しますか。</p><button className="secondary" onClick={()=>setPendingGraphChange(null)}>計画に戻る</button><button disabled={!enabled} onClick={()=>{referenceCopy.clear();setGraphBundle(pendingGraphChange.bundle);setPendingGraphChange(null);}}>参考計画を破棄して変更</button></div>}
      {confirmLoad && <div className="banner load-confirm" role="alert"><p>読込すると、現在の未保存の変更と未適用の下書きが破棄され、未着工計画を含め保存時点の状態に置き換わります。</p><button className="secondary" disabled={busy} onClick={() => setConfirmLoad(false)}>戻る</button><button disabled={busy} onClick={()=>load()}>読込を続ける</button></div>}
      {pendingDelete && <div className="banner load-confirm" role="alert"><p>{pendingDelete.kind === "station" ? "駅「" + (stationsById.get(pendingDelete.id)?.name ?? pendingDelete.id) + "」"+(world?.station_groups?.some(g=>g.members.includes(pendingDelete.id))?"の選択ホーム":"") : "線路 ID " + pendingDelete.id}を削除しますか。連鎖削除は行いません。適用後は保存が必要です。</p><button className="secondary" disabled={busy} onClick={() => setPendingDelete(null)}>削除をやめる</button><button className="danger" disabled={!enabled || unapplied || !!(pendingDelete.kind === "station" ? stationReferences(pendingDelete.id) : trackReferences(pendingDelete.id))} onClick={deleteEquipment}>削除を確定</button></div>}
      <div className="workspace">
        <section className="map-panel" aria-labelledby="map-title">
          <div className="map-notices" aria-label="通知">
      {closeGuard.closeError && !closeGuard.confirmClose && <div className="banner" role="alert">{closeGuard.closeError}</div>}

      {status === "preview" && <p className="banner">ブラウザでの画面確認です。表示はサンプルデータで、駅の配置・線路接続・保存・読込はデスクトップ版で利用できます。</p>}
      {status === "loading" && <p className="banner" role="status">共通エンジンからワールドを読み込んでいます。</p>}
      {status === "error" && <div className="banner error"><p>共通エンジンに接続できませんでした。</p><button onClick={() => setAttempt((x) => x + 1)}>再試行</button></div>}
      {autosave.permission==="blocked" && <div className="banner" role="status">以前の自動保存を保護するため、自動保存を保留しています。<button className="secondary" onClick={()=>chooseMainTab("saves")}>自動保存の保留を確認</button></div>}
      {autosave.message.includes("失敗") && <div className="banner" role="status">オートセーブの確認または保存に失敗しています。<button className="secondary" onClick={()=>chooseMainTab("saves")}>自動保存の状態を確認</button></div>}
      {saveBlocked && <p className="draft-warning">駅設定・会社・路線・系統の下書きを適用、または破棄してから手動保存してください。自動保存も適用済みの内容だけが対象です。</p>}
      {hasPlans && <p className="draft-warning">未着工計画は保存されません。建設済みワールドは保存でき、画面上の計画はそのまま残ります。</p>}
      {unapplied && (activeTool === "stations" || activeTool === "tracks") && <p className="draft-warning">設備削除は未適用の下書きを適用・破棄してから行えます。</p>}
      {error && !companySetupRequired && <ErrorNotice message={error} revision={errorRevision} onDismiss={()=>setError("")}/>}
      {displaySettings.error && <div className="display-settings-warning" role="status">{displaySettings.status}</div>}
          {background !== "sample" && <div className={`tile-status ${tileStatus.failed ? "tile-error" : ""}`}>
            <p role="status">{cameraPending ? "表示移動中。新しいタイルの取得を待機しています。" : tileStatus.loaded + tileStatus.failed < tileStatus.total ? `${provider.title}を読み込み中（${tileStatus.loaded} / ${tileStatus.total}）` : tileStatus.failed ? "地図の一部を表示できません。" : `${provider.title}を表示中（${tileStatus.total}枚）`}</p>
            {tileStatus.failed > 0 && <p role="alert">{tileStatus.failed}枚の読み込みに失敗しました。自動再試行やサンプルへの自動切替は行いません。</p>}
            <button className="secondary" onClick={() => setBackground("sample")}>サンプル背景に戻す</button>
          </div>}
          </div>
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
            <span>ZL {Number(camera.zoom.toFixed(2))} / Web Mercator</span><span>駅表示：ZL13以上はホーム帯、ZL13未満は点（寸法は不変）</span>
          </div>
          <div className="track-selection-controls" role="group" aria-label="線路選択">
            <button type="button" className="secondary" disabled={!enabled} aria-pressed={mapMode==="move"&&activeTool==="inactive"} onClick={()=>{setActiveTool("inactive");changeMapMode("move");setTrackDeleteConfirm(false);}}>選択</button>
            {selectedTrackExists&&mapMode==="move"&&<>{!(mainTab==="construction"&&activeTool==="inactive")&&<span>{selectedPlanPath!==null?"計画線路 "+selectedPlanPath:"線路 "+alignmentId}</span>}<button type="button" className="danger" disabled={!enabled||unapplied||!!selectedTrackReferences} onClick={()=>setTrackDeleteConfirm(true)}>選択線路を削除</button>{selectedTrackReferences&&<small>削除不可：{selectedTrackReferences}が使用中</small>}</>}
            {trackDeleteUndo&&activeTool!=="delete"&&<button type="button" className="secondary" disabled={!enabled||!canUndoTrackDelete} onClick={undoTrackDelete}>線路削除を元に戻す</button>}
          </div>
          {trackDeleteConfirm&&selectedTrackExists&&<TrackDeleteDialog busy={busy} blocked={!enabled||unapplied||!!selectedTrackReferences} error={error} onCancel={()=>setTrackDeleteConfirm(false)} onDelete={deleteSelectedTrack}/>}
          <div className="camera-controls" aria-label="地図表示操作">
            <div className="map-mode-controls" role="group" aria-label="地図の操作モード"><button className="secondary" disabled={activeTool === "tracks"} aria-pressed={mapMode === "move"} onClick={() => { changeMapMode("move"); }}>移動</button><button className="secondary" aria-pressed={mapMode === "place"} disabled={!enabled || activeTool !== "stations"} onClick={() => { changeMapMode("place"); }}>駅配置</button><span>ドラッグで移動 / 縦ホイールでカーソル位置をズーム</span></div>
            <div className="camera-buttons"><button className="secondary" aria-label="西へ移動" onClick={() => changeCamera(panCamera(camera, -300, 0))}>←</button><button className="secondary" aria-label="北へ移動" onClick={() => changeCamera(panCamera(camera, 0, -180))}>↑</button><button className="secondary" aria-label="南へ移動" onClick={() => changeCamera(panCamera(camera, 0, 180))}>↓</button><button className="secondary" aria-label="東へ移動" onClick={() => changeCamera(panCamera(camera, 300, 0))}>→</button><button className="secondary" aria-label="縮小" disabled={camera.zoom <= MIN_ZOOM} onClick={() => zoomFromControls(-1)}>−</button><button className="secondary" aria-label="拡大" disabled={camera.zoom >= MAX_ZOOM} onClick={() => zoomFromControls(1)}>＋</button><button className="secondary" onClick={() => changeCamera(DEFAULT_CAMERA)}>東京へ戻る</button></div>
            <form className="camera-coordinate-form" onSubmit={moveToCoordinates}><label htmlFor="camera-latitude">中心緯度</label><input id="camera-latitude" type="number" step="any" min={BOUNDS.south} max={BOUNDS.north} value={centerLatitude} onChange={(event) => setCenterLatitude(event.target.value)} /><label htmlFor="camera-longitude">中心経度</label><input id="camera-longitude" type="number" step="any" min={BOUNDS.west} max={BOUNDS.east} value={centerLongitude} onChange={(event) => setCenterLongitude(event.target.value)} /><button className="secondary" type="submit">座標へ移動</button></form>
            <p>閲覧・建設範囲: 北緯20〜46度 / 東経122〜154度。領土境界を示すものではありません。範囲端では表示を矩形内に制限します。</p>
            {cameraError && <p role="alert">{cameraError}</p>}
          </div>
          <ReferenceGraphControls bundle={graphBundle} visible={graphVisible} onBundle={bundle=>{if(referenceCopy.active)setPendingGraphChange({bundle});else setGraphBundle(bundle);}} onVisible={setGraphVisible} total={graphDisplay.total} shown={graphDisplay.edges.length} zoom={camera.zoom}/>
          <ReferenceStationControls pack={referencePack} visible={referenceVisible} onPack={setReferencePack} onVisible={setReferenceVisible} total={referenceResult.total} shown={referenceResult.stations.length}/>
          </details>
            <div className="map-surface">
            {deleteTarget&&<DeletionDialog defaultDelete={deleteTarget.kind==='built-section'} label={deletionLabel(deleteTarget)} busy={busy} blocked={deletionBlocked(deleteTarget)} error={error} onCancel={()=>setDeleteTarget(null)} onDelete={confirmDeletion}/>}
            {activeTool==="delete"&&deleteHover&&!dragging&&<div className="construction-tooltip deletion-tooltip" role="status" style={{left:Math.max(8,Math.min(mapSize.width-240,deleteHover.x+18)),top:Math.max(8,Math.min(mapSize.height-80,deleteHover.y+18))}}>{deletionLabel(deleteHover.target)}を{deleteHover.target.kind==="built-section"||deleteHover.target.kind==="station"?"撤去":"削除"}</div>}
            {activeTool==="stations"&&<fieldset className="construction-toolbar station-toolbar" disabled={!enabled} aria-label="駅設置設定"><PlatformLengthInput id="platform-length" label="駅の長さ" value={platformLength} onChange={setPlatformLength} compact/></fieldset>}
            {activeTool==="stations"&&(hoveredStation||stationJoinPreview)&&cursorPosition&&<div className="construction-tooltip" role="status" style={{left:Math.max(8,Math.min(mapSize.width-240,cursorPosition.x+18)),top:Math.max(8,Math.min(mapSize.height-80,cursorPosition.y+18))}}>{stationJoinPreview?.name??hoveredStation?.name}{stationJoinPreview&&stationJoinOrigin&&stationSelection?.target?<div>{stationJoinPreview.name}から {(distanceM(stationJoinOrigin,stationSelection.target.center)/1000).toFixed(2)} km</div>:hoveredStation?.lengthM!==undefined&&<> · {hoveredStation.lengthM/20}両</>}</div>}
            {activeTool==="stations"&&mapMode==="place"&&!dragging&&cursorPosition&&stationCursor&&!hoveredStation&&stationSelection?.error&&<div className="construction-tooltip station-placement-error" role="status" style={{left:Math.max(8,Math.min(mapSize.width-240,cursorPosition.x+18)),top:Math.max(8,Math.min(mapSize.height-80,cursorPosition.y+18))}}>{stationSelection.error}</div>}
            {activeTool === "tracks" && <div className="construction-toolbar" role="group" aria-label="線路敷設設定">            <TrackTypeControls value={trackType} disabled={!enabled||referenceCopy.active||!!draftExtension} onChange={chooseTrackType}/><div className="track-follow-options" role="group" aria-label="線路の追従"><button type="button" className="secondary follow-icon" aria-label="実在の路線網に合わせる" title="実在の路線網に合わせる" aria-pressed={referencePanel&&!roadFollowing} disabled={!enabled||!!draftExtension} onClick={()=>{cancelParallel();setParallelMode(false);referenceCopy.clear();referenceCopy.setMode("rail");setRoadFollowing(false);setReferencePanel(value=>!value||roadFollowing);}}><TrackFollowIcon kind="rail"/></button><button type="button" className="secondary follow-icon" aria-label="道路に合わせる" title="道路に合わせる" aria-pressed={roadFollowing} disabled={!enabled||!!draftExtension} onClick={()=>{cancelParallel();setParallelMode(false);referenceCopy.clear();setRoadFollowing(value=>!value);setReferencePanel(!roadFollowing);setHoverPoint(null);}}><TrackFollowIcon kind="road"/></button><button type="button" className="secondary follow-icon" aria-label="線路の横に沿わせる" title="線路の横に沿わせる" aria-pressed={parallelMode} disabled={!enabled||!copyCanStart} onClick={()=>{cancelParallel();referenceCopy.clear();setReferencePanel(false);setRoadFollowing(false);setParallelMode(value=>!value);setHoverPoint(null);}}><TrackFollowIcon kind="parallel"/><span className="follow-tip">線路の横に沿わせる<br/>青い範囲にカーソルを合わせると、別の線路と平行に線路を描きます</span></button></div>
            <div className="target-speed-control"><label htmlFor="track-target-speed">目標最高速度</label><span className="speed-input"><button type="button" className="secondary" aria-label="目標最高速度を20 km/h下げる" disabled={!enabled||!!draftExtension||referenceCopy.active||designSpeedKmh<=1} onClick={()=>setDesignSpeedKmh(value=>Math.max(1,value-20))}>−</button><input id="track-target-speed" aria-label="線路の目標最高速度" type="number" min="1" max="600" step="1" value={designSpeedKmh} disabled={!enabled||!!draftExtension||referenceCopy.active} onChange={event=>{const value=Number(event.target.value);if(Number.isFinite(value)&&value>=1&&value<=600)setDesignSpeedKmh(value);}}/><button type="button" className="secondary" aria-label="目標最高速度を20 km/h上げる" disabled={!enabled||!!draftExtension||referenceCopy.active||designSpeedKmh>=600} onClick={()=>setDesignSpeedKmh(value=>Math.min(600,value+20))}>＋</button><span>km/h</span></span></div>
</div>}
            {activeTool === "tracks" && mapMode==="draw" && !parallelMode && !referenceCopy.active && !dragging && !anchorPreview && cursorPosition && !endpointCandidate && !draftFinished && !showPlanHover && (draftPoints.length>0 || roadFollowingBlocked || !!previewError) && <div className="construction-tooltip drawing-tooltip drawing-metrics" role="status" style={{left:Math.max(8,Math.min(mapSize.width-210,cursorPosition.x+18)),top:Math.max(8,Math.min(mapSize.height-90,cursorPosition.y+18))}}>
              {(roadFollowingBlocked || previewError)&&<strong>{roadFollowingBlocked?"道路追従を開始するか、道路に合わせるをOFFにしてください":`敷設不可：${previewError}`}</strong>}
              {draftPoints.length>0&&<><div>長さ {Math.round(alignmentLength(draftLine))} m{candidateLine.length>1&&!previewError?` ＋ ${Math.round(alignmentLength(candidateLine))} m`:""}</div>{!previewError&&<div>速度上限 <span className="drawing-speed">{Math.min(designSpeedKmh,circleGuide?(provisionalCurveSpeed(circleGuide.radiusM)??designSpeedKmh):designSpeedKmh)} km/h</span>{circleGuide&&<small>（暫定）</small>}</div>}</>}
            </div>}
            {anchorPreview?.preview&&(anchorPreview.error || anchorPreview.snapped)&&cursorPosition&&<div className="construction-tooltip drawing-tooltip" role="status" style={{left:Math.max(8,Math.min(mapSize.width-256,cursorPosition.x+18)),top:Math.max(8,Math.min(mapSize.height-80,cursorPosition.y+18))}}>{anchorPreview.error??"位置合わせ（接続は変更しません）"}</div>}
            {destinationCandidate&&destinationError&&cursorPosition&&<div className="construction-tooltip drawing-tooltip" role="status" style={{left:cursorPosition.x+18,top:cursorPosition.y+18}}>{destinationError}</div>}
            {endpointCandidate&&!endpointCandidateAllowed&&cursorPosition&&<div className="construction-tooltip drawing-tooltip" role="status" style={{left:Math.max(8,Math.min(mapSize.width-256,cursorPosition.x+18)),top:Math.max(8,Math.min(mapSize.height-100,cursorPosition.y+18))}}>
              <strong>他社の線路は延伸できません</strong>
            </div>}
            {activeTool==="tracks"&&mapMode==="draw"&&draftExtension&&draftPoints.length>0&&!referenceCopy.active&&<div className="extension-status" role="status">延伸元を選択済み：{alignments.find(a=>a.id===draftExtension.alignmentId)?.name||"既存線路"}の{draftExtension.endpoint==="start"?"始点":"終点"}（着工前）</div>}
            {showPlanHover&&!endpointCandidate&&planHover&&hoveredPlan&&<div className="construction-tooltip plan-hover-tooltip" role="status" style={{left:Math.max(8,Math.min(mapSize.width-240,planHover.x+18)),top:Math.max(8,Math.min(mapSize.height-80,planHover.y+18))}}><strong>{hoveredPlan.name.trim()||"線路計画"}（ホバー中）</strong>{hoveredGeometry?.error&&<div>敷設不可：{hoveredGeometry.error}</div>}<div>高さ：未設定 · {hoveredSpeed===null?"制限速度：未算定":`${hoveredSpeed} km/h（暫定）`}</div></div>}
            <svg ref={svgRef} tabIndex={0} onKeyDown={event=>{
              if(activeTool!=="stations"||mapMode!=="place"||!enabled||event.ctrlKey||event.metaKey||event.altKey)return;
              if(event.key==="Enter"){event.preventDefault();if(!event.repeat){const rect=event.currentTarget.getBoundingClientRect();placePlatformAt(stationCursor??{latitude:camera.latitude,longitude:camera.longitude},rect.width,rect.height);}return;}
              if(!["ArrowLeft","ArrowRight","ArrowUp","ArrowDown"].includes(event.key))return;
              event.preventDefault();
              const p=stationCursor??{latitude:camera.latitude,longitude:camera.longitude};
              const moved=panCamera({...camera,latitude:p.latitude,longitude:p.longitude},event.key==="ArrowLeft"?-5:event.key==="ArrowRight"?5:0,event.key==="ArrowUp"?-5:event.key==="ArrowDown"?5:0);
              setStationCursor({latitude:moved.latitude,longitude:moved.longitude});
              const rect=event.currentTarget.getBoundingClientRect();if(rect.width>0&&rect.height>0){const viewport=mapViewport(rect.width,rect.height),screenPoint=project(moved.latitude,moved.longitude,camera);setCursorPosition({x:(screenPoint.x-viewport.x)*viewport.scale,y:(screenPoint.y-viewport.y)*viewport.scale,width:rect.width,height:rect.height});}
            }} onPointerDown={pointerDown} onPointerMove={pointerMove} onPointerUp={(event) => pointerEnd(event)} onPointerCancel={(event) => pointerEnd(event, true)} onPointerLeave={() => { drawingCursor.current=null;if(anchorGesture.current)return;referenceCopy.clearHover();setCursorPosition(null);setDeleteHover(null);setPlanHover(null);setBuiltHover(null); setHoverPoint(null);setStationCursor(null); if (gestureRef.current && !gestureRef.current.dragged) cancelMapGesture(); }} onLostPointerCapture={(event) => { if (event.target === event.currentTarget) pointerEnd(event, true); }} role="img" aria-label={background === "sample" ? "駅配置用サンプル地図" : background === "osm" ? "駅配置用OpenStreetMap" : "駅配置用地理院地図"} viewBox={`0 0 ${MAP_VIEWBOX.width} ${MAP_VIEWBOX.height}`} preserveAspectRatio="xMidYMid slice" onClick={mapClick} className={dragging ? "map-dragging" : enabled && mapMode !== "move" ? "placement-enabled" : "map-move-mode"}>
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
                {!(mainTab==="construction"&&activeTool==="inactive")&&<title>{alignment.name}</title>}
                <path d={drawPath(alignment.points)} className="track-outline" fill="none" />
                <path d={drawPath(alignment.points)} className="alignment-sleepers" />
                <path d={drawPath(alignment.points)} className={alignmentId === alignment.id ? "alignment-path alignment-selected" : "alignment-path"} />
                {routeVisual.alignmentSections.get(alignment.id)?.map((points,index)=><path key={index} d={drawPath(points)} className="alignment-path route-assigned-section" />)}
                <path d={drawPath(alignment.points)} className="alignment-hit" onPointerEnter={event=>hoverBuilt(event,alignment.id)} onPointerMove={event=>hoverBuilt(event,alignment.id)} onPointerLeave={()=>setBuiltHover(null)} onClick={(event) => { if (mapMode === "move" && !suppressSelectionClick.current) { event.stopPropagation(); setAlignmentId(alignment.id); setSelectedPlanPath(null);setTrackDeleteConfirm(false); } }} />
              </g>)}
              {referenceCopy.active&&referenceCopy.roadFollow&&(referenceCopy.candidates.length?referenceCopy.candidates:referenceCopy.hoverCandidates).map(candidate=>{const a=graphNodesById.get(candidate.edge.from),b=graphNodesById.get(candidate.edge.to);return a&&b?<path key={candidate.edge.id} d={drawPath([a,b])} className="road-follow-candidate" stroke="#298746" strokeWidth="4" opacity=".7" fill="none" pointerEvents="none"/>:null;})}
              {referenceCopy.active&&referenceCopy.plan.build&&<path d={drawPath(referenceCopy.plan.build.points)} className="draft-path reference-copy-preview" style={referenceCopy.roadFollow?{stroke:"#298746"}:undefined} pointerEvents="none"/>}
              {parallelMode&&activeTool==="tracks"&&parallelSources.map(source=>{const p=source.points[0],a=project(p.latitude,p.longitude,camera),b=project(p.latitude+PARALLEL_RANGE_M/111195,p.longitude,camera);return <path key={source.id} d={drawPath(source.points)} fill="none" stroke="#61afcf" strokeOpacity=".28" strokeWidth={2*Math.abs(a.y-b.y)} strokeLinecap="round" pointerEvents="none"/>;})}
              {parallelMode&&parallelStart&&(()=>{const q=project(parallelStart.point.latitude,parallelStart.point.longitude,camera);return <circle cx={q.x} cy={q.y} r={5/viewport.scale} fill="white" stroke="#287c9e" strokeWidth={1.5/viewport.scale} pointerEvents="none"/>;})()}
              {parallelMode&&parallelPreview&&<path d={drawPath(parallelPreview.line)} className="draft-path candidate-draft" pointerEvents="none"/>}
              {parallelMode&&parallelCandidate&&!parallelStart&&(()=>{const line=offsetTrack(parallelCandidate.source.points,parallelCandidate.projection!.offset),p=line&&hoverPoint&&parallelProjection(line,hoverPoint)?.point;if(!p)return null;const q=project(p.latitude,p.longitude,camera);return <circle cx={q.x} cy={q.y} r={5/viewport.scale} fill="white" stroke="#287c9e" strokeWidth={1.5/viewport.scale} pointerEvents="none"/>;})()}
              {parkedPaths.map(plan=><g key={plan.id} className={plan.planId===activePlanId?undefined:"inactive-plan"}><path d={drawPath(plan.line)} className={"draft-path parked-plan"+(mapMode==="move"&&selectedPlanPath===plan.id?" track-explicit-selection":"")} data-plan-id={plan.id} pointerEvents="none" /><path d={drawPath(plan.line)} className="plan-hover-hit" data-plan-hover-id={plan.id} onClick={event=>{if(mapMode==="move"){event.stopPropagation();selectTrackPath(plan.id);}}} fill="none" stroke="transparent" strokeWidth="14" pointerEvents="stroke" onPointerEnter={event=>hoverPlan(event,plan.id)} onPointerMove={event=>hoverPlan(event,plan.id)} onPointerLeave={()=>setPlanHover(null)} />{drawAnchors(plan.points,true,plan.id)}</g>)}
              {tangentGuide&&<line className="construction-tangent-guide" aria-label="円弧・直線の先端延長ガイド" x1={tangentGuide.start.x} y1={tangentGuide.start.y} x2={tangentGuide.end.x} y2={tangentGuide.end.y} stroke="#65716b" strokeWidth="1" strokeDasharray="6 4" opacity=".8" vectorEffect="non-scaling-stroke" pointerEvents="none"/>}

              {!parallelMode && !referenceCopy.active && guideCenter && guideEdge && <circle className="curve-circle-guide" cx={guideCenter.x} cy={guideCenter.y} r={Math.hypot(guideEdge.x-guideCenter.x,guideEdge.y-guideCenter.y)} fill="none" stroke={previewError ? "#b34c4c" : "#65716b"} strokeWidth="1" opacity=".6" vectorEffect="non-scaling-stroke" pointerEvents="none" />}
              {!referenceCopy.active&&draftLine.length > 1 && <path d={drawPath(draftLine)} className={"draft-path confirmed-draft"+(mapMode==="move"&&selectedPlanPath===activePathId?" track-explicit-selection":"")} pointerEvents="none" />}
              {!parallelMode&&!referenceCopy.active&&candidateLine.length > 1 && <path d={drawPath(candidateLine)} className={"draft-path candidate-draft"+(previewError?" invalid-candidate":"")} pointerEvents="none" />}
              {!referenceCopy.active && draftLine.length > 1 && <path d={drawPath(draftLine)} className="plan-hover-hit" data-plan-hover-id={activePathId} onClick={event=>{if(mapMode==="move"){event.stopPropagation();selectTrackPath(activePathId);}}} fill="none" stroke="transparent" strokeWidth="14" pointerEvents="stroke" onPointerEnter={event=>hoverPlan(event,activePathId)} onPointerMove={event=>hoverPlan(event,activePathId)} onPointerLeave={()=>setPlanHover(null)} />}
              {enabled&&(activeTool==="tracks"||activeTool==="inactive")&&!referenceCopy.active&&!dragging&&!anchorPreview&&!endpointCandidate&&builtHover&&hoveredBuilt&&lineFocusMarker(hoveredBuilt.points,builtHover.point,"built-hover-point")}
              {destinationCandidate&&focusMarker(destinationCandidate.point,"anchor","destination-snap-ring",!destinationError)}
              {endpointCandidate&&focusMarker(endpointCandidate.point,"anchor","endpoint-snap-ring",endpointCandidateAllowed)}
              {showPlanHover&&!endpointCandidate&&planHover&&hoveredPlan&&(planHover.anchor===undefined?lineFocusMarker(hoveredPlan.line,planHover.point,"plan-hover-point"):focusMarker(planHover.point,"anchor","plan-hover-point"))}
              {dragSpeedLabels.map(label=>{const p=project(label.point.latitude,label.point.longitude,camera);return <g key={label.interval} className="drag-segment-speed" data-interval={label.interval} pointerEvents="none" transform={`translate(${p.x} ${p.y}) scale(${1/viewport.scale})`}><rect x="-34" y="-27" width="68" height="19" rx="3"/><text x="0" y="-14" textAnchor="middle">{label.speed} km/h</text></g>;})}
              {drawAnchors(draftPoints)}
              {anchorPreview?.preview&&<g pointerEvents="none"><path d={drawPath(anchorPreview.preview.line)} className={"draft-path anchor-drag-preview"+(anchorPreview.error?" invalid-candidate":"")} />{focusMarker(anchorPreview.preview.anchors[anchorPreview.index],"anchor",anchorPreview.snapped?"anchor-snap-ring snapped":"anchor-snap-ring",!anchorPreview.error)}</g>}
              {detailedStations&&<><defs><filter id="station-group-outline" x="-10%" y="-10%" width="120%" height="120%"><feMorphology in="SourceAlpha" operator="dilate" radius={1/viewport.scale} result="expanded"/><feFlood floodColor="#333"/><feComposite in2="expanded" operator="in"/><feMerge><feMergeNode/><feMergeNode in="SourceGraphic"/></feMerge></filter></defs>{stationGroupDisplay.filter(g=>g.members.length>1).map(group=><g key={group.id} className="station-group-footprint" data-station-group={group.id} pointerEvents="none"><title>{group.name}</title><g filter="url(#station-group-outline)">{group.members.map(member=>{const planned=plannedStations.find(s=>s.id===member.id),inactive=planned&&stationPlanId(planned)!==activePlanId;return <path key={member.id} data-group-platform={member.id} d={drawPath(member.polygon)+"Z"} fill={inactive?"#b1bac3":"white"} stroke="none"/>;})}</g></g>)}</>}
              {plannedPlatforms.map(({station,polygon,valid})=>{const p=project(station.point.latitude,station.point.longitude,camera);return <g key={station.id} className={"planned-station"+(stationPlanId(station)===activePlanId?"":" inactive-plan")} pointerEvents="none" opacity=".7"><title>{station.name}（未着工）{valid?"":"・線形の修正が必要"}</title>{detailedStations&&valid?<path d={drawPath(polygon)+"Z"} className="station-platform" visibility={mergedStationIds.has(station.id)?"hidden":undefined}/>:<circle visibility={stationDisplayGroup(station.id)&&stationDisplayGroup(station.id)?.id!==station.id?"hidden":undefined} cx={p.x} cy={p.y} r="5" fill={valid?"#777":"#b34c4c"}/>}{stationNamesVisible&&stationDisplayGroup(station.id)?.id===station.id&&<text x={p.x+10} y={p.y-8}>{stationDisplayGroup(station.id)?.name??station.name}（計画）</text>}</g>;})}
              {stationPreview.length >= 4 && <path d={drawPath(stationPreview) + "Z"} className="station-platform station-placement-preview" opacity=".65" pointerEvents="none" />}
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
                return <g key={station.id} pointerEvents={activeTool==="stations"&&mapMode==="place"?"none":undefined} className={"station-marker" + (selected === station.id ? " selected" : "") + (routeVisual.stations.has(station.id) ? "" : " route-unassigned")} onClick={(event) => { event.stopPropagation(); if (!suppressSelectionClick.current) selectStation(station.id); }}>
                  <title>{station.name}{placement ? ` / ホーム ${placement.length_m}m` : ""}</title>{!detailedStations ? <circle visibility={stationDisplayGroup(station.id)&&stationDisplayGroup(station.id)?.id!==station.id?"hidden":undefined} cx={x} cy={y} r="5" className="station-dot" /> : polygon ? <path d={drawPath(polygon) + "Z"} className="station-platform" style={mergedStationIds.has(station.id)?{fill:"transparent",stroke:"none"}:undefined} /> : <rect x={x - 13} y={y - 5} width="26" height="10" className="station-platform" />}{stationNamesVisible&&(!stationDisplayGroup(station.id)||stationDisplayGroup(station.id)?.id===station.id)&&<text x={x + 18} y={y + 5}>{station.name}</text>}
                </g>;
              })}
              {activeTool==="delete"&&enabled&&<g className="deletion-layer">
                {alignments.flatMap(line=>deletionSections(line).map(section=>{const target:DeleteTarget={kind:"built-section",alignmentId:line.id,sectionIndex:section.index};return <path key={line.id+":"+section.index} data-delete-alignment={line.id} data-delete-section={section.index} d={drawPath(section.points)} {...deleteHitProps(target)}/>;}))}
                {[...parkedPaths.map(path=>({id:path.id,points:path.points,line:path.line,whole:!!path.reference||path.roadFollowing})),{id:activePathId,points:draftPoints,line:draftLine,whole:roadFollowing}].map(path=><g key={path.id}>
                  {deletionSections({points:path.line,anchor_indices:path.whole?undefined:anchorIndices(path.points,path.line)}).map(section=><path key={section.index} data-delete-plan-path={path.id} data-delete-plan-section={section.index} d={drawPath(section.points)} {...deleteHitProps(path.whole||!anchorIndices(path.points,path.line)?{kind:"plan-path",pathId:path.id}:{kind:"plan-section",pathId:path.id,sectionIndex:section.index})}/>)}
                  {path.points.map((point,index)=>{const p=project(point.latitude,point.longitude,camera);return <rect key={index} data-delete-anchor={index} data-delete-path={path.id} x={p.x-8/viewport.scale} y={p.y-8/viewport.scale} width={16/viewport.scale} height={16/viewport.scale} {...deleteHitProps({kind:"plan-anchor",pathId:path.id,index},true)}/>;})}
                </g>)}
                {plannedPlatforms.map(({station,polygon})=>{const p=project(station.point.latitude,station.point.longitude,camera),target:DeleteTarget={kind:"plan-station",stationId:station.id};return polygon.length?<path key={station.id} data-delete-planned-station={station.id} d={drawPath(polygon)+"Z"} {...deleteHitProps(target,true)}/>:<circle key={station.id} data-delete-planned-station={station.id} cx={p.x} cy={p.y} r={8/viewport.scale} {...deleteHitProps(target,true)}/>;})}
                {world?.stations.map(station=>{const p=project(station.latitude,station.longitude,camera),polygon=stationPolygons.get(station.id),target:DeleteTarget={kind:"station",stationId:station.id};return detailedStations&&polygon?<path key={station.id} data-delete-station={station.id} d={drawPath(polygon)+"Z"} {...deleteHitProps(target,true)}/>:<circle key={station.id} data-delete-station={station.id} cx={p.x} cy={p.y} r={8/viewport.scale} {...deleteHitProps(target,true)}/>;})}
              </g>}
              </g>
            </svg>
            <div className="map-instruction">{status !== "ready" ? "ドラッグで地図を移動 / 建設はデスクトップ版" : mapMode === "draw" ? "クリックで線形点を追加 / 最後の点をもう一度クリックで計画終了" : mapMode === "place" ? "線路にカーソルを重ねてクリックで駅を配置" : "ドラッグで移動 · ホイールで拡大縮小"}</div>
          </div>
          <MapCredits background={background} reference={referenceVisible && referencePack ? referencePack.dataset : null} graphVisible={!!graphBundle && graphVisible}/>
        </section>
        <aside className={panelCollapsed ? "side-panel collapsed" : "side-panel"}>
          <WorkspaceTabs value={mainTab} onChange={chooseMainTab}/>
          <div className="inspector-title"><span>RAILWAY WORKSPACE</span><button className="secondary" aria-label={panelCollapsed ? "操作パネルを開く" : "操作パネルを畳む"} onClick={() => setPanelCollapsed(!panelCollapsed)}>{panelCollapsed ? "+" : "−"}</button></div>
          <nav className="editor-tabs construction-tabs" role="tablist" aria-label="建設ツール" hidden={mainTab!=="construction"}>
            {([["tracks","線路"],["stations","駅設置"],["delete","削除"]] as const).map(([tool,label],index)=><button key={tool} role="tab" id={"tab-"+tool} aria-controls={"panel-"+tool} aria-selected={activeTool===tool} tabIndex={activeTool===tool?0:-1} onClick={()=>{setError("");setActiveTool(tool);changeMapMode(tool==="tracks"?"draw":tool==="stations"?"place":"move");}} onKeyDown={event=>{if(!["ArrowLeft","ArrowRight","Home","End"].includes(event.key))return;event.preventDefault();const tools=["tracks","stations","delete"] as const;const next=tools[event.key==="Home"?0:event.key==="End"?2:(index+(event.key==="ArrowLeft"?2:1))%3];setActiveTool(next);changeMapMode(next==="tracks"?"draw":next==="stations"?"place":"move");document.getElementById("tab-"+next)?.focus();}}>{label}</button>)}
          </nav>
          <section id="workspace-panel-finance" role="tabpanel" aria-labelledby="workspace-tab-finance" hidden={mainTab!=="finance"}><h3>財務</h3><p>未実装です。収支・運賃・経費の画面は今後追加します。</p></section>
          <section id="workspace-panel-city" role="tabpanel" aria-labelledby="workspace-tab-city" hidden={mainTab!=="city"}><h3>都市</h3><p>未実装です。人口・需要・都市発展の画面は今後追加します。</p></section>
          <section id="workspace-panel-saves" role="tabpanel" aria-labelledby="workspace-tab-saves" hidden={mainTab!=="saves"}><h3>セーブデータ</h3><p>この試作は1つのローカル保存先を使います。セーブ一覧・複数スロットは未実装です。</p><button disabled={!enabled||saveBlocked} onClick={save}>ワールドを保存</button><button className="secondary" disabled={!enabled} onClick={()=>requestLoad("manual")}>ワールドを読み込む</button><h4>オートセーブ</h4><p>ワールドと未着工計画を専用ファイルに自動保存します。未適用の編集フォーム・未確定の参照候補は対象外。手動保存を上書きしません。最新1件のみ保持します。</p><p role="status">{autosave.message}</p>{autosave.permission==="blocked"&&<button className="secondary" disabled={!enabled} onClick={autosave.allowReplacement}>このワールドで自動保存を開始（以前の自動保存を置き換える）</button>}{autosave.permission==="failed"&&<button className="secondary" disabled={status!=="ready"||busy} onClick={autosave.retryStatus}>自動保存データを再確認</button>}<button className="secondary" disabled={!enabled||!autosave.available} onClick={()=>requestLoad("auto")}>自動保存から復元</button></section>
          <div id="workspace-panel-construction" className="construction-content" role="tabpanel" aria-labelledby="workspace-tab-construction" hidden={mainTab!=="construction"}>
          <section className="shared-plan" aria-label="建設計画">
            <div className="plan-heading"><label htmlFor="construction-plan-name">計画</label><input id="construction-plan-name" aria-label="計画名（任意）" maxLength={80} placeholder={`計画 ${activePlanId}`} disabled={!enabled||referenceCopy.active} value={planName} onChange={event=>setPlanName(event.target.value)}/><button type="button" className="secondary" aria-label="別の線路計画を追加" title="新しい計画" disabled={!enabled||referenceCopy.active||!(currentPaths.some(path=>path.points.length>0)||currentStations.length>0)} onClick={newIndependentPlan}>＋</button></div>
            {(plans.length>1||currentPaths.some(path=>path.points.length>0)||currentStations.length>0)&&<div className="plan-switcher" role="group" aria-label="計画を選択">{plans.map(plan=><button key={plan.id} className="secondary" aria-pressed={plan.id===activePlanId} disabled={!enabled||referenceCopy.active} onClick={()=>choosePlan(plan.id)}>{plan.name.trim()||`計画 ${plan.id}`}{plan.id!==activePlanId&&<span className="sr-only"> を編集</span>}</button>)}</div>}
            <div className="plan-totals"><span>{currentPaths.filter(path=>path.points.length>0).length}経路 · {new Set(currentStations.map(p=>stationDisplayGroup(p.station.id)?.id??p.station.id)).size}駅</span><span title="費用計算は未実装です">建設費 — / 維持費 —</span></div>
            <details className="plan-breakdown" open={currentStations.some(item=>!item.valid)||undefined}><summary>内訳</summary><p className="muted">建設費・維持費は未計算</p><div role="group" aria-label="計画内の経路" hidden>{currentPaths.map(path=><button key={path.id} className="secondary" disabled={!enabled||referenceCopy.active||path.id===activePathId||!!path.reference} aria-pressed={path.id===activePathId} onClick={()=>selectPath(path.id)}>経路 {path.id}{path.reference?"（実在形状）":""}</button>)}</div><ul aria-label="計画中の駅">{currentStations.map(({station,valid})=><li key={station.id}><input aria-label="計画駅名" maxLength={80} readOnly={stationDisplayGroup(station.id)?.members.some(m=>!m.planned)} value={stationDisplayGroup(station.id)?.name??station.name} disabled={!enabled} onChange={event=>renamePlannedStation(station.id,event.target.value)} onBlur={()=>{if(!station.name.trim())renamePlannedStation(station.id,"新駅");}} />{!valid&&<span role="status"> 線路に収まらないため着工不可</span>}<button type="button" className="secondary" aria-label={`計画駅 ${station.name} を削除`} disabled={!enabled} onClick={()=>{setPlannedStations(items=>items.filter(item=>item.id!==station.id));setStationUndo(items=>items.filter(item=>item.planned?.id!==station.id));setStationRedo(items=>items.filter(item=>item.planned?.id!==station.id));}}>×</button></li>)}</ul></details>
            {deletePlanConfirm&&<PlanDeleteDialog name={planName.trim()||`計画 ${activePlanId}`} onCancel={()=>setDeletePlanConfirm(false)} onDelete={deleteCurrentPlan}/>}
            <div className="plan-actions"><button className="secondary" disabled={!enabled||referenceCopy.active} onClick={()=>setDeletePlanConfirm(true)}>計画を削除</button><button className="secondary" disabled={!enabled||referenceCopy.active||!planHasContent} onClick={clearCurrentPlan}>下書きを消去</button><button disabled={!enabled||referenceCopy.active||!planBuildable} onClick={commitAlignment}>計画を着工</button></div>
          </section>
          <section role="tabpanel" id="panel-delete" aria-labelledby="tab-delete" hidden={activeTool!=="delete"}>{trackDeleteUndo&&<button className="secondary" disabled={!enabled||!canUndoTrackDelete} onClick={undoTrackDelete}>削除を元に戻す</button>}</section>
          <section className="station-editor" role="tabpanel" id="panel-stations" aria-labelledby="tab-stations" hidden={activeTool !== "stations"}>

            <div hidden><StationNamePreview pack={referencePack} repository={referenceRepository} point={stationPreview.length>=4?previewCenter:null} existingNames={[...(world?.stations.map(s=>s.name)??[]),...plannedStations.map(s=>s.name)]}/></div>



          </section>
          <section className="track-editor" role="tabpanel" id="panel-tracks" aria-labelledby="tab-tracks" hidden={activeTool !== "tracks" && activeTool !== "inactive"}>

            <details className="reference-tools" hidden={!referencePanel&&!referenceCopy.active} open><summary>実在線路・道路に沿う</summary><ReferenceCopyControls extensionError={referenceExtensionError} extensionControls={<label>敷設方法<select aria-label="敷設方法" value={referenceExtensionKey} onChange={event=>{setReferenceExtensionKey(event.target.value);referenceCopy.setConfirmed(false);}}><option value="">独立した新しい線路</option>{referenceExtensionCandidates.map(candidate=><option key={candidate.key} value={candidate.key}>既存線路 {candidate.alignment.id} の{candidate.endpoint==="start"?"始点":"終点"}から延伸</option>)}</select><small>起点から8px以内で、参照形状から正確に再現できる既存端点を選択できます。形状を平行移動せず参照区間の起点を補正します。既存の線路種別を引き継ぎます。複数候補から自動接続はしません。</small></label>} copy={{...referenceCopy,begin:()=>{if(!copyCanStart)return;newPath();referenceCopy.begin();changeMapMode("draw");setHoverPoint(null);}}} enabled={enabled} canStart={copyCanStart} hasBundle={!!graphBundle}/></details>

            <ul className="construction-list" hidden={mapMode==="draw"}>{alignments.map((a) => <li key={a.id}><button className="secondary" aria-pressed={a.id === alignmentId} onClick={() => { setAlignmentId(a.id);setSelectedPlanPath(null);setTrackDeleteConfirm(false);changeMapMode("move"); changeCamera(cameraAt(a.points[0].latitude,a.points[0].longitude,camera.zoom)); }}>{a.name&&a.name!=="新しい線路"?a.name:`線路 ${a.id}`}</button></li>)}</ul>
            {selectedAlignment&&world?.alignment_provenance?.some(p=>p.alignment_id===selectedAlignment.id)&&<AlignmentProvenance alignment={selectedAlignment} provenances={world?.alignment_provenance?.filter(p=>p.alignment_id===selectedAlignment.id)} sourceRecords={world?.source_records??[]}/>}

          </section>
          </div>
          <section hidden={activeTool!=="station-management"} className="station-history"><p className="field-hint">取消対象: {stationUndo.at(-1)?.station.name ?? "なし"} / Undo {stationUndo.length}件・Redo {stationRedo.length}件</p><div className="route-actions"><button type="button" className="secondary" disabled={!enabled || !stationUndo.length} onClick={undoStation}>駅設置を取り消す</button><button type="button" className="secondary" disabled={!enabled || !stationRedo.length} onClick={redoStation}>駅設置をやり直す</button></div></section>
          <section id="workspace-panel-stations" role="tabpanel" className="station-directory" aria-labelledby="workspace-tab-stations" hidden={activeTool !== "station-management"}>            <details className="legacy-tools"><summary>旧ワールド用：単独駅の座標配置</summary>
            <form onSubmit={submitStation}><fieldset disabled={!enabled}>
              <div className="coordinate-fields"><div><label htmlFor="station-latitude">緯度</label><input id="station-latitude" type="number" min={BOUNDS.south} max={BOUNDS.north} step="any" value={latitude} onChange={(event) => setLatitude(event.target.value)} /></div><div><label htmlFor="station-longitude">経度</label><input id="station-longitude" type="number" min={BOUNDS.west} max={BOUNDS.east} step="any" value={longitude} onChange={(event) => setLongitude(event.target.value)} /></div></div>
              <button className="full-width" type="submit">指定座標に配置</button>
            </fieldset></form></details><div className="directory-heading"><h3 id="stations-title">配置済みの駅</h3><span className="count">{directoryStations.length}</span></div>
            {directoryStations.length ? <ul>{directoryStations.map((station) => <li key={station.id}><button className={`station-entry ${selected === station.id ? "selected" : ""}`} onClick={() => selectStation(station.id)}><span>{station.name}</span><small>{station.latitude.toFixed(5)}, {station.longitude.toFixed(5)}</small></button></li>)}</ul> : <p className="empty-state">まだ駅はありません。<br />最初の駅を配置してみましょう。</p>}
          </section>
          <section hidden={activeTool !== "station-management"}>
            {selectedStation && <div className="equipment-editor"><h4>駅設定 · ID {selectedStation.id}</h4><p className="field-hint">設置後も駅名・ホーム長を変更できます。有人・無人・委託などの運営形態、維持費・満足度への反映は今後実装予定です。</p><div className="station-detail-name"><strong onDoubleClick={beginStationNameEdit}>{selectedStation.name}</strong><button ref={stationNameEditButton} type="button" className="secondary" disabled={!enabled} aria-label="選択駅の名前を編集" onClick={beginStationNameEdit}>✎ 編集</button></div>
              {world?.station_placements?.some((p) => p.station_id === selectedStation.id) && <form onSubmit={event=>event.preventDefault()}><fieldset disabled={!enabled&&!stationResizePending}><div className="coordinate-fields"><PlatformLengthInput id="edit-platform-length" label="選択駅の長さ" value={editLength} onChange={value=>{setStationResizeFailure(null);setEditLength(value);}}/></div><div className="route-actions">{stationResizePending&&<span role="status">反映中</span>}{!resizeValid&&<span role="status">2〜20両で入力してください</span>}{stationResizeFailure===resizeKey&&<button type="button" className="secondary" disabled={!enabled||!resizeValid} onClick={()=>{setStationResizeFailure(null);void resizePlatform();}}>長さ変更を再試行</button>}<button type="button" className="secondary" disabled={!platformDirty||stationResizePending} onClick={() => { setStationResizeFailure(null);if(selectedPlacement){setEditLength(String(selectedPlacement.length_m));setEditWidth(String(selectedPlacement.width_m));} }}>ホーム長の下書きを破棄</button></div></fieldset></form>}
              {world?.station_groups?.find(g=>g.members.includes(selected??""))&&<label>ホーム<select aria-label="編集するホーム" value={selected??""} onChange={event=>selectStation(event.target.value)}>{world.station_groups.find(g=>g.members.includes(selected??""))!.members.map((id,index)=><option key={id} value={id}>ホーム {index+1}</option>)}</select></label>}<form onSubmit={renameStation}><fieldset disabled={!enabled}>
              <label htmlFor="edit-station-name">選択駅の名前</label><input ref={stationNameInput} id="edit-station-name" onKeyDown={event=>{if(event.key!=="Escape" || event.nativeEvent.isComposing || event.nativeEvent.keyCode===229)return;event.preventDefault();event.stopPropagation();if(!enabled || busyRef.current)return;setEditStationName(selectedStation.name);stationNameEditButton.current?.focus();}} maxLength={80} value={editStationName} onChange={(event) => setEditStationName(event.target.value)} /><div className="route-actions"><button type="submit" disabled={!enabled || !stationDirty || !editStationName.trim()}>駅名を適用</button><button type="button" className="secondary" disabled={!enabled || !stationDirty} onClick={() => setEditStationName(selectedStation.name)}>駅名下書きを破棄</button></div></fieldset></form>
              <button className="secondary danger" disabled={!enabled || unapplied || !!stationReferences(selectedStation.id)} onClick={() => setPendingDelete({ kind: "station", id: selectedStation.id })}>{world?.station_groups?.some(g=>g.members.includes(selectedStation.id))?"選択ホームを撤去":"選択駅を削除"}</button>
              {stationReferences(selectedStation.id) && <p className="field-hint">削除不可: {stationReferences(selectedStation.id)}</p>}
            </div>}
            {pendingStation && <div className="inline-confirm" role="alert"><p>{platformDirty ? "駅名・ホーム長の下書きを破棄して別駅を選びますか。" : "駅名の下書きを破棄して別駅を選びますか。"}</p><button className="secondary" disabled={busy} onClick={() => setPendingStation(null)}>駅名編集を続ける</button><button disabled={busy} onClick={() => { const placement = world?.station_placements?.find(p=>p.station_id===pendingStation.id); setEditLength(String(placement?.length_m ?? 200));setEditWidth(String(placement?.width_m ?? 12));setMainTab("stations"); setActiveTool("station-management");setSelected(pendingStation.id); setEditStationName(stationsById.get(pendingStation.id)?.name ?? ""); setPendingStation(null); setPendingDelete(null); }}>{platformDirty ? "駅設定下書きを破棄して切替" : "駅名下書きを破棄して切替"}</button></div>}
          </section>
          <section className="route-editor" role="tabpanel" id="workspace-panel-routes" aria-labelledby="workspace-tab-routes" hidden={activeTool !== "routes"}>
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
            {selectedRoutePath && <section aria-label="保存済み路線の経路"><h4>{routeName}</h4><p>新方式の路線です。旧手動エディターでは上書きできません。既存の所属駅変更は未対応です。</p><p>所属駅: {selectedRoutePath.member_stations.map(id=>stationsById.get(id)?.name ?? id).join(" → ")}</p><ol>{selectedRoutePath.segments.map((segment,index)=><li key={index}>{segment.kind === "track" ? "駅間接続 " + segment.track_id : (segment.kind==="alignment"?"建設線形 " + (alignments.find(a=>a.id===segment.alignment_id)?.name ?? segment.alignment_id):"接続経路") + ": " + (stationsById.get(segment.from)?.name ?? segment.from) + " → " + (stationsById.get(segment.to)?.name ?? segment.to)}</li>)}</ol></section>}
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
            {selectedServicePath&&world&&<ServicePathCreator key={`replace-${serviceId}-${pathDraftEpoch}`} routes={world.routes} routePaths={world.route_paths??[]} stations={world.stations} world={world} enabled={enabled} canStart={!serviceDirty} onDirty={setPathServiceDirty} replaceService={{id:serviceId!,name:world.services.find(s=>s.id===serviceId)!.name,path:selectedServicePath}} onSave={async draft=>{if(!enabled||busyRef.current||serviceDirty)throw Error("別の系統編集を適用または破棄してください");busyRef.current=true;setBusy(true);try{const snapshot=await invoke<World>("replace_service_path",{serviceId,expectedPath:selectedServicePath,...draft});setWorld(snapshot);setDirty(true);setPathServiceDirty(false);setConfirmLoad(false);resetService(snapshot.services.find(s=>s.id===serviceId));setNotice("同じ系統IDで経路と停車設定を更新しました。ファイルへは保存してください。");}finally{busyRef.current=false;setBusy(false);}}}/>}
            {selectedServicePath&&world&&<ServicePathEditor key={serviceId!} service={{id:serviceId!,name:world.services.find(s=>s.id===serviceId)!.name,start:serviceStart}} path={selectedServicePath} world={world} stations={world.stations} enabled={enabled&&!pathServiceDirty} onDirty={setServiceDirty} onSave={async(name,stops)=>{if(!enabled||busyRef.current)throw Error("処理中です");busyRef.current=true;setBusy(true);try{const snapshot=await invoke<World>("update_service_path",{serviceId,name,stops});setWorld(snapshot);setDirty(true);setConfirmLoad(false);resetService(snapshot.services.find(s=>s.id===serviceId));setNotice("系統名と停車設定を適用しました。ファイルへは保存してください。");}finally{busyRef.current=false;setBusy(false);}}}/> }
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
            <h3>設定</h3><UserGuide /><h4>地図表示</h4><label><input type="checkbox" checked={stationNamesVisible} onChange={event=>setStationNamesVisible(event.target.checked)}/>駅名を表示</label><h4>会社名</h4><p className="muted">この試作では1プレイヤー・1会社です。会社の追加や子会社には未対応です。</p>
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
