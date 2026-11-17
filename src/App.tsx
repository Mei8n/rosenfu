import {roadTileSnapshots} from './automaticRoadData';
import {toolForTab,mapModeForTool,constructionMapMode,mapInstruction,type EditorTool,type MapMode,type Status} from './editorModes';
import {startConstructionTrace,timeConstructionIPC,type ConstructionTrace} from './constructionTiming';
import {ConstructionLayer} from './ConstructionLayer';
import {usePointerFrame} from './usePointerFrame';
import {constructionEditorReducer,constructionEditorState,type PathEdit} from './constructionEditor';
import {BuiltTrackLayer} from './BuiltTrackLayer';
import {StationLayer} from './StationLayer';
import {RouteLayer} from './RouteLayer';
import {EditHistory,planEditKey,worldEditKey,sameWorldEdit,FINAL_WORLD_COMMANDS,UNDO_WORLD_COMMANDS} from "./editHistory";
import { ServiceDelete } from "./ServiceDelete";
import {ServiceFromRoute} from './ServiceFromRoute';
import {ServiceMapCreator} from './ServiceMapCreator';
import {StationDirectory} from './StationDirectory';
import {TrackStroke} from "./TrackStroke";
import {constructionValidationError,hasConstructionValidation} from './constructionPlacementGuard';
import {useRoadDrawing} from './useRoadDrawing';
import {SlopeBoundaryMarkers} from './SlopeBoundaryMarkers';
import {useKeyboardPan} from "./useKeyboardPan";
import {ofmEndpointPick} from './ofmEndpointPick';
import {useWaterModelAcquisition} from './useWaterModelAcquisition';
import {waterAcquisitionBounds} from './waterAcquisitionBounds';
import {useOfmSeamCopy} from './useOfmSeamCopy';
import {buildTileRoadPath,type TilePathPick} from './ofmDerivedTilePath';
import type {DraftTileSnapshot} from './ofmReferenceDraft';
import {parseOfmPathDescriptor} from './ofmPlannedReference';
import {constructionRoadBounds,constructionRoadAttachment,type SavedRoadSnapshot} from './constructionRoads';
import {useRoadAssessments} from './RoadCrossings';
import {CrossingEstimate} from './CrossingEstimate';
import {useAutomaticRoad} from './useAutomaticRoad';
import {useAutomaticRail} from './useAutomaticRail';
import {AutomaticRailControls} from './AutomaticRailControls';
import {currentBuiltHeightSelection,equalWorldSnapshots,builtAnchorHeight,BuiltAnchorHeightHandles,BuiltAnchorHeightToolbar,type BuiltHeightSelection} from './BuiltAnchorHeight';
import {useConstructionPreview} from './useConstructionPreview';
import {anchorHeights,changeAnchorHeight} from './anchorHeights';
import {useTrackRenderGeometry} from './TrackRenderGeometry';
import {sharedServiceSections,serviceRenderChains} from './serviceMapAppearance';
import "./PlanBreakdown.css";
import {ServiceAppearanceControls,ServiceBadge,initialServiceSymbol, type ServiceSymbolShape} from './ServiceAppearanceControls';
import {JunctionPoints} from './JunctionPoints';
import {nearestJunctionSource,junctionDirection,junctionPrevious,type JunctionSource} from './junctionSelection';
import {constructionPlanFingerprint} from './constructionPlans';
import {DEFAULT_INFRASTRUCTURE,COUNT_LABELS,LEVEL_LABELS,endpointInfrastructure,schematicTrackPaths,type SectionInfrastructure} from './sectionInfrastructure';
import {mergeSectionSelections,selectAlignmentSections,selectionBounds,type SelectionBox,type SectionSelection} from "./boxSelection";
import "./boxSelection.css";
import {HeightControls} from './HeightControls';
import {SaveSlots,type SaveSlot} from "./SaveSlots";
import {sameRouteBinding,ServiceRouteBindingEditor,type ServiceRouteBindingRecord} from "./ServiceRouteBinding";
import {RouteDirectory} from "./RouteDirectory";
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
import {plannedTrackHeight} from "./plannedTrackHeight";
import {plannedTrackSpeed} from "./plannedTrackSpeed";
import {emptyConstructionPlans,recoverLegacyRedoPlans,compactEmptyPlans,removeConstructionPlan,nextConstructionIds,type ConstructionPlans,type PlannedPath,type PlannedStation,type Extension} from "./constructionPlans";
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
import {UnsavedCloseDialog} from "./UnsavedCloseDialog";
import {useUnsavedCloseGuard} from "./useUnsavedCloseGuard";
import { ErrorNotice } from "./ErrorNotice";
import { routeAppearance } from "./routeAppearance";
import {useReferenceCopy,sameReferenceSession,type ReferenceSessionSnapshot} from "./useReferenceCopy";
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
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, useReducer } from "react";
import type { MouseEvent, FormEvent, CSSProperties, PointerEvent as ReactPointerEvent } from "react";
import { invoke as nativeInvoke, isTauri } from "@tauri-apps/api/core";
import { MAP_BOUNDS as BOUNDS, MAP_VIEWBOX, GSI_TILES, project, coordinatesFromClient, DEFAULT_CAMERA, cameraAt, panCamera, visibleTiles, positionTile, viewBounds, mapViewport } from "./map";
import { GsiBackground } from "./GsiBackground";
import type { Camera } from "./map";
import type { TileStatus } from "./GsiBackground";
import { provisionalCurveSpeed, snapStraight } from "./circularAlignment";
import { nearestOnAlignment, stationPolygon, alignmentLength, distanceM, pointOnAlignment } from "./alignment";
type MapPointerMove=Pick<ReactPointerEvent<SVGSVGElement>,"currentTarget"|"clientX"|"clientY"|"pointerId"|"pointerType"|"ctrlKey"|"type">;
type GeoPoint = { latitude: number; longitude: number };
type Alignment = { infrastructure?:SectionInfrastructure[]; anchor_indices?:number[]|null; id: string; owner: string; name: string; points: GeoPoint[]; track_type: TrackType; design_speed_kmh?:number|null };
type StationPlacement = { station_id: string; alignment_id: string; position: number; length_m: number; width_m: number };

type EngineInfo = { name: string; version: string; company_setup_required?: boolean };
type Station = { id: string; owner: string; name: string; latitude: number; longitude: number };
type Route = { id: string; owner: string; name: string; start: string };
type OperatingService = { id: string; owner: string; name: string; start: string };
type RoutePath = { route_id: string; segments: Segment[]; member_stations: string[] };
export type World = {
  road_snapshots?:SavedRoadSnapshot[];
  road_evidence_descriptors?:readonly unknown[];
  service_route_bindings?:ServiceRouteBindingRecord[];
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
  routes: Route[];
  services: OperatingService[];
  service_appearances?: {service_id:string;color:string;shape?:ServiceSymbolShape;symbol?:string|null}[];
};
type DeleteTarget=PlannedDeleteTarget|{kind:"built-section";alignmentId:string;sectionIndex:number}|{kind:"station";stationId:string};
type StationHistoryEntry = { station: Station; placement?: StationPlacement; planned?:PlannedStation };

const previewWorld: World = {
  next_id: "8", players: [{ id: "1", name: "Preview" }],
  companies: [{ id: "2", owner: "1", name: "サンプル鉄道" }],
  stations: [
    { id: "3", owner: "2", name: "サンプル中央", latitude: 35.685, longitude: 139.735 },
    { id: "4", owner: "2", name: "サンプル東", latitude: 35.66, longitude: 139.79 },
  ],
  alignments: [{id:"5",owner:"2",name:"サンプル線形",track_type:"normal",points:[{latitude:35.685,longitude:139.735},{latitude:35.66,longitude:139.79}]}],
  station_placements: [{station_id:"3",alignment_id:"5",position:0,length_m:100,width_m:12},{station_id:"4",alignment_id:"5",position:1,length_m:100,width_m:12}],
  routes: [{id:"6",owner:"2",name:"サンプル本線",start:"3"}],
  route_paths: [{route_id:"6",segments:[{kind:"alignment",alignment_id:"5",from:"3",to:"4"}],member_stations:["3","4"]}],
  services: [{id:"7",owner:"2",name:"サンプル系統",start:"3"}],
  service_paths: [{service_id:"7",next_leg_key:"2",legs:[{key:"1",segment:{kind:"alignment",alignment_id:"5",from:"3",to:"4"}}],stops:[{kind:"start"},{kind:"on_leg",leg_key:"1",station_id:"4"}]}],
  service_route_bindings:[{service_id:"7",binding:{kind:"route",route_id:"6"}}],
};
const detail = (error: unknown) => error instanceof Error ? error.message : String(error);

export function App() {
  type EditSnapshot={world:World;plans:ConstructionPlans};
  const editJournal=useRef(new EditHistory<EditSnapshot>());
  const [historyAvailability,setHistoryAvailability]=useState({undo:false,redo:false});
  function refreshHistoryAvailability(){const next={undo:!!editJournal.current.peek(false),redo:!!editJournal.current.peek(true)};setHistoryAvailability(old=>old.undo===next.undo&&old.redo===next.redo?old:next);}
  const editBaseline=useRef<EditSnapshot|null>(null);
  const historyRestoring=useRef(false);
  const pendingWorldAction=useRef<{kind:"edit"|"final"|"load"|"stage";result:unknown}|null>(null);
  async function invoke<T>(command:string,args?:Record<string,unknown>):Promise<T>{
    const result=await (args===undefined?nativeInvoke<T>(command):nativeInvoke<T>(command,args));
    if(FINAL_WORLD_COMMANDS.has(command)||UNDO_WORLD_COMMANDS.has(command)||['load_save_slot','load_autosave','create_new_world','stage_ofm_construction_plan'].includes(command)){
      pendingWorldAction.current={kind:FINAL_WORLD_COMMANDS.has(command)?"final":UNDO_WORLD_COMMANDS.has(command)?"edit":command==='stage_ofm_construction_plan'?"stage":"load",result};
    }
    return result;
  }

  const constructionCommit=useRef<{world:World;trace:ConstructionTrace;end:()=>void}|null>(null);
  const svgRef = useRef<SVGSVGElement | null>(null);
  const cameraRef = useRef<Camera>(DEFAULT_CAMERA);
  const [pixelRatio, setPixelRatio] = useState(() => window.devicePixelRatio || 1);
  const [mapSize, setMapSize] = useState<{width:number;height:number}>(MAP_VIEWBOX);
  const gestureRef = useRef<{ pointerId: number; startX: number; startY: number; camera: Camera; scale: number; dragged: boolean; mayPlace: boolean } | null>(null);
  const boxGesture=useRef<{pointerId:number;startX:number;startY:number;box:SelectionBox;camera:Camera;additive:boolean}|null>(null);
  const [selectionBox,setSelectionBox]=useState<SelectionBox|null>(null);
  const [rangeSelection,setRangeSelection]=useState<SectionSelection[]>([]);
  const [confirmRangeDelete,setConfirmRangeDelete]=useState(false);
  const [rangeTrackCount,setRangeTrackCount]=useState<1|2|4|null>(null);
  const [rangeTrackType,setRangeTrackType]=useState<"normal"|"high_speed"|null>(null);
  const [rangeLevel,setRangeLevel]=useState<SectionInfrastructure["level"]|null>(null);
  const suppressMapClick = useRef(false);
  const suppressSelectionClick = useRef(false);
  const [mapMode, setMapMode] = useState<MapMode>("move");
  const [dragging, setDragging] = useState(false);
  const [camera, setCamera] = useState<Camera>(DEFAULT_CAMERA);
  const [requestedCamera, setRequestedCamera] = useState<Camera>(DEFAULT_CAMERA);
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
  const bounds = useMemo(()=>viewBounds(camera,mapSize),[camera,mapSize]);
  const [manualGraphBundle,setGraphBundle]=useState<ReferenceGraphBundle|null>(null);
  const [graphVisible,setGraphVisible]=useState(true);
  const [pendingGraphChange,setPendingGraphChange]=useState<{bundle:ReferenceGraphBundle|null}|null>(null);
  const [candidateDirty,setCandidateDirty]=useState(false);
  const [routeRenameDirty,setRouteRenameDirty]=useState(false);
  const [referencePack,setReferencePack]=useState<ReferencePack|null>(null);
  const [referenceVisible,setReferenceVisible]=useState(true);
  const referenceRepository=useMemo(()=>referencePack?createReferenceRepository(referencePack):null,[referencePack]);
  const referenceResult=useMemo(()=>referenceRepository?.query(bounds) ?? {stations:[],total:0},[referenceRepository,bounds]);
  const cameraPending = camera !== requestedCamera;
  // Anchor the fictional offline scene to the same camera as railway geometry.
  const sampleScale = 2 ** (camera.zoom - DEFAULT_CAMERA.zoom);
  const sampleOrigin = project(DEFAULT_CAMERA.latitude, DEFAULT_CAMERA.longitude, camera);
  const sampleTransform = "translate(" + (sampleOrigin.x - MAP_VIEWBOX.width / 2 * sampleScale) + " " + (sampleOrigin.y - MAP_VIEWBOX.height / 2 * sampleScale) + ") scale(" + sampleScale + ")";
  useEffect(() => {
    if (!cameraPending) return;
    // Throttle tile requests during movement instead of postponing them until
    // the user stops dragging. Read the newest camera when the window ends.
    const timer = window.setTimeout(() => setRequestedCamera(cameraRef.current), 250);
    return () => window.clearTimeout(timer);
  }, [cameraPending, requestedCamera]);
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
    if(boxGesture.current)cancelMapGesture();
    cameraRef.current = next; setCamera(next);
  }
  function changeCamera(next: Camera) { smoothZoom.cancel(); renderCamera(next); }
  function zoomFromControls(delta: number) { if(boxGesture.current)cancelMapGesture();smoothZoom.zoomBy(delta, { x: MAP_VIEWBOX.width / 2, y: MAP_VIEWBOX.height / 2 }); }
  useEffect(()=>{smoothZoom.cancel();},[background]);

  const [tileStatus, setTileStatus] = useState<TileStatus>({ loaded: 0, failed: 0, total: GSI_TILES.length });
  const [status, setStatus] = useState<Status>("loading");
  const [info, setInfo] = useState<EngineInfo | null>(null);
  const [world, setWorld] = useState<World | null>(null);
  useLayoutEffect(()=>{
    const pending=constructionCommit.current;
    if(!pending||pending.world!==world)return;
    constructionCommit.current=null;pending.end();
    if(typeof requestAnimationFrame==='function'){
      const end=pending.trace.span('world.layout-to-frame');
      requestAnimationFrame(()=>{end();pending.trace.complete();});
    }else pending.trace.complete();
  },[world]);
  const [attempt, setAttempt] = useState(0);
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const [error, setErrorMessage] = useState("");
  const [errorRevision,setErrorRevision]=useState(0);
  function setError(message:string){setErrorMessage(message);setErrorRevision(value=>value+1);}
  const [notice, setNotice] = useState("");
  const [planRecoveryNotice,setPlanRecoveryNotice]=useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [editStationName, setEditStationName] = useState("");
  const stationNameInput=useRef<HTMLInputElement>(null);
  const stationNameEditButton=useRef<HTMLButtonElement>(null);
  const [pendingStation, setPendingStation] = useState<{ id: string } | null>(null);
  const [pendingDelete, setPendingDelete] = useState<{ kind: "station"; id: string } | null>(null);
  const [dirty, setDirty] = useState(false);
  const [confirmLoad, setConfirmLoad] = useState(false);
  const [saveSlots,setSaveSlots]=useState<SaveSlot[]>([]);
  const [deletedSaveSlots,setDeletedSaveSlots]=useState<SaveSlot[]>([]);
  const [activeSlotId,setActiveSlotId]=useState<string|null>(null);
  const [slotsReady,setSlotsReady]=useState(false);
  const [slotsLoading,setSlotsLoading]=useState(false);
  const [slotsError,setSlotsError]=useState("");
  const slotGeneration=useRef(0);
  const [slotAction,setSlotAction]=useState<{kind:"load";id:string}|{kind:"new";name:string}|null>(null);
  const [slotDialog,setSlotDialog]=useState<"save-as"|"new"|null>(null);
  const [slotName,setSlotName]=useState("");
  const [slotPicker,setSlotPicker]=useState(false);
  const [loadSource,setLoadSource]=useState<"manual"|"auto">("manual");
  const [worldEpoch,setWorldEpoch]=useState(0);
  const [railLoadEpoch,setRailLoadEpoch]=useState(0);
  const [restoredAutosaveEpoch,setRestoredAutosaveEpoch]=useState(-1);
  const [mainTab,setMainTab]=useState<WorkspaceTab>("construction");
  const [routeMapEdit,setRouteMapEdit]=useState<{id:string;name:string}|null>(null);
  const [routeSequenceKey,setRouteSequenceKey]=useState(0);
  const [routeCreating,setRouteCreating]=useState(false);
  const [routeServiceDirty,setRouteServiceDirty]=useState(false);
  const [serviceCreating,setServiceCreating]=useState(false);
  const [serviceSequenceKey,setServiceSequenceKey]=useState(0);
  const [routeMapPick,setRouteMapPick]=useState<"start"|"end"|"via"|"sequence"|null>(null);
  const routeMapSelect=useRef<((id:string)=>void)|null>(null);
  const cancelRouteMapPick=useCallback(()=>{routeMapSelect.current=null;setRouteMapPick(null);},[]);
  useEffect(()=>{cancelRouteMapPick();},[mainTab,worldEpoch]);
  useEffect(()=>{setRouteMapEdit(null);setRouteSequenceKey(0);setRouteCreating(false);},[worldEpoch]);
  const [companySetupRequired,setCompanySetupRequired]=useState(false);
  const [activeTool, setActiveTool] = useState<EditorTool>("tracks");
  function chooseMainTab(tab:WorkspaceTab){if(seamMutationPending.current)return;setError("");setMainTab(tab);setPanelCollapsed(false);const tool=toolForTab(tab);setActiveTool(tool);changeMapMode(mapModeForTool(tool));}
  const [panelCollapsed, setPanelCollapsed] = useState(false);
  // Internal compatibility label only; physical tracks are not user-named routes.
  const alignmentName = "新しい線路";
  const pointerFrame=usePointerFrame();
  useEffect(()=>pointerFrame.cancel(),[camera,world,activeTool,mapMode,pointerFrame.cancel]);
  const [constructionEditor,dispatchConstruction]=useReducer(constructionEditorReducer,undefined,()=>constructionEditorState());
  const {draftFinished,draftPoints,draftEditedGeometry,draftExtension,draftDestination,draftRedoDestination,parkedPaths,activePathId,plans,activePlanId,redoPoints,draftJunctionSource,draftAnchorLevels,redoAnchorLevels,startLevel}=constructionEditor;
  type AnchorEdit={kind?:"point";id:number;before:PathEdit;after:PathEdit};
  const [anchorUndo,setAnchorUndo]=useState<AnchorEdit[]>([]),[anchorRedo,setAnchorRedo]=useState<AnchorEdit[]>([]);
  // Keep session-only edit history with its path while another plan is selected.
  const parkedAnchorHistory=useRef(new Map<number,{undo:AnchorEdit[];redo:AnchorEdit[]}>());
  // Remaining plans survive construction, but their pre-construction edits do not.
  const constructionUndoFloor=useRef(new Map<number,number>());
  function parkAnchorHistory(){
    parkedAnchorHistory.current.set(activePathId,{undo:anchorUndo.filter(edit=>edit.id===activePathId),redo:anchorRedo.filter(edit=>edit.id===activePathId)});
  }
  type AnchorGesture={pointerId:number;id:number;index:number;startX:number;startY:number;points:GeoPoint[];line:GeoPoint[];geometry?:EditedGeometry;redo:GeoPoint[];destination?:Extension;redoDestination?:Extension;previous?:GeoPoint;preview:EditedGeometry|null;error:string|null;snapped:boolean};
  const anchorGesture=useRef<AnchorGesture|null>(null);
  const [anchorPreview,setAnchorPreview]=useState<AnchorGesture|null>(null);
  useEffect(()=>{cancelAnchorGesture();},[camera,activeTool,mapSize]);
  const nextPlanId=useRef(2);
  const planName=plans.find(plan=>plan.id===activePlanId)?.name??"";
  function setPlanName(name:string){dispatchConstruction({type:"RENAME_PLAN",name});}
  const [builtHeightSelection,setBuiltHeightSelection]=useState<BuiltHeightSelection|null>(null);
  const [heightAnchor,setHeightAnchor]=useState<{id:number;index:number}|null>(null);
  const [infrastructure,setInfrastructure]=useState<SectionInfrastructure>(DEFAULT_INFRASTRUCTURE);
  const [trackType,setTrackType]=useState<TrackType>("normal");
  const [designSpeedKmh,setDesignSpeedKmh]=useState(120);
  const [roadFollowing,setRoadFollowing]=useState(false);
  const [legacyRoadDraft,setLegacyRoadDraft]=useState(false);
  const [referencePanel,setReferencePanel]=useState(false);
  const [parallelMode,setParallelMode]=useState(false);
  const [parallelStart,setParallelStart]=useState<{sourceId:string;offset:number;point:GeoPoint}|null>(null);
  function cancelParallel(){setParallelStart(null);}
  useEffect(()=>{cancelParallel();},[activeTool,activePlanId,world]);
  const roadFollowingBlocked=false;
  const [seamStageSending,setSeamStageSending]=useState(false);
  const seamMutationPending=useRef(false);
  const [frozenRoadTiles,setFrozenRoadTiles]=useState<readonly DraftTileSnapshot[]>([]);
  const [frozenRoadBundle,setFrozenRoadBundle]=useState<ReferenceGraphBundle|null>(null);
  const automaticRoad=useAutomaticRoad(bounds,status==="ready"&&roadFollowing,worldEpoch,!!frozenRoadBundle);
  const graphBundle=roadFollowing?(frozenRoadBundle??automaticRoad.snapshot?.bundle??manualGraphBundle):manualGraphBundle;
  const graphIndex=useMemo(()=>graphBundle?indexGraphDisplay(graphBundle.graph):[],[graphBundle]);
  const graphDisplay=useMemo(()=>queryGraphDisplay(graphIndex,viewBounds(camera,mapSize),camera.zoom,graphVisible),[graphIndex,camera,mapSize,graphVisible]);

  function chooseTrackType(value:TrackType){cancelParallel();setParallelMode(false);setTrackType(value);setDesignSpeedKmh(defaultDesignSpeed(value));setRoadFollowing(value==="tram");setReferencePanel(value==="tram");setHoverPoint(null);}
  const nextPathId = useRef(2);
  const [plannedStations,setPlannedStations]=useState<PlannedStation[]>([]);
  const nextPlannedStationId=useRef(1);
  const [stationCursor,setStationCursor]=useState<GeoPoint|null>(null);
  const stationInputCursor=useRef<GeoPoint|null>(null);
  useEffect(()=>{stationInputCursor.current=null;},[camera,activeTool,mapMode]);
  const [stationUndo,setStationUndo]=useState<StationHistoryEntry[]>([]);
  const [stationRedo,setStationRedo]=useState<StationHistoryEntry[]>([]);
  function clearStationHistory(){setStationUndo([]);setStationRedo([]);}
  function recordStation(snapshot:World){const station=snapshot.stations.find(item=>!world?.stations.some(old=>old.id===item.id));if(station){setStationUndo(history=>[...history,{station,placement:snapshot.station_placements?.find(p=>p.station_id===station.id)}].slice(-50));setStationRedo([]);}}
  const [hoverPoint, setHoverPoint] = useState<GeoPoint | null>(null);
  const {theme,lineWidth,contrast,panelWidth,textSize}=displaySettings.settings;
  const setTheme=(value:"light"|"dark")=>displaySettings.update({theme:value});
  const setLineWidth=(value:number)=>displaySettings.update({lineWidth:value});
  const setContrast=(value:number)=>displaySettings.update({contrast:value});
  const setPanelWidth=(value:number)=>displaySettings.update({panelWidth:value});
  const setTextSize=(value:number)=>displaySettings.update({textSize:value});
  const pendingPathHistory=useRef<{id:number;redo:boolean}|null>(null);
  function adjacentHistoryPath(redo:boolean){
    return [...parkedPaths].reverse().find(path=>path.planId===activePlanId&&!path.reference&&!path.ofmPath&&(
      redo ? path.redo.length>0 || !!parkedAnchorHistory.current.get(path.id)?.redo.length
        : path.points.length>(constructionUndoFloor.current.get(path.id)??0) || !!parkedAnchorHistory.current.get(path.id)?.undo.length
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
  function undoPoint() { stepEditHistory(false); }
  function redoPoint() { stepEditHistory(true); }
  const [alignmentId, setAlignmentId] = useState("");
  const [selectedPlanPath,setSelectedPlanPath]=useState<number|null>(null);
  const [trackDeleteConfirm,setTrackDeleteConfirm]=useState(false);
  const [deleteTarget,setDeleteTarget]=useState<DeleteTarget|null>(null);
  const deletionTooltipRef=useRef<HTMLDivElement>(null);
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
  const alignments = useMemo(()=>world?.alignments ?? [],[world]);
  const alignmentConnections=useMemo(()=>world?.alignment_connections??[],[world]);
  const trackRender=useTrackRenderGeometry(world,status==="ready"&&(!!world?.alignment_connections?.length||alignments.some(line=>line.infrastructure?.some((section,index)=>index>0&&section.track_count!==line.infrastructure![index-1].track_count))));
  const detailedStations = camera.zoom >= 13;
  const [stationResizePending,setStationResizePending]=useState(false);
  const [stationResizeFailure,setStationResizeFailure]=useState<string|null>(null);
  const stationPolygons=useMemo(()=>new Map((world?.station_placements??[]).map(placement=>{
    const alignment=world?.alignments?.find(a=>a.id===placement.alignment_id);
    return [placement.station_id,alignment?stationPolygon(alignment.points,placement.position,placement.length_m,placement.width_m):null];
  })),[world]);
  const routeVisual = useMemo(() => routeAppearance(world), [world]);
  const serviceMapSections=useMemo(()=>serviceRenderChains(sharedServiceSections(world),world?.alignment_connections??[]),[world]);
  const selectedAlignment = alignments.find((a) => a.id === alignmentId);
  const extensionPrevious=useCallback((extension:Extension|null)=>{
    const alignment=alignments.find(a=>a.id===extension?.alignmentId);
    return alignment&&extension?(extension.endpoint==="start"?alignment.points[1]:alignment.points.at(-2)):undefined;
  },[alignments]);
  const directedJunction=useMemo(()=>draftJunctionSource&&draftPoints[0]?(draftPoints[1]||hoverPoint?junctionDirection(draftJunctionSource,draftPoints[0],draftPoints[1]??hoverPoint!):draftJunctionSource):undefined,[draftJunctionSource,draftPoints,hoverPoint]);
  const previousPoint=useMemo(()=>directedJunction&&draftPoints[0]?junctionPrevious(directedJunction,draftPoints[0]):extensionPrevious(draftExtension),[directedJunction,draftPoints,extensionPrevious,draftExtension]);
  const [cursorPosition,setCursorPosition]=useState<{x:number;y:number;width:number;height:number}|null>(null);
  const draftGeometry = useMemo(() => plannedGeometry(draftPoints,draftEditedGeometry,previousPoint), [draftPoints,draftEditedGeometry,previousPoint]);
  function fittedDestination(anchors:GeoPoint[],edited:typeof draftEditedGeometry,target:ReturnType<typeof destinationTarget>) {
    const inside=target?.endpoint==="start"?target.alignment.points[1]:target?.alignment.points.at(-2);
    return inside?twoEndedJoin(anchors,edited,previousPoint,inside)??edited:edited;
  }
  const previewAnchors=useMemo(()=>hoverPoint && !draftFinished && draftPoints.length && mapMode === "draw" ? [...draftPoints, hoverPoint] : draftPoints,[hoverPoint,draftFinished,draftPoints,mapMode]);
  const previewGeometry = useMemo(()=>previewAnchors===draftPoints?draftGeometry:plannedGeometry(previewAnchors,fittedDestination(previewAnchors,draftEditedGeometry,hoverPoint&&cursorPosition?destinationTarget(hoverPoint,cursorPosition.width,cursorPosition.height):null),previousPoint),[previewAnchors,draftPoints,draftGeometry,draftEditedGeometry,hoverPoint,cursorPosition?.width,cursorPosition?.height,previousPoint,draftFinished,alignments,draftExtension,camera]);
  const draftLine = draftGeometry.points;
  const previewLine = previewGeometry.points;
  const draftInvalid = Boolean(draftGeometry.error);
  const previewError = previewGeometry.error;
  // Invalid geometry is diagnostic only. Never commit this fallback chord.
  const candidateLine = useMemo(()=>hoverPoint && !draftFinished && draftPoints.length ? (previewLine.length > draftLine.length ? previewLine.slice(Math.max(0,draftLine.length-1)) : [draftPoints[draftPoints.length-1],hoverPoint]) : [],[hoverPoint,draftFinished,draftPoints,previewLine,draftLine]);
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
    return draftPoints.length || redoPoints.length ? {id:activePathId,planId:activePlanId,name:planName,points:draftPoints,redo:redoPoints,line:draftLine,editedGeometry:draftEditedGeometry,designSpeedKmh,trackType,infrastructure,startLevel,anchorLevels:draftAnchorLevels,redoAnchorLevels,junctionSource:directedJunction,roadFollowing:legacyRoadDraft,extension:draftExtension,destination:draftDestination,redoDestination:draftRedoDestination} : null;
  }
  function newPath(discardEmptyRedo=false) {
    setLegacyRoadDraft(false);
    // A new drawing replaces a fully undone path; switching plans still preserves its Redo.
    const discard=discardEmptyRedo&&!draftPoints.length;
    const current=discard?null:parkCurrentPlan();
    if(discard)discardPlanStations(activePathId);
    if(current)parkAnchorHistory();
    dispatchConstruction({type:"NEW_PATH",id:nextPathId.current++,current});
    setHeightAnchor(null);setAnchorUndo([]);setAnchorRedo([]);setHoverPoint(null);changeMapMode(constructionMapMode(activeTool));
  }
  function newIndependentPlan() {
    if (!enabled || referenceCopy.active) return;
    newPath();const id=nextPlanId.current++;dispatchConstruction({type:"NEW_PLAN",id});
  }
  // Reverse the sampled curve, not just its control points: recomputing the
  // curve from the other end would change geometry and station positions.
  function reversePlanForExtension(path:PlannedPath){
    const points=[...path.points].reverse();
    const geometry:EditedGeometry={version:1,anchors:points,line:[...path.line].reverse()};
    const edit:AnchorEdit={id:path.id,
      before:{anchorLevels:path.anchorLevels,redoAnchorLevels:path.redoAnchorLevels,startLevel:path.startLevel??path.infrastructure?.level??"ground",infrastructure:path.infrastructure??DEFAULT_INFRASTRUCTURE,points:path.points,geometry:path.editedGeometry,redo:path.redo,extension:path.extension,destination:path.destination,redoDestination:path.redoDestination},
      after:{anchorLevels:path.anchorLevels?[...path.anchorLevels].reverse():undefined,redoAnchorLevels:[],points,geometry,redo:[],startLevel:path.infrastructure?.level??"ground",infrastructure:{...(path.infrastructure??DEFAULT_INFRASTRUCTURE),level:path.startLevel??path.infrastructure?.level??"ground"},extension:path.destination??null}};
    return {points,geometry,edit};
  }
  function selectPath(id:number,resume=true,endpoint:"start"|"end"="end") {
    if(!enabled || referenceCopy.active)return;
    const found=parkedPaths.find(path=>path.id===id);if(!found||found.reference||found.ofmPath)return;const target={...found};
    const current=parkCurrentPlan();
    if(current)parkAnchorHistory();
    const history=parkedAnchorHistory.current.get(id);parkedAnchorHistory.current.delete(id);

    if(resume&&endpoint==="start"&&target.points.length>1&&!target.extension){
      const reversed=reversePlanForExtension(target);
      setAnchorUndo([...(history?.undo??[]),reversed.edit]);setAnchorRedo([]);
      target.anchorLevels=reversed.edit.after.anchorLevels;target.redoAnchorLevels=[];target.startLevel=reversed.edit.after.startLevel;target.infrastructure=reversed.edit.after.infrastructure;target.points=reversed.points;target.editedGeometry=reversed.geometry;target.redo=[];target.extension=target.destination??null;target.destination=undefined;target.redoDestination=undefined;
    }else {setAnchorUndo(history?.undo??[]);setAnchorRedo(history?.redo??[]);}
    dispatchConstruction({type:"SELECT_PATH",target,current,resume});setHeightAnchor(null);setInfrastructure(target.infrastructure??DEFAULT_INFRASTRUCTURE);setTrackType(target.trackType);setDesignSpeedKmh(target.designSpeedKmh);setRoadFollowing(target.roadFollowing);setLegacyRoadDraft(target.roadFollowing);setHoverPoint(null);changeMapMode(constructionMapMode(activeTool));
  }
  function choosePlan(id:number){
    if(id===activePlanId)return;
    const path=parkedPaths.find(path=>path.planId===id&&!path.reference&&!path.ofmPath);
    if(path)selectPath(path.id,false);else{newPath();dispatchConstruction({type:"SELECT_EMPTY_PLAN",id});}
  }
  function stationPlanId(station:PlannedStation){return station.planId??(station.pathId===activePathId?activePlanId:parkedPaths.find(path=>path.id===station.pathId)?.planId);}
  const [deletePlanConfirm,setDeletePlanConfirm]=useState(false);
  function deleteCurrentPlan(){
    const saved=savedPlanFingerprint,nextPlans=removeConstructionPlan(constructionPlans,activePlanId);restoreConstructionPlans(nextPlans);setSavedPlanFingerprint(saved);
    setStationUndo([]);setStationRedo([]);setHoverPoint(null);setDeletePlanConfirm(false);
  }
  function clearCurrentPlan(){
    setLegacyRoadDraft(false);
    const ids=new Set([activePathId,...parkedPaths.filter(path=>path.planId===activePlanId).map(path=>path.id)]);
    ids.forEach(id=>parkedAnchorHistory.current.delete(id));
    ids.forEach(discardPlanStations);setPlannedStations(items=>items.filter(station=>stationPlanId(station)!==activePlanId));setStationUndo(items=>items.filter(entry=>!entry.planned||stationPlanId(entry.planned)!==activePlanId));setStationRedo(items=>items.filter(entry=>!entry.planned||stationPlanId(entry.planned)!==activePlanId));dispatchConstruction({type:"CLEAR_PLAN"});
    setHeightAnchor(null);setAnchorUndo([]);setAnchorRedo([]);setHoverPoint(null);
  }
  const [planHover, setPlanHover] = useState<{id:number;x:number;y:number;point:GeoPoint;anchor?:number} | null>(null);
  const [builtHover,setBuiltHover]=useState<{id:string;point:GeoPoint}|null>(null);
  useEffect(() => { setPlanHover(null);setBuiltHover(null); }, [draftPoints, parkedPaths, activeTool, camera, world]);
  const hoverBuilt=useCallback((event:ReactPointerEvent<SVGPathElement>,id:string)=>{
    if(gestureRef.current||anchorGesture.current||event.pointerType==="touch")return;
    const rect=svgRef.current?.getBoundingClientRect();
    if(rect){const point=coordinatesFromClient(event.clientX,event.clientY,rect,camera);if(point){pointerFrame.schedule("hover",()=>{setPlanHover(null);setBuiltHover({id,point});});}}
  },[camera,pointerFrame.schedule]);
  const hoveredBuilt=builtHover?alignments.find(line=>line.id===builtHover.id):null;
  const dragDesignSpeed=anchorPreview?.id===activePathId?designSpeedKmh:parkedPaths.find(path=>path.id===anchorPreview?.id)?.designSpeedKmh;
  const dragSpeedLabels=anchorPreview?.preview&&!anchorPreview.error&&dragDesignSpeed!==undefined?anchorDragFeedback(anchorPreview.preview,anchorPreview.index,dragDesignSpeed):[];
  const hoveredPlan = planHover?.id === activePathId ? {name:planName,line:draftLine,points:draftPoints,anchorLevels:draftAnchorLevels,startLevel,infrastructure,editedGeometry:draftEditedGeometry,reference:undefined,designSpeedKmh,extension:draftExtension,destination:draftDestination,redoDestination:draftRedoDestination} : parkedPaths.find(plan=>plan.id===planHover?.id);
  const hoveredExtension=hoveredPlan?.extension?alignments.find(a=>a.id===hoveredPlan.extension!.alignmentId):null;
  const hoveredPrevious=hoveredExtension?(hoveredPlan!.extension!.endpoint==="end"?hoveredExtension.points.at(-2):hoveredExtension.points[1]):undefined;
  const hoveredGeometry = hoveredPlan&&!hoveredPlan.reference ? plannedGeometry(hoveredPlan.points,hoveredPlan.editedGeometry,hoveredPrevious) : null;
  const hoveredSpeed=hoveredGeometry&&planHover?plannedTrackSpeed(hoveredGeometry,planHover.point,hoveredPlan!.designSpeedKmh):null;

  function hoverPlan(event: ReactPointerEvent<SVGElement>, id:number, anchor?:number) {
    if (gestureRef.current || anchorGesture.current || event.pointerType === "touch") return;
    setBuiltHover(null);
    const rect=svgRef.current?.getBoundingClientRect();
    if(rect){const points=id===activePathId?draftPoints:parkedPaths.find(path=>path.id===id)?.points;const point=anchor===undefined?coordinatesFromClient(event.clientX,event.clientY,rect,camera):points?.[anchor];if(point){const value={id,x:event.clientX-rect.left,y:event.clientY-rect.top,point,anchor};pointerFrame.schedule("hover",()=>setPlanHover(value));}}
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
  const drawInfrastructurePath=(points:GeoPoint[],value:SectionInfrastructure=DEFAULT_INFRASTRUCTURE)=>schematicTrackPaths(points.map(point=>project(point.latitude,point.longitude,camera)),value.track_count).join(" ");
  function changeHeight(level:SectionInfrastructure["level"]){
    if(!heightAnchor){setInfrastructure(value=>({...value,level}));return;}
    const path=heightAnchor.id===activePathId?parkCurrentPlan():parkedPaths.find(p=>p.id===heightAnchor.id);
    if(!path||heightAnchor.index>=path.points.length){setHeightAnchor(null);return;}
    if((heightAnchor.index===0&&(path.extension||path.junctionSource))||(heightAnchor.index===path.points.length-1&&path.destination)){setError("接続された端点の高さは接続先で固定されています");return;}
    const levels=anchorHeights(path.points.length,path.anchorLevels,path.startLevel,(path.infrastructure??DEFAULT_INFRASTRUCTURE).level);
    const before={points:path.points,geometry:path.editedGeometry,redo:path.redo,anchorLevels:levels,redoAnchorLevels:path.redoAnchorLevels,extension:path.extension,destination:path.destination,redoDestination:path.redoDestination,startLevel:path.startLevel,infrastructure:path.infrastructure};
    const after={...before,anchorLevels:changeAnchorHeight(levels,heightAnchor.index,level)};
    const oldHistory=parkedAnchorHistory.current.get(path.id)??{undo:[],redo:[]};
    applyAnchorEdit(path.id,after);const edit:AnchorEdit={id:path.id,before,after};
    if(path.id===activePathId){setAnchorUndo(items=>[...items,edit].slice(-50));setAnchorRedo([]);}
    else {parkedAnchorHistory.current.set(path.id,{undo:[...oldHistory.undo,edit].slice(-50),redo:[]});}
  }
  const selectedHeightPath=heightAnchor?(heightAnchor.id===activePathId?parkCurrentPlan():parkedPaths.find(p=>p.id===heightAnchor.id)):null;
  const selectedHeight=selectedHeightPath&&heightAnchor?selectedHeightPath.anchorLevels?.[heightAnchor.index]??(heightAnchor.index===0?selectedHeightPath.startLevel:undefined)??selectedHeightPath.infrastructure?.level:undefined;
  const drawAnchors=(points:GeoPoint[],parked=false,id=activePathId)=>points.map((point,index) => {
    const p=project(point.latitude,point.longitude,camera),size=5/viewport.scale,hit=22/viewport.scale,endpoint=index===0||index===points.length-1;
    return <g key={index}><rect x={p.x-size/2} y={p.y-size/2} width={size} height={size} className={(parked?"parked-anchor":"draft-anchor")+(endpoint?" draft-endpoint":"")} vectorEffect="non-scaling-stroke"/>
      <rect onClick={event=>{if(mapMode==="move"){event.stopPropagation();selectTrackPath(id);setBuiltHeightSelection(null);setHeightAnchor({id,index});}}} onPointerEnter={event=>hoverPlan(event,id,index)} onPointerMove={event=>hoverPlan(event,id,index)} onPointerLeave={()=>{pointerFrame.cancel();setPlanHover(null);}} onPointerDown={event=>beginAnchorGesture(event,id,index)} data-anchor-index={index} data-anchor-path={id} x={p.x-hit/2} y={p.y-hit/2} width={hit} height={hit} className="anchor-drag-hit"/></g>;
  });
  const [companyName, setCompanyName] = useState("");
  const [routeId, setRouteId] = useState<string | null>(null);
  const [routeName, setRouteName] = useState("新路線");
  const [routeStart, setRouteStart] = useState("");
  const [routeHint, setRouteHint] = useState("");
  const [serviceId, setServiceId] = useState<string | null>(null);
  const [serviceStart, setServiceStart] = useState("");
  const [serviceAppearanceDirty,setServiceAppearanceDirty]=useState(false);
  const [serviceDirty, setServiceDirty] = useState(false);
  const [bindingDirty,setBindingDirty]=useState(false);
  const [pathServiceDirty,setPathServiceDirty]=useState(false);
  const [pathDraftEpoch,setPathDraftEpoch]=useState(0);
  const [serviceHint, setServiceHint] = useState("");
  const [pendingServiceChoice, setPendingServiceChoice] = useState<{ id: string | null } | null>(null);
  const enabled = status === "ready" && !busy && !companySetupRequired;

  const stationsById = useMemo(()=>new Map(world?.stations.map((station) => [station.id, station])),[world]);

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
      if(boxGesture.current)cancelMapGesture();
      smoothZoom.zoomBy(wheelZoomDelta(event.deltaY,event.deltaMode,rect.height),project(location.latitude,location.longitude,cameraRef.current));
    }
    svg.addEventListener("wheel", wheel, { passive: false });
    return () => svg.removeEventListener("wheel", wheel);
  }, []);
  useEffect(() => { smoothZoom.cancel(); changeMapMode(mapModeForTool(activeTool)); setHoverPoint(null); setPanelCollapsed(false); }, [activeTool]);
  const selectedServicePath=world?.service_paths?.find(path=>path.service_id===serviceId);

  const selectedRoutePath = world?.route_paths?.find(path => path.route_id === routeId);
  const companyDirty = !!world && companyName !== (world.companies[0]?.name ?? "");
  const selectedStation = world?.stations.find((station) => station.id === selected);

  const stationDirty = !!selectedStation && editStationName !== selectedStation.name;
  const selectedPlacement = world?.station_placements?.find(p => p.station_id === selected);
  const platformDirty = !!selectedPlacement && (Number(editLength) !== selectedPlacement.length_m || Number(editWidth) !== selectedPlacement.width_m);
  const stationSettingsDirty = stationDirty || platformDirty;
  const copyCanStart=!(serviceAppearanceDirty||serviceDirty||pathServiceDirty||companyDirty||stationSettingsDirty);
  const [seamExtensionKey,setSeamExtensionKey]=useState("");
  const [seamDestinationKey,setSeamDestinationKey]=useState("");
  const [seamFocusKey,setSeamFocusKey]=useState("");
  const seamEndpoint=(key:string)=>alignments.flatMap(alignment=>(["start","end"] as const).map(endpoint=>({key:alignment.id+":"+endpoint,alignment,endpoint,point:endpoint==="start"?alignment.points[0]:alignment.points.at(-1)!}))).find(candidate=>candidate.key===key);
  const seamExtension=seamEndpoint(seamExtensionKey),seamDestination=seamEndpoint(seamDestinationKey);
  const seamFocusedEndpoint=seamEndpoint(seamFocusKey)||seamExtension||seamDestination;
  const seamEndpointLevel=(candidate:NonNullable<ReturnType<typeof seamEndpoint>>)=>{const index=candidate.endpoint==="start"?0:(candidate.alignment.anchor_indices?.length??2)-1;const height=builtAnchorHeight(candidate.alignment,index);const levels:SectionInfrastructure["level"][]=['underground_3','underground_2','underground','ground','elevated','elevated_2','elevated_3'];const level=levels.find((_,i)=>Math.abs((i-3)*6-height)<1e-6);if(!level)throw Error('接続端点の高さを6m段階で指定できません。');return level;};
  const correctedSeamPick=(pick:TilePathPick,key:string)=>{
    if(!key)return pick;const endpoint=seamEndpoint(key);
    if(!endpoint||endpoint.alignment.owner!==world?.companies[0]?.id)throw Error('接続先の線路を再選択してください。');
    const corrected=ofmEndpointPick(frozenRoadTiles.flatMap(t=>t.tileLines),pick,endpoint.point);
    if(!corrected)throw Error('接続端点は元の道路形状と一致しません。');return corrected;
  };
  const seamPreview=useCallback((start:TilePathPick,end:TilePathPick)=>{
    const lines=frozenRoadTiles.flatMap(t=>t.tileLines);const path=buildTileRoadPath(lines.filter(l=>['primary','secondary','tertiary','minor','service'].includes(l.class)),correctedSeamPick(start,seamExtensionKey),correctedSeamPick(end,seamDestinationKey));
    if(path.orderedSpans.some(p=>p.attributes.access!==undefined&&!['yes','permissive','designated'].includes(p.attributes.access)))throw Error('この道路のアクセス分類は追従対象外です。');
    const points=path.points.map(([longitude,latitude])=>({latitude,longitude}));if(alignmentLength(points)<20)throw Error("建設長が20m未満です。");for(const [index,key] of [[0,seamExtensionKey],[points.length-1,seamDestinationKey]] as const){const target=key?seamEndpoint(key)?.point:null;if(target&&(points[index].longitude!==target.longitude||points[index].latitude!==target.latitude))throw Error("近くに既設端点がありますが、道路から再生成した端点と完全に一致しません。");}return {points};
  },[frozenRoadTiles,seamExtensionKey,seamDestinationKey,world]);
  const manualReferenceCopyState=useReferenceCopy({bundle:roadFollowing?null:graphBundle,worldVersion:world,enabled,canStart:copyCanStart,tool:activeTool,name:alignmentName,onBuild:buildReferenceAlignment,roadFollow:roadFollowingBlocked,seamPreview:roadFollowing&&frozenRoadTiles.length?seamPreview:undefined,onBuildSeam:stageSeamPlan});
  const automaticRailSelected=referencePanel&&!roadFollowing&&!graphBundle;
  const automaticRail=useAutomaticRail({selected:automaticRailSelected,enabled,canStart:copyCanStart,worldEpoch:railLoadEpoch,name:alignmentName,bounds,zoom:camera.zoom,radius:Math.min(1000,12*40075016.686*Math.cos(camera.latitude*Math.PI/180)/(256*2**camera.zoom*mapViewport(mapSize.width||1000,mapSize.height||600).scale)),onBuild:buildReferenceAlignment});
  const referenceCopyState=automaticRailSelected?automaticRail.copy:manualReferenceCopyState;
  const rawSeamPicks=manualReferenceCopyState.picks.map(p=>{const edge=graphBundle?.graph.edges.find(e=>e.id===p.edgeId);return edge?{fragmentId:edge.osmWayId,segmentIndex:edge.segmentIndex,fraction:p.fraction}:null;});
  const seamPicks=rawSeamPicks.map((pick,index)=>{try{return pick?correctedSeamPick(pick,index===0?seamExtensionKey:seamDestinationKey):null;}catch{return null;}});
  const seamConnectionCandidates=(index:number)=>{const pick=rawSeamPicks[index];if(!pick)return [];const line=frozenRoadTiles.flatMap(t=>t.tileLines).find(l=>l.id===pick.fragmentId);if(!line)return [];const a=line.coordinates[pick.segmentIndex],b=line.coordinates[pick.segmentIndex+1];if(!a||!b)return [];const origin={longitude:a[0]+(b[0]-a[0])*pick.fraction,latitude:a[1]+(b[1]-a[1])*pick.fraction};return alignments.filter(a=>a.owner===world?.companies[0]?.id).flatMap(alignment=>(["start","end"] as const).flatMap(endpoint=>{const point=endpoint==="start"?alignment.points[0]:alignment.points.at(-1)!;return point&&endpointDistance(point,origin,mapSize.width||1000,mapSize.height||600)<=8&&ofmEndpointPick([line],pick,point)?[{key:alignment.id+":"+endpoint,alignment,endpoint}]:[];}));};
  const seamCandidateLabel=(index:number,candidate:ReturnType<typeof seamConnectionCandidates>[number])=>{const name=candidate.alignment.name?.trim()||"名前未設定の線路";const peers=seamConnectionCandidates(index).filter(c=>(c.alignment.name?.trim()||"名前未設定の線路")===name&&c.endpoint===candidate.endpoint);return `${name}${peers.length>1?`（候補${peers.findIndex(c=>c.key===candidate.key)+1}）`:""}の${candidate.endpoint==="start"?"始点":"終点"}`;};
  const seamRequired=useMemo(()=>frozenRoadTiles.map(t=>{const p=t.bundle.source.source.split('/');return {x:Number(p.at(-2)),y:Number(p.at(-1)?.replace('.pbf',''))};}),[frozenRoadTiles]);
  const seamCopy=useOfmSeamCopy({enabled:status==="ready"&&!companySetupRequired&&roadFollowing,world,tool:activeTool,frozenTiles:frozenRoadTiles,requiredTiles:seamRequired,start:seamPicks[0]??null,end:seamPicks[1]??null,readTileEvidenceBatch:automaticRoad.readTileEvidenceBatch});

  const [referenceExtensionKey,setReferenceExtensionKey]=useState("");
  const referenceBuild=referenceCopyState.plan.build;
  const referenceExtensionCandidates=referenceBuild&&graphBundle?alignments.flatMap(alignment=>(["start","end"] as const).flatMap(endpoint=>{
    const point=endpoint==="start"?alignment.points[0]:alignment.points.at(-1),origin=referenceBuild.points[0];
    if(!point||endpointDistance(point,origin,mapSize.width||1000,mapSize.height||600)>8)return [];
    const build=snapReferenceStart(graphBundle,referenceCopyState.picks,point,alignmentName);
    return build?[{key:alignment.id+":"+endpoint,alignment,endpoint,build}]:[];
  })):[];
  const referenceExtensionLabel=(candidate:typeof referenceExtensionCandidates[number])=>{const name=candidate.alignment.name?.trim()||"名前未設定の線路";const peers=referenceExtensionCandidates.filter(item=>(item.alignment.name?.trim()||"名前未設定の線路")===name&&item.endpoint===candidate.endpoint);return `${name}${peers.length>1?`（候補${peers.indexOf(candidate)+1}）`:""}の${candidate.endpoint==="start"?"始点":"終点"}から延伸`;};
  const referenceExtension=referenceExtensionCandidates.find(candidate=>candidate.key===referenceExtensionKey);
  const referenceExtensionError=referenceExtensionKey?(!referenceExtension?"延伸対象の端点が参照形状に一致しません。起点を選び直してください。":referenceExtension.alignment.owner!==world?.companies[0]?.id?"他社の線路には延伸できません。":extensionJoinError(referenceExtension.build.points,extensionPrevious({alignmentId:referenceExtension.alignment.id,endpoint:referenceExtension.endpoint}))):null;
  const referencePreviewPoints=manualReferenceCopyState.seamPlan?.points??referenceExtension?.build.points??referenceCopyState.plan.build?.points;
  const roadDrawingTiles=useMemo(()=>roadTileSnapshots(automaticRoad.snapshot).flatMap(tile=>tile.tileLines),[automaticRoad.snapshot]);
  const roadDrawing=useRoadDrawing(graphBundle,enabled&&roadFollowing&&activeTool==="tracks"&&mapMode==="draw",roadDrawingTiles);
  useEffect(()=>{roadDrawing.cancel();},[draftPoints,draftFinished,infrastructure,world,activeTool,mapMode]);
  const roadFinishAfterPlacement=useRef(false);
  const referenceCopy={...referenceCopyState,plan:referenceExtension?{build:referenceExtension.build,error:referenceCopyState.plan.error}:referenceCopyState.plan};
  type ReferenceDraftEdit={session:ReferenceSessionSnapshot;stations:PlannedStation[];extension:string;seamExtension:string;seamDestination:string};
  const referenceJournal=useRef(new EditHistory<ReferenceDraftEdit>());
  const referenceBaseline=useRef<ReferenceDraftEdit|null>(null);
  const referenceRestoring=useRef(false);
  const [referenceHistoryAvailable,setReferenceHistoryAvailable]=useState({undo:false,redo:false});
  function refreshReferenceHistory(){const next={undo:!!referenceJournal.current.peek(false),redo:!!referenceJournal.current.peek(true)};setReferenceHistoryAvailable(old=>old.undo===next.undo&&old.redo===next.redo?old:next);}
  useLayoutEffect(()=>{
    if(busyRef.current||referenceCopy.busy)return;
    if(!referenceCopy.active){referenceJournal.current.clear();referenceBaseline.current=null;referenceRestoring.current=false;refreshReferenceHistory();return;}
    const next:ReferenceDraftEdit={session:referenceCopy.sessionSnapshot,stations:plannedStations.filter(s=>s.pathId===-1),extension:referenceExtensionKey,seamExtension:seamExtensionKey,seamDestination:seamDestinationKey};
    const before=referenceBaseline.current;
    if(referenceRestoring.current){referenceRestoring.current=false;referenceBaseline.current=next;return;}
    if(before&&before.session.bundleIdentity===next.session.bundleIdentity&&before.session.provider===next.session.provider){
      if(!sameReferenceSession(before.session,next.session)||JSON.stringify([before.stations,before.extension,before.seamExtension,before.seamDestination])!==JSON.stringify([next.stations,next.extension,next.seamExtension,next.seamDestination]))referenceJournal.current.record(before,next,false);
    }else referenceJournal.current.clear();
    referenceBaseline.current=next;refreshReferenceHistory();
  },[referenceCopy.sessionSnapshot,referenceCopy.busy,plannedStations,referenceExtensionKey,seamExtensionKey,seamDestinationKey,busy]);
  function stepReferenceHistory(redo:boolean){
    if(!enabled||busyRef.current||referenceCopy.busy||!referenceCopy.active||saveBlocked)return;
    const entry=referenceJournal.current.peek(redo);if(!entry)return;
    const target=redo?entry.after:entry.before;
    if(!referenceCopy.restoreSession(target.session,referenceCopy.sessionSnapshot)){setError('参照経路の状態が変わったため履歴を復元できません。');return;}
    referenceRestoring.current=true;referenceJournal.current.complete(redo);
    setPlannedStations(items=>[...items.filter(s=>s.pathId!==-1),...target.stations]);setReferenceExtensionKey(target.extension);setSeamExtensionKey(target.seamExtension);setSeamDestinationKey(target.seamDestination);refreshReferenceHistory();
  }
  useEffect(()=>{setReferenceExtensionKey("");setSeamExtensionKey("");setSeamDestinationKey("");setSeamFocusKey("");if(!referenceCopy.active){setFrozenRoadBundle(null);setFrozenRoadTiles(old=>old.length?[]:old);}},[referenceCopy.active,graphBundle]);
  useEffect(()=>{
    if(roadFollowing&&referenceCopy.active&&graphBundle&&!frozenRoadBundle){
      setFrozenRoadBundle(graphBundle);setFrozenRoadTiles(roadTileSnapshots(automaticRoad.snapshot));
    }
  },[roadFollowing,referenceCopy.active,graphBundle,frozenRoadBundle]);
  const stationLines=useMemo<StationLine[]>(()=>[...alignments,...(referenceCopy.active&&referencePreviewPoints?[{id:"reference-plan",pathId:-1,points:referencePreviewPoints}]:[]),...parkedPaths.map(plan=>({id:"plan:"+plan.id,pathId:plan.id,points:plan.line,invalid:(plan.reference||plan.ofmPath)?!!extensionJoinError(plan.points,extensionPrevious(plan.extension)):!!plannedGeometry(plan.points,plan.editedGeometry,extensionPrevious(plan.extension)).error})),...(draftLine.length>1?[{id:"plan:"+activePathId,pathId:activePathId,points:draftLine,invalid:draftInvalid||roadFollowingBlocked}]:[])],[alignments,referenceCopy.active,referencePreviewPoints,parkedPaths,extensionPrevious,draftLine,activePathId,draftInvalid,roadFollowingBlocked]);
  const plannedPlatforms=useMemo(()=>plannedStations.map(station=>{
    const line=stationLines.find(line=>station.alignmentId?line.pathId===undefined&&line.id===station.alignmentId:line.pathId===station.pathId);
    const nearest=line?nearestOnAlignment(line.points,station.point):null;
    const polygon=line&&nearest?stationPolygon(line.points,nearest.position,station.lengthM,12):[];
    const half=line?station.lengthM/alignmentLength(line.points)/2:0;
    const position=nearest?Math.max(half,Math.min(1-half,nearest.position)):0;
    const center=line&&polygon.length>=4?pointOnAlignment(line.points,position):null;
    return {station,line,position,polygon,valid:!!line&&!line.invalid&&!!center&&distanceM(center,station.point)<1&&polygon.length>=4};
  }),[plannedStations,stationLines]);
  const stationGroupDisplay=useMemo(()=>stationGroupViews([
    ...(world?.stations??[]).flatMap(station=>{const placement=world?.station_placements?.find(p=>p.station_id===station.id),polygon=stationPolygons.get(station.id);return placement&&polygon?[{id:station.id,name:station.name,owner:station.owner,lineId:placement.alignment_id,polygon,planned:false}]:[];}),
    ...plannedPlatforms.flatMap(p=>p.line?[{id:p.station.id,name:p.station.name,owner:world?.companies[0]?.id??"",lineId:p.line.id,polygon:p.polygon,planned:true}]:[])
  ],world?.station_groups),[world,stationPolygons,plannedPlatforms]);
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
  const saveBlocked = routeRenameDirty || serviceAppearanceDirty || bindingDirty || !!anchorPreview || candidateDirty || serviceDirty || pathServiceDirty || companyDirty || stationSettingsDirty;
  const hasPlans = plannedStations.length>0 || draftPoints.length > 0 || redoPoints.length > 0 || parkedPaths.length > 0 || referenceCopy.active;
  const constructionPlans=useMemo<ConstructionPlans>(()=>({version:1,draftAnchorLevels:draftPoints.length?draftAnchorLevels:undefined,redoAnchorLevels:redoPoints.length?redoAnchorLevels:undefined,plans,parkedPaths,activePathId,activePlanId,draftPoints,draftEditedGeometry,redoPoints,draftExtension,draftDestination,draftRedoDestination,trackType,designSpeedKmh,draftJunctionSource:(draftPoints.length||redoPoints.length)?directedJunction:undefined,startLevel:(draftPoints.length||redoPoints.length)?startLevel:undefined,infrastructure:infrastructure.track_count===1&&infrastructure.level==="ground"?undefined:infrastructure,roadFollowing:legacyRoadDraft,plannedStations:plannedStations.filter(station=>station.pathId!==-1),draftFinished}),[draftAnchorLevels,redoAnchorLevels,plans,parkedPaths,activePathId,activePlanId,draftPoints,draftEditedGeometry,redoPoints,draftExtension,draftDestination,draftRedoDestination,trackType,designSpeedKmh,infrastructure,startLevel,directedJunction,roadFollowing,legacyRoadDraft,plannedStations,draftFinished]);
  useLayoutEffect(()=>{
    if(!world||busyRef.current)return;
    const next={world,plans:constructionPlans},before=editBaseline.current,action=pendingWorldAction.current;
    if(historyRestoring.current){historyRestoring.current=false;pendingWorldAction.current=null;editBaseline.current=next;return;}
    if(!before){editBaseline.current=next;pendingWorldAction.current=null;return;}
    const changedWorld=!sameWorldEdit(before.world,world),changedPlan=before.plans!==constructionPlans&&planEditKey(before.plans)!==planEditKey(constructionPlans);
    if(action&&sameWorldEdit(action.result,world)){
      pendingWorldAction.current=null;
      if(action.kind==='load'||(action.kind==='final'&&changedWorld)){editJournal.current.clear();editBaseline.current=next;refreshHistoryAvailability();return;}
      if(action.kind==='stage'&&(changedWorld||changedPlan)){editJournal.current.record(before,next,changedWorld);editBaseline.current=next;refreshHistoryAvailability();return;}
    }
    if(changedWorld){
      // World mutation must have a matching successful native editing operation.
      if(action?.kind==='edit')editJournal.current.record(before,next,true);
      else editJournal.current.clear();
    }else if(changedPlan){
      const last=editJournal.current.peek(false);
      // A stroke's first segment is one edit; its first anchor alone is only its start.
      if(last&&!last.world&&last.after.plans.activePathId===next.plans.activePathId&&(last.before.plans.draftPoints.length===0||last.before.plans.activePathId!==next.plans.activePathId)&&last.after.plans.draftPoints.length===1&&next.plans.draftPoints.length===2&&last.after.plans.parkedPaths.length===next.plans.parkedPaths.length){last.after=next;editJournal.current.redo=[];}
      else editJournal.current.record(before,next,false);
    }else {
      const last=editJournal.current.peek(false);
      if(last&&planEditKey(last.after.plans)===planEditKey(next.plans)&&sameWorldEdit(last.after.world,next.world))last.after=next;
    }
    editBaseline.current=next;refreshHistoryAvailability();
  },[world,constructionPlans,busy]);
  function stepEditHistory(redo:boolean){
    if(!enabled||busyRef.current||historyRestoring.current||!world)return;
    if(unapplied||stationResizePending){setError("編集中の変更を適用または取り消してください。");return;}
    const entry=editJournal.current.peek(redo);if(!entry){setNotice("元に戻せる操作はありません。");return;}
    const target=redo?entry.after:entry.before;
    const restore=(snapshot:World)=>{
      historyRestoring.current=true;editJournal.current.complete(redo);refreshHistoryAvailability();
      const saved=savedPlanFingerprint;restoreConstructionPlans({...target.plans,draftFinished:target.plans.draftPoints.length>=2});setSavedPlanFingerprint(saved);
      setWorld(snapshot);setDirty(true);setTrackDeleteUndo(null);setTrackDeleteRedo(null);clearStationHistory();
      setPathDraftEpoch(value=>value+1);resetRoute();resetService();setStationCursor(null);setSelected(null);setEditStationName("");setPendingDelete(null);setPendingStation(null);setCompanyName(snapshot.companies[0]?.name??"");
      setNotice(redo?"操作をやり直しました。":"操作を元に戻しました。");
    };
    if(entry.world){void perform(async()=>{const snapshot=await nativeInvoke<World>(redo?'redo_world_edit':'undo_world_edit',{expectedWorld:{...world,construction_plans:target.plans,view_state:camera}});restore(snapshot);});}
    else restore(world);
  }
  const persistedPlans=useMemo(()=>compactEmptyPlans(constructionPlans),[constructionPlans]);
  const planFingerprint=useMemo(()=>constructionPlanFingerprint(constructionPlans),[constructionPlans]);
  const seamStageCurrent=useRef({world,worldEpoch,planFingerprint,picks:JSON.stringify([manualReferenceCopyState.picks,seamExtensionKey,seamDestinationKey]),activeTool});
  seamStageCurrent.current={world,worldEpoch,planFingerprint,picks:JSON.stringify([manualReferenceCopyState.picks,seamExtensionKey,seamDestinationKey]),activeTool};

  const [savedPlanFingerprint,setSavedPlanFingerprint]=useState(()=>constructionPlanFingerprint(emptyConstructionPlans()));
  const plansDirty=planFingerprint!==savedPlanFingerprint;
  const unapplied = saveBlocked || referenceCopy.active;
  const viewDirty=JSON.stringify(camera)!==savedViewFingerprint;
  const autosaveSnapshot=useMemo(()=>({world,constructionPlans,camera}),[world,constructionPlans,camera]);
  const autosave=useAutosave({slotId:activeSlotId,ready:status==="ready"&&slotsReady&&activeSlotId!==null,dirty:(dirty||plansDirty||viewDirty)&&!companySetupRequired,world:autosaveSnapshot,constructionPlans:persistedPlans,viewState:camera,epoch:worldEpoch,restoredEpoch:restoredAutosaveEpoch,busyRef,setBusy});
  const closeGuard = useUnsavedCloseGuard(dirty || plansDirty || unapplied || stationResizePending);
  function restoreConstructionPlans(value:ConstructionPlans|null|undefined,compact=false){
    parkedAnchorHistory.current.clear();
    const raw=value??emptyConstructionPlans(),original={...raw,infrastructure:raw.infrastructure?.track_count===1&&raw.infrastructure.level==="ground"?undefined:raw.infrastructure},restored=compact?compactEmptyPlans(original):original,ids=nextConstructionIds(restored);
    dispatchConstruction({type:"RESTORE_PLANS",plans:restored});setAnchorUndo([]);setAnchorRedo([]);
    setHeightAnchor(null);setInfrastructure(restored.infrastructure??DEFAULT_INFRASTRUCTURE);setTrackType(restored.trackType);setDesignSpeedKmh(restored.designSpeedKmh);setRoadFollowing(restored.roadFollowing);setLegacyRoadDraft(restored.roadFollowing);setPlannedStations(restored.plannedStations);
    nextPlanId.current=ids.plan;nextPathId.current=ids.path;nextPlannedStationId.current=ids.station;
    setSavedPlanFingerprint(constructionPlanFingerprint(restored));
  }
  function restoreLoadedPlans(snapshot:World){
    const original=snapshot.construction_plans??emptyConstructionPlans();
    const recovery=recoverLegacyRedoPlans(original,extension=>{const line=snapshot.alignments?.find(a=>a.id===extension.alignmentId);return extension.endpoint==='start'?line?.points[1]:line?.points.at(-2);});
    restoreConstructionPlans(recovery.plans,true);
    setSavedPlanFingerprint(constructionPlanFingerprint(compactEmptyPlans(original)));
    setPlanRecoveryNotice([recovery.recoveredPathIds.length?`旧形式の取り消し中データから未着工経路を${recovery.recoveredPathIds.length}件復旧しました。内容を確認して保存してください。`:'',recovery.unresolvedPathIds.length?`復旧を確定できない旧経路（${recovery.unresolvedPathIds.join('、')}）は元データを保持しています。保存を上書きせず確認してください。`:''].filter(Boolean).join(' '));
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
      const after:ConstructionPlans=id===activePathId?{...before,draftPoints:[],draftEditedGeometry:undefined,redoPoints:[],draftAnchorLevels:undefined,redoAnchorLevels:undefined,startLevel:undefined,draftJunctionSource:undefined,draftExtension:null,draftDestination:undefined,draftRedoDestination:undefined,draftFinished:false}:{...before,parkedPaths:before.parkedPaths.filter(p=>p.id!==id)};
      const saved=savedPlanFingerprint;
      restoreConstructionPlans(after);setSavedPlanFingerprint(saved);
      setTrackDeleteUndo({kind:"plan",before,after:constructionPlanFingerprint(after),world});
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
  // Splitting built sections can remap extension/station targets in pending plans.
  function restoreRemappedPlans(snapshot:World){
    if(!snapshot.construction_plans)return planFingerprint;
    const saved=savedPlanFingerprint;
    restoreConstructionPlans(snapshot.construction_plans);setSavedPlanFingerprint(saved);
    return constructionPlanFingerprint(snapshot.construction_plans);
  }
  function undoTrackDelete(){ stepEditHistory(false); }
  function redoTrackDelete(){ stepEditHistory(true); }
  useEffect(()=>{setRangeSelection([]);setConfirmRangeDelete(false);setRangeTrackCount(null);setRangeLevel(null);setRangeTrackType(null);const selection=boxGesture.current;boxGesture.current=null;setSelectionBox(null);const svg=svgRef.current;if(selection&&svg?.hasPointerCapture?.(selection.pointerId))svg.releasePointerCapture(selection.pointerId);},[world,activeTool]);
  useEffect(()=>{setBuiltHeightSelection(selection=>currentBuiltHeightSelection(selection,world?.alignments??[],activeTool==="select"));},[world,activeTool]);
  function updateBuiltAnchorHeight(level:SectionInfrastructure["level"]){
    if(!enabled||unapplied||!builtHeightSelection)return;
    const {alignment,anchorIndex}=builtHeightSelection;
    const targetHeight=({ground:0,elevated:6,elevated_2:12,elevated_3:18,underground:-6,underground_2:-12,underground_3:-18} as const)[level];
    if(Math.abs(builtAnchorHeight(alignment,anchorIndex)-targetHeight)<1e-6)return;
    const args={alignmentId:alignment.id,anchorIndex,level,expectedAlignment:alignment,constructionPlans};
    void perform(async()=>{const snapshot=await invoke<World>("update_alignment_anchor_height",args);if(equalWorldSnapshots(snapshot,world))return;setWorld(snapshot);setDirty(true);setTrackDeleteRedo(null);setTrackDeleteUndo({kind:"built",world:snapshot,plans:planFingerprint,command:"update_alignment_anchor_height",args});setBuiltHeightSelection(null);});
  }
  const rangeCount=rangeSelection.reduce((sum,row)=>sum+row.sectionIndices.length,0);
  const selectedInfrastructure=rangeSelection.flatMap(row=>row.sectionIndices.map(index=>alignments.find(line=>line.id===row.alignmentId)?.infrastructure?.[index]??DEFAULT_INFRASTRUCTURE));
  const selectedCountValues=new Set(selectedInfrastructure.map(value=>value.track_count));
  const selectedLevelValues=new Set(selectedInfrastructure.map(value=>value.level));
  function updateRangeTrackType(){
    if(!enabled||unapplied||!rangeCount||rangeTrackType===null)return;
    const args={selections:rangeSelection,expectedAlignments:rangeSelection.map(row=>alignments.find(line=>line.id===row.alignmentId)!),constructionPlans,trackType:rangeTrackType};
    void perform(async()=>{const snapshot=await invoke<World>("update_alignment_section_track_type",args);if(equalWorldSnapshots(snapshot,world))return;setWorld(snapshot);setDirty(true);setTrackDeleteRedo(null);setTrackDeleteUndo({kind:"built",world:snapshot,plans:restoreRemappedPlans(snapshot),command:"update_alignment_section_track_type",args});});
  }
  function updateRangeInfrastructure(){
    if(!enabled||unapplied||!rangeCount||(rangeTrackCount===null&&rangeLevel===null))return;
    const args={selections:rangeSelection,expectedAlignments:rangeSelection.map(row=>alignments.find(line=>line.id===row.alignmentId)!),constructionPlans,trackCount:rangeTrackCount,level:rangeLevel};
    void perform(async()=>{const snapshot=await invoke<World>("update_alignment_section_infrastructure",args);setWorld(snapshot);setDirty(true);setTrackDeleteRedo(null);setTrackDeleteUndo({kind:"built",world:snapshot,plans:planFingerprint,command:"update_alignment_section_infrastructure",args});});
  }
  function deleteRange(){
    if(!enabled||unapplied||!rangeCount)return;
    const args={selections:rangeSelection,expectedAlignments:rangeSelection.map(row=>alignments.find(line=>line.id===row.alignmentId)!),constructionPlans};
    void perform(async()=>{const snapshot=await invoke<World>("delete_alignment_sections",args);setWorld(snapshot);setDirty(true);setTrackDeleteRedo(null);setTrackDeleteUndo({kind:"built",world:snapshot,plans:planFingerprint,command:"delete_alignment_sections",args});setConfirmRangeDelete(false);setRangeSelection([]);});
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
    if(target.kind==="plan-section"){const path=target.pathId===activePathId?parkCurrentPlan():parkedPaths.find(path=>path.id===target.pathId);if(path?.startLevel!==undefined&&path.startLevel!==(path.infrastructure??DEFAULT_INFRASTRUCTURE).level)return "勾配を保持した計画区間の分割削除は未対応です。計画全体の削除は可能です。";}
    if(target.kind==="plan-section"||target.kind==="plan-anchor"){const path=target.pathId===activePathId?parkCurrentPlan():parkedPaths.find(path=>path.id===target.pathId);if(path?.junctionSource)return "分岐計画の部分削除は未対応です。計画全体の削除は可能です。";}
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
    const rect=svgRef.current?.getBoundingClientRect();if(!rect)return;
    const x=event.clientX-rect.left,y=event.clientY-rect.top;
    // Moving within one target only moves its tooltip, not the entire map and
    // construction model. A target change still renders the new highlight.
    if(deletionTooltipRef.current){
      deletionTooltipRef.current.style.left=`${Math.max(8,Math.min(mapSize.width-240,x+18))}px`;
      deletionTooltipRef.current.style.top=`${Math.max(8,Math.min(mapSize.height-80,y+18))}px`;
    }
    setDeleteHover(previous=>JSON.stringify(previous?.target)===JSON.stringify(target)?previous:{target,x,y});
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
      setTrackDeleteUndo({kind:"plan",before,after:constructionPlanFingerprint(after),world});setDeleteTarget(null);setSelectedPlanPath(null);
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
    const routes = world?.routes.filter(route => route.start === id || world.route_paths?.some(path => path.route_id === route.id && (path.member_stations.includes(id) || path.segments.some(segment => segment.from === id || segment.to === id)))) ?? [];
    const services = world?.services.filter(service => service.start === id || world.service_paths?.some(path => {if(path.service_id !== service.id)return false;try{return resolveServicePath(service.start,path.legs,world).some(o=>o.station_id===id);}catch{return true;}})) ?? [];
    return [...routes.map(route=>"路線: "+route.name),...services.map(service=>"系統: "+service.name)].join(" / ");
  }
  const selectStation=useCallback((id: string) => {
    if (busyRef.current) return;
    if ((mainTab === "routes" || mainTab === "services") && routeMapSelect.current) {
      if (!enabled || !stationsById.has(id)) return;
      const select=routeMapSelect.current;if(routeMapPick!=="sequence")cancelRouteMapPick();select(id);return;
    }
    if (stationSettingsDirty && id !== selected) { setPendingStation({ id }); return; }
    if (id === selected) { setMainTab("stations"); setActiveTool("station-management"); setPanelCollapsed(false); return; }
    setMainTab("stations"); setActiveTool("station-management");
    setSelected(id); setEditStationName(stationsById.get(id)?.name ?? ""); setPendingStation(null); setPendingDelete(null);
    const placement = world?.station_placements?.find((entry) => entry.station_id === id);
    setEditLength(String(placement?.length_m ?? 200)); setEditWidth(String(placement?.width_m ?? 12)); setPanelCollapsed(false);
  },[mainTab,enabled,stationsById,routeMapPick,cancelRouteMapPick,stationSettingsDirty,selected,world]);
  const selectMapStation=useCallback((id:string)=>{if(!suppressSelectionClick.current)selectStation(id);},[selectStation]);
  const clearBuiltHover=useCallback(()=>{pointerFrame.cancel();setBuiltHover(null);},[pointerFrame.cancel]);
  const selectBuiltTrack=useCallback((event:MouseEvent<SVGPathElement>,id:string)=>{
    if(mapMode==="move"&&activeTool!=="select"&&!suppressSelectionClick.current){
      event.stopPropagation();setAlignmentId(id);setSelectedPlanPath(null);setTrackDeleteConfirm(false);
    }
  },[mapMode,activeTool]);
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
      setDirty(true); setConfirmLoad(false); setNotice("駅名を変更しました。保存でファイルに反映します。Undoで駅名を戻せます。");clearStationHistory();
    });
  }
  function deleteEquipment() {
    if (!enabled || !pendingDelete || unapplied) return;
    const { kind, id } = pendingDelete;
    if (stationReferences(id)) return;
    void perform(async () => {
      const snapshot = await invoke<World>("delete_station", { stationId: id });
      setWorld(snapshot); setDirty(true); setConfirmLoad(false); setPendingDelete(null);
      if (kind === "station") {
        clearStationHistory();
        if (selected === id) { setSelected(null); setEditStationName(""); setPendingStation(null); }
        if (routeStart === id) resetRoute();
        if (serviceStart === id) resetService();
      }
      setNotice("設備を削除しました。保存でファイルに反映します。");
    });
  }
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
        if (active) { setInfo(engine); setCompanySetupRequired(engine.company_setup_required===true); setWorld(snapshot); restoreLoadedPlans(snapshot);restoreView(snapshot.view_state);setCompanyName(snapshot.companies[0]?.name ?? ""); setStatus("ready"); }
      },
      (failure) => {
        if (active) { setError(detail(failure)); setStatus("error"); }
      },
    );
    return () => { active = false; };
  }, [attempt]);

  async function perform(operation: () => Promise<void>,trace?:ConstructionTrace) {
    if (status !== "ready" || busyRef.current) return;
    const endPerform=trace?.span("perform");
    busyRef.current = true;
    setBusy(true); setError(""); setNotice("");
    try { await operation(); }
    catch (failure) { setError(detail(failure)); }
    finally { busyRef.current = false; setBusy(false);endPerform?.();if(trace&&!trace.worldScheduled)trace.complete(); }
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
  function cancelMapGesture() {
    const selection=boxGesture.current;boxGesture.current=null;setSelectionBox(null);
    const gesture = gestureRef.current;
    gestureRef.current = null; suppressMapClick.current = true; suppressSelectionClick.current = true; setDragging(false);
    const svg = svgRef.current;
    if(selection&&svg?.hasPointerCapture?.(selection.pointerId))svg.releasePointerCapture(selection.pointerId);
    if (gesture && svg?.hasPointerCapture?.(gesture.pointerId)) svg.releasePointerCapture(gesture.pointerId);
  }
  function changeMapMode(mode: MapMode) {
    setError("");
    const wasGesturing = gestureRef.current !== null;
    pointerFrame.cancel();stationInputCursor.current=null;cancelAnchorGesture();cancelMapGesture(); setMapMode(mode);setStationCursor(null); setCursorPosition(null); setHoverPoint(null); setPlanHover(null);setBuiltHover(null);
    if (!wasGesturing) { suppressMapClick.current = false; suppressSelectionClick.current = false; }
  }
  function applyAnchorEdit(id:number,value:PathEdit,type:"APPLY_PATH_EDIT"|"UNDO_POINT"|"REDO_POINT"="APPLY_PATH_EDIT"){
    if(id===activePathId){if(value.infrastructure)setInfrastructure(value.infrastructure);setHoverPoint(null);}
    else parkedAnchorHistory.current.delete(id);
    const path=parkedPaths.find(path=>path.id===id);
    dispatchConstruction({type,id,edit:value,previous:extensionPrevious("extension" in value?value.extension??null:path?.extension??null)});
  }
  function undoLastSegment(historyEntries=0){
    const entry:AnchorEdit={kind:"point",id:activePathId,
      before:{anchorLevels:[],redoAnchorLevels:[...redoAnchorLevels,...anchorHeights(draftPoints.length,draftPoints.length?draftAnchorLevels:undefined,startLevel,infrastructure.level).reverse()],points:[],geometry:draftEditedGeometry,redo:[...redoPoints,draftPoints[1],draftPoints[0]],redoDestination:draftDestination??draftRedoDestination},
      after:{anchorLevels:draftAnchorLevels,redoAnchorLevels,points:draftPoints,geometry:draftEditedGeometry,redo:redoPoints,destination:draftDestination,redoDestination:draftRedoDestination}};
    applyAnchorEdit(activePathId,entry.before);
    if(historyEntries)setAnchorUndo(items=>items.slice(0,-historyEntries));
    setAnchorRedo(items=>[...items,entry]);
  }
  function undoAnchor(redo:boolean){
    const history=redo?anchorRedo:anchorUndo,entry=history.at(-1);if(!entry)return;
    if(!redo&&entry.kind==="point"&&entry.id===activePathId&&draftFinished&&draftPoints.length===2&&entry.before.points.length===1&&!(constructionUndoFloor.current.get(activePathId)??0)){
      const start=history.at(-2);
      undoLastSegment(start?.kind==="point"&&start.id===activePathId&&start.before.points.length===0&&start.after.points.length===1?2:1);return;
    }
    applyAnchorEdit(entry.id,redo?entry.after:entry.before,redo?"REDO_POINT":"UNDO_POINT");
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
    if(!path||path.reference||path.ofmPath||path.roadFollowing||path.points.length<2||(index===0&&(path.extension||path.junctionSource)))return;
    event.stopPropagation();smoothZoom.cancel();cancelMapGesture();
    const gesture:AnchorGesture={pointerId:event.pointerId,id,index,startX:event.clientX,startY:event.clientY,points:path.points,line:path.line,geometry:path.editedGeometry,redo:path.redo,destination:path.destination,redoDestination:path.redoDestination,previous:extensionPrevious(path.extension),preview:null,error:null,snapped:false};
    anchorGesture.current=gesture;setAnchorPreview(gesture);setHoverPoint(null);setPlanHover(null);setBuiltHover(null);
    svgRef.current?.setPointerCapture?.(event.pointerId);
  }
  function moveAnchorGesture(event:MapPointerMove){
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
    pointerFrame.cancel();
    smoothZoom.cancel();
    setPlanHover(null);setBuiltHover(null);referenceCopy.clearHover();
    if (![0,1].includes(event.button) || event.isPrimary === false || gestureRef.current || boxGesture.current) return;
    if(event.button===1)event.preventDefault();
    if(event.button===0&&activeTool==="select"&&enabled){
      const p=coordinatesFromClient(event.clientX,event.clientY,event.currentTarget.getBoundingClientRect(),cameraRef.current);if(!p)return;
      const point=project(p.latitude,p.longitude,cameraRef.current),box={start:point,end:point};
      boxGesture.current={pointerId:event.pointerId,startX:event.clientX,startY:event.clientY,box,camera:cameraRef.current,additive:event.shiftKey};setSelectionBox(box);if(!event.shiftKey)setRangeSelection([]);setConfirmRangeDelete(false);setRangeTrackCount(null);setRangeLevel(null);setRangeTrackType(null);
      suppressMapClick.current=true;suppressSelectionClick.current=true;event.currentTarget.setPointerCapture?.(event.pointerId);return;
    }
    suppressMapClick.current = true;
    const rect = event.currentTarget.getBoundingClientRect();
    if (!coordinatesFromClient(event.clientX, event.clientY, rect, cameraRef.current)) return;
    const { scale } = mapViewport(rect.width, rect.height);
    const existingEquipment = activeTool!=="stations" && event.target instanceof Element && !!event.target.closest(".station-marker, .track-connection");
    gestureRef.current = { pointerId: event.pointerId, startX: event.clientX, startY: event.clientY, camera: cameraRef.current, scale, dragged: false, mayPlace: event.button===0 && ((mapMode === "place" && activeTool === "stations") || (mapMode === "draw" && activeTool === "tracks")) && !existingEquipment };
    suppressMapClick.current = false;
    suppressSelectionClick.current = false;
  }
  function pointerMove(event: MapPointerMove) {
    const selecting=boxGesture.current;
    if(selecting&&selecting.pointerId===event.pointerId){
      const rect=event.currentTarget.getBoundingClientRect(),x=Math.max(rect.left,Math.min(rect.right,event.clientX)),y=Math.max(rect.top,Math.min(rect.bottom,event.clientY));
      const p=coordinatesFromClient(x,y,rect,selecting.camera);if(p){selecting.box={...selecting.box,end:project(p.latitude,p.longitude,selecting.camera)};setSelectionBox(selecting.box);}return;
    }
    if(anchorGesture.current){moveAnchorGesture(event);return;}
    if(enabled&&referenceCopy.active&&referenceCopy.roadFollow&&mapMode==="draw"&&!gestureRef.current){const p=coordinatesFromClient(event.clientX,event.clientY,event.currentTarget.getBoundingClientRect(),camera);if(p)void referenceCopy.hover(p);}

    if(enabled&&activeTool==="stations"&&mapMode==="place"&&!gestureRef.current){const rect=event.currentTarget.getBoundingClientRect();setStationCursor(coordinatesFromClient(event.clientX,event.clientY,rect,camera));setCursorPosition({x:event.clientX-rect.left,y:event.clientY-rect.top,width:rect.width,height:rect.height});}
    if (enabled && !referenceCopy.active && mapMode === "draw" && activeTool === "tracks" && !gestureRef.current) {
      const rect=event.currentTarget.getBoundingClientRect(),point=coordinatesFromClient(event.clientX,event.clientY,rect,camera);
      setCursorPosition(point?{x:event.clientX-rect.left,y:event.clientY-rect.top,width:rect.width,height:rect.height}:null);
      drawingCursor.current=point?{point,width:rect.width,height:rect.height}:null;
      if(roadFollowing&&point&&!event.ctrlKey&&!parallelMode){roadDrawing.hover({point,start:!draftFinished?draftPoints.at(-1):undefined,radiusM:20});}else roadDrawing.cancel();
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
  const pointerMoveRef=useRef(pointerMove);
  useLayoutEffect(()=>{pointerMoveRef.current=pointerMove;});
  function schedulePointerMove(event:ReactPointerEvent<SVGSVGElement>){
    if(gestureRef.current||anchorGesture.current||boxGesture.current){pointerMove(event);return;}
    const {currentTarget,clientX,clientY,pointerId,pointerType,ctrlKey,type}=event;
    const input={currentTarget,clientX,clientY,pointerId,pointerType,ctrlKey,type};
    if(!gestureRef.current&&!anchorGesture.current){
      const rect=currentTarget.getBoundingClientRect(),point=coordinatesFromClient(clientX,clientY,rect,camera);
      if(activeTool==="stations"&&mapMode==="place")stationInputCursor.current=point;
      if(activeTool==="tracks"&&mapMode==="draw")drawingCursor.current=point?{point,width:rect.width,height:rect.height}:null;
    }
    pointerFrame.schedule("move",()=>pointerMoveRef.current(input));
  }
  function pointerEnd(event: ReactPointerEvent<SVGSVGElement>, canceled = false) {
    pointerFrame.cancel();
    const selecting=boxGesture.current;
    if(selecting&&selecting.pointerId===event.pointerId){
      if(!canceled)pointerMove(event);
      if(!canceled&&Math.hypot(event.clientX-selecting.startX,event.clientY-selecting.startY)>=5)setRangeSelection(previous=>{const selected=selectAlignmentSections(alignments,world?.companies[0]?.id,selecting.box,selecting.camera);return selecting.additive?mergeSectionSelections(previous,selected):selected;});
      boxGesture.current=null;setSelectionBox(null);suppressMapClick.current=true;suppressSelectionClick.current=true;
      if(event.currentTarget.hasPointerCapture?.(event.pointerId))event.currentTarget.releasePointerCapture(event.pointerId);return;
    }
    if(anchorGesture.current){endAnchorGesture(event,canceled);return;}
    const gesture = gestureRef.current;
    if (!gesture || gesture.pointerId !== event.pointerId) return;
    if (!canceled) pointerMove(event);
    suppressMapClick.current = canceled || gesture.dragged || !gesture.mayPlace;
    suppressSelectionClick.current = canceled || gesture.dragged;
    gestureRef.current = null; setDragging(false);
    if (event.currentTarget.hasPointerCapture?.(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
  }
  async function stageSeamPlan(){
    if(!enabled||busyRef.current||!copyCanStart)throw Error('別の処理中です。');
    if(referenceExtensionKey)throw Error('境界道路の接続先は専用の始点・終点欄で選択してください。');
    const captured={...seamStageCurrent.current};
    const current=()=>{const now=seamStageCurrent.current;return now.world===captured.world&&now.worldEpoch===captured.worldEpoch&&now.planFingerprint===captured.planFingerprint&&now.picks===captured.picks&&now.activeTool===captured.activeTool;};
    busyRef.current=true;setBusy(true);
    try{
      const expectedWorldToken=await invoke<string>('ofm_world_token');
      if(!current())throw Error('計画が変更されたため処理を中止しました。');
      const draft=await seamCopy.prepare();
      if(!draft||!current())throw Error('道路証拠を取得できないか、計画が変更されました。再選択してください。');
      const descriptor=parseOfmPathDescriptor(draft.persistentProposal.alignment);
      const preview=manualReferenceCopyState.seamPlan?.points;if(!preview)throw Error('境界形状を再選択してください。');
      if(plannedPlatforms.some(p=>p.station.pathId===-1&&!p.valid))throw Error('計画駅が線路に収まりません。');
      const id=nextPathId.current;
      const inheritedInfrastructure=seamExtension?{...endpointInfrastructure(seamExtension.alignment,seamExtension.endpoint),level:seamEndpointLevel(seamExtension)}:infrastructure;
      const path:PlannedPath={id,planId:activePlanId,name:planName,points:preview,line:preview,redo:[],infrastructure:inheritedInfrastructure,designSpeedKmh:seamExtension?(seamExtension.alignment.design_speed_kmh??defaultDesignSpeed(seamExtension.alignment.track_type??"normal")):designSpeedKmh,trackType:seamExtension?.alignment.track_type??trackType,roadFollowing:false,extension:seamExtension?{alignmentId:seamExtension.alignment.id,endpoint:seamExtension.endpoint}:null,destination:seamDestination?{alignmentId:seamDestination.alignment.id,endpoint:seamDestination.endpoint}:undefined,anchorLevels:preview.map((_,index)=>index===0&&seamExtension?seamEndpointLevel(seamExtension):index===preview.length-1&&seamDestination?seamEndpointLevel(seamDestination):inheritedInfrastructure.level),ofmPath:descriptor};
      const nextPlans={...constructionPlans,parkedPaths:[...constructionPlans.parkedPaths,path],plannedStations:plannedStations.map(p=>p.pathId===-1?{...p,pathId:id}:p)};
      if(!current())throw Error('計画が変更されたため処理を中止しました。');
      seamMutationPending.current=true;setSeamStageSending(true);
      const snapshot=await invoke<World>('stage_ofm_construction_plan',{expectedWorldToken,plans:nextPlans,rawEvidence:draft.persistentProposal.newRawEvidence,tileBindings:draft.persistentProposal.tileBindings});
      // A successful mutation is authoritative: never discard its returned world.
      // Editing/loading is busy-locked; navigation is locked during dispatch.
      const saved=savedPlanFingerprint;setWorld(snapshot);restoreConstructionPlans(snapshot.construction_plans);setSavedPlanFingerprint(saved);setDirty(true);
      const moveHistory=(entries:StationHistoryEntry[])=>entries.map(entry=>entry.planned?.pathId===-1?{...entry,planned:{...entry.planned,pathId:id}}:entry);
      setStationUndo(moveHistory);setStationRedo(moveHistory);
      setNotice('道路境界の出典と未着工計画を保持しました。計画を着工で建設します。');
    }finally{seamMutationPending.current=false;setSeamStageSending(false);busyRef.current=false;setBusy(false);}
  }
  async function buildReferenceAlignment(build:ReferenceBuild){
    if(!enabled||busyRef.current||!copyCanStart)throw Error("別の下書きまたは処理中です。");
    if(referenceExtensionError)throw Error(referenceExtensionError);
    build=referenceExtension?.build??build;
    const stations=plannedPlatforms.filter(item=>item.station.pathId===-1);
    if(stations.some(item=>!item.valid))throw Error("計画駅が線路に収まりません。");
    const id=nextPathId.current++;
    dispatchConstruction({type:"PARK_REFERENCE_PATH",path:{id,planId:activePlanId,name:planName,points:build.points,redo:[],line:build.points,infrastructure:referenceExtension?endpointInfrastructure(referenceExtension.alignment,referenceExtension.endpoint):infrastructure,designSpeedKmh:referenceExtension?(referenceExtension.alignment.design_speed_kmh??defaultDesignSpeed(referenceExtension.alignment.track_type??"normal")):designSpeedKmh,trackType:referenceExtension?.alignment.track_type??trackType,roadFollowing:false,extension:referenceExtension?{alignmentId:referenceExtension.alignment.id,endpoint:referenceExtension.endpoint}:null,reference:build}});
    setPlannedStations(items=>items.map(station=>station.pathId===-1?{...station,pathId:id}:station));
    const moveHistory=(entries:StationHistoryEntry[])=>entries.map(entry=>entry.planned?.pathId===-1?{...entry,planned:{...entry.planned,pathId:id}}:entry);
    setStationUndo(moveHistory);setStationRedo(moveHistory);
    setNotice("実在形状に沿う経路を選択中の計画へ追加しました。『計画を着工』で他の経路・駅とまとめて建設します。");
  }
  function destinationTarget(point:GeoPoint,width:number,height:number) {
    if(draftFinished||!draftPoints.length)return null;
    return alignments.flatMap(alignment=>[
      {alignment,endpoint:"start" as const,point:alignment.points[0]},
      {alignment,endpoint:"end" as const,point:alignment.points.at(-1)!},
    ]).filter(e=>e.point&&!(e.alignment.id===draftExtension?.alignmentId&&e.endpoint===draftExtension?.endpoint)).map(e=>({...e,distance:endpointDistance(point,e.point,width,height)}))
      .filter(e=>e.distance<=8).sort((a,b)=>a.distance-b.distance)[0]??null;
  }
  function joinError(line:GeoPoint[],source:Extension|null,destination:Extension|undefined,pathTrackType=trackType){
    return destinationJoinError(line,source,destination,alignments,world?.alignment_connections??[],world?.companies[0]?.id,pathTrackType);
  }
  function startTarget(coordinates:GeoPoint,width:number,height:number) {
    const candidates=[...(draftFinished?[{id:activePathId,points:draftPoints,extension:draftExtension,destination:draftDestination,junctionSource:draftJunctionSource}]:[]),...parkedPaths.filter(path=>path.planId===activePlanId&&!path.reference&&!path.ofmPath)]
      .filter(plan=>plan.points.length>=1)
      .flatMap(plan=>[
        {id:plan.id,endpoint:"end" as const,point:plan.points.at(-1)!,blocked:!!plan.destination},
        ...(plan.points.length>1?[{id:plan.id,endpoint:"start" as const,point:plan.points[0],blocked:!!plan.extension||!!("junctionSource" in plan&&plan.junctionSource)}]:[]),
      ]).map(end=>({...end,distance:endpointDistance(coordinates,end.point,width,height)}))
      .filter(end=>end.distance<=8).sort((a,b)=>a.distance-b.distance);
    if(candidates.length)return {kind:"plan" as const,...candidates[0]};
        const builtEnds=alignments.flatMap(alignment=>[
          {alignment,endpoint:"start" as const,point:alignment.points[0]},
          {alignment,endpoint:"end" as const,point:alignment.points.at(-1)!},
        ]).filter(item=>item.point).map(item=>({...item,distance:endpointDistance(coordinates,item.point,width,height)}))
          .filter(item=>item.distance<=8).sort((a,b)=>a.distance-b.distance);

    return builtEnds.length?{kind:"built" as const,...builtEnds[0]}:null;
  }
  const endpointCandidate=!parallelMode&&enabled&&activeTool==="tracks"&&mapMode==="draw"&&!referenceCopy.active&&!roadFollowingBlocked&&!dragging&&(draftFinished||!draftPoints.length)&&hoverPoint&&cursorPosition
    ?startTarget(hoverPoint,cursorPosition.width,cursorPosition.height):null;
  const endpointCandidateError=endpointCandidate?.kind==="plan"?(endpointCandidate.blocked?"この端点は接続済みです":null):endpointCandidate?.kind!=="built"?null:endpointCandidate.alignment.owner!==world?.companies[0]?.id?"他社の線路は延伸できません":occupiedEndpoint(world?.alignment_connections??[],{alignmentId:endpointCandidate.alignment.id,endpoint:endpointCandidate.endpoint})?"この端点は接続済みです":null;
  const junctionCandidate=!parallelMode&&enabled&&activeTool==="tracks"&&mapMode==="draw"&&!referenceCopy.active&&!roadFollowingBlocked&&!dragging&&!anchorPreview&&!endpointCandidate&&(draftFinished||!draftPoints.length)&&hoverPoint&&cursorPosition
    ?nearestJunctionSource(alignments,hoverPoint,camera,viewport.scale,world?.companies[0]?.id,infrastructure.level):null;
  const endpointCandidateAllowed=endpointCandidateError===null;
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
  async function placeParallel(point:GeoPoint){
    if(!parallelStart){
      const pick=parallelSources.map(source=>({source,p:parallelProjection(source.points,point)})).filter(item=>item.p&&item.p.distance<=PARALLEL_RANGE_M&&Math.abs(item.p.offset)>=3).sort((a,b)=>a.p!.distance-b.p!.distance)[0];
      if(!pick)return;
      const line=offsetTrack(pick.source.points,pick.p!.offset),start=line&&parallelProjection(line,point)?.point;if(!start)return;
      setParallelStart({sourceId:pick.source.id,offset:pick.p!.offset,point:start});return;
    }
    if(!parallelOffsetLine)return;
    const target=parallelProjection(parallelSource!.points,point);if(!target||target.distance>PARALLEL_RANGE_M)return;
    const geometry=parallelSection(parallelOffsetLine,parallelStart.point,point);if(!geometry)return;
    const request={name:alignmentName,points:geometry.line,anchorIndices:anchorIndices(geometry.anchors,geometry.line),trackType,designSpeedKmh,infrastructure,anchorLevels:geometry.anchors.map(()=>infrastructure.level),stations:[]};
    if(!await validatePlannedPlacement(request))return;
    newPath(true);dispatchConstruction({type:"SET_PARALLEL_PATH",geometry});setAnchorUndo([]);setAnchorRedo([]);setParallelStart(null);setHoverPoint(null);
  }

  async function validatePlannedPlacement(request:unknown,finishOnSuccess=false):Promise<boolean>{
        if(anchorPlacementPending.current)return false;
        const context=anchorPlacementContext.current;
        finishPendingPlacement.current=finishOnSuccess;
        anchorPlacementPending.current=true;setAnchorPlacementInProgress(true);
        const placementToken=++anchorPlacementToken.current;
        const waitingTimer=setTimeout(()=>{if(anchorPlacementToken.current===placementToken)setAnchorPlacementWaiting(true);},300);
        try{
          const result=await invoke('preview_construction_plan',{alignments:[request],stations:[],roads:roadAttachment.roads,validationOnly:true});
          if(anchorPlacementToken.current!==placementToken||anchorPlacementContext.current.world!==context.world||anchorPlacementContext.current.key!==context.key)return false;
          if(!hasConstructionValidation(result))throw Error('Incomplete placement verdict');
          const refusal=constructionValidationError(result);
          if(refusal){setError(refusal);return false;}
          setError('');
        }catch{
          if(anchorPlacementToken.current!==placementToken||anchorPlacementContext.current.world!==context.world||anchorPlacementContext.current.key!==context.key)return false;
          setError('敷設判定を取得できません。もう一度クリックして再試行してください。');
          return false;
        }finally{clearTimeout(waitingTimer);if(anchorPlacementToken.current===placementToken){anchorPlacementPending.current=false;setAnchorPlacementInProgress(false);setAnchorPlacementWaiting(false);}}
        return true;
  }

  async function placeTrackPoint(coordinates:GeoPoint,rect:{width:number;height:number},event:{detail:number;ctrlKey:boolean;clientX?:number;clientY?:number}) {
      if(parallelMode){if(event.detail<2)placeParallel(coordinates);return;}
      if(referenceCopy.active){void referenceCopy.click(coordinates);return;}
      if(roadFollowingBlocked)return;
      // A fresh click on the last anchor finishes the path, independent of
      // double-click timing. Ignore trailing clicks of that same gesture.
      if(draftFinished || !draftPoints.length){
        if(event.detail>=2)return;
        const target=startTarget(coordinates,rect.width,rect.height);
        if(target?.kind==="plan"){
          if(target.blocked)return;
          if(target.id===activePathId){
            if(target.endpoint==="start"){
              const reversed=reversePlanForExtension(parkCurrentPlan()!);
              applyAnchorEdit(activePathId,reversed.edit.after);
              setAnchorUndo(items=>[...items,reversed.edit]);setAnchorRedo([]);
            }
            dispatchConstruction({type:"RESUME_PATH"});setHoverPoint(null);
          }else selectPath(target.id,true,target.endpoint);
          return;
        }
        if(target?.kind==="built"){
          if(target.alignment.owner!==world?.companies[0]?.id){setError("他社の線路は延伸できません。");return;}
          if(occupiedEndpoint(world?.alignment_connections??[],{alignmentId:target.alignment.id,endpoint:target.endpoint})){setError("この端点は接続済みです");return;}
          if(draftFinished)newPath(true);

          setInfrastructure(endpointInfrastructure(target.alignment,target.endpoint));setTrackType(target.alignment.track_type??"normal");setDesignSpeedKmh(target.alignment.design_speed_kmh??defaultDesignSpeed(target.alignment.track_type??"normal"));/* Keep the selected drawing assistance when starting from built track. */
          dispatchConstruction({type:"START_PATH",point:target.point,level:endpointInfrastructure(target.alignment,target.endpoint).level,extension:{alignmentId:target.alignment.id,endpoint:target.endpoint}});setHoverPoint(null);
          setNotice("建設済み線路の"+(target.endpoint==="start"?"始点":"終点")+"から延伸を計画しています。着工まで既存線路は変更されません。");return;
        }
        const junction=nearestJunctionSource(alignments,coordinates,camera,mapViewport(rect.width,rect.height).scale,world?.companies[0]?.id,infrastructure.level);
        if(junction){if(draftFinished)newPath(true);dispatchConstruction({type:"START_PATH",point:junction.point,level:infrastructure.level,junction:junction.source});setHoverPoint(null);return;}
        dispatchConstruction({type:"CLEAR_CONNECTIONS"});
        if(draftFinished){newPath(true);dispatchConstruction({type:"START_PATH",point:coordinates,level:infrastructure.level});return;}
      }
      if(snappedEndpoint(coordinates,rect.width,rect.height)){
        if(draftPoints.length>=2&&!draftInvalid){dispatchConstruction({type:"FINISH_PATH"});setHoverPoint(null);}
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

      const priorLevels=anchorHeights(draftPoints.length,draftPoints.length?draftAnchorLevels:undefined,startLevel,infrastructure.level),nextLevel=destination?endpointInfrastructure(destination.alignment,destination.endpoint).level:infrastructure.level;
      const nextLevels=[...priorLevels,nextLevel];
      // Free/Ctrl placements while road assistance is selected must obey the
      // same exact-candidate verdict, not bypass it through normal drawing.
      if(draftPoints.length){
        const activeRequest=requests.find((_request,index)=>buildPaths[index].id===activePathId);if(!activeRequest)return;
        const request={...activeRequest,points:candidate.points,anchorIndices:anchorIndices([...draftPoints,coordinates],candidate.points),anchorLevels:nextLevels,destination:targetRef,junctionSource:draftJunctionSource&&draftPoints[0]?junctionDirection(draftJunctionSource,draftPoints[0],draftPoints[1]??coordinates):undefined,stations:plannedPlatforms.filter(item=>item.station.pathId===activePathId).map(item=>({name:stationDisplayGroup(item.station.id)?.name??item.station.name,position:nearestOnAlignment(candidate.points,item.station.point).position,lengthM:item.station.lengthM,widthM:12}))};
        pendingPlacementPoint.current={x:event.clientX??0,y:event.clientY??0};
        if(!await validatePlannedPlacement(request,event.detail>=2))return;
      }
      if((roadFollowing&&roadFinishAfterPlacement.current)||finishPendingPlacement.current){dispatchConstruction({type:"FINISH_PATH"});roadFinishAfterPlacement.current=false;finishPendingPlacement.current=false;}
      setHeightAnchor(null);
      setAnchorUndo(items=>[...items,{kind:"point" as const,id:activePathId,before:{anchorLevels:priorLevels,redoAnchorLevels:[...redoAnchorLevels,nextLevel],points:draftPoints,geometry:draftEditedGeometry,redo:[...redoPoints,coordinates],destination:draftDestination,redoDestination:targetRef??draftRedoDestination},after:{anchorLevels:nextLevels,redoAnchorLevels:[],points:[...draftPoints,coordinates],geometry:editedForAppend,redo:[],destination:targetRef}}].slice(-50));setAnchorRedo([]);dispatchConstruction({type:"ADD_POINT",point:coordinates,geometry:editedForAppend,destination:targetRef,anchorLevels:nextLevels,startLevel:infrastructure.level}); setHoverPoint(null); return;
  }
  async function placeRoadDrawing(point:GeoPoint,rect:{width:number;height:number},event:{detail:number;ctrlKey:boolean;clientX?:number;clientY?:number}) {
    if(legacyRoadDraft){setError('以前の道路追従下書きです。新しい計画から敷設してください');return;}
    if(event.detail>=2){if(roadDrawing.clickPending()){roadFinishAfterPlacement.current=true;return;}if(draftPoints.length>1)dispatchConstruction({type:"FINISH_PATH"});roadDrawing.cancel();return;}
    if(roadDrawing.clickPending())return;roadFinishAfterPlacement.current=false;
    // Existing endpoints, planned endpoints, junction selection and finishing are
    // owned by normal construction, not inferred from road geometry.
    if(((!draftPoints.length||draftFinished)&&startTarget(point,rect.width,rect.height))||snappedEndpoint(point,rect.width,rect.height)) {placeTrackPoint(point,rect,event);roadDrawing.cancel();return;}
    const captured=anchorPlacementContext.current;
    const nearEnd=(!draftFinished&&draftPoints.length)?startTarget(point,rect.width,rect.height):null;
    const destination=nearEnd?.kind==='built'?nearEnd:(!draftFinished&&draftPoints.length)?destinationTarget(point,rect.width,rect.height):null;
    const result=await roadDrawing.click({point:destination?.point??point,start:!draftFinished?draftPoints.at(-1):undefined,radiusM:20});
    const now=anchorPlacementContext.current;
    if(!result||captured.world!==now.world||captured.key!==now.key)return;
    if(result.kind==='error'){setError(result.error);return;}
    if(result.kind==='free'){placeTrackPoint(point,rect,{...event,ctrlKey:true});return;}
    if(result.kind==='point'){placeTrackPoint(result.point,rect,{...event,ctrlKey:true});return;}
    const end=destination?.point??result.point,line=[...draftLine,...result.line.slice(1)];line[line.length-1]=end;
    const points=[...draftPoints,end],geometry:EditedGeometry={version:1,anchors:points,line};
    const targetRef=destination?{alignmentId:destination.alignment.id,endpoint:destination.endpoint}:undefined;
    const invalid=plannedGeometry(points,geometry,previousPoint).error??joinError(line,draftExtension,targetRef);
    if(invalid){setError(invalid);return;}
    if(points.length>128){setError('アンカーが多いため区間を分けてください');return;}
    const levels=anchorHeights(draftPoints.length,draftAnchorLevels,startLevel,infrastructure.level),nextLevel=destination?endpointInfrastructure(destination.alignment,destination.endpoint).level:infrastructure.level;
    const activeRequest=requests.find((_request,index)=>buildPaths[index].id===activePathId);
    if(!activeRequest)return;
    const request={...activeRequest,points:line,anchorIndices:anchorIndices(points,line),anchorLevels:[...levels,nextLevel],destination:targetRef,junctionSource:draftJunctionSource&&draftPoints[0]?junctionDirection(draftJunctionSource,draftPoints[0],draftPoints[1]??end):undefined,stations:plannedPlatforms.filter(item=>item.station.pathId===activePathId).map(item=>({name:stationDisplayGroup(item.station.id)?.name??item.station.name,position:nearestOnAlignment(line,item.station.point).position,lengthM:item.station.lengthM,widthM:12}))};
    pendingPlacementPoint.current={x:event.clientX??0,y:event.clientY??0};
    if(!await validatePlannedPlacement(request))return;
    const before={points:draftPoints,geometry:draftEditedGeometry,redo:redoPoints,anchorLevels:draftAnchorLevels,redoAnchorLevels,extension:draftExtension,destination:draftDestination,redoDestination:draftRedoDestination};
    const after={points,geometry,redo:[],anchorLevels:[...levels,nextLevel],redoAnchorLevels:[],extension:draftExtension,destination:targetRef,redoDestination:undefined};
    applyAnchorEdit(activePathId,after);setAnchorUndo(history=>[...history,{kind:'point' as const,id:activePathId,before,after}].slice(-50));setAnchorRedo([]);dispatchConstruction({type:targetRef||roadFinishAfterPlacement.current?"FINISH_PATH":"RESUME_PATH"});roadFinishAfterPlacement.current=false;setHoverPoint(null);setError('');roadDrawing.cancel();
  }

  function mapClick(event: MouseEvent<SVGSVGElement>) {
    pointerFrame.cancel();
    if(anchorPlacementPending.current){const at=pendingPlacementPoint.current;if(event.detail>=2&&at&&Math.hypot(event.clientX-at.x,event.clientY-at.y)<=8){finishPendingPlacement.current=true;if(roadFollowing)roadFinishAfterPlacement.current=true;}return;}
    smoothZoom.cancel();
    if (!enabled || suppressMapClick.current || mapMode === "move") return;
    const rect = event.currentTarget.getBoundingClientRect();
    const rawCoordinates = coordinatesFromClient(event.clientX, event.clientY, rect, camera);
    if (!rawCoordinates) return;
    let coordinates:GeoPoint=rawCoordinates;
    if (activeTool === "tracks" && mapMode === "draw") {
      if(roadFollowing&&!event.ctrlKey&&!parallelMode){void placeRoadDrawing(coordinates,rect,{detail:event.detail,ctrlKey:event.ctrlKey,clientX:event.clientX,clientY:event.clientY});return;}
      placeTrackPoint(coordinates,rect,event);return;
    }
    if (activeTool !== "stations" || mapMode !== "place") return;
    if(event.detail>=2)return;
    placePlatformAt(coordinates,rect.width,rect.height);
  }

  const currentPaths=useMemo<PlannedPath[]>(()=>[...parkedPaths.filter(path=>path.planId===activePlanId),...(draftPoints.length||redoPoints.length?[{id:activePathId,planId:activePlanId,name:planName,points:draftPoints,redo:redoPoints,line:draftLine,editedGeometry:draftEditedGeometry,designSpeedKmh,trackType,infrastructure,startLevel,anchorLevels:draftAnchorLevels,redoAnchorLevels,junctionSource:directedJunction,roadFollowing:legacyRoadDraft,extension:draftExtension,destination:draftDestination,redoDestination:draftRedoDestination}]:[])],[constructionPlans,draftLine]);
  const currentStations=useMemo(()=>plannedPlatforms.filter(item=>stationPlanId(item.station)===activePlanId),[plannedPlatforms,activePlanId,activePathId,parkedPaths]);
  const planHasContent=currentPaths.length>0||currentStations.length>0;
  // Keep fully undone paths for Redo, but do not submit them as empty alignments.
  const buildPaths=useMemo(()=>currentPaths.filter(path=>path.points.length>0),[currentPaths]);
  const planBuildable=useMemo(()=>(buildPaths.length>0||currentStations.some(item=>item.station.alignmentId))&&currentStations.every(item=>item.station.pathId===undefined||buildPaths.some(path=>path.id===item.station.pathId))&&buildPaths.every(path=>path.line.length>=2&&!((path.reference||path.ofmPath)?extensionJoinError(path.points,extensionPrevious(path.extension)):plannedGeometry(path.points,path.editedGeometry,extensionPrevious(path.extension)).error)&&!joinError(path.line,path.extension,path.destination,path.trackType)&&!(path.roadFollowing)),[buildPaths,currentStations,extensionPrevious,world,trackType]);
  const requests=useMemo(()=>buildPaths.map(path=>({name:alignmentName,points:path.line,anchorIndices:anchorIndices(path.points,path.line),trackType:path.trackType,designSpeedKmh:path.designSpeedKmh,infrastructure:path.infrastructure??DEFAULT_INFRASTRUCTURE,startLevel:path.startLevel,anchorLevels:path.anchorLevels,junctionSource:path.junctionSource,extension:path.extension??undefined,destination:path.destination,reference:path.reference?{sourceRecord:path.reference.sourceRecord,provenance:path.reference.provenance}:undefined,ofmPath:path.ofmPath,stations:plannedPlatforms.filter(item=>item.station.pathId===path.id).map(item=>({name:stationDisplayGroup(item.station.id)?.name??item.station.name,position:item.position,lengthM:item.station.lengthM,widthM:12}))})),[buildPaths,plannedPlatforms,stationGroupDisplay]);
  const stations=useMemo(()=>currentStations.filter(item=>item.station.alignmentId).map(item=>({alignmentId:item.station.alignmentId!,name:stationDisplayGroup(item.station.id)?.name??item.station.name,position:item.position,lengthM:item.station.lengthM,widthM:12})),[currentStations,stationGroupDisplay]);
  const constructionRequest=useMemo(()=>({alignments:requests,stations}),[requests,stations]);
  const roadCheckAbort=useRef<AbortController|null>(null);
  const [roadCheckPending,setRoadCheckPending]=useState(false);
  const buildIdentity=useMemo(()=>JSON.stringify(constructionRequest),[constructionRequest]);
  const latestBuild=useRef({world,key:buildIdentity});latestBuild.current={world,key:buildIdentity};
  const constructionRoad=useAutomaticRoad(constructionRoadBounds(buildPaths,bounds),status==="ready"&&buildPaths.length>0,worldEpoch);
  const roadAttachment=useMemo(()=>constructionRoadAttachment(constructionRoad.snapshot,world),[constructionRoad.snapshot,world]);
  const roadChecks=useRoadAssessments(world,status==="ready");
  const roadCoverageUnknown=roadChecks.value.filter(([,r])=>!r.roads_ready).length;
  const roadHeightUnknown=roadChecks.value.filter(([,r])=>r.unknown_road_height).length;
  const constructionRoadReason=buildPaths.length>0&&(constructionRoad.status!=='ready'||roadAttachment.roads.some(r=>!r.coverage_verified))?(constructionRoad.status==='ready'?roadAttachment.reason:constructionRoad.message):'';
  const crossingCount=roadChecks.value.reduce((n,[,r])=>n+r.crossings.length,0);
  const roadStatusReason=constructionRoadReason||(roadChecks.error?`踏切判定未取得：${roadChecks.error}`:roadHeightUnknown?'道路の高さを確認できない区間があります':roadCoverageUnknown?'道路範囲を確認できない線路があります':alignments.length&&!world?.road_snapshots?.length?'道路参照が未保存のため踏切未確認です':'');
  const roadCursorPreview=roadFollowing&&!draftFinished&&draftPoints.length>0&&roadDrawing.preview?.kind==='route'?roadDrawing.preview:null;
  const cursorLine=useMemo(()=>roadCursorPreview?[...draftLine,...roadCursorPreview.line.slice(1)]:previewLine,[roadCursorPreview,draftLine,previewLine]);
  const cursorAnchors=useMemo(()=>roadCursorPreview?[...draftPoints,roadCursorPreview.point]:previewAnchors,[roadCursorPreview,draftPoints,previewAnchors]);
  const roadPreviewGeometry=useMemo(()=>{if(!roadCursorPreview)return null;const anchors=[...draftPoints,roadCursorPreview.point],line=[...draftLine,...roadCursorPreview.line.slice(1)];return plannedGeometry(anchors,{version:1,anchors,line},previousPoint);},[roadCursorPreview,draftPoints,draftLine,previousPoint]);
  const drawingPreviewError=roadPreviewGeometry?.error??(roadCursorPreview?null:previewError);
  const drawingPreviewSpeed=roadPreviewGeometry?Math.min(designSpeedKmh,...roadPreviewGeometry.arcs.map(arc=>provisionalCurveSpeed(arc.radiusM)??0)):Math.min(designSpeedKmh,circleGuide?(provisionalCurveSpeed(circleGuide.radiusM)??designSpeedKmh):designSpeedKmh);
  const cursorPreviewActive=!!roadCursorPreview||!planHover&&activeTool==="tracks"&&mapMode==="draw"&&!parallelMode&&!referenceCopy.active&&previewAnchors!==draftPoints&&previewAnchors.length>=2&&!previewError;
  const cursorDestination=useMemo(()=>cursorPreviewActive&&hoverPoint&&cursorPosition?destinationTarget(hoverPoint,cursorPosition.width,cursorPosition.height):null,[cursorPreviewActive,hoverPoint,cursorPosition?.width,cursorPosition?.height,draftFinished,draftPoints,alignments,draftExtension,camera]);
  const previewRequests=useMemo(()=>cursorPreviewActive?requests.map((request,index)=>buildPaths[index].id!==activePathId?request:{...request,points:cursorLine,anchorIndices:anchorIndices(cursorAnchors,cursorLine),anchorLevels:[...anchorHeights(draftPoints.length,draftAnchorLevels,startLevel,infrastructure.level),cursorDestination?endpointInfrastructure(cursorDestination.alignment,cursorDestination.endpoint).level:infrastructure.level],destination:cursorDestination?{alignmentId:cursorDestination.alignment.id,endpoint:cursorDestination.endpoint}:undefined,stations:plannedPlatforms.filter(item=>item.station.pathId===activePathId).map(item=>({name:stationDisplayGroup(item.station.id)?.name??item.station.name,position:nearestOnAlignment(cursorLine,item.station.point).position,lengthM:item.station.lengthM,widthM:12}))}):requests,[cursorPreviewActive,requests,buildPaths,activePathId,cursorLine,cursorAnchors,draftPoints,draftAnchorLevels,startLevel,infrastructure,cursorDestination,plannedPlatforms,stationGroupDisplay]);
  const previewRequestValid=cursorPreviewActive?previewRequests.every(request=>request.points.length>=2):planBuildable;
  const waterModelRevision=useWaterModelAcquisition(status==="ready",world,waterAcquisitionBounds(previewRequests.flatMap(r=>r.points),camera));
  const nativeConstructionPreview=useConstructionPreview(world,status==="ready"&&previewRequestValid?{alignments:previewRequests,stations,roads:roadAttachment.roads}:null,waterModelRevision);
  const nativeValidationError=constructionValidationError(nativeConstructionPreview?.value);
  const [anchorPlacementWaiting,setAnchorPlacementWaiting]=useState(false);
  const [anchorPlacementInProgress,setAnchorPlacementInProgress]=useState(false);
  const anchorPlacementPending=useRef(false);
  const anchorPlacementToken=useRef(0);
  const finishPendingPlacement=useRef(false);
  const pendingPlacementPoint=useRef<{x:number;y:number}|null>(null);
  // Preserve intent by reference, without serializing full geometry on every hover.
  const placementContextKey=useMemo(()=>({}),[enabled,activePlanId,activePathId,activeTool,mapMode,draftPoints,draftEditedGeometry,draftAnchorLevels,draftFinished,draftExtension,draftDestination,infrastructure,trackType,designSpeedKmh,roadFollowing,parallelMode,parallelStart,referenceCopy.active,plannedStations,alignmentName]);
  const anchorPlacementContext=useRef<{world:unknown;key:unknown}>({world,key:placementContextKey});
  anchorPlacementContext.current={world,key:placementContextKey};
  useEffect(()=>{anchorPlacementPending.current=false;anchorPlacementToken.current++;finishPendingPlacement.current=false;setAnchorPlacementInProgress(false);setAnchorPlacementWaiting(false);},[world,placementContextKey]);
  useEffect(()=>()=>{anchorPlacementContext.current={world:null,key:'unmounted'};},[]);

  function commitAlignment() {
    if (!enabled || busyRef.current || !planBuildable || anchorGesture.current || anchorPlacementPending.current) return;
    const trace=startConstructionTrace();
    void perform(async () => {
      const endPrepare=trace?.span("prepare");
      const ids=new Set(buildPaths.map(path=>path.id));
      if(currentStations.some(item=>!item.valid))throw Error("計画駅が線路に収まりません。線形を元に戻すか、計画駅を取り消してください。");
      const {alignments:requests,stations}=constructionRequest;
      const first=requests[0];
      const batch=stations.length>0||requests.length>1||requests.some(request=>request.extension||request.destination||request.reference||request.ofmPath||request.junctionSource);
      let attachment=roadAttachment;
      let roadConfirmation='';endPrepare?.();
      if(requests.length){
        setNotice("道路を確認中（最大25秒）。未取得の場合も通常建設を続行します。");
        const controller=new AbortController();roadCheckAbort.current=controller;setRoadCheckPending(true);
        let loaded:Awaited<ReturnType<typeof constructionRoad.ensureCurrent>>;
        const endRoad=trace?.span("road.ensureCurrent");
        try{loaded=await constructionRoad.ensureCurrent(controller.signal);}
        finally{endRoad?.();roadCheckAbort.current=null;setRoadCheckPending(false);}
        if(controller.signal.aborted){setNotice("着工を取り消しました。下書きは保持しています。");return;}
        if(loaded.status==='stale'||latestBuild.current.world!==world||latestBuild.current.key!==buildIdentity)throw Error("計画またはワールドが変更されたため着工を中止しました。");
        const endAttachment=trace?.span("road.attachment");
        attachment=constructionRoadAttachment(loaded.status==='ready'?loaded.snapshot:undefined,world);
        endAttachment?.();
        roadConfirmation=loaded.status==='unavailable'?`踏切未確認：${loaded.reason.replace(/^道路未確認：/,'')}。後から道路を取得しただけでは既設線路へ反映されません。`:attachment.reason;
      }
      const roads=attachment.roads;
      const snapshot=await timeConstructionIPC("build",()=>invoke<World>(roads.length?"build_plan_with_roads":batch?"build_plan":first.stations.length?"build_alignment_with_stations":"build_alignment",roads.length?{alignments:requests,stations,roads}:batch?{alignments:requests,stations}:first.stations.length?first:{name:first.name,points:first.points,anchorIndices:first.anchorIndices,trackType:first.trackType,designSpeedKmh:first.designSpeedKmh,infrastructure:first.infrastructure,startLevel:first.startLevel,anchorLevels:first.anchorLevels}));
      const endApply=trace?.span("world.apply-state");
      if(trace){trace.worldScheduled=true;constructionCommit.current={world:snapshot,trace,end:trace.span("world.schedule-to-layout")};}
      ids.forEach(discardPlanStations);setPlannedStations(items=>items.filter(station=>stationPlanId(station)!==activePlanId));dispatchConstruction({type:"CLEAR_PLAN"});
      const created=snapshot.alignments?.find(a=>!alignments.some(old=>old.id===a.id));
      setWorld(snapshot);setDirty(true);setConfirmLoad(false);dispatchConstruction({type:"RESUME_PATH"});setHeightAnchor(null);setAnchorUndo([]);setAnchorRedo([]);setHoverPoint(null);dispatchConstruction({type:"CLEAR_CONNECTIONS"});
      const saved=savedPlanFingerprint,remaining=removeConstructionPlan(constructionPlans,activePlanId);
      const nextPlans:ConstructionPlans={...remaining,redoPoints:[],redoAnchorLevels:[],draftRedoDestination:undefined,parkedPaths:remaining.parkedPaths.map(path=>({...path,redo:[],redoAnchorLevels:[],redoDestination:undefined}))};
      restoreConstructionPlans(nextPlans);setSavedPlanFingerprint(saved);
      constructionUndoFloor.current=new Map([[nextPlans.activePathId,nextPlans.draftPoints.length],...nextPlans.parkedPaths.map((path):[number,number]=>[path.id,path.points.length])]);
      pendingPathHistory.current=null;clearStationHistory();setTrackDeleteUndo(null);setTrackDeleteRedo(null);
      if(created)setAlignmentId(created.id);
      changeMapMode(constructionMapMode(activeTool));setNotice((requests.length?"線路を建設しました。計画駅も反映しました。":"計画駅を建設しました。")+(roadConfirmation?` ${roadConfirmation}`:""));
      endApply?.();
    },trace);
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
  function undoStation(){ stepEditHistory(false); }
  function redoStation(){ stepEditHistory(true); }
  useKeyboardPan((dx,dy)=>changeCamera(panCamera(cameraRef.current,dx,dy)),
    ()=>!!(seamMutationPending.current||busyRef.current||slotDialog||slotPicker||confirmLoad||deleteTarget||confirmRangeDelete||gestureRef.current||anchorGesture.current||boxGesture.current));
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if(seamMutationPending.current)return;
      if(slotDialog||slotPicker||confirmLoad)return;
      if (event.key === "Escape" && routeMapSelect.current) {event.preventDefault();cancelRouteMapPick();return;}
      if(deleteTarget||confirmRangeDelete||document.querySelector('[aria-label="系統削除の確認"]'))return;
      if (event.target instanceof HTMLElement && (event.target.isContentEditable || /INPUT|SELECT|TEXTAREA/.test(event.target.tagName))) return;
      if (event.key === "Delete" && !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey && !event.isComposing && event.keyCode !== 229 && !event.repeat && !event.defaultPrevented) {
        if (mainTab === "construction" && activeTool === "select" && enabled && !busyRef.current && !unapplied && rangeCount > 0 && !boxGesture.current && !gestureRef.current && !anchorGesture.current && !document.querySelector('dialog[open], [role="dialog"]')) {
          event.preventDefault();setError("");setConfirmRangeDelete(true);
        }
        return;
      }
      if (!event.ctrlKey && !event.metaKey && !event.altKey && !event.isComposing && (event.key === "+" || event.key === "-" || event.code === "NumpadAdd" || event.code === "NumpadSubtract")) {
        event.preventDefault();
        zoomFromControls(event.key === "+" || event.code === "NumpadAdd" ? 1 : -1);
        return;
      }
      if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase()==="z") {
        event.preventDefault();if(event.repeat || busyRef.current)return;
        if(parallelStart){if(!event.shiftKey)cancelParallel();return;}
        if(anchorGesture.current){cancelAnchorGesture();return;}
        if(referenceCopy.active){stepReferenceHistory(event.shiftKey);return;}
        stepEditHistory(event.shiftKey);return;
      }
      if (event.key === "Escape" && anchorGesture.current) {event.preventDefault();cancelAnchorGesture();return;}
      if (event.key === "Escape") {roadDrawing.cancel();cancelParallel(); if(activeTool==="tracks")setActiveTool("inactive"); changeMapMode("move"); setStationCursor(null); }
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
      pointerFrame.cancel();
      if(event.target instanceof HTMLElement&&(event.target.isContentEditable||/INPUT|SELECT|TEXTAREA/.test(event.target.tagName)))return;
      if(roadFollowing&&event.type==="keydown")roadDrawing.cancel();
      if(roadFollowing&&event.type==="keyup"&&drawingCursor.current)roadDrawing.hover({point:drawingCursor.current.point,start:!draftFinished?draftPoints.at(-1):undefined,radiusM:20});
      refresh(event.type==="keydown");
    };
    const blur=()=>{pointerFrame.cancel();stationInputCursor.current=null;drawingCursor.current=null;setHoverPoint(null);};
    window.addEventListener("keydown",modifier);window.addEventListener("keyup",modifier);window.addEventListener("blur",blur);
    return ()=>{window.removeEventListener("keydown",modifier);window.removeEventListener("keyup",modifier);window.removeEventListener("blur",blur);};
  });
  function resetRoute(route?: Route) {
    setRouteId(route?.id ?? null); setRouteName(route?.name ?? "新路線");
    setRouteStart(route?.start ?? "");
    setRouteHint("");
  }
  function chooseRoute(id: string | null) {
    if (!enabled) return;
    if (candidateDirty || routeMapEdit) {setRouteHint("先に地図選択の候補を適用するか、変更を取り消してください。");return;}
    resetRoute(world?.routes.find((route) => route.id === id));
  }
  function startCompany(event:FormEvent){
    event.preventDefault();if(status!=="ready"||busy||!companyName.trim())return;
    void perform(async()=>{const snapshot=await invoke<World>("rename_company",{name:companyName.trim()});setWorld(snapshot);setCompanyName(snapshot.companies[0]?.name??"");setCompanySetupRequired(false);setDirty(true);const saved=await invoke<{saved:boolean}>("save_world_as",{name:companyName.trim()});if(!saved.saved)throw Error("初回保存に失敗しました。編集中の内容は保持しています。");await refreshSaveSlots();setDirty(false);setNotice("会社を設定しました。自動保存を開始します。");});
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
  async function renameSavedRoute(id:string,name:string,expectedName:string) {
    if(!enabled||busyRef.current||candidateDirty||routeCreating||routeMapEdit)throw new Error("先に編集中の路線を確定または取り消してください。");
    busyRef.current=true;setBusy(true);
    try {
      const snapshot=await invoke<World>("rename_route",{routeId:id,name,expectedName});
      setWorld(snapshot);setDirty(true);setConfirmLoad(false);
      if(routeId===id)resetRoute(snapshot.routes.find(route=>route.id===id));
    } finally {busyRef.current=false;setBusy(false);}
  }
  async function applyPlannedRoute(name: string, membership: Membership) {
    if (!enabled) throw new Error("先に路線下書きを適用または破棄してください。");
    if (busyRef.current) throw new Error("処理中です。");
    busyRef.current=true;setBusy(true);
    try {
      const snapshot=await invoke<World>("save_planned_route",{routeId:routeMapEdit?.id??null,name,start:membership.path.stations[0],segments:membership.path.segments,excludedStations:membership.excluded_stations});
      const created=routeMapEdit?snapshot.routes.find(route=>route.id===routeMapEdit.id):snapshot.routes.find(route=>!world?.routes.some(old=>old.id===route.id));
      setRouteMapEdit(null);setRouteSequenceKey(0);setRouteCreating(false);
      setWorld(snapshot);setDirty(true);setConfirmLoad(false);resetRoute(created);setNotice("路線と所属駅を適用しました。ファイルへの保存は「保存」で行います。");
    } finally {busyRef.current=false;setBusy(false);}
  }
  function resetService(service?: OperatingService) {
    setServiceCreating(false);setRouteServiceDirty(false);cancelRouteMapPick();
    setServiceAppearanceDirty(false);
    setBindingDirty(false);
    setServiceId(service?.id ?? null);
    setServiceStart(service?.start ?? "");
    setServiceDirty(false); setServiceHint(""); setPendingServiceChoice(null);
  }
  function chooseService(id: string | null) {
    if (!enabled) return;
    if(serviceAppearanceDirty){setServiceHint("先に系統色の変更を適用または取り消してください。");return;}
    if(bindingDirty){setServiceHint("先に関連付けの変更を適用または破棄してください。");return;}
    if(pathServiceDirty){setServiceHint("先に新形式下書きを破棄または適用してください。");return;}
    if (serviceDirty) { setPendingServiceChoice({ id }); return; }
    resetService(world?.services.find((service) => service.id === id));
  }
  async function createPathService(draft:CreateServicePath){
    if(!enabled||busyRef.current||serviceDirty||serviceId!==null)throw Error("別の系統を編集中です。");
    busyRef.current=true;setBusy(true);try{const snapshot=await invoke<World>("create_service_path",draft);setWorld(snapshot);setDirty(true);setPathServiceDirty(false);setNotice("系統を作成しました。ファイルへは保存してください。");}finally{busyRef.current=false;setBusy(false);}
  }
  function completeSave() {
    setSavedViewFingerprint(JSON.stringify(camera));setSavedPlanFingerprint(planFingerprint);
    setDirty(false);setStationUndo(history=>history.filter(entry=>entry.planned));setStationRedo(history=>history.filter(entry=>entry.planned));
    setNotice(referenceCopy.active?"ワールドと未着工計画を保存しました。選択中の参照候補は計画に追加するまで保存されません。":"ワールドと未着工計画を保存しました。");
  }
  async function refreshSaveSlots(){
    const generation=++slotGeneration.current;setSlotsLoading(true);setSlotsError("");
    try{
      const result=await invoke<{active_slot_id:string|null;slots:SaveSlot[];deleted_slots?:SaveSlot[]}>("list_save_slots");
      if(!Array.isArray(result?.slots)||!(result.active_slot_id===null||typeof result.active_slot_id==="string"))throw Error("保存一覧を確認できません。");
      if(generation!==slotGeneration.current)return;
      setSaveSlots(result.slots);setDeletedSaveSlots(result.deleted_slots??[]);setActiveSlotId(result.active_slot_id);setSlotsReady(true);
    }catch(error){if(generation===slotGeneration.current){setSlotsError(detail(error));setSlotsReady(false);}}
    finally{if(generation===slotGeneration.current)setSlotsLoading(false);}
  }
  useEffect(()=>{if(status==="ready")void refreshSaveSlots();return()=>{slotGeneration.current++;};},[status]);
  async function moveSavedWorld(id:string,restore:boolean){
    if(busyRef.current||status!=="ready")throw Error("ほかの処理が完了してから操作してください。");
    busyRef.current=true;setBusy(true);
    try{
      await invoke("move_save_slot",{slotId:id,restore});
      if(!restore&&id===activeSlotId){setActiveSlotId(null);setDirty(true);setWorldEpoch(value=>value+1);}
      await refreshSaveSlots();
      setNotice(restore?"セーブデータを復元しました。":"セーブデータを削除しました。削除済みデータから復元できます。");
    }finally{busyRef.current=false;setBusy(false);}
  }
  function openSlotPicker(){setSlotPicker(true);void refreshSaveSlots();}
  function requestSlotAction(action:{kind:"load";id:string}|{kind:"new";name:string}){
    if(busyRef.current)return;
    setSlotAction(action);
    if(dirty||plansDirty||viewDirty||unapplied){setConfirmLoad(true);return;}
    applySlotAction(action);
  }
  function applySlotAction(action=slotAction){
    if(!action)return;setConfirmLoad(false);
    void perform(async()=>{
      const snapshot=await invoke<World>(action.kind==="load"?"load_save_slot":"create_new_world",action.kind==="load"?{slotId:action.id}:{companyName:action.name});
      acceptLoadedWorld(snapshot,"manual");
      setDirty(action.kind==="new");setSlotAction(null);setSlotDialog(null);setSlotPicker(false);
      setNotice(action.kind==="new"?"新しいワールドを作成しました。初回保存で別のセーブを作ります。":"保存したワールドを読み込みました。");
      if(action.kind==="new"){setActiveSlotId(null);const saved=await invoke<{saved:boolean}>("save_world_as",{name:action.name});if(!saved.saved)throw Error("初回保存に失敗しました。編集中の内容は保持しています。");setDirty(false);setNotice("新しいワールドを作成しました。自動保存を開始します。");}
      await refreshSaveSlots();
    });
  }
  function submitSlotName(event:FormEvent){
    event.preventDefault();if(busyRef.current||!slotName.trim())return;
    if(slotDialog==="new"){requestSlotAction({kind:"new",name:slotName.trim()});return;}
    if(saveBlocked)return;
    void perform(async()=>{
      const result=await invoke<{saved:boolean}>("save_world_as",{name:slotName.trim(),constructionPlans:persistedPlans,viewState:camera});
      if(!result.saved)throw Error("保存結果を確認できませんでした。編集中の内容は保持しています。");
      completeSave();setSlotDialog(null);setWorldEpoch(value=>value+1);await refreshSaveSlots();
      setNotice("別のセーブとして保存しました。元の保存データは変更していません。");
    });
  }
  function save() {
    if (saveBlocked) return;
    void perform(async () => {
      const result=await invoke<{saved:boolean;replacement_required?:{token:string;company_name:string}}>("save_world",{constructionPlans:persistedPlans,viewState:camera}).catch(error=>{if(detail(error).includes("save file changed since it was observed"))throw Error("保存先が読み込み時から変更されています。編集中の内容は保持しています。未保存のまま読込や終了をせず、保存先を確認してください。");throw error;});
      if(result.replacement_required)throw Error("既存セーブの置き換えは行いません。新しい保存先を確認してください。");
      if(!result.saved)throw Error("保存結果を確認できませんでした。編集中の内容は保持しています。");
      completeSave();setWorldEpoch(value=>value+1);await refreshSaveSlots();
    });
  }
  function acceptLoadedWorld(snapshot:World,source:"manual"|"auto"){
      setRailLoadEpoch(value=>value+1);
      cancelMapGesture();cancelAnchorGesture();cancelParallel();cancelRouteMapPick();
      setRouteMapEdit(null);setRouteSequenceKey(0);setRouteCreating(false);setDeleteTarget(null);setTrackDeleteRedo(null);
      setPlanHover(null);setBuiltHover(null);
      restoreView(snapshot.view_state);
      setRestoredAutosaveEpoch(source==="auto"?worldEpoch+1:-1);
      setWorldEpoch(value=>value+1);
      referenceCopy.clear();setCandidateDirty(false);setPendingGraphChange(null);
      setCompanySetupRequired(false);
      clearStationHistory();
      setTrackDeleteUndo(null);setSelectedPlanPath(null);setTrackDeleteConfirm(false);setPathDraftEpoch(value=>value+1);setPathServiceDirty(false);setWorld(snapshot); setDirty(source==="auto"); restoreLoadedPlans(snapshot);setStationCursor(null); setHoverPoint(null); setStationCursor(null); setAlignmentId(""); setSelected(null); setEditStationName(""); setPendingStation(null); setPendingDelete(null); setCompanyName(snapshot.companies[0]?.name ?? ""); resetRoute(); resetService();
  }
  function requestLoad(source:"manual"|"auto"){setSlotAction(null);setLoadSource(source);if(dirty||plansDirty||viewDirty||unapplied)setConfirmLoad(true);else load(source);}
  function load(source: "manual"|"auto" = loadSource) {
    setConfirmLoad(false);
    void perform(async () => {
      if(!activeSlotId)throw Error("読み込むセーブを選択してください。");
      const snapshot = await invoke<World>(source==="auto"?"load_autosave":"load_save_slot",{slotId:activeSlotId});
      acceptLoadedWorld(snapshot,source);await refreshSaveSlots();
      setNotice(source==="auto"?"自動保存から復元しました。手動保存データは上書きしていません。必要なら保存してください。":"保存したワールドを読み込みました。");
    });
  }

  const workbenchStyle:CSSProperties & Record<`--${string}`,string|number>={"--rail-width":lineWidth,"--map-contrast":contrast/100,"--panel-width":panelWidth+"px","--ui-font":textSize+"px"};
  const showPlanHover=!!planHover&&!!hoveredPlan&&!referenceCopy.active&&!dragging&&(activeTool==="tracks"||activeTool==="inactive");
  return (
    <main className="workbench" data-theme={theme} style={workbenchStyle}>
      <header className="app-header">
        <div className="session-status"><span className={`connection-dot ${status}`} aria-hidden="true" />{status === "ready" ? `${info?.version} / エンジン接続済み` : status === "preview" ? "ブラウザプレビュー" : status === "loading" ? "接続中" : "接続エラー"}</div>
      </header>
      <div className="toolbar">
        <div><span className="eyebrow">CONSTRUCTION LAB</span><h2>線路を描き、街をつなぐ。</h2></div>
        <div className="save-controls"><span className="save-state">{unapplied ? "未適用の下書きあり" : dirty||plansDirty ? "未保存の変更あり" : "ローカルワールド"}</span><button className="secondary" disabled={!enabled} onClick={()=>activeSlotId?requestSlotAction({kind:"load",id:activeSlotId}):openSlotPicker()}>読込</button><button disabled={!enabled || saveBlocked} onClick={save}>保存</button></div>
      </div>
      {closeGuard.confirmClose && <UnsavedCloseDialog error={closeGuard.closeError} closing={closeGuard.closing} onCancel={closeGuard.cancelClose} onDiscard={()=>void closeGuard.discardAndClose()} />}
      {pendingGraphChange && <div className="banner load-confirm" role="alert"><p>参考線データを変更すると、現在の実在形状に沿う敷設・道路追従の未着工計画を破棄します。変更しますか。</p><button className="secondary" onClick={()=>setPendingGraphChange(null)}>計画に戻る</button><button disabled={!enabled} onClick={()=>{referenceCopy.clear();setGraphBundle(pendingGraphChange.bundle);setPendingGraphChange(null);}}>参考計画を破棄して変更</button></div>}

      {pendingDelete && <div className="banner load-confirm" role="alert"><p>{pendingDelete.kind === "station" ? "駅「" + (stationsById.get(pendingDelete.id)?.name ?? pendingDelete.id) + "」"+(world?.station_groups?.some(g=>g.members.includes(pendingDelete.id))?"の選択ホーム":"") : "線路 ID " + pendingDelete.id}を削除しますか。連鎖削除は行いません。適用後は保存が必要です。</p><button className="secondary" disabled={busy} onClick={() => setPendingDelete(null)}>削除をやめる</button><button className="danger" disabled={!enabled || unapplied || !!(stationReferences(pendingDelete.id))} onClick={deleteEquipment}>削除を確定</button></div>}
      <div className="workspace">
        <section className="map-panel" aria-labelledby="map-title">
          <div className="map-notices" aria-label="通知">
            {planRecoveryNotice&&<p role="status">{planRecoveryNotice}</p>}
      {closeGuard.closeError && !closeGuard.confirmClose && <div className="banner" role="alert">{closeGuard.closeError}</div>}

      {status === "preview" && <p className="banner">ブラウザでの画面確認です。表示はサンプルデータで、駅の配置・線路接続・保存・読込はデスクトップ版で利用できます。</p>}
      {status === "loading" && <p className="banner" role="status">共通エンジンからワールドを読み込んでいます。</p>}
      {status === "error" && <div className="banner error"><p>共通エンジンに接続できませんでした。</p><button onClick={() => setAttempt((x) => x + 1)}>再試行</button></div>}
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
            {selectedTrackExists&&mapMode==="move"&&<>{!(mainTab==="construction"&&activeTool==="inactive")&&<span>{selectedPlanPath!==null?"計画線路":selectedAlignment?.name?.trim()||"名前未設定の線路"}</span>}<button type="button" className="danger" disabled={!enabled||unapplied||!!selectedTrackReferences} onClick={()=>setTrackDeleteConfirm(true)}>選択線路を削除</button>{selectedTrackReferences&&<small>削除不可：{selectedTrackReferences}が使用中</small>}</>}
            <button type="button" className="secondary" disabled={!enabled||saveBlocked||!(referenceCopy.active?referenceHistoryAvailable.undo:historyAvailability.undo)} onClick={()=>referenceCopy.active?stepReferenceHistory(false):stepEditHistory(false)}>元に戻す</button>
            <button type="button" className="secondary" disabled={!enabled||saveBlocked||!(referenceCopy.active?referenceHistoryAvailable.redo:historyAvailability.redo)} onClick={()=>referenceCopy.active?stepReferenceHistory(true):stepEditHistory(true)}>やり直す</button>
          </div>
          {trackDeleteConfirm&&selectedTrackExists&&<TrackDeleteDialog busy={busy} blocked={!enabled||unapplied||!!selectedTrackReferences} error={error} onCancel={()=>setTrackDeleteConfirm(false)} onDelete={deleteSelectedTrack}/>}
          <div className="camera-controls" aria-label="地図表示操作">
            <div className="map-mode-controls" role="group" aria-label="地図の操作モード"><button className="secondary" disabled={activeTool === "tracks"} aria-pressed={mapMode === "move"} onClick={() => { changeMapMode("move"); }}>移動</button><button className="secondary" aria-pressed={mapMode === "place"} disabled={!enabled || activeTool !== "stations"} onClick={() => { changeMapMode("place"); }}>駅配置</button><span>ドラッグで移動 / 縦ホイールでカーソル位置をズーム</span></div>
            <p>閲覧・建設範囲: 北緯20〜46度 / 東経122〜154度。領土境界を示すものではありません。範囲端では表示を矩形内に制限します。</p>
          </div>
          {roadFollowing?<label><input type="checkbox" aria-label="道路参考線を表示" checked={graphVisible} onChange={event=>setGraphVisible(event.target.checked)}/>道路参考線を表示</label>:<ReferenceGraphControls bundle={graphBundle} visible={graphVisible} onBundle={bundle=>{if(referenceCopy.active)setPendingGraphChange({bundle});else setGraphBundle(bundle);}} onVisible={setGraphVisible} total={graphDisplay.total} shown={graphDisplay.edges.length} zoom={camera.zoom}/>}
          <ReferenceStationControls pack={referencePack} visible={referenceVisible} onPack={setReferencePack} onVisible={setReferenceVisible} total={referenceResult.total} shown={referenceResult.stations.length}/>
          </details>
            <div className="map-surface">
            {trackRender.error&&<div role="status" className="field-error">ポイント形状を表示できません: {trackRender.error}</div>}
            {confirmRangeDelete&&<DeletionDialog defaultDelete label={`選択した${rangeCount}区間`} busy={busy} blocked={unapplied?"編集中の変更を適用または取り消してください":rangeCount?"":"区間が選択されていません"} error={error} onCancel={()=>setConfirmRangeDelete(false)} onDelete={deleteRange}/>}
            {deleteTarget&&<DeletionDialog defaultDelete={deleteTarget.kind==='built-section'||deleteTarget.kind==='plan-section'} label={deletionLabel(deleteTarget)} busy={busy} blocked={deletionBlocked(deleteTarget)} error={error} onCancel={()=>setDeleteTarget(null)} onDelete={confirmDeletion}/>}
            {activeTool==="delete"&&deleteHover&&!dragging&&<div ref={deletionTooltipRef} className="construction-tooltip deletion-tooltip" role="status" style={{left:Math.max(8,Math.min(mapSize.width-240,deleteHover.x+18)),top:Math.max(8,Math.min(mapSize.height-80,deleteHover.y+18))}}>{deletionLabel(deleteHover.target)}を{deleteHover.target.kind==="built-section"||deleteHover.target.kind==="station"?"撤去":"削除"}</div>}
            {activeTool==="stations"&&<fieldset className="construction-toolbar station-toolbar" disabled={!enabled} aria-label="駅設置設定"><PlatformLengthInput id="platform-length" label="駅の長さ" value={platformLength} onChange={setPlatformLength} compact/></fieldset>}
            {activeTool==="stations"&&(hoveredStation||stationJoinPreview)&&cursorPosition&&<div className="construction-tooltip" role="status" style={{left:Math.max(8,Math.min(mapSize.width-240,cursorPosition.x+18)),top:Math.max(8,Math.min(mapSize.height-80,cursorPosition.y+18))}}>{stationJoinPreview?.name??hoveredStation?.name}{stationJoinPreview&&stationJoinOrigin&&stationSelection?.target?<div>{stationJoinPreview.name}から {(distanceM(stationJoinOrigin,stationSelection.target.center)/1000).toFixed(2)} km</div>:hoveredStation?.lengthM!==undefined&&<> · {hoveredStation.lengthM/20}両</>}</div>}
            {activeTool==="stations"&&mapMode==="place"&&!dragging&&cursorPosition&&stationCursor&&!hoveredStation&&stationSelection?.error&&<div className="construction-tooltip station-placement-error" role="status" style={{left:Math.max(8,Math.min(mapSize.width-240,cursorPosition.x+18)),top:Math.max(8,Math.min(mapSize.height-80,cursorPosition.y+18))}}>{stationSelection.error}</div>}
            {builtHeightSelection&&<BuiltAnchorHeightToolbar selection={builtHeightSelection} disabled={!enabled||unapplied} onChange={updateBuiltAnchorHeight}/>}
            {!builtHeightSelection&&activeTool!=="tracks"&&selectedHeightPath&&heightAnchor&&<div className="construction-toolbar" role="group" aria-label="アンカーの高さ"><HeightControls value={selectedHeight??infrastructure.level} disabled={!enabled} onChange={changeHeight}/></div>}
            {nativeConstructionPreview?.error&&activeTool==="tracks"&&<div role="status" className="construction-tooltip">敷設判定を取得できません。着工時に再確認します。</div>}
            {nativeValidationError&&((activeTool!=="tracks"&&activeTool!=="inactive")||referenceCopy.active||parallelMode)&&<div role="status" className="construction-tooltip">敷設不可：{nativeValidationError}</div>}
            {anchorPlacementWaiting&&<div role="status" className="construction-tooltip">この位置に敷設できるか確認中…</div>}
            {activeTool === "tracks" && <div className="construction-toolbar track-construction-toolbar" role="group" aria-label="線路敷設設定"><HeightControls value={selectedHeight??infrastructure.level} disabled={!enabled||referenceCopy.active||(!heightAnchor&&!!draftDestination)} onChange={changeHeight}/><div className="construction-mode-row"><TrackTypeControls value={trackType} disabled={!enabled||referenceCopy.active||!!draftExtension} onChange={chooseTrackType}/><div className="track-follow-options" role="group" aria-label="線路の追従"><button type="button" className="secondary follow-icon" aria-label="実在の路線網に合わせる" aria-describedby="follow-tip-rail" aria-pressed={referencePanel&&!roadFollowing} disabled={!enabled||!!draftExtension} onClick={()=>{cancelParallel();setParallelMode(false);referenceCopy.clear();referenceCopy.setMode("rail");setRoadFollowing(false);setReferencePanel(value=>!value||roadFollowing);}}><TrackFollowIcon kind="rail"/><span id="follow-tip-rail" role="tooltip" className="follow-tip">実在の路線網に合わせる<br/>実在の線路に沿って、線路の経路を描きます</span></button><button type="button" className="secondary follow-icon" aria-label="道路に合わせる" aria-describedby="follow-tip-road" aria-pressed={roadFollowing} disabled={!enabled} onClick={()=>{cancelParallel();setParallelMode(false);referenceCopy.clear();roadDrawing.cancel();setRoadFollowing(value=>!value);setReferencePanel(!roadFollowing);setHoverPoint(null);}}><TrackFollowIcon kind="road"/><span id="follow-tip-road" role="tooltip" className="follow-tip">道路に合わせる<br/>道路に沿って、線路の経路を描きます</span></button><button type="button" className="secondary follow-icon" aria-label="線路の横に沿わせる" aria-describedby="follow-tip-parallel" aria-pressed={parallelMode} disabled={!enabled||!copyCanStart} onClick={()=>{cancelParallel();referenceCopy.clear();setReferencePanel(false);setRoadFollowing(false);setParallelMode(value=>!value);setHoverPoint(null);}}><TrackFollowIcon kind="parallel"/><span id="follow-tip-parallel" role="tooltip" className="follow-tip">線路の横に沿わせる<br/>青い範囲にカーソルを合わせると、別の線路と平行に線路を描きます</span></button></div></div>
            <div className="construction-options-row"><label>線路数<select aria-label="敷設する線路数" value={infrastructure.track_count} disabled={!enabled||!!draftExtension||draftPoints.length>0||referenceCopy.active} onChange={e=>setInfrastructure(value=>({...value,track_count:Number(e.target.value) as 1|2|4}))}>{([1,2,4] as const).map(count=><option key={count} value={count}>{COUNT_LABELS[count]}</option>)}</select></label><details className="track-advanced-settings"><summary>詳細設定</summary><div className="target-speed-control"><label htmlFor="track-target-speed">目標最高速度</label><span className="speed-input"><button type="button" className="secondary" aria-label="目標最高速度を20 km/h下げる" disabled={!enabled||!!draftExtension||referenceCopy.active||designSpeedKmh<=1} onClick={()=>setDesignSpeedKmh(value=>Math.max(1,value-20))}>−</button><input id="track-target-speed" aria-label="線路の目標最高速度" type="number" min="1" max="600" step="1" value={designSpeedKmh} disabled={!enabled||!!draftExtension||referenceCopy.active} onChange={event=>{const value=Number(event.target.value);if(Number.isFinite(value)&&value>=1&&value<=600)setDesignSpeedKmh(value);}}/><button type="button" className="secondary" aria-label="目標最高速度を20 km/h上げる" disabled={!enabled||!!draftExtension||referenceCopy.active||designSpeedKmh>=600} onClick={()=>setDesignSpeedKmh(value=>Math.min(600,value+20))}>＋</button><span>km/h</span></span></div></details></div>
</div>}
            {activeTool === "tracks" && mapMode==="draw" && !parallelMode && !referenceCopy.active && !dragging && !anchorPreview && cursorPosition && !endpointCandidate && !draftFinished && !showPlanHover && (draftPoints.length>0 || (roadFollowing&&!graphBundle) || roadDrawing.preview?.kind==='error' || !!drawingPreviewError) && <div className="construction-tooltip drawing-tooltip drawing-metrics" role="status" style={{left:Math.max(8,Math.min(mapSize.width-210,cursorPosition.x+18)),top:Math.max(8,Math.min(mapSize.height-90,cursorPosition.y+18))}}>
              {((roadFollowing&&!graphBundle) || roadDrawing.preview?.kind==='error' || drawingPreviewError || nativeValidationError)&&<strong>{roadFollowing&&!graphBundle?automaticRoad.message:roadDrawing.preview?.kind==='error'?roadDrawing.preview.error:`敷設不可：${drawingPreviewError??nativeValidationError}`}</strong>}
              {draftPoints.length>0&&!roadFollowing&&constructionRoadReason&&<small>{constructionRoadReason}</small>}
              {draftPoints.length>0&&!roadFollowing&&!drawingPreviewError&&!nativeConstructionPreview?.error&&<CrossingEstimate value={nativeConstructionPreview?.value} pending={!nativeConstructionPreview||constructionRoad.status==="loading"}/>}
              {draftPoints.length>0&&<><div>長さ {Math.round(alignmentLength(draftLine))} m{candidateLine.length>1&&!drawingPreviewError?` ＋ ${Math.round(alignmentLength(roadCursorPreview?.line??candidateLine))} m`:""}</div>{!drawingPreviewError&&<div>速度上限 <span className="drawing-speed">{drawingPreviewSpeed} km/h</span>{(circleGuide||!!roadPreviewGeometry?.arcs.length)&&<small>（暫定）</small>}</div>}</>}
            </div>}
            {anchorPreview?.preview&&(anchorPreview.error || anchorPreview.snapped)&&cursorPosition&&<div className="construction-tooltip drawing-tooltip" role="status" style={{left:Math.max(8,Math.min(mapSize.width-256,cursorPosition.x+18)),top:Math.max(8,Math.min(mapSize.height-80,cursorPosition.y+18))}}>{anchorPreview.error??"位置合わせ（接続は変更しません）"}</div>}
            {destinationCandidate&&destinationError&&cursorPosition&&<div className="construction-tooltip drawing-tooltip" role="status" style={{left:cursorPosition.x+18,top:cursorPosition.y+18}}>{destinationError}</div>}
            {endpointCandidate&&!endpointCandidateAllowed&&cursorPosition&&<div className="construction-tooltip drawing-tooltip" role="status" style={{left:Math.max(8,Math.min(mapSize.width-256,cursorPosition.x+18)),top:Math.max(8,Math.min(mapSize.height-100,cursorPosition.y+18))}}>
              <strong>{endpointCandidateError}</strong>
            </div>}
            {activeTool==="tracks"&&mapMode==="draw"&&draftExtension&&draftPoints.length>0&&!referenceCopy.active&&<div className="extension-status" role="status">延伸元を選択済み：{alignments.find(a=>a.id===draftExtension.alignmentId)?.name||"既存線路"}の{draftExtension.endpoint==="start"?"始点":"終点"}（着工前）</div>}
            {showPlanHover&&!endpointCandidate&&planHover&&hoveredPlan&&<div className="construction-tooltip plan-hover-tooltip" role="status" style={{left:Math.max(8,Math.min(mapSize.width-240,planHover.x+18)),top:Math.max(8,Math.min(mapSize.height-80,planHover.y+18))}}><strong>{hoveredPlan.name.trim()||"線路計画"}</strong>{hoveredGeometry?.error?<div>敷設不可：{hoveredGeometry.error}</div>:!cursorPreviewActive&&buildPaths.some(path=>path.id===planHover.id)&&nativeValidationError&&<div>この計画は敷設不可：{nativeValidationError}</div>}<div>{plannedTrackHeight(hoveredPlan,hoveredGeometry?.points??hoveredPlan.line,planHover.point)} · {hoveredSpeed===null?"制限速度：未算定":`${hoveredSpeed} km/h（暫定）`}</div></div>}
            <svg ref={svgRef} tabIndex={0} onKeyDown={event=>{
              if(activeTool!=="stations"||mapMode!=="place"||!enabled||event.ctrlKey||event.metaKey||event.altKey)return;
              if(event.key==="Enter"){pointerFrame.cancel();event.preventDefault();if(!event.repeat){const rect=event.currentTarget.getBoundingClientRect();placePlatformAt(stationInputCursor.current??stationCursor??{latitude:camera.latitude,longitude:camera.longitude},rect.width,rect.height);}return;}
              if(!["ArrowLeft","ArrowRight","ArrowUp","ArrowDown"].includes(event.key))return;
              pointerFrame.cancel();event.preventDefault();
              const p=stationInputCursor.current??stationCursor??{latitude:camera.latitude,longitude:camera.longitude};
              const moved=panCamera({...camera,latitude:p.latitude,longitude:p.longitude},event.key==="ArrowLeft"?-5:event.key==="ArrowRight"?5:0,event.key==="ArrowUp"?-5:event.key==="ArrowDown"?5:0);
              stationInputCursor.current={latitude:moved.latitude,longitude:moved.longitude};setStationCursor(stationInputCursor.current);
              const rect=event.currentTarget.getBoundingClientRect();if(rect.width>0&&rect.height>0){const viewport=mapViewport(rect.width,rect.height),screenPoint=project(moved.latitude,moved.longitude,camera);setCursorPosition({x:(screenPoint.x-viewport.x)*viewport.scale,y:(screenPoint.y-viewport.y)*viewport.scale,width:rect.width,height:rect.height});}
            }} onAuxClick={event=>{if(event.button===1)event.preventDefault();}} onPointerDown={pointerDown} onPointerMove={schedulePointerMove} onPointerUp={(event) => pointerEnd(event)} onPointerCancel={(event) => pointerEnd(event, true)} onPointerLeave={() => { pointerFrame.cancel();stationInputCursor.current=null;drawingCursor.current=null;if(anchorGesture.current)return;referenceCopy.clearHover();setCursorPosition(null);setDeleteHover(null);setPlanHover(null);setBuiltHover(null); setHoverPoint(null);setStationCursor(null); if (gestureRef.current && !gestureRef.current.dragged) cancelMapGesture(); }} onLostPointerCapture={(event) => { if (event.target === event.currentTarget) pointerEnd(event, true); }} role="img" aria-label={background === "sample" ? "駅配置用サンプル地図" : background === "osm" ? "駅配置用OpenStreetMap" : "駅配置用地理院地図"} viewBox={`0 0 ${MAP_VIEWBOX.width} ${MAP_VIEWBOX.height}`} preserveAspectRatio="xMidYMid slice" onClick={mapClick} className={(enabled && (mainTab==="routes"||mainTab==="services") && routeMapPick ? "route-station-select" : dragging ? "map-dragging" : enabled && mapMode !== "move" ? "placement-enabled" : "map-move-mode")+(activeTool==="select"?" map-selection-mode":activeTool==="delete"?" map-delete-mode":"")}>
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
              {automaticRailSelected&&automaticRail.view.chains.map(chain=><path key={chain.id} d={drawPath(chain.points)} className="automatic-rail-reference" stroke="#777" strokeWidth="1.5" strokeDasharray="4 4" fill="none" pointerEvents="none"/>)}
              <BuiltTrackLayer alignments={alignments} camera={camera} geometry={trackRender.value} selectedId={referenceCopy.active?((manualReferenceCopyState.seamPlan?seamFocusedEndpoint?.alignment.id:referenceExtension?.alignment.id)??""):alignmentId} showTitles={!(mainTab==="construction"&&activeTool==="inactive")} onHover={hoverBuilt} onLeave={clearBuiltHover} onSelect={selectBuiltTrack}/>
              <RouteLayer alignments={alignments} camera={camera} routeVisual={routeVisual} serviceMapSections={serviceMapSections} showServices={mainTab==="routes"||mainTab==="services"}/>
              {referenceCopy.active&&referenceCopy.roadFollow&&(referenceCopy.candidates.length?referenceCopy.candidates:referenceCopy.hoverCandidates).map(candidate=>{const a=graphNodesById.get(candidate.edge.from),b=graphNodesById.get(candidate.edge.to);return a&&b?<path key={candidate.edge.id} d={drawPath([a,b])} className="road-follow-candidate" stroke="#298746" strokeWidth="4" opacity=".7" fill="none" pointerEvents="none"/>:null;})}
              {roadFollowing&&(roadDrawing.preview?.kind==='point'||roadDrawing.preview?.kind==='route')&&focusMarker(roadDrawing.preview.point,'anchor','road-snap-point')}
              {roadFollowing&&roadDrawing.preview?.kind==='route'&&<TrackStroke d={drawInfrastructurePath(roadDrawing.preview.line,infrastructure)} className={'draft-path road-drawing-preview'+(constructionValidationError(nativeConstructionPreview?.value)?' invalid-candidate':'')} pointerEvents="none"/>}
              {referenceCopy.active&&referencePreviewPoints&&<TrackStroke d={drawInfrastructurePath(referencePreviewPoints,referenceExtension?endpointInfrastructure(referenceExtension.alignment,referenceExtension.endpoint):infrastructure)} data-track-count={(referenceExtension?endpointInfrastructure(referenceExtension.alignment,referenceExtension.endpoint):infrastructure).track_count} className="draft-path reference-copy-preview" style={referenceCopy.roadFollow?{stroke:"#298746"}:undefined} pointerEvents="none"/>}
              {parallelMode&&activeTool==="tracks"&&parallelSources.map(source=>{const p=source.points[0],a=project(p.latitude,p.longitude,camera),b=project(p.latitude+PARALLEL_RANGE_M/111195,p.longitude,camera);return <path key={source.id} d={drawPath(source.points)} fill="none" stroke="#61afcf" strokeOpacity=".28" strokeWidth={2*Math.abs(a.y-b.y)} strokeLinecap="round" pointerEvents="none"/>;})}
              {parallelMode&&parallelStart&&(()=>{const q=project(parallelStart.point.latitude,parallelStart.point.longitude,camera);return <circle cx={q.x} cy={q.y} r={5/viewport.scale} fill="white" stroke="#287c9e" strokeWidth={1.5/viewport.scale} pointerEvents="none"/>;})()}
              {parallelMode&&parallelPreview&&<TrackStroke d={drawInfrastructurePath(parallelPreview.line,infrastructure)} data-track-count={infrastructure.track_count} className="draft-path candidate-draft" pointerEvents="none"/>}
              {parallelMode&&parallelCandidate&&!parallelStart&&(()=>{const line=offsetTrack(parallelCandidate.source.points,parallelCandidate.projection!.offset),p=line&&hoverPoint&&parallelProjection(line,hoverPoint)?.point;if(!p)return null;const q=project(p.latitude,p.longitude,camera);return <circle cx={q.x} cy={q.y} r={5/viewport.scale} fill="white" stroke="#287c9e" strokeWidth={1.5/viewport.scale} pointerEvents="none"/>;})()}
              <JunctionPoints key={worldEpoch} lines={alignments} connections={alignmentConnections} camera={camera} selectable={enabled&&mapMode==="move"&&activeTool!=="delete"}/>
              {parkedPaths.map(plan=><g key={plan.id} className={plan.planId===activePlanId?undefined:"inactive-plan"}><ConstructionLayer points={plan.line} infrastructure={plan.infrastructure} camera={camera} className={"draft-path parked-plan"+(mapMode==="move"&&selectedPlanPath===plan.id?" track-explicit-selection":"")} pathId={plan.id} /><path d={drawPath(plan.line)} className="plan-hover-hit" data-plan-hover-id={plan.id} onClick={event=>{if(mapMode==="move"){event.stopPropagation();selectTrackPath(plan.id);}}} fill="none" stroke="transparent" strokeWidth="14" pointerEvents="stroke" onPointerEnter={event=>hoverPlan(event,plan.id)} onPointerMove={event=>hoverPlan(event,plan.id)} onPointerLeave={()=>{pointerFrame.cancel();setPlanHover(null);}} />{drawAnchors(plan.points,true,plan.id)}</g>)}
              {tangentGuide&&<line className="construction-tangent-guide" aria-label="円弧・直線の先端延長ガイド" x1={tangentGuide.start.x} y1={tangentGuide.start.y} x2={tangentGuide.end.x} y2={tangentGuide.end.y} stroke="#65716b" strokeWidth="1" strokeDasharray="6 4" opacity=".8" vectorEffect="non-scaling-stroke" pointerEvents="none"/>}

              {!roadCursorPreview && !parallelMode && !referenceCopy.active && guideCenter && guideEdge && <circle className="curve-circle-guide" cx={guideCenter.x} cy={guideCenter.y} r={Math.hypot(guideEdge.x-guideCenter.x,guideEdge.y-guideCenter.y)} fill="none" stroke={previewError ? "#b34c4c" : "#65716b"} strokeWidth="1" opacity=".6" vectorEffect="non-scaling-stroke" pointerEvents="none" />}
              {!referenceCopy.active&&draftLine.length > 1 && <ConstructionLayer points={draftLine} infrastructure={infrastructure} camera={camera} className={"draft-path confirmed-draft"+(mapMode==="move"&&selectedPlanPath===activePathId?" track-explicit-selection":"")} />}
              {!roadCursorPreview&&!parallelMode&&!referenceCopy.active&&candidateLine.length > 1 && <ConstructionLayer points={candidateLine} infrastructure={infrastructure} camera={camera} className={"draft-path candidate-draft"+(previewError||nativeValidationError?" invalid-candidate":"")} />}
              {!referenceCopy.active && draftLine.length > 1 && <path d={drawPath(draftLine)} className="plan-hover-hit" data-plan-hover-id={activePathId} onClick={event=>{if(mapMode==="move"){event.stopPropagation();selectTrackPath(activePathId);}}} fill="none" stroke="transparent" strokeWidth="14" pointerEvents="stroke" onPointerEnter={event=>hoverPlan(event,activePathId)} onPointerMove={event=>hoverPlan(event,activePathId)} onPointerLeave={()=>{pointerFrame.cancel();setPlanHover(null);}} />}
              {enabled&&(activeTool==="tracks"||activeTool==="inactive")&&!referenceCopy.active&&!dragging&&!anchorPreview&&!endpointCandidate&&builtHover&&hoveredBuilt&&lineFocusMarker(hoveredBuilt.points,builtHover.point,"built-hover-point")}
              {junctionCandidate&&(()=>{const p=project(junctionCandidate.point.latitude,junctionCandidate.point.longitude,camera),r=9.5/viewport.scale;return <g className="junction-snap-preview" pointerEvents="none"><title>ここから分岐</title><circle className="junction-snap-ring" cx={p.x} cy={p.y} r={r} fill="#b83945" stroke="white" strokeWidth="2" vectorEffect="non-scaling-stroke"/><path d={`M${p.x-r*.45},${p.y+r*.45} L${p.x},${p.y} L${p.x+r*.45},${p.y-r*.45} M${p.x},${p.y} L${p.x+r*.45},${p.y+r*.45}`} stroke="white" strokeWidth="2" fill="none" vectorEffect="non-scaling-stroke"/></g>;})()}
              {destinationCandidate&&focusMarker(destinationCandidate.point,"anchor","destination-snap-ring",!destinationError)}
              {endpointCandidate&&focusMarker(endpointCandidate.point,"anchor","endpoint-snap-ring",endpointCandidateAllowed)}
              {showPlanHover&&!endpointCandidate&&planHover&&hoveredPlan&&(planHover.anchor===undefined?lineFocusMarker(hoveredPlan.line,planHover.point,"plan-hover-point"):focusMarker(planHover.point,"anchor","plan-hover-point"))}
              {dragSpeedLabels.map(label=>{const p=project(label.point.latitude,label.point.longitude,camera);return <g key={label.interval} className="drag-segment-speed" data-interval={label.interval} pointerEvents="none" transform={`translate(${p.x} ${p.y}) scale(${1/viewport.scale})`}><rect x="-34" y="-27" width="68" height="19" rx="3"/><text x="0" y="-14" textAnchor="middle">{label.speed} km/h</text></g>;})}
              {drawAnchors(draftPoints)}
              {mainTab==="construction"&&<SlopeBoundaryMarkers lines={alignments} plans={previewRequests} camera={camera} scale={viewport.scale}/>}
              {activeTool==="select"&&mapMode==="move"&&<BuiltAnchorHeightHandles lines={alignments} owner={world?.companies[0]?.id} camera={camera} scale={viewport.scale} onSelect={selection=>{setHeightAnchor(null);setBuiltHeightSelection(selection);setRangeSelection([]);}}/>}
              {anchorPreview?.preview&&<g pointerEvents="none"><TrackStroke d={drawPath(anchorPreview.preview.line)} className={"draft-path anchor-drag-preview"+(anchorPreview.error?" invalid-candidate":"")} />{focusMarker(anchorPreview.preview.anchors[anchorPreview.index],"anchor",anchorPreview.snapped?"anchor-snap-ring snapped":"anchor-snap-ring",!anchorPreview.error)}</g>}
              {detailedStations&&<><defs><filter id="station-group-outline" x="-10%" y="-10%" width="120%" height="120%"><feMorphology in="SourceAlpha" operator="dilate" radius={1/viewport.scale} result="expanded"/><feFlood floodColor="#333"/><feComposite in2="expanded" operator="in"/><feMerge><feMergeNode/><feMergeNode in="SourceGraphic"/></feMerge></filter></defs>{stationGroupDisplay.filter(g=>g.members.length>1).map(group=><g key={group.id} className="station-group-footprint" data-station-group={group.id} pointerEvents="none"><title>{group.name}</title><g filter="url(#station-group-outline)">{group.members.map(member=>{const planned=plannedStations.find(s=>s.id===member.id),inactive=planned&&stationPlanId(planned)!==activePlanId;return <path key={member.id} data-group-platform={member.id} d={drawPath(member.polygon)+"Z"} fill={planned?(inactive?"#b1bac3":"#c4e6f3"):"white"} stroke={planned?(inactive?"#78838e":"#287c9e"):"none"} strokeWidth="1.5" strokeDasharray={planned?"4 3":undefined} vectorEffect="non-scaling-stroke"><title>{group.name}{planned?"（未着工）":""}</title></path>;})}</g></g>)}</>}
              {plannedPlatforms.map(({station,polygon,valid})=>{const p=project(station.point.latitude,station.point.longitude,camera);return <g key={station.id} className={"planned-station"+(stationPlanId(station)===activePlanId?"":" inactive-plan")} pointerEvents="none" opacity=".7"><title>{station.name}（未着工）{valid?"":"・線形の修正が必要"}</title>{detailedStations&&valid?<path d={drawPath(polygon)+"Z"} className="station-platform" visibility={mergedStationIds.has(station.id)?"hidden":undefined}/>:<circle visibility={stationDisplayGroup(station.id)&&stationDisplayGroup(station.id)?.id!==station.id?"hidden":undefined} cx={p.x} cy={p.y} r="5" fill={valid?"#777":"#b34c4c"}/>}{stationNamesVisible&&stationDisplayGroup(station.id)?.id===station.id&&<text x={p.x+10} y={p.y-8}>{stationDisplayGroup(station.id)?.name??station.name}（計画）</text>}</g>;})}
              {stationPreview.length >= 4 && <path d={drawPath(stationPreview) + "Z"} className="station-platform station-placement-preview" opacity=".65" pointerEvents="none" />}
              {referenceVisible && referenceResult.stations.map(station=>{const p=project(station.latitude,station.longitude,camera);return <g key={station.osmType+"/"+station.osmId} className="reference-station" pointerEvents="none"><title>参考駅: {station.name} ({station.kind})</title><path d={`M${p.x},${p.y-5} l5,5 -5,5 -5,-5 Z`}/><text x={p.x+9} y={p.y-7}>参考: {station.name}</text></g>;})}
              <StationLayer stations={world?.stations} placements={world?.station_placements} stationPolygons={stationPolygons} stationGroupDisplay={stationGroupDisplay} camera={camera} selected={selected} routeVisual={routeVisual} placing={activeTool==="stations"&&mapMode==="place"} detailedStations={detailedStations} stationNamesVisible={stationNamesVisible} onSelect={selectMapStation}/>
              {activeTool==="select"&&<g className="range-selection-layer" pointerEvents="none">{rangeSelection.flatMap(row=>{const line=alignments.find(line=>line.id===row.alignmentId);return line?deletionSections(line).filter(section=>row.sectionIndices.includes(section.index)).map(section=><path key={row.alignmentId+":"+section.index} data-selected-alignment={row.alignmentId} data-selected-section={section.index} className="selected-built-section" strokeWidth={8/viewport.scale} d={drawPath(section.points)}/>):[];})}{selectionBox&&(()=>{const b=selectionBounds(selectionBox);return <rect className="selection-box" x={b.left} y={b.top} width={b.right-b.left} height={b.bottom-b.top}/>;})()}</g>}
              {activeTool==="delete"&&enabled&&<g className="deletion-layer">
                {alignments.flatMap(line=>deletionSections(line).map(section=>{const target:DeleteTarget={kind:"built-section",alignmentId:line.id,sectionIndex:section.index};return <path key={line.id+":"+section.index} data-delete-alignment={line.id} data-delete-section={section.index} d={drawPath(section.points)} {...deleteHitProps(target)}/>;}))}
                {[...parkedPaths.map(path=>({id:path.id,points:path.points,line:path.line,whole:!!path.reference||!!path.ofmPath||path.ofmPath||path.roadFollowing})),{id:activePathId,points:draftPoints,line:draftLine,whole:legacyRoadDraft}].map(path=><g key={path.id}>
                  {deletionSections({points:path.line,anchor_indices:path.whole?undefined:anchorIndices(path.points,path.line)}).map(section=><path key={section.index} data-delete-plan-path={path.id} data-delete-plan-section={section.index} d={drawPath(section.points)} {...deleteHitProps(path.whole||!anchorIndices(path.points,path.line)?{kind:"plan-path",pathId:path.id}:{kind:"plan-section",pathId:path.id,sectionIndex:section.index})}/>)}
                  {path.points.map((point,index)=>{const p=project(point.latitude,point.longitude,camera);return <rect key={index} data-delete-anchor={index} data-delete-path={path.id} x={p.x-8/viewport.scale} y={p.y-8/viewport.scale} width={16/viewport.scale} height={16/viewport.scale} {...deleteHitProps({kind:"plan-anchor",pathId:path.id,index},true)}/>;})}
                </g>)}
                {plannedPlatforms.map(({station,polygon})=>{const p=project(station.point.latitude,station.point.longitude,camera),target:DeleteTarget={kind:"plan-station",stationId:station.id};return polygon.length?<path key={station.id} data-delete-planned-station={station.id} d={drawPath(polygon)+"Z"} {...deleteHitProps(target,true)}/>:<circle key={station.id} data-delete-planned-station={station.id} cx={p.x} cy={p.y} r={8/viewport.scale} {...deleteHitProps(target,true)}/>;})}
                {world?.stations.map(station=>{const p=project(station.latitude,station.longitude,camera),polygon=stationPolygons.get(station.id),target:DeleteTarget={kind:"station",stationId:station.id};return detailedStations&&polygon?<path key={station.id} data-delete-station={station.id} d={drawPath(polygon)+"Z"} {...deleteHitProps(target,true)}/>:<circle key={station.id} data-delete-station={station.id} cx={p.x} cy={p.y} r={8/viewport.scale} {...deleteHitProps(target,true)}/>;})}
              </g>}
              </g>
            </svg>
            <div className="map-instruction">{mapInstruction(status,mapMode)}</div>
          </div>
          <MapCredits waterModelVisible={status==="ready"} openFreeMapVisible={!!world?.road_evidence_descriptors?.length||parkedPaths.some(p=>!!p.ofmPath)||!!graphBundle?.graph.dataset.id.startsWith("ofm-tiles/")||!!constructionRoad.snapshot?.tileSnapshots?.length||!!world?.road_snapshots?.some(s=>s.source_url.startsWith("https://tiles.openfreemap.org/planet/"))||!!world?.source_records?.some(s=>s.dataset_id.startsWith("ofm-tiles/"))||roadFollowing||!!draftEditedGeometry||parkedPaths.some(p=>!!p.editedGeometry)||!!world?.alignments?.length} derivedOsmVisible={!!world?.road_evidence_descriptors?.length||crossingCount>0||(roadFollowing&&!!graphBundle)||alignments.some(line=>line.points.length>1&&world?.alignment_provenance?.some(origin=>origin.alignment_id===line.id))||parkedPaths.some(path=>(!!path.reference||!!path.ofmPath)&&path.line.length>1)||!!(referenceCopy.active&&referencePreviewPoints&&referencePreviewPoints.length>1)} background={background} reference={referenceVisible && referencePack ? referencePack.dataset : null} graphVisible={(!!graphBundle && graphVisible)||(automaticRailSelected&&automaticRail.view.chains.length>0)}/>
        </section>
        <aside className={panelCollapsed ? "side-panel collapsed" : "side-panel"}>
          <WorkspaceTabs value={mainTab} onChange={chooseMainTab}/>
          <div className="inspector-title"><span>RAILWAY WORKSPACE</span><button className="secondary" aria-label={panelCollapsed ? "操作パネルを開く" : "操作パネルを畳む"} onClick={() => setPanelCollapsed(!panelCollapsed)}>{panelCollapsed ? "+" : "−"}</button></div>
          <nav className="editor-tabs construction-tabs" role="tablist" aria-label="建設ツール" hidden={mainTab!=="construction"}>
            {([["tracks","線路"],["stations","駅設置"],["delete","削除"],["select","選択"]] as const).map(([tool,label],index)=><button key={tool} disabled={seamStageSending} role="tab" id={"tab-"+tool} aria-controls={"panel-"+tool} aria-selected={activeTool===tool} tabIndex={activeTool===tool?0:-1} onClick={()=>{if(seamMutationPending.current)return;setError("");setActiveTool(tool);changeMapMode(tool==="tracks"?"draw":tool==="stations"?"place":"move");}} onKeyDown={event=>{if(seamMutationPending.current)return;if(!["ArrowLeft","ArrowRight","Home","End"].includes(event.key))return;event.preventDefault();const tools=["tracks","stations","delete","select"] as const;const next=tools[event.key==="Home"?0:event.key==="End"?3:(index+(event.key==="ArrowLeft"?3:1))%4];setActiveTool(next);changeMapMode(next==="tracks"?"draw":next==="stations"?"place":"move");document.getElementById("tab-"+next)?.focus();}}>{label}</button>)}
          </nav>
          <section id="workspace-panel-finance" role="tabpanel" aria-labelledby="workspace-tab-finance" hidden={mainTab!=="finance"}><h3>財務</h3><p>未実装です。収支・運賃・経費の画面は今後追加します。</p></section>
          <section id="workspace-panel-city" role="tabpanel" aria-labelledby="workspace-tab-city" hidden={mainTab!=="city"}><h3>都市</h3><p>未実装です。人口・需要・都市発展の画面は今後追加します。</p></section>
          <section id="workspace-panel-saves" role="tabpanel" aria-labelledby="workspace-tab-saves" hidden={mainTab!=="saves"}><h3>セーブデータ</h3>
            <SaveSlots deletedSlots={deletedSaveSlots} onDelete={id=>moveSavedWorld(id,false)} onRestore={id=>moveSavedWorld(id,true)} onSave={save} saveDisabled={!enabled||saveBlocked} slots={saveSlots} current={activeSlotId} loading={slotsLoading} error={slotsError} busy={busy||status!=="ready"} canSave={!saveBlocked&&!companySetupRequired} onRefresh={()=>void refreshSaveSlots()} onLoad={id=>requestSlotAction({kind:"load",id})} onSaveAs={()=>{setSlotName(companyName);setSlotDialog("save-as");}} onNew={()=>{setSlotName("");setSlotDialog("new");}}/>
            {!activeSlotId&&<p>初回保存後に自動保存を開始します。</p>}
            <h4>オートセーブ</h4><p>2分ごとに自動保存します。手動保存とは別に、ワールドと建設計画を保存します。</p><p role="status">{!slotsReady?"保存先の確認が完了するまで自動保存は停止しています。":activeSlotId?autosave.message:"初回保存待ち"}</p>{autosave.permission==="failed"&&<button className="secondary" disabled={status!=="ready"||busy} onClick={autosave.retryStatus}>自動保存データを再確認</button>}<button className="secondary" disabled={!enabled||!autosave.available} onClick={()=>requestLoad("auto")}>自動保存から復元</button></section>
          <div id="workspace-panel-construction" className="construction-content" role="tabpanel" aria-labelledby="workspace-tab-construction" hidden={mainTab!=="construction"}>
          <section className="shared-plan" aria-label="建設計画">
            <div className="plan-heading"><label htmlFor="construction-plan-name">計画</label><input id="construction-plan-name" autoComplete="off" aria-label="計画名（任意）" maxLength={80} placeholder={`計画 ${activePlanId}`} disabled={!enabled||referenceCopy.active} value={planName} onChange={event=>setPlanName(event.target.value)}/><button type="button" className="secondary" aria-label="別の線路計画を追加" title="新しい計画" disabled={!enabled||referenceCopy.active||!(currentPaths.some(path=>path.points.length>0)||currentStations.length>0)} onClick={newIndependentPlan}>＋</button></div>
            {(plans.length>1||currentPaths.some(path=>path.points.length>0)||currentStations.length>0)&&<div className="plan-switcher" role="group" aria-label="計画を選択">{plans.map(plan=><button key={plan.id} className="secondary" aria-pressed={plan.id===activePlanId} disabled={!enabled||referenceCopy.active} onClick={()=>choosePlan(plan.id)}>{plan.name.trim()||`計画 ${plan.id}`}{plan.id!==activePlanId&&<span className="sr-only"> を編集</span>}</button>)}</div>}
            <div className="plan-totals"><span>{currentPaths.filter(path=>path.points.length>0).length}経路 · {new Set(currentStations.map(p=>stationDisplayGroup(p.station.id)?.id??p.station.id)).size}駅</span><span title="費用計算は未実装です">建設費 — / 維持費 —</span></div>
            {(roadCheckPending||roadChecks.pending||roadStatusReason)&&<div role="status" aria-label="踏切・道路確認状態" className="field-hint">
              {roadCheckPending?<button type="button" className="secondary" onClick={()=>roadCheckAbort.current?.abort()}>着工を取り消す</button>:<span title={roadStatusReason}>{roadChecks.pending||constructionRoad.status==='loading'?'道路確認中':'踏切未確認'}</span>}
              {!roadCheckPending&&buildPaths.length>0&&constructionRoad.status!=='loading'&&roadStatusReason&&<button type="button" className="secondary" aria-label="道路再取得" title="道路再取得" onClick={constructionRoad.retry}>↻</button>}
            </div>}
            <details className="plan-breakdown" open={currentStations.some(item=>!item.valid)||undefined}><summary>内訳</summary><p className="muted">建設費・維持費は未計算</p><div role="group" aria-label="計画内の経路" hidden>{currentPaths.map(path=><button key={path.id} className="secondary" disabled={!enabled||referenceCopy.active||path.id===activePathId||!!path.reference||!!path.ofmPath} aria-pressed={path.id===activePathId} onClick={()=>selectPath(path.id)}>経路 {path.id}{path.reference?"（実在形状）":""}</button>)}</div><ul aria-label="計画中の駅">{currentStations.map(({station,valid})=><li className="planned-station-row" key={station.id}><input aria-label="計画駅名" maxLength={80} readOnly={stationDisplayGroup(station.id)?.members.some(m=>!m.planned)} value={stationDisplayGroup(station.id)?.name??station.name} disabled={!enabled} onChange={event=>renamePlannedStation(station.id,event.target.value)} onBlur={()=>{if(!station.name.trim())renamePlannedStation(station.id,"新駅");}} />{!valid&&<span role="status"> 線路に収まらないため着工不可</span>}<button type="button" className="secondary" aria-label={`計画駅 ${station.name} を削除`} disabled={!enabled} onClick={()=>{setPlannedStations(items=>items.filter(item=>item.id!==station.id));setStationUndo(items=>items.filter(item=>item.planned?.id!==station.id));setStationRedo(items=>items.filter(item=>item.planned?.id!==station.id));}}>×</button></li>)}</ul></details>
            {deletePlanConfirm&&<PlanDeleteDialog name={planName.trim()||`計画 ${activePlanId}`} onCancel={()=>setDeletePlanConfirm(false)} onDelete={deleteCurrentPlan}/>}
            <div className="plan-actions"><button className="secondary" disabled={!enabled||referenceCopy.active} onClick={()=>setDeletePlanConfirm(true)}>計画を削除</button><button className="secondary" disabled={!enabled||referenceCopy.active||!planHasContent} onClick={clearCurrentPlan}>下書きを消去</button><button disabled={!enabled||referenceCopy.active||anchorPlacementInProgress||!planBuildable} onClick={commitAlignment}>計画を着工</button></div>
          </section>
          <section role="tabpanel" id="panel-select" aria-labelledby="tab-select" hidden={activeTool!=="select"}><div className="range-selection-actions"><span>{rangeCount}区間を選択</span>{rangeCount>0&&<><span aria-label="選択区間の線路数">{selectedCountValues.size>1?"線路数: 混在":COUNT_LABELS[selectedInfrastructure[0].track_count]}</span><span aria-label="選択区間の高さ">{selectedLevelValues.size>1?"高さ: 混在":LEVEL_LABELS[selectedInfrastructure[0].level]}</span><label>線路数<select aria-label="選択区間の線路数変更" value={rangeTrackCount??""} disabled={!enabled} onChange={event=>setRangeTrackCount(event.target.value?Number(event.target.value) as 1|2|4:null)}><option value="">変更しない</option>{Object.entries(COUNT_LABELS).map(([value,label])=><option key={value} value={value}>{label}</option>)}</select></label><label>高さ<select aria-label="選択区間の高さ変更" value={rangeLevel??""} disabled={!enabled} onChange={event=>setRangeLevel(event.target.value?event.target.value as SectionInfrastructure["level"]:null)}><option value="">変更しない</option>{Object.entries(LEVEL_LABELS).map(([value,label])=><option key={value} value={value}>{label}</option>)}</select></label><button disabled={!enabled||unapplied||(rangeTrackCount===null&&rangeLevel===null)} onClick={updateRangeInfrastructure}>選択区間へ適用</button><label>線路種別<select aria-label="選択区間の線路種別変更" value={rangeTrackType??""} disabled={!enabled} onChange={event=>setRangeTrackType(event.target.value?event.target.value as "normal"|"high_speed":null)}><option value="">変更しない</option><option value="normal">通常</option><option value="high_speed">高速</option></select></label><button disabled={!enabled||unapplied||rangeTrackType===null} onClick={updateRangeTrackType}>線路種別を適用</button></>}<button disabled={!enabled||!rangeCount||unapplied} onClick={()=>{setError("");setConfirmRangeDelete(true);}}>選択区間を撤去</button><button className="secondary" disabled={!rangeCount} onClick={()=>setRangeSelection([])}>選択解除</button></div></section>
          <section role="tabpanel" id="panel-delete" aria-labelledby="tab-delete" hidden={activeTool!=="delete"} />
          <section className="station-editor" role="tabpanel" id="panel-stations" aria-labelledby="tab-stations" hidden={activeTool !== "stations"}>

            <div hidden><StationNamePreview pack={referencePack} repository={referenceRepository} point={stationPreview.length>=4?previewCenter:null} existingNames={[...(world?.stations.map(s=>s.name)??[]),...plannedStations.map(s=>s.name)]}/></div>



          </section>
          <section className="track-editor" role="tabpanel" id="panel-tracks" aria-labelledby="tab-tracks" hidden={activeTool !== "tracks" && activeTool !== "inactive"}>

            <details className="reference-tools" hidden={!referencePanel&&!referenceCopy.active} open><summary>実在線路・道路に沿う</summary>{roadFollowing&&<div role="status" aria-label="道路データ状態">{automaticRoad.message}<button type="button" className="secondary" disabled={referenceCopy.active} onClick={automaticRoad.retry}>再取得</button></div>}{roadFollowing?<p role="status">{roadDrawing.preview?.kind==="error"?roadDrawing.preview.error:"道路上で始点と次の地点をクリック。Ctrl を押す間は自由に敷設"}</p>:automaticRailSelected?<AutomaticRailControls copy={{...referenceCopy,canUndo:referenceHistoryAvailable.undo,canRedo:referenceHistoryAvailable.redo,undo:()=>stepReferenceHistory(false),redoPick:()=>stepReferenceHistory(true),begin:()=>{if(!copyCanStart)return;if(roadFollowing){setFrozenRoadBundle(graphBundle);setFrozenRoadTiles(roadTileSnapshots(automaticRoad.snapshot));}newPath();referenceCopy.begin();changeMapMode("draw");setHoverPoint(null);}}} status={automaticRail.status} retry={automaticRail.retry} enabled={enabled} canStart={copyCanStart} zoom={camera.zoom} truncated={automaticRail.view.truncated}/>:<ReferenceCopyControls cancelDisabled={seamStageSending} extensionError={referenceExtensionError} extensionControls={manualReferenceCopyState.seamPlan?<>{([0,1] as const).map(index=><label key={index}>{index===0?"道路追従の始点接続":"道路追従の終点接続"}<select aria-label={index===0?"道路追従の始点接続":"道路追従の終点接続"} value={index===0?seamExtensionKey:seamDestinationKey} onChange={e=>{(index===0?setSeamExtensionKey:setSeamDestinationKey)(e.target.value);setSeamFocusKey(e.target.value);referenceCopy.setConfirmed(false);}}><option value="">接続しない</option>{seamConnectionCandidates(index).map(candidate=><option key={candidate.key} value={candidate.key}>{seamCandidateLabel(index,candidate)}</option>)}</select></label>)}<small>元の道路形状上に正確に一致する既設端点だけ選択できます。形状は移動しません。</small></>:<label>敷設方法<select aria-label="敷設方法" value={referenceExtensionKey} onChange={event=>{setReferenceExtensionKey(event.target.value);referenceCopy.setConfirmed(false);}}><option value="">独立した新しい線路</option>{referenceExtensionCandidates.map(candidate=><option key={candidate.key} value={candidate.key}>{referenceExtensionLabel(candidate)}</option>)}</select><small>起点から8px以内で、参照形状から正確に再現できる既存端点を選択できます。形状を平行移動せず参照区間の起点を補正します。既存の線路種別を引き継ぎます。複数候補から自動接続はしません。</small></label>} copy={{...referenceCopy,canUndo:referenceHistoryAvailable.undo,canRedo:referenceHistoryAvailable.redo,undo:()=>stepReferenceHistory(false),redoPick:()=>stepReferenceHistory(true),begin:()=>{if(!copyCanStart)return;if(roadFollowing){setFrozenRoadBundle(graphBundle);setFrozenRoadTiles(roadTileSnapshots(automaticRoad.snapshot));}newPath();referenceCopy.begin();changeMapMode("draw");setHoverPoint(null);}}} enabled={enabled} canStart={copyCanStart} hasBundle={!!graphBundle}/> }</details>

            {selectedAlignment&&world?.alignment_provenance?.some(p=>p.alignment_id===selectedAlignment.id)&&<AlignmentProvenance alignment={selectedAlignment} provenances={world?.alignment_provenance?.filter(p=>p.alignment_id===selectedAlignment.id)} sourceRecords={world?.source_records??[]}/>}

          </section>
          </div>
          <section id="workspace-panel-stations" role="tabpanel" className="station-directory" aria-labelledby="workspace-tab-stations" hidden={activeTool !== "station-management"}>
            <StationDirectory key={worldEpoch} stations={world?.stations??[]} groups={world?.station_groups} routes={world?.routes??[]} paths={world?.route_paths} placements={world?.station_placements} selected={selected} onSelect={selectStation}/>
          </section>
          <section className="station-inspector" aria-label="選択した駅" hidden={activeTool !== "station-management"}>
            {selectedStation && <div className="equipment-editor"><h4>駅の詳細</h4><div className="station-detail-name"><strong onDoubleClick={beginStationNameEdit}>{selectedStation.name}</strong><button ref={stationNameEditButton} type="button" className="secondary" disabled={!enabled} aria-label="選択駅の名前を編集" onClick={beginStationNameEdit} title="駅名を変更">✎</button></div>
              {world?.station_placements?.some((p) => p.station_id === selectedStation.id) && <form onSubmit={event=>event.preventDefault()}><fieldset disabled={!enabled&&!stationResizePending}><div className="coordinate-fields"><PlatformLengthInput id="edit-platform-length" label="選択駅の長さ" value={editLength} onChange={value=>{setStationResizeFailure(null);setEditLength(value);}}/></div><div className="route-actions">{stationResizePending&&<span role="status">反映中</span>}{!resizeValid&&<span role="status">2〜20両で入力してください</span>}{stationResizeFailure===resizeKey&&<button type="button" className="secondary" disabled={!enabled||!resizeValid} onClick={()=>{setStationResizeFailure(null);void resizePlatform();}}>長さ変更を再試行</button>}<button type="button" className="secondary" hidden={!platformDirty} disabled={!platformDirty||stationResizePending} onClick={() => { setStationResizeFailure(null);if(selectedPlacement){setEditLength(String(selectedPlacement.length_m));setEditWidth(String(selectedPlacement.width_m));} }}>ホーム長の下書きを破棄</button></div></fieldset></form>}
              {world?.station_groups?.find(g=>g.members.includes(selected??""))&&<label>ホーム<select aria-label="編集するホーム" value={selected??""} onChange={event=>selectStation(event.target.value)}>{world.station_groups.find(g=>g.members.includes(selected??""))!.members.map((id,index)=><option key={id} value={id}>ホーム {index+1}</option>)}</select></label>}<form onSubmit={renameStation}><fieldset disabled={!enabled}>
              <label htmlFor="edit-station-name">選択駅の名前</label><input ref={stationNameInput} id="edit-station-name" onKeyDown={event=>{if(event.key!=="Escape" || event.nativeEvent.isComposing || event.nativeEvent.keyCode===229)return;event.preventDefault();event.stopPropagation();if(!enabled || busyRef.current)return;setEditStationName(selectedStation.name);stationNameEditButton.current?.focus();}} maxLength={80} value={editStationName} onChange={(event) => setEditStationName(event.target.value)} /><div className="route-actions" hidden={!stationDirty}><button type="submit" disabled={!enabled || !stationDirty || !editStationName.trim()}>駅名を適用</button><button type="button" className="secondary" disabled={!enabled || !stationDirty} onClick={() => setEditStationName(selectedStation.name)}>駅名下書きを破棄</button></div></fieldset></form>
              <button className="secondary danger" disabled={!enabled || unapplied || !!stationReferences(selectedStation.id)} onClick={() => setPendingDelete({ kind: "station", id: selectedStation.id })}>{world?.station_groups?.some(g=>g.members.includes(selectedStation.id))?"選択ホームを撤去":"選択駅を削除"}</button>
              {stationReferences(selectedStation.id) && <p className="field-hint">削除不可: {stationReferences(selectedStation.id)}</p>}
            </div>}
            {pendingStation && <div className="inline-confirm" role="alert"><p>{platformDirty ? "駅名・ホーム長の下書きを破棄して別駅を選びますか。" : "駅名の下書きを破棄して別駅を選びますか。"}</p><button className="secondary" disabled={busy} onClick={() => setPendingStation(null)}>駅名編集を続ける</button><button disabled={busy} onClick={() => { const placement = world?.station_placements?.find(p=>p.station_id===pendingStation.id); setEditLength(String(placement?.length_m ?? 200));setEditWidth(String(placement?.width_m ?? 12));setMainTab("stations"); setActiveTool("station-management");setSelected(pendingStation.id); setEditStationName(stationsById.get(pendingStation.id)?.name ?? ""); setPendingStation(null); setPendingDelete(null); }}>{platformDirty ? "駅設定下書きを破棄して切替" : "駅名下書きを破棄して切替"}</button></div>}
          </section>
          <section className="route-editor" role="tabpanel" id="workspace-panel-routes" aria-labelledby="workspace-tab-routes" hidden={activeTool !== "routes"}>
            <h3>路線</h3>
            <RouteDirectory key={`routes-${worldEpoch}`} onRename={renameSavedRoute} onDirty={setRouteRenameDirty} world={world??previewWorld} enabled={enabled&&!candidateDirty&&!routeCreating&&!routeMapEdit} selected={routeId} onChoose={id=>{if(candidateDirty||routeMapEdit){chooseRoute(id);return;}chooseRoute(id);cancelRouteMapPick();setRouteCreating(id===null);if(id===null)setRouteSequenceKey(key=>key+1);}}/>
            {(routeCreating||routeMapEdit)&&<RouteCandidatePreview key={routeMapEdit?.id??"new-route"} editRoute={routeMapEdit??undefined} defaultName={(()=>{let n=1;while(world?.routes.some(r=>r.name===`路線${n}`))n++;return `路線${n}`;})()} onCancelEdit={()=>{cancelRouteMapPick();setRouteMapEdit(null);setRouteSequenceKey(0);setRouteCreating(false);setCandidateDirty(false);}} startSequenceKey={routeSequenceKey} mapPickTarget={routeMapPick} onMapPick={(target,select)=>{if(!target){cancelRouteMapPick();return;}if(!enabled||mainTab!=="routes")return;routeMapSelect.current=select??null;setRouteMapPick(target);changeMapMode("move");}} resetKey={worldEpoch} onDirty={setCandidateDirty} stations={world?.stations ?? []} worldVersion={world} enabled={enabled} canApply={true} onSave={applyPlannedRoute} />}

            {routeId && <button type="button" className="secondary" disabled={!enabled||candidateDirty||routeRenameDirty||!!routeMapEdit} onClick={()=>{const route=world?.routes.find(r=>r.id===routeId);if(!route)return;cancelRouteMapPick();setRouteMapEdit({id:route.id,name:route.name});setRouteSequenceKey(key=>key+1);}}>地図で路線の経路を選び直す</button>}
            {selectedRoutePath && <section aria-label="保存済み路線の経路"><h4>{routeName}</h4><p>所属駅: {selectedRoutePath.member_stations.map(id=>stationsById.get(id)?.name ?? id).join(" → ")}</p><ol>{selectedRoutePath.segments.map((segment,index)=><li key={index}>{(segment.kind==="alignment"?"建設線形 " + (alignments.find(a=>a.id===segment.alignment_id)?.name ?? segment.alignment_id):"接続経路") + ": " + (stationsById.get(segment.from)?.name ?? segment.from) + " → " + (stationsById.get(segment.to)?.name ?? segment.to)}</li>)}</ol></section>}
          </section>
          <section className="service-editor" role="tabpanel" id="workspace-panel-services" aria-labelledby="workspace-tab-services" hidden={activeTool !== "services"}>
            <h3>系統</h3>
            {status==="preview"&&<p className="field-hint">閲覧専用のサンプルです。種別・ダイヤ・列車運行は未実装です。</p>}
            <div className="service-directory"><button className="secondary" disabled={!enabled} onClick={() => {if(serviceDirty||pathServiceDirty||bindingDirty||serviceAppearanceDirty){chooseService(null);return;}chooseService(null);cancelRouteMapPick();setServiceCreating(true);setServiceSequenceKey(key=>key+1);}}>新規系統</button><table aria-label="系統一覧"><thead><tr><th>名前</th><th>乗客</th><th>収支</th><th>車両数</th></tr></thead><tbody>{world?.services.map((service) => <tr key={service.id}><th><button className={`route-choice ${service.id === serviceId ? "selected" : ""}`} disabled={!enabled} onClick={() => chooseService(service.id===serviceId?null:service.id)} aria-label={`系統編集: ${service.name}`} aria-pressed={service.id===serviceId}><ServiceBadge color={world.service_appearances?.find(row=>row.service_id===service.id)?.color??'#3b82f6'} shape={world.service_appearances?.find(row=>row.service_id===service.id)?.shape??'circle'} symbol={world.service_appearances?.find(row=>row.service_id===service.id)?.symbol??initialServiceSymbol(service.name)}/>{service.name}</button></th>{["乗客","収支","車両数"].map(label=><td key={label} title="未実装" aria-label={`${label}: 未実装`}>—</td>)}</tr>)}</tbody></table></div>
            {serviceCreating&&serviceId===null&&<ServiceMapCreator key={`map-service-${pathDraftEpoch}`} world={world??previewWorld} stations={world?.stations??[]} enabled={enabled&&mainTab==="services"} defaultName={(()=>{let n=1;while(world?.services.some(row=>row.name===`系統${n}`))n++;return `系統${n}`;})()} onDirty={setPathServiceDirty} onSave={async draft=>{await createPathService(draft);setServiceCreating(false);cancelRouteMapPick();}} onCancel={()=>{setServiceCreating(false);setPathServiceDirty(false);cancelRouteMapPick();}} onMapPick={(target,select)=>{if(!target){cancelRouteMapPick();return;}if(!enabled||mainTab!=="services")return;routeMapSelect.current=select??null;setRouteMapPick(target);changeMapMode("move");}} mapPickTarget={routeMapPick} startSequenceKey={serviceSequenceKey}/>}
            {!serviceCreating&&serviceId===null&&<ServiceFromRoute key={`route-service-${worldEpoch}-${pathDraftEpoch}`} routes={world?.routes??[]} paths={world?.route_paths??[]} stations={world?.stations??[]} world={world??previewWorld} enabled={enabled&&!serviceDirty&&(!pathServiceDirty||routeServiceDirty)} onDirty={dirty=>{setRouteServiceDirty(dirty);setPathServiceDirty(dirty);}} onSave={createPathService}/>}
            <details><summary>区間・直通を指定して作成</summary>
            <ServicePathCreator key={pathDraftEpoch} routes={world?.routes??[]} routePaths={world?.route_paths??[]} stations={world?.stations??[]} world={world??previewWorld} enabled={enabled} canStart={!serviceDirty&&serviceId===null&&!serviceCreating&&!routeServiceDirty} onDirty={setPathServiceDirty} onSave={createPathService}/></details>

            {pendingServiceChoice && <div className="inline-confirm" role="alert"><p>未適用の系統下書きを破棄して切り替えますか。</p><button className="secondary" disabled={busy} onClick={() => setPendingServiceChoice(null)}>系統編集を続ける</button><button disabled={busy} onClick={() => resetService(world?.services.find((service) => service.id === pendingServiceChoice.id))}>系統下書きを破棄して切替</button></div>}
            {serviceHint&&<p role="status">{serviceHint}</p>}
            {serviceId&&selectedServicePath&&world&&<ServiceDelete key={`delete-${serviceId}-${worldEpoch}`} name={world.services.find(s=>s.id===serviceId)?.name??""} enabled={enabled} hasEdits={serviceDirty||pathServiceDirty||bindingDirty||serviceAppearanceDirty} onDelete={async()=>{
              if(!enabled||busyRef.current||serviceDirty||pathServiceDirty||bindingDirty||serviceAppearanceDirty)throw Error("編集中の変更を適用または取り消してください。");
              const source=world.services.find(s=>s.id===serviceId);if(!source)throw Error("系統が見つかりません。");
              const args={serviceId,expectedName:source.name,expectedPath:selectedServicePath};
              busyRef.current=true;setBusy(true);
              try{const snapshot=await invoke<World>("delete_service",args);setWorld(snapshot);setDirty(true);setConfirmLoad(false);setTrackDeleteRedo(null);setTrackDeleteUndo({kind:"built",world:snapshot,plans:planFingerprint,command:"delete_service",args});resetService();setNotice("系統を削除しました。Ctrl+Zで元に戻せます。保存でファイルに反映します。");}
              finally{busyRef.current=false;setBusy(false);}
            }}/>}
            {serviceId&&world&&<ServiceRouteBindingEditor key={`binding-${serviceId}-${worldEpoch}`} binding={world.service_route_bindings?.find(b=>b.service_id===serviceId)?.binding} routes={world.routes.filter(r=>r.owner===world.services.find(s=>s.id===serviceId)?.owner)} enabled={enabled&&!serviceDirty&&!pathServiceDirty&&!serviceAppearanceDirty} onDirty={setBindingDirty} onSave={async binding=>{if(!enabled||busyRef.current||serviceDirty||pathServiceDirty)throw Error("別の系統編集を適用または破棄してください。");busyRef.current=true;setBusy(true);try{const snapshot=await invoke<World>("set_service_route_binding",{serviceId,binding});const saved=snapshot.service_route_bindings?.find(b=>b.service_id===serviceId)?.binding;if(!sameRouteBinding(saved,binding))throw Error("関連付けの保存結果を確認できません。");setWorld(snapshot);setDirty(true);setConfirmLoad(false);setBindingDirty(false);}finally{busyRef.current=false;setBusy(false);}}}/>}
            {selectedServicePath&&world&&<ServicePathCreator key={`replace-${serviceId}-${pathDraftEpoch}`} routes={world.routes} routePaths={world.route_paths??[]} stations={world.stations} world={world} enabled={enabled} canStart={!serviceDirty&&!bindingDirty&&!serviceAppearanceDirty} onDirty={setPathServiceDirty} replaceService={{id:serviceId!,name:world.services.find(s=>s.id===serviceId)!.name,path:selectedServicePath}} onSave={async draft=>{if(!enabled||busyRef.current||serviceDirty)throw Error("別の系統編集を適用または破棄してください");busyRef.current=true;setBusy(true);try{const snapshot=await invoke<World>("replace_service_path",{serviceId,expectedPath:selectedServicePath,...draft});setWorld(snapshot);setDirty(true);setPathServiceDirty(false);setConfirmLoad(false);resetService(snapshot.services.find(s=>s.id===serviceId));setNotice("経路と停車設定を更新しました。ファイルへは保存してください。");}finally{busyRef.current=false;setBusy(false);}}}/>}
            {serviceId&&selectedServicePath&&world&&<ServiceAppearanceControls key={`appearance-${serviceId}`} color={world.service_appearances?.find(row=>row.service_id===serviceId)?.color??null} serviceName={world.services.find(s=>s.id===serviceId)?.name??''} appearance={world.service_appearances?.find(row=>row.service_id===serviceId)} onSaveAppearance={async(color,shape,symbol)=>{if(!enabled||busyRef.current||serviceDirty||pathServiceDirty||bindingDirty)throw Error("別の系統編集を適用または取り消してください。");busyRef.current=true;setBusy(true);try{const snapshot=await invoke<World>("set_service_appearance",{serviceId,color,shape,symbol,expectedAppearance:world.service_appearances?.find(row=>row.service_id===serviceId)??null});setWorld(snapshot);setDirty(true);setServiceAppearanceDirty(false);setNotice("系統色を適用しました。記号と形も保存対象です。");}finally{busyRef.current=false;setBusy(false);}}} enabled={enabled&&!serviceDirty&&!pathServiceDirty&&!bindingDirty} onDirty={setServiceAppearanceDirty} onDuplicate={async()=>{if(!enabled||busyRef.current||serviceDirty||pathServiceDirty||bindingDirty||serviceAppearanceDirty)throw Error("編集中の変更を適用または取り消してください。");const source=world.services.find(s=>s.id===serviceId);if(!source)throw Error("系統が見つかりません。");busyRef.current=true;setBusy(true);try{const snapshot=await invoke<World>("duplicate_service",{serviceId,expectedPath:selectedServicePath,expectedName:source.name});const created=snapshot.services.find(s=>!world.services.some(old=>old.id===s.id));setWorld(snapshot);setDirty(true);setPathServiceDirty(false);resetService(created);setNotice("別の系統として複製しました。元の系統は変更していません。");}finally{busyRef.current=false;setBusy(false);}}}/>}
            {selectedServicePath&&world&&<ServicePathEditor key={serviceId!} service={{id:serviceId!,name:world.services.find(s=>s.id===serviceId)!.name,start:serviceStart}} path={selectedServicePath} world={world} stations={world.stations} enabled={enabled&&!pathServiceDirty&&!bindingDirty&&!serviceAppearanceDirty} onDirty={setServiceDirty} onSaved={()=>setNotice("系統名と停車設定を適用しました。ファイルへは保存してください。")} onSave={async(name,stops)=>{if(!enabled||busyRef.current)throw Error("処理中です");busyRef.current=true;setBusy(true);try{const snapshot=await invoke<World>("update_service_path",{serviceId,name,stops});setWorld(snapshot);setDirty(true);setConfirmLoad(false);resetService(snapshot.services.find(s=>s.id===serviceId));}finally{busyRef.current=false;setBusy(false);}}}/> }
          </section>
          <section className="company-editor" role="tabpanel" id="workspace-panel-settings" aria-labelledby="workspace-tab-settings" hidden={activeTool !== "company"}>
            <h3>設定</h3><UserGuide /><h4>地図表示</h4><label><input type="checkbox" checked={stationNamesVisible} onChange={event=>setStationNamesVisible(event.target.checked)}/>駅名を表示</label><h4>会社名</h4><p className="muted">この試作では1プレイヤー・1会社です。会社の追加や子会社には未対応です。</p>
            <form onSubmit={submitCompany}><fieldset disabled={!enabled}><label htmlFor="company-name">会社名</label><input id="company-name" value={companyName} maxLength={80} onChange={(event) => setCompanyName(event.target.value)} /><button className="full-width track-submit" type="submit" disabled={!enabled || !companyName.trim() || !companyDirty}>設定変更</button><button className="secondary full-width track-submit" type="button" disabled={!enabled || !companyDirty} onClick={() => setCompanyName(world?.companies[0]?.name ?? "")}>会社名の入力を戻す</button></fieldset></form>
            <FontLicenses />
          </section>
        </aside>
        {!panelCollapsed&&<PanelResizer value={panelWidth} onChange={setPanelWidth}/>}
      </div>
      {companySetupRequired&&<CompanySetup name={companyName} onName={setCompanyName} busy={busy} error={error} errorRevision={errorRevision} onDismissError={()=>setError("")} onStart={startCompany} onLoad={openSlotPicker} onRestore={autosave.available?()=>load("auto"):undefined}/>}
      {slotPicker&&<div className="company-setup-backdrop"><section className="company-setup save-slot-dialog" role="dialog" aria-modal="true" aria-label="ワールドを選択"><h2>セーブデータ</h2><SaveSlots deletedSlots={deletedSaveSlots} onDelete={id=>moveSavedWorld(id,false)} onRestore={id=>moveSavedWorld(id,true)} slots={saveSlots} current={activeSlotId} loading={slotsLoading} error={slotsError} busy={busy} canSave={false} onRefresh={()=>void refreshSaveSlots()} onLoad={id=>requestSlotAction({kind:"load",id})} onSaveAs={()=>{}} onNew={()=>{setSlotPicker(false);setSlotName("");setSlotDialog("new");}}/><button disabled={busy} onClick={()=>setSlotPicker(false)}>閉じる</button>{error&&<p role="alert">{error}</p>}</section></div>}
      {slotDialog&&<div className="company-setup-backdrop"><section className="company-setup" role="dialog" aria-modal="true" aria-label={slotDialog==="new"?"新しいワールド":"別のセーブとして保存"}><h2>{slotDialog==="new"?"新しいワールド":"別のセーブとして保存"}</h2><form autoComplete="off" onSubmit={submitSlotName}><label>名前<input autoComplete="off" autoFocus maxLength={80} value={slotName} disabled={busy} onChange={event=>setSlotName(event.target.value)}/></label><button type="submit" disabled={busy||!slotName.trim()||confirmLoad}>{slotDialog==="new"?"作成":"保存"}</button><button type="button" disabled={busy} onClick={()=>{setSlotDialog(null);setSlotAction(null);setConfirmLoad(false);}}>キャンセル</button></form>{error&&<p role="alert">{error}</p>}</section></div>}
      {confirmLoad && <div className="banner load-confirm" style={{position:"fixed",zIndex:2000,left:"20%",right:"20%",top:"20%"}} role="alert"><p>{slotAction?.kind==="new"?"新しいワールドを作成すると、現在の未保存の変更と未適用の下書きは破棄されます。保存済みのワールドは変更しません。":"読込すると、現在の未保存の変更と未適用の下書きが破棄され、未着工計画を含め保存時点の状態に置き換わります。"}</p><button className="secondary" disabled={busy} onClick={() => {setConfirmLoad(false);setSlotAction(null);}}>戻る</button><button disabled={busy} onClick={()=>slotAction?applySlotAction():load()}>{slotAction?.kind==="new"?"変更を破棄して作成":"読込を続ける"}</button></div>}
      <footer className="app-footer"><p role="status" aria-live="polite">{busy ? (roadCheckPending ? "道路を確認中（最大25秒）。未取得の場合も通常建設を続行します。" : "ワールドを更新しています。") : notice || "線路 → 駅 → 運行経路の順に建設"}</p><span>列車種別・ダイヤ・列車運行・旅客流動は未実装</span></footer>
    </main>
  );
}
