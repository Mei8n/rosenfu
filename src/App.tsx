import {OFM_EVIDENCE_RULE,type OpenFreeMapEvidenceRequest} from "./openFreeMapRoadData";
import {InstalledSeamBusPanel,type AcquireSeamBusArgs,type InstallSeamBusArgs} from "./InstalledSeamBusPanel";
import {StationWalkLinks,type StationWalkEdit,type StationWalkLink} from "./StationWalkLinks";
import {StationWalkMapLayer,selectedStationPassages} from "./StationWalkMapLayer";
import {OrdinaryBusPanel,useOrdinaryBusDraft} from './OrdinaryBusPanel';
import {OrdinaryBusStopsPanel,useOrdinaryBusStops} from './OrdinaryBusStopsPanel';
import {prepareOrdinaryBusQuery,readObservedRoadTilePairs,observedTramRoadAnchor,prepareAuthoredTramChainQuery,type OrdinaryBusPath,type OrdinaryBusPreviewReply,type OrdinaryBusQuery} from './ordinaryBusDraft';
import {fitRoadFollow} from './roadFollowFit';
import {isRoadFollowEdgeEligible} from './roadFollowing';
import type {RoadDrawingResult} from './roadDrawingEngine';
import {bindTramRoadSnap,reacquireTramRoadSeam,type AcceptedTramRoadSnap} from './reacquireTramRoadSeam';
import {BusOperations,CrossingOperations,OperationClock,OperationProvider,OperationVehicleLayer,ServiceOperations,ServicePathChangeNotice} from "./OperationsPanel";
import {RoadTransportPanel,type RoadTransportData,type RoadTransportMutation,type RoadTransportArgs,type RoadMapSelection,type RoadPlayerMapRequest,type RoadPlayerMapSelection} from "./RoadTransportPanel";
import {RailJourneyReplayPanel} from "./RailJourneyReplayPanel";
import {FixedOdPanel,type FixedOdScenario,type FixedOdScenarioView} from "./FixedOdPanel";
import {mapRenderMetrics} from './mapRenderMetrics';
import {ServiceDirectory} from "./ServiceDirectory";
import { EditingMapFrame } from "./EditingMapFrame";
import {RouteStationSelection} from './RouteStationSelection';
import {EditHistory,planEditKey,worldEditKey,sameWorldEdit,FINAL_WORLD_COMMANDS,UNDO_WORLD_COMMANDS} from "./editHistory";
import { ServiceDelete } from "./ServiceDelete";
import {ServiceFromRoute} from './ServiceFromRoute';
import {ServiceMapCreator} from './ServiceMapCreator';
import {StationDirectory} from './StationDirectory';
import {stationRouteNames} from './stationRouteNames';
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
import {TrackGeometryLayer,useTrackRenderGeometry} from './TrackRenderGeometry';
import {sharedServiceSections,serviceBandPaths,serviceRenderChains} from './serviceMapAppearance';
import "./PlanBreakdown.css";
import {ServiceAppearanceControls,initialServiceSymbol, type ServiceSymbolShape} from './ServiceAppearanceControls';
import {JunctionPoints} from './JunctionPoints';
import {nearestJunctionSource,junctionDirection,junctionPrevious,type JunctionSource} from './junctionSelection';
import {constructionPlanFingerprint} from './constructionPlans';
import {DEFAULT_INFRASTRUCTURE,COUNT_LABELS,LEVEL_LABELS,endpointInfrastructure,displayInfrastructureSections,schematicTrackPaths,type SectionInfrastructure} from './sectionInfrastructure';
import {mergeSectionSelections,selectAlignmentSections,selectionBounds,type SelectionBox,type SectionSelection} from "./boxSelection";
import "./boxSelection.css";
import {HeightControls} from './HeightControls';
import {SaveSlots,type ArchivedSaveSlot,type SaveSlot} from "./SaveSlots";
import {sameRouteBinding,ServiceRouteBindingEditor,type ServiceRouteBindingRecord} from "./ServiceRouteBinding";
import {RouteDirectory} from "./RouteDirectory";
import {PARALLEL_RANGE_M,parallelProjection,offsetTrack,parallelSection} from "./parallelTrack";
import {TrackFollowIcon} from "./TrackFollowIcon";
import {firstByKey} from "./mapLookupIndex";
import {stationGroupViews,footprintsIntersect,type StationGroup} from "./stationGroups";
import {twoEndedJoin} from "./twoEndedJoin";
import {DeletionDialog} from "./DeletionDialog";
import {removePlannedTarget,type PlannedDeleteTarget} from "./plannedDeletion";
import {deletionSections,anchorIndices} from "./deletionSections";
import {branchStemEnds,destinationJoinError,endpointKey,mixedTramTracks,occupiedEndpoint,type BranchPair} from "./destinationJoin";
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
import { routeAppearance, routeSegmentSections } from "./routeAppearance";
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
import type { Segment, Membership, Candidate } from "./RouteCandidatePreview";
import {ServiceMapEditor} from "./ServiceMapEditor";
import { RouteCandidatePreview } from "./RouteCandidatePreview";
import { MAP_PROVIDERS } from "./mapProviders";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { MouseEvent, FormEvent, CSSProperties, PointerEvent as ReactPointerEvent } from "react";
import { invoke as nativeInvoke, isTauri } from "@tauri-apps/api/core";
import { MAP_BOUNDS as BOUNDS, MAP_VIEWBOX, GSI_TILES, project, coordinatesFromClient, DEFAULT_CAMERA, cameraAt, panCamera, visibleTiles, requestedTileCoverage, viewBounds, mapViewport, vehicleViewport } from "./map";
import { GsiBackground } from "./GsiBackground";
import type { Camera } from "./map";
import {trackBounds,trackViewport,intersectsTrackView} from "./trackViewport";
import {RetainedRoadLayer,useRetainedRoadView} from "./RetainedRoadLayer";
import type { TileStatus } from "./GsiBackground";
import { provisionalCurveSpeed, snapStraight } from "./circularAlignment";
import { nearestOnAlignment, stationPolygon, alignmentLength, distanceM, pointOnAlignment } from "./alignment";
type GeoPoint = { latitude: number; longitude: number };
type StreetTramPreview={
  world:World; basis:GeoPoint[]; plan:number; path:number;
  continuation?: Extension;
  args:Pick<OrdinaryBusQuery,"expectedWorldToken"|"rawEvidence"|"tileBinding"|"contextTiles">&(
    {guide:Pick<OrdinaryBusPath,"tileKey"|"tileRawSha256"|"start"|"end">&{startFractionQ:number;endFractionQ:number;adjacentEnd?:{tileKey:string;tileRawSha256:string}}}|
    {chainGuide:Awaited<ReturnType<typeof prepareAuthoredTramChainQuery>>["chainGuide"]}
  );
  points:GeoPoint[];
};
type Alignment = { infrastructure?:SectionInfrastructure[]; anchor_indices?:number[]|null; id: string; owner: string; name: string; points: GeoPoint[]; track_type: TrackType; design_speed_kmh?:number|null };
type StationPlacement = { station_id: string; alignment_id: string; position: number; length_m: number; width_m: number };

type EngineInfo = { standard_schedule_enabled?: boolean; crossing_planning_enabled?: boolean; authored_tram_road_guide_enabled?: boolean; authored_street_tram_enabled?: boolean; authored_adjacent_street_tram_enabled?: boolean; authored_chain_street_tram_enabled?: boolean; ordinary_ofm_bus_enabled?: boolean; ordinary_ofm_bus_fractional_enabled?: boolean; ordinary_ofm_bus_ground_fractional_enabled?: boolean; ordinary_ofm_bus_ground_fractional_stops_enabled?: boolean; ordinary_ofm_bus_rectangle_enabled?: boolean; ordinary_ofm_bus_multiple_enabled?: boolean; installed_seam_bus_enabled?: boolean; ordinary_ofm_bus_round_trip_enabled?: boolean; ordinary_ofm_bus_waypoints_enabled?: boolean; ordinary_ofm_bus_interior_stops_enabled?: boolean; ordinary_ofm_bus_auto_stops_enabled?: boolean; ordinary_ofm_bus_closed_loop_enabled?: boolean; ordinary_ofm_bus_observed_grade_enabled?: boolean; road_transport_enabled?: boolean; road_tram_enabled?: boolean; bus_operation_enabled?: boolean; name: string; version: string; company_setup_required?: boolean; operation_enabled?: boolean; fixed_od_enabled?: boolean; rail_journey_replay_enabled?: boolean; fixed_od_scenario_enabled?: boolean };
type Station = { id: string; owner: string; name: string; latitude: number; longitude: number };
type Route = { id: string; owner: string; name: string; start: string };
type OperatingService = { id: string; owner: string; name: string; start: string };
type RoutePath = { route_id: string; segments: Segment[]; member_stations: string[] };
type StreetTramEndSource={
  alignment_id:string;end_point_index:number;to_position:number;
  path:{method:string;end:{fragmentId:string;segmentIndex:number;fraction:number};
    tramGuide?:{tileKey:string;tileRawSha256:string;adjacentEnd?:{tileKey:string;tileRawSha256:string}};
    tramChainGuide?:{tiles:{tileKey:string;tileRawSha256:string}[]}
  };
};
type World = {
  road_path_provenance?:StreetTramEndSource[];
  road_transport?:RoadTransportData;
  ofm_gameplay_bus?:OrdinaryBusPath|null;
  ofm_gameplay_bus_routes?:Record<string,OrdinaryBusPath>;
  ofm_installed_seam_bus_routes?:Record<string,{version:1;catalogKey:string}>;
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
  station_walk_links?: StationWalkLink[];
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
type Status = "loading" | "preview" | "ready" | "error";

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

function mapPath(points: GeoPoint[], camera: Camera) {
  return points.map((point, index) => {
    const p = project(point.latitude, point.longitude, camera);
    return (index ? "L" : "M") + p.x + "," + p.y;
  }).join(" ");
}

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

  const svgRef = useRef<SVGSVGElement | null>(null);
  const cameraRef = useRef<Camera>(DEFAULT_CAMERA);
  const [pixelRatio, setPixelRatio] = useState(() => window.devicePixelRatio || 1);
  const [mapSize, setMapSize] = useState<{width:number;height:number}>(MAP_VIEWBOX);
  const gestureRef = useRef<{ pointerId: number; startX: number; startY: number; camera: Camera; scale: number; dragged: boolean; mayPlace: boolean; busControlId?:string; busLegAfter?:string } | null>(null);
  const boxGesture=useRef<{pointerId:number;startX:number;startY:number;box:SelectionBox;camera:Camera;additive:boolean}|null>(null);
  const [selectionBox,setSelectionBox]=useState<SelectionBox|null>(null);
  const [rangeSelection,setRangeSelection]=useState<SectionSelection[]>([]);
  const [confirmRangeDelete,setConfirmRangeDelete]=useState(false);
  const [rangeTrackCount,setRangeTrackCount]=useState<1|2|4|null>(null);
  const [rangeTrackType,setRangeTrackType]=useState<"normal"|"high_speed"|null>(null);
  const [rangeLevel,setRangeLevel]=useState<SectionInfrastructure["level"]|null>(null);
  const suppressMapClick = useRef(false);
  const suppressSelectionClick = useRef(false);
  const [mapMode, setMapMode] = useState<"move" | "place" | "draw">("move");
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
  const markerViewport=useMemo(()=>vehicleViewport(camera,mapSize),[camera,mapSize]);
  const bounds = useMemo(()=>viewBounds(camera,mapSize),[camera,mapSize]);
  const [manualGraphBundle,setGraphBundle]=useState<ReferenceGraphBundle|null>(null);
  const [graphVisible,setGraphVisible]=useState(true);
  const [retainedRoadVisible,setRetainedRoadVisible]=useState(false);
  const [pendingGraphChange,setPendingGraphChange]=useState<{bundle:ReferenceGraphBundle|null}|null>(null);
  const [candidateDirty,setCandidateDirty]=useState(false);
  const [routeRenameDirty,setRouteRenameDirty]=useState(false);
  const [referencePack,setReferencePack]=useState<ReferencePack|null>(null);
  const [referenceVisible,setReferenceVisible]=useState(true);
  const referenceRepository=useMemo(()=>referencePack?createReferenceRepository(referencePack):null,[referencePack]);
  const referenceResult=useMemo(()=>referenceRepository?.query(bounds) ?? {stations:[],total:0},[referenceRepository,bounds]);
  const cameraPending = camera !== requestedCamera;
  const lastRasterRequestAt=useRef(-Infinity);
  const rasterCoverageLow=background!=="sample"&&cameraPending&&requestedTileCoverage(requestedTiles,camera,mapSize)<0.6;
  // Anchor the fictional offline scene to the same camera as railway geometry.
  const sampleScale = 2 ** (camera.zoom - DEFAULT_CAMERA.zoom);
  const sampleOrigin = project(DEFAULT_CAMERA.latitude, DEFAULT_CAMERA.longitude, camera);
  const sampleTransform = "translate(" + (sampleOrigin.x - MAP_VIEWBOX.width / 2 * sampleScale) + " " + (sampleOrigin.y - MAP_VIEWBOX.height / 2 * sampleScale) + ") scale(" + sampleScale + ")";
  useEffect(() => {
    if (!cameraPending) return;
    // Throttle tile requests during movement instead of postponing them until
    // the user stops dragging. Read the newest camera when the window ends.
    const timer = window.setTimeout(() => {
      if(performance.now()-lastRasterRequestAt.current<250)return;
      lastRasterRequestAt.current=performance.now();
      setRequestedCamera(cameraRef.current);
    }, 250);
    return () => window.clearTimeout(timer);
  }, [cameraPending, requestedCamera]);
  useEffect(() => {
    if (!rasterCoverageLow) return;
    // A large jump can uncover most of the old raster footprint. Coalesce
    // loss for 125 ms, without exceeding the usual rate during sustained motion.
    const wait=Math.max(125,250-Math.max(0,performance.now()-lastRasterRequestAt.current));
    const timer=window.setTimeout(()=>{
      if(performance.now()-lastRasterRequestAt.current<250)return;
      lastRasterRequestAt.current=performance.now();
      setRequestedCamera(cameraRef.current);
    },Math.ceil(wait));
    return ()=>window.clearTimeout(timer);
  }, [rasterCoverageLow,requestedCamera]);
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
  const [deletedSaveSlots,setDeletedSaveSlots]=useState<ArchivedSaveSlot[]>([]);
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
  const [operationRunning,setOperationRunning]=useState(false);
  const [operationRevision,setOperationRevision]=useState(0);
  const [railLoadEpoch,setRailLoadEpoch]=useState(0);
  const [restoredAutosaveEpoch,setRestoredAutosaveEpoch]=useState(-1);
  const [mainTab,setMainTab]=useState<WorkspaceTab>("construction");
  const [routeMapEdit,setRouteMapEdit]=useState<{id:string;name:string;path?:import("./RouteCandidatePreview").Candidate;memberStations?:string[];excludedStations?:string[]}|null>(null);
  const [routeSequenceKey,setRouteSequenceKey]=useState(0);
  const [routeCreating,setRouteCreating]=useState(false);
  const [routeServiceDirty,setRouteServiceDirty]=useState(false);
  const [serviceCreating,setServiceCreating]=useState(false);
  const [serviceSequenceKey,setServiceSequenceKey]=useState(0);
  const [routeSelectedCandidate,setRouteSelectedCandidate]=useState<Candidate|null>(null);
  const [routeSelectedStations,setRouteSelectedStations]=useState<string[]>([]);
  const [routeMapPick,setRouteMapPick]=useState<"start"|"end"|"via"|"sequence"|null>(null);
  const routeMapSelect=useRef<((id:string)=>void)|null>(null);
  function cancelRouteMapPick(){routeMapSelect.current=null;setRouteMapPick(null);}
  useEffect(()=>{cancelRouteMapPick();},[mainTab,worldEpoch]);
  useEffect(()=>{setRouteMapEdit(null);setRouteSequenceKey(0);setRouteCreating(false);},[worldEpoch]);
  const [companySetupRequired,setCompanySetupRequired]=useState(false);
  const retainedRoadBasis=useMemo(()=>retainedRoadVisible?JSON.stringify(world?.road_evidence_descriptors??[]):"",[retainedRoadVisible,world?.road_evidence_descriptors]);
  const retainedRoadDisplay=retainedRoadVisible&&isTauri()&&status==="ready"&&!companySetupRequired;
  const retainedRoad=useRetainedRoadView(retainedRoadDisplay,bounds,worldEpoch,retainedRoadBasis);
  const [activeTool, setActiveTool] = useState<"stations" | "tracks" | "routes" | "services" | "company" | "station-management" | "delete" | "select" | "road-transport" | "ordinary-bus" | "inactive">("tracks");
  const [roadMapFrom,setRoadMapFrom]=useState<string|null>(null);
  const [roadMapSelection,setRoadMapSelection]=useState<RoadMapSelection>();
  const [roadPlayerMapPick,setRoadPlayerMapPick]=useState<{request:RoadPlayerMapRequest;graph:RoadTransportData;epoch:number;edge:RoadTransportData['edges'][number]}>();
  const [roadPlayerMapSelection,setRoadPlayerMapSelection]=useState<{selection:RoadPlayerMapSelection;graph:RoadTransportData;epoch:number}>();
  const roadMapRevision=useRef(0);
  function cancelRoadMapPick(){setRoadMapFrom(null);setRoadPlayerMapPick(undefined);}
  useEffect(()=>{cancelRoadMapPick();setRoadMapSelection(undefined);setRoadPlayerMapSelection(undefined);},[mainTab,activeTool,worldEpoch,world?.road_transport]);
  function chooseMainTab(tab:WorkspaceTab){if(seamMutationPending.current)return;setError("");setMainTab(tab);setPanelCollapsed(false);setActiveTool(tab==="construction"?"tracks":tab==="stations"?"station-management":tab==="settings"?"company":tab==="routes"?"routes":tab==="services"?"services":"inactive");changeMapMode(tab==="construction"?"draw":"move");}
  const [panelCollapsed, setPanelCollapsed] = useState(false);
  // Internal compatibility label only; physical tracks are not user-named routes.
  const alignmentName = "新しい線路";
  const [draftFinished, setDraftFinished] = useState(false);
  const [draftPoints, setDraftPoints] = useState<GeoPoint[]>([]);
  const [streetTramPreview,setStreetTramPreview]=useState<StreetTramPreview|null>(null);
  const [draftEditedGeometry,setDraftEditedGeometry]=useState<EditedGeometry|undefined>();
  type AnchorEdit={kind?:"point";id:number;before:{points:GeoPoint[];geometry?:EditedGeometry;redo:GeoPoint[];extension?:Extension|null;destination?:Extension;redoDestination?:Extension;anchorLevels?:SectionInfrastructure["level"][];redoAnchorLevels?:SectionInfrastructure["level"][];startLevel?:SectionInfrastructure["level"];infrastructure?:SectionInfrastructure};after:{points:GeoPoint[];geometry?:EditedGeometry;redo:GeoPoint[];extension?:Extension|null;destination?:Extension;redoDestination?:Extension;anchorLevels?:SectionInfrastructure["level"][];redoAnchorLevels?:SectionInfrastructure["level"][];startLevel?:SectionInfrastructure["level"];infrastructure?:SectionInfrastructure}};
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
  const [draftExtension,setDraftExtension]=useState<Extension|null>(null);
  const [draftDestination,setDraftDestination]=useState<Extension|undefined>();
  const [draftRedoDestination,setDraftRedoDestination]=useState<Extension|undefined>();
  const [parkedPaths, setParkedPaths] = useState<PlannedPath[]>([]);
  const [activePathId, setActivePathId] = useState(1);
  const [branchPicker,setBranchPicker]=useState<{epoch:number;basis:World;source:string;destination:string}|null>(null);
  const [branchPermit,setBranchPermit]=useState<(BranchPair&{epoch:number;basis:World;pathId:number;sourceAnchor:GeoPoint})|null>(null);

  const [plans,setPlans]=useState([{id:1,name:""}]);
  const [activePlanId,setActivePlanId]=useState(1);
  const nextPlanId=useRef(2);
  const planName=plans.find(plan=>plan.id===activePlanId)?.name??"";
  function setPlanName(name:string){setPlans(items=>items.map(plan=>plan.id===activePlanId?{...plan,name}:plan));}
  const [draftJunctionSource,setDraftJunctionSource]=useState<JunctionSource|undefined>(undefined);
  const [draftAnchorLevels,setDraftAnchorLevels]=useState<SectionInfrastructure["level"][]|undefined>(undefined);
  const [redoAnchorLevels,setRedoAnchorLevels]=useState<SectionInfrastructure["level"][]>([]);
  const [builtHeightSelection,setBuiltHeightSelection]=useState<BuiltHeightSelection|null>(null);
  const [heightAnchor,setHeightAnchor]=useState<{id:number;index:number}|null>(null);
  const [startLevel,setStartLevel]=useState<SectionInfrastructure["level"]|undefined>(undefined);
  const [infrastructure,setInfrastructure]=useState<SectionInfrastructure>(DEFAULT_INFRASTRUCTURE);
  const [trackType,setTrackType]=useState<TrackType>("normal");
  const selectedTrackType=useRef<TrackType|null>(null);
  const [designSpeedKmh,setDesignSpeedKmh]=useState(120);
  const [roadFollowing,setRoadFollowing]=useState(false);
  const [tramCtrl,setTramCtrl]=useState(false);
  const [tramOverridePrepared,setTramOverridePrepared]=useState(false);
  const tramDrawing=trackType==="tram"&&activeTool==="tracks"&&mapMode==="draw";
  const nativeTramGuide=tramDrawing&&info?.authored_tram_road_guide_enabled===true;
  const usesRoadSnap=(ctrl:boolean)=>trackType==="tram"?roadFollowing!==ctrl:roadFollowing&&!ctrl;
  const roadSnapShown=tramDrawing?usesRoadSnap(tramCtrl):roadFollowing;
  const roadDrawingEnabled=roadFollowing||(tramDrawing&&(tramCtrl||tramOverridePrepared));
  function updateTramCtrl(held:boolean){
    setTramCtrl(held);
    // An on-demand source/worker stays available for this drawing tool until
    // switching tools. Releasing Ctrl cannot cancel a click already sent.
    if(held&&!roadFollowing)setTramOverridePrepared(true);
  }
  useEffect(()=>{setTramCtrl(false);setTramOverridePrepared(false);},[trackType,activeTool,mapMode,worldEpoch]);
  const [legacyRoadDraft,setLegacyRoadDraft]=useState(false);
  const [referencePanel,setReferencePanel]=useState(false);
  const [parallelMode,setParallelMode]=useState(false);
  const tramRoadGuide=tramDrawing&&roadSnapShown&&!parallelMode;
  const roadSnapMarkerClass=tramRoadGuide?"road-snap-point tram-road-guide-marker":"road-snap-point";
  const [parallelStart,setParallelStart]=useState<{sourceId:string;offset:number;point:GeoPoint}|null>(null);
  function cancelParallel(){setParallelStart(null);}
  useEffect(()=>{cancelParallel();},[activeTool,activePlanId,world]);
  const roadFollowingBlocked=false;
  const [seamStageSending,setSeamStageSending]=useState(false);
  const seamMutationPending=useRef(false);
  const [frozenRoadTiles,setFrozenRoadTiles]=useState<readonly DraftTileSnapshot[]>([]);
  const [frozenRoadBundle,setFrozenRoadBundle]=useState<ReferenceGraphBundle|null>(null);
  const ordinaryBusAvailable=info?.ordinary_ofm_bus_enabled===true;
  const ordinaryBusActive=ordinaryBusAvailable&&mainTab==="construction"&&activeTool==="ordinary-bus";
  const automaticRoad=useAutomaticRoad(bounds,status==="ready"&&(roadDrawingEnabled||ordinaryBusActive),worldEpoch,!!frozenRoadBundle&&!ordinaryBusActive);
  const roadDrawingBundle=frozenRoadBundle??automaticRoad.snapshot?.bundle??manualGraphBundle;
  const graphBundle=roadFollowing?roadDrawingBundle:manualGraphBundle;
  const graphDisplayBundle=tramDrawing?(tramRoadGuide?roadDrawingBundle:manualGraphBundle):graphBundle;
  const graphIndex=useMemo(()=>graphDisplayBundle?indexGraphDisplay(graphDisplayBundle.graph):[],[graphDisplayBundle]);
  const tramRoadGuideIds=useMemo(()=>tramRoadGuide&&roadDrawingBundle?new Set(roadDrawingBundle.graph.edges.filter(edge=>isRoadFollowEdgeEligible(roadDrawingBundle,edge)).map(edge=>edge.id)):undefined,[tramRoadGuide,roadDrawingBundle]);
  const graphDisplay=useMemo(()=>queryGraphDisplay(graphIndex,viewBounds(camera,mapSize),camera.zoom,graphVisible),[graphIndex,camera,mapSize,graphVisible]);

  function chooseTrackType(value:TrackType){selectedTrackType.current=value;cancelParallel();setParallelMode(false);setTrackType(value);setDesignSpeedKmh(defaultDesignSpeed(value));setRoadFollowing(value==="tram");setReferencePanel(value==="tram");setHoverPoint(null);}
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
  const [junctionSelection,setJunctionSelection]=useState<{epoch:number;id:string}|null>(null);
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
  const alignments = useMemo(()=>world?.alignments ?? [],[world?.alignments]);
  const branchSelection=branchPicker?.epoch===worldEpoch&&branchPicker.basis===world?branchPicker:null;
  const currentBranchPermit=branchPermit?.epoch===worldEpoch&&branchPermit.basis===world?branchPermit:null;
  // A path number alone can be reused by clearing or restoring plans. The
  // original in-memory source anchor survives normal park/resume and Undo,
  // but is never shared by a new draft or by serialized restoration.
  function branchForDraft(id:number,points:GeoPoint[],extension:Extension|null,destination:Extension|undefined){
    const branch=currentBranchPermit;
    return branch&&branch.pathId===id&&branch.sourceAnchor===points[0]&&extension&&
      endpointKey(extension)===endpointKey(branch.source)&&
      (!destination||endpointKey(destination)===endpointKey(branch.destination))?branch:null;
  }
  const activeBranch=branchForDraft(activePathId,draftPoints,draftExtension,draftDestination);
  useEffect(()=>{
    setBranchPicker(picker=>picker?.basis===world?picker:null);
    setBranchPermit(permit=>permit?.basis===world?permit:null);
  },[world]);

  const branchEnds=useMemo(()=>(branchSelection||currentBranchPermit)
    ?branchStemEnds(alignments,world?.alignment_connections??[],world?.companies[0]?.id)
    :[],[alignments,world,!!branchSelection,!!currentBranchPermit]);
  const selectedBranchStart=branchEnds.find(end=>end.key===branchSelection?.source);
  const selectedBranchEnd=branchEnds.find(end=>end.key===branchSelection?.destination);
  const branchResumes=!!draftExtension&&!!selectedBranchStart&&endpointKey(draftExtension)===selectedBranchStart.key&&
    (!draftDestination||endpointKey(draftDestination)===selectedBranchEnd?.key)&&trackType==="normal"&&!draftJunctionSource;
  const branchDraftAvailable=(branchResumes||!draftPoints.length&&!redoPoints.length)&&
    (!currentBranchPermit||currentBranchPermit.pathId===activePathId||!parkedPaths.some(path=>branchForDraft(path.id,path.points,path.extension,path.destination)));
  function branchEndLabel(end:(typeof branchEnds)[number]){
    const inside=end.endpoint==="start"?end.alignment.points[1]:end.alignment.points.at(-2);
    if(!inside)return end.alignment.name;
    const x=(inside.longitude-end.point.longitude)*Math.cos(end.point.latitude*Math.PI/180);
    const y=inside.latitude-end.point.latitude;
    const compass=Math.abs(x)>Math.abs(y)?x<0?"西":"東":y<0?"南":"北";
    return `${end.alignment.name}・${end.endpoint==="start"?"始点":"終点"}（線路は${compass}側）`;
  }


  const alignmentsById=useMemo(()=>firstByKey(alignments,line=>line.id),[alignments]);
  const stationPlacementById=useMemo(()=>firstByKey(world?.station_placements??[],placement=>placement.station_id),[world?.station_placements]);
  const trackRender=useTrackRenderGeometry(world,status==="ready"&&(!!world?.alignment_connections?.length||alignments.some(line=>line.infrastructure?.some((section,index)=>index>0&&section.track_count!==line.infrastructure![index-1].track_count))),worldEpoch);
  const detailedStations = camera.zoom >= 13;
  const [stationResizePending,setStationResizePending]=useState(false);
  const [stationResizeFailure,setStationResizeFailure]=useState<string|null>(null);
  const stationPolygons=useMemo(()=>new Map((world?.station_placements??[]).map(placement=>{
    const alignment=alignmentsById.get(placement.alignment_id);
    return [placement.station_id,alignment?stationPolygon(alignment.points,placement.position,placement.length_m,placement.width_m):null];
  })),[world?.station_placements,alignmentsById]);
  const selectedRouteSections = useMemo(()=>world&&routeSelectedCandidate?routeSegmentSections(world,routeSelectedCandidate.segments):new Map(),[world,routeSelectedCandidate]);
  function candidateLength(path:Candidate){
    if(!world)return Infinity;
    return path.segments.reduce((total,segment)=>{const sections=routeSegmentSections(world,[segment]);return total+(sections.size?[...sections.values()].flat().reduce((sum,points)=>sum+alignmentLength(points),0):Infinity);},0);
  }
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
  const previewAnchors=hoverPoint && !draftFinished && draftPoints.length && mapMode === "draw" ? [...draftPoints, hoverPoint] : draftPoints;
  const previewGeometry = previewAnchors===draftPoints?draftGeometry:plannedGeometry(previewAnchors,fittedDestination(previewAnchors,draftEditedGeometry,hoverPoint&&cursorPosition?destinationTarget(hoverPoint,cursorPosition.width,cursorPosition.height):null),previousPoint);
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
    return draftPoints.length || redoPoints.length ? {id:activePathId,planId:activePlanId,name:planName,points:draftPoints,redo:redoPoints,line:draftLine,editedGeometry:draftEditedGeometry,designSpeedKmh,trackType,infrastructure,startLevel,anchorLevels:draftAnchorLevels,redoAnchorLevels,junctionSource:directedJunction,roadFollowing:legacyRoadDraft,extension:draftExtension,destination:draftDestination,redoDestination:draftRedoDestination} : null;
  }
  function newPath(discardEmptyRedo=false) {
    setLegacyRoadDraft(false);
    // A new drawing replaces a fully undone path; switching plans still preserves its Redo.
    const discard=discardEmptyRedo&&!draftPoints.length;
    const current=discard?null:parkCurrentPlan();
    if(discard)discardPlanStations(activePathId);
    if(current)parkAnchorHistory();
    setParkedPaths(paths=>current?[...paths,current]:paths);
    setDraftAnchorLevels(undefined);setRedoAnchorLevels([]);setHeightAnchor(null);setDraftJunctionSource(undefined);setStartLevel(undefined);setDraftFinished(false);const id=nextPathId.current++;setActivePathId(id);setDraftExtension(null);setDraftDestination(undefined);setDraftRedoDestination(undefined);setDraftPoints([]);setDraftEditedGeometry(undefined);setAnchorUndo([]);setAnchorRedo([]);setRedoPoints([]);setHoverPoint(null);setDraftExtension(null);setDraftDestination(undefined);setDraftRedoDestination(undefined);changeMapMode(activeTool==="stations"?"place":"draw");return id;
  }
  function newIndependentPlan() {
    if (!enabled || referenceCopy.active) return;
    newPath();const id=nextPlanId.current++;setPlans(items=>[...items,{id,name:""}]);setActivePlanId(id);
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
    selectedTrackType.current=null;
    const current=parkCurrentPlan();
    if(current)parkAnchorHistory();
    const history=parkedAnchorHistory.current.get(id);parkedAnchorHistory.current.delete(id);
    setParkedPaths(paths=>[...paths.filter(path=>path.id!==id),...(current?[current]:[])]);
    if(resume&&endpoint==="start"&&target.points.length>1&&!target.extension){
      const reversed=reversePlanForExtension(target);
      setAnchorUndo([...(history?.undo??[]),reversed.edit]);setAnchorRedo([]);
      target.anchorLevels=reversed.edit.after.anchorLevels;target.redoAnchorLevels=[];target.startLevel=reversed.edit.after.startLevel;target.infrastructure=reversed.edit.after.infrastructure;target.points=reversed.points;target.editedGeometry=reversed.geometry;target.redo=[];target.extension=target.destination??null;target.destination=undefined;target.redoDestination=undefined;
    }else {setAnchorUndo(history?.undo??[]);setAnchorRedo(history?.redo??[]);}
    setActivePlanId(target.planId);setDraftFinished(!resume||!!target.destination);setActivePathId(target.id);setDraftExtension(target.extension);setDraftDestination(target.destination);setDraftRedoDestination(target.redoDestination);setDraftPoints(target.points);setDraftEditedGeometry(target.editedGeometry);setRedoPoints(target.redo);setDraftJunctionSource(target.junctionSource);setDraftAnchorLevels(target.anchorLevels);setRedoAnchorLevels(target.redoAnchorLevels??[]);setHeightAnchor(null);setStartLevel(target.startLevel);setInfrastructure(target.infrastructure??DEFAULT_INFRASTRUCTURE);setTrackType(target.trackType);setDesignSpeedKmh(target.designSpeedKmh);setRoadFollowing(target.roadFollowing);setLegacyRoadDraft(target.roadFollowing);setHoverPoint(null);changeMapMode(activeTool==="stations"?"place":"draw");
  }
  function choosePlan(id:number){
    if(id===activePlanId)return;
    const path=parkedPaths.find(path=>path.planId===id&&!path.reference&&!path.ofmPath);
    if(path)selectPath(path.id,false);else{newPath();setActivePlanId(id);}
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
    setBranchPicker(null);
    if(currentBranchPermit&&ids.has(currentBranchPermit.pathId))setBranchPermit(null);
    ids.forEach(id=>parkedAnchorHistory.current.delete(id));
    ids.forEach(discardPlanStations);setPlannedStations(items=>items.filter(station=>stationPlanId(station)!==activePlanId));setStationUndo(items=>items.filter(entry=>!entry.planned||stationPlanId(entry.planned)!==activePlanId));setStationRedo(items=>items.filter(entry=>!entry.planned||stationPlanId(entry.planned)!==activePlanId));setParkedPaths(paths=>paths.filter(path=>path.planId!==activePlanId));
    setDraftFinished(false);setDraftAnchorLevels(undefined);setRedoAnchorLevels([]);setHeightAnchor(null);setDraftPoints([]);setDraftEditedGeometry(undefined);setAnchorUndo([]);setAnchorRedo([]);setRedoPoints([]);setHoverPoint(null);setDraftExtension(null);setDraftDestination(undefined);setDraftRedoDestination(undefined);
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
  const hoveredPlan = planHover?.id === activePathId ? {name:planName,line:draftLine,points:draftPoints,anchorLevels:draftAnchorLevels,startLevel,infrastructure,editedGeometry:draftEditedGeometry,reference:undefined,designSpeedKmh,extension:draftExtension,destination:draftDestination,redoDestination:draftRedoDestination} : parkedPaths.find(plan=>plan.id===planHover?.id);
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
  const viewport = useMemo(()=>mapViewport(mapSize.width,mapSize.height),[mapSize.width,mapSize.height]);
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
  const drawPath = (points: GeoPoint[]) => mapPath(points, camera);
  // Camera changes filter before building SVG paths; editing still owns all geometry.
  const alignmentBounds=useMemo(()=>alignments.map(alignment=>({alignment,bounds:trackBounds(alignment.points)})),[alignments]);
  const trackView=useMemo(()=>trackViewport(camera,mapSize),[camera,mapSize]);
  const visibleAlignmentPaths=useMemo(
    ()=>alignmentBounds.filter(row=>intersectsTrackView(row.bounds,trackView)).map(({alignment})=>({alignment,hit:mapPath(alignment.points,camera)})),
    [alignmentBounds,camera,trackView],
  );
  // Leave the original section index attached to a target after filtering.
  // No splitting or projection of offscreen lines when entering deletion mode.
  const visibleDeletionSections=useMemo(()=>activeTool!=="delete"?[]:visibleAlignmentPaths.flatMap(({alignment})=>
    deletionSections(alignment).filter(section=>intersectsTrackView(trackBounds(section.points),trackView))
      .map(section=>({alignmentId:alignment.id,sectionIndex:section.index,path:mapPath(section.points,camera)}))
  ),[activeTool,visibleAlignmentPaths,trackView,camera]);
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
      <rect onClick={event=>{if(mapMode==="move"&&!roadMapPicking){event.stopPropagation();selectTrackPath(id);setBuiltHeightSelection(null);setHeightAnchor({id,index});}}} onPointerEnter={event=>hoverPlan(event,id,index)} onPointerMove={event=>hoverPlan(event,id,index)} onPointerLeave={()=>setPlanHover(null)} onPointerDown={event=>beginAnchorGesture(event,id,index)} data-anchor-index={index} data-anchor-path={id} x={p.x-hit/2} y={p.y-hit/2} width={hit} height={hit} className="anchor-drag-hit"/></g>;
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
  const [pathServiceEditingDirty,setPathServiceDirty]=useState(false);
  const [extensionServiceDirty,setExtensionServiceDirty]=useState(false);
  const pathServiceDirty=pathServiceEditingDirty||extensionServiceDirty;
  const serviceActionHasEdits=serviceDirty||pathServiceDirty||bindingDirty||serviceAppearanceDirty;
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
  useEffect(() => { smoothZoom.cancel(); changeMapMode(activeTool === "tracks" ? "draw" : activeTool==="stations"?"place":"move"); setHoverPoint(null); setPanelCollapsed(false); }, [activeTool]);
  const selectedServicePath=world?.service_paths?.find(path=>path.service_id===serviceId);

  const stationSearchActive=routeMapPick!==null;
  const stationSearchDescription=useMemo(()=>{
    if(!stationSearchActive)return undefined;
    const routes=stationRouteNames(world?.routes??[],world?.route_paths??[]);
    const companies=firstByKey(world?.companies??[],company=>company.id);
    return new Map((world?.stations??[]).map(station=>{
      const position=Number.isFinite(station.latitude)&&Number.isFinite(station.longitude)
        ?`緯度 ${station.latitude.toFixed(5)}・経度 ${station.longitude.toFixed(5)}`:'';
      return [station.id,[[...(routes.get(station.id)??[])].join('・'),companies.get(station.owner)?.name,position].filter(Boolean).join(' / ')];
    }));
  },[stationSearchActive,world?.routes,world?.route_paths,world?.companies,world?.stations]);
  function locateSearchStation(id:string){
    const station=world?.stations.find(row=>row.id===id);
    if(!station||!Number.isFinite(station.latitude)||!Number.isFinite(station.longitude))return;
    changeCamera(cameraAt(Math.max(BOUNDS.south,Math.min(BOUNDS.north,station.latitude)),Math.max(BOUNDS.west,Math.min(BOUNDS.east,station.longitude)),Math.max(cameraRef.current.zoom,16)));
  }
  const selectedRoutePath = world?.route_paths?.find(path => path.route_id === routeId);
  const companyDirty = !!world && companyName !== (world.companies[0]?.name ?? "");
  const selectedStation = selected===null?undefined:stationsById.get(selected);
  const selectedStationWalkLinks=useMemo(()=>selected&&activeTool==="station-management"
    ?selectedStationPassages(selected,world?.station_walk_links??[],stationsById):[],
    [selected,activeTool,world?.station_walk_links,stationsById]);

  const stationDirty = !!selectedStation && editStationName !== selectedStation.name;
  const selectedPlacement = selected===null?undefined:stationPlacementById.get(selected);
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
  const roadDrawingTiles=useMemo(()=>(automaticRoad.snapshot?.tileSnapshots??[]).flatMap(t=>"tileLines" in t&&Array.isArray(t.tileLines)?t.tileLines:[]) as import('./ofmBoundaryContinuations').TileRoadLine[],[automaticRoad.snapshot]);
  const roadDrawing=useRoadDrawing(roadDrawingBundle,enabled&&roadDrawingEnabled&&activeTool==="tracks"&&mapMode==="draw",roadDrawingTiles);
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
      setFrozenRoadBundle(graphBundle);setFrozenRoadTiles((automaticRoad.snapshot?.tileSnapshots??[]).filter(t=>"tileLines" in t&&Array.isArray(t.tileLines)) as unknown as readonly DraftTileSnapshot[]);
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
    ...(world?.stations??[]).flatMap(station=>{const placement=stationPlacementById.get(station.id),polygon=stationPolygons.get(station.id);return placement&&polygon?[{id:station.id,name:station.name,owner:station.owner,lineId:placement.alignment_id,polygon,planned:false}]:[];}),
    ...plannedPlatforms.flatMap(p=>p.line?[{id:p.station.id,name:p.station.name,owner:world?.companies[0]?.id??"",lineId:p.line.id,polygon:p.polygon,planned:true}]:[])
  ],world?.station_groups),[world,stationPlacementById,stationPolygons,plannedPlatforms]);
  const {stationGroupByMember,mergedStationIds}=useMemo(()=>{
    const stationGroupByMember=new Map<string,(typeof stationGroupDisplay)[number]>();
    const mergedStationIds=new Set<string>();
    for(const group of stationGroupDisplay)for(const member of group.members){
      if(!stationGroupByMember.has(member.id))stationGroupByMember.set(member.id,group);
      if(group.members.length>1)mergedStationIds.add(member.id);
    }
    return {stationGroupByMember,mergedStationIds};
  },[stationGroupDisplay]);
  const stationDisplayGroup=(id:string)=>stationGroupByMember.get(id);
  const occupiedStationPlatforms=[...(world?.station_placements??[]).map(p=>({lineId:p.alignment_id,position:p.position,lengthM:p.length_m})),...plannedPlatforms.flatMap(p=>p.line?[{lineId:p.line.id,position:p.position,lengthM:p.station.lengthM}]:[])];
  const hoveredPlannedStation=stationCursor?plannedPlatforms.find(p=>stationContainsPoint(p.polygon,stationCursor)):undefined;
  const hoveredBuiltStation=stationCursor?world?.stations.find(station=>stationContainsPoint(stationPolygons.get(station.id)??[],stationCursor)):undefined;
  const hoveredStation=hoveredPlannedStation?{name:stationDisplayGroup(hoveredPlannedStation.station.id)?.name??hoveredPlannedStation.station.name,lengthM:hoveredPlannedStation.station.lengthM}:hoveredBuiltStation?{name:hoveredBuiltStation.name,lengthM:stationPlacementById.get(hoveredBuiltStation.id)?.length_m}:undefined;
  const stationSelection=stationCursor?stationTarget(stationLines,stationCursor,Number(platformLength),camera,mapSize.width>0&&mapSize.height>0?mapViewport(mapSize.width,mapSize.height).scale:1,occupiedStationPlatforms):null;
  const stationJoinPreview=stationSelection?.target&&!stationSelection.error?stationGroupDisplay.find(g=>g.members.some(m=>m.owner===world?.companies[0]?.id&&m.lineId!==stationSelection.target!.line.id&&footprintsIntersect(m.polygon,stationSelection.target!.polygon))):undefined;
  const stationJoinOrigin=stationJoinPreview?(world?.stations.find(s=>s.id===stationJoinPreview.id)??plannedStations.find(s=>s.id===stationJoinPreview.id)?.point):undefined;
  const stationPreview=activeTool==="stations"&&mapMode==="place"&&!dragging&&!stationSelection?.error?stationSelection?.target?.polygon??[]:[];
  const previewCenter=stationSelection?.target?.center??null;
  useEffect(()=>{if(!referenceCopy.active)discardPlanStations(-1);},[referenceCopy.active]);
  const graphNodesById=useMemo(()=>new Map(graphBundle?.graph.nodes.map(node=>[node.osmId,node])??[]),[graphBundle]);
  useEffect(()=>{referenceCopy.clearHover();},[camera]);
  const saveBlocked = routeRenameDirty || serviceAppearanceDirty || bindingDirty || !!anchorPreview || candidateDirty || serviceDirty || pathServiceDirty || companyDirty || stationSettingsDirty;
  const hasPlans = plannedStations.some(station=>station.pathId!==-1) || draftPoints.length > 0 || redoPoints.length > 0 || parkedPaths.length > 0;
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
  const autosaveSnapshot=useMemo(()=>({world,constructionPlans,camera,operationRevision}),[world,constructionPlans,camera,operationRevision]);
  const autosave=useAutosave({slotId:activeSlotId,ready:status==="ready"&&slotsReady&&activeSlotId!==null,dirty:(dirty||plansDirty||viewDirty||operationRunning)&&!companySetupRequired,continuous:operationRunning,world:autosaveSnapshot,constructionPlans:persistedPlans,viewState:camera,epoch:worldEpoch,restoredEpoch:restoredAutosaveEpoch,busyRef,setBusy});
  const demandVersion=useMemo(()=>({world,operationRevision}),[world,operationRevision]);
  const closeGuard = useUnsavedCloseGuard(dirty || plansDirty || unapplied || stationResizePending);
  function restoreConstructionPlans(value:ConstructionPlans|null|undefined,compact=false){
    // Wholesale restore, including deletion/Undo history, cannot grant a
    // revived or reused path the preceding draft's transient branch intent.
    setBranchPicker(null);setBranchPermit(null);
    parkedAnchorHistory.current.clear();selectedTrackType.current=null;
    const raw=value??emptyConstructionPlans(),original={...raw,infrastructure:raw.infrastructure?.track_count===1&&raw.infrastructure.level==="ground"?undefined:raw.infrastructure},restored=compact?compactEmptyPlans(original):original,ids=nextConstructionIds(restored);
    setPlans(restored.plans);setParkedPaths(restored.parkedPaths);setActivePathId(restored.activePathId);setActivePlanId(restored.activePlanId);
    setDraftPoints(restored.draftPoints);setDraftEditedGeometry(restored.draftEditedGeometry);setAnchorUndo([]);setAnchorRedo([]);setRedoPoints(restored.redoPoints);setDraftExtension(restored.draftExtension);setDraftDestination(restored.draftDestination);setDraftRedoDestination(restored.draftRedoDestination);setDraftFinished(restored.draftFinished);
    setDraftJunctionSource(restored.draftJunctionSource);setDraftAnchorLevels(restored.draftAnchorLevels);setRedoAnchorLevels(restored.redoAnchorLevels??[]);setHeightAnchor(null);setStartLevel(restored.startLevel);setInfrastructure(restored.infrastructure??DEFAULT_INFRASTRUCTURE);setTrackType(restored.trackType);setDesignSpeedKmh(restored.designSpeedKmh);setRoadFollowing(restored.roadFollowing);setLegacyRoadDraft(restored.roadFollowing);setPlannedStations(restored.plannedStations);
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
    const line=alignmentsById.get(target.alignmentId),section=line&&deletionSections(line)[target.sectionIndex];
    return section?`建設済みの線路 ${(alignmentLength(section.points)/1000).toFixed(2)} km`:"建設済みの線路";
  }
  function deletionBlocked(target:DeleteTarget):string{
    if(unapplied)return "編集中の変更を適用または取り消してください";
    if(target.kind==="plan-section"){const path=target.pathId===activePathId?parkCurrentPlan():parkedPaths.find(path=>path.id===target.pathId);if(path?.startLevel!==undefined&&path.startLevel!==(path.infrastructure??DEFAULT_INFRASTRUCTURE).level)return "勾配を保持した計画区間の分割削除は未対応です。計画全体の削除は可能です。";}
    if(target.kind==="plan-section"||target.kind==="plan-anchor"){const path=target.pathId===activePathId?parkCurrentPlan():parkedPaths.find(path=>path.id===target.pathId);if(path?.junctionSource)return "分岐計画の部分削除は未対応です。計画全体の削除は可能です。";}
    // Section safety depends on the removed span, not references to the whole line.
    // The transactional backend validates station overlap and surviving endpoints.
    if(target.kind==="built-section")return alignmentsById.get(target.alignmentId)?.owner!==world?.companies[0]?.id?"他社の線路は削除できません":"";
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
    return [...routes.map(route=>"路線: "+route.name),...services.map(service=>"系統: "+service.name),
      ...(world?.station_walk_links?.some(link=>link.from===id||link.to===id)?["駅間の徒歩連絡"]:[])].join(" / ");
  }
  function selectStation(id: string) {
    if (busyRef.current||roadMapFrom!==null||roadPlayerMapPick) return;
    if ((mainTab === "routes" || mainTab === "services") && routeMapSelect.current) {
      if (!enabled || !stationsById.has(id)) return;
      const select=routeMapSelect.current;if(routeMapPick==="via")cancelRouteMapPick();select(id);return;
    }
    clearTrackSelection();
    if (stationSettingsDirty && id !== selected) { setPendingStation({ id }); return; }
    if (id === selected) { setMainTab("stations"); setActiveTool("station-management"); setPanelCollapsed(false); return; }
    setMainTab("stations"); setActiveTool("station-management");
    setSelected(id); setEditStationName(stationsById.get(id)?.name ?? ""); setPendingStation(null); setPendingDelete(null);
    const placement = stationPlacementById.get(id);
    setEditLength(String(placement?.length_m ?? 200)); setEditWidth(String(placement?.width_m ?? 12)); setPanelCollapsed(false);
  }
  function beginStationNameEdit(){
    if(!enabled || busyRef.current || !selectedStation)return;
    stationNameInput.current?.focus();stationNameInput.current?.select();
  }
  function editStationWalk(edit:StationWalkEdit){
    if(!enabled||unapplied||stationSettingsDirty||busyRef.current)return;
    void perform(async()=>{
      const snapshot=await invoke<World>("edit_station_walk_link",edit);
      setWorld(snapshot);setDirty(true);setConfirmLoad(false);clearStationHistory();
      setNotice(edit.seconds===null?"徒歩連絡を外しました。保存でファイルに反映します。":"徒歩連絡を登録しました。保存でファイルに反映します。");
    });
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
  function cancelMapGesture() {
    const selection=boxGesture.current;boxGesture.current=null;setSelectionBox(null);
    const gesture = gestureRef.current;
    gestureRef.current = null; suppressMapClick.current = true; suppressSelectionClick.current = true; setDragging(false);
    const svg = svgRef.current;
    if(selection&&svg?.hasPointerCapture?.(selection.pointerId))svg.releasePointerCapture(selection.pointerId);
    if (gesture && svg?.hasPointerCapture?.(gesture.pointerId)) svg.releasePointerCapture(gesture.pointerId);
  }
  function constructionMapMode(tool:typeof constructionTools[number][0]) {
    return tool==="tracks"?"draw":tool==="stations"?"place":"move";
  }
  function chooseConstructionTool(tool:typeof constructionTools[number][0]) {
    if(seamMutationPending.current)return;
    setActiveTool(tool);
    changeMapMode(constructionMapMode(tool));
  }
  function changeMapMode(mode: "move" | "place" | "draw") {
    setError("");
    const wasGesturing = gestureRef.current !== null;
    cancelAnchorGesture();cancelMapGesture(); setMapMode(mode);setStationCursor(null); setCursorPosition(null); setHoverPoint(null); setPlanHover(null);setBuiltHover(null);
    if (!wasGesturing) { suppressMapClick.current = false; suppressSelectionClick.current = false; }
  }
  function applyAnchorEdit(id:number,value:{points:GeoPoint[];geometry?:EditedGeometry;redo:GeoPoint[];extension?:Extension|null;destination?:Extension;redoDestination?:Extension;anchorLevels?:SectionInfrastructure["level"][];redoAnchorLevels?:SectionInfrastructure["level"][];startLevel?:SectionInfrastructure["level"];infrastructure?:SectionInfrastructure}){
    if(id===activePathId){if("anchorLevels" in value)setDraftAnchorLevels(value.anchorLevels);if("redoAnchorLevels" in value)setRedoAnchorLevels(value.redoAnchorLevels??[]);if("startLevel" in value)setStartLevel(value.startLevel);if(value.infrastructure)setInfrastructure(value.infrastructure);if("extension" in value)setDraftExtension(value.extension??null);setDraftDestination(value.destination);setDraftRedoDestination(value.redoDestination);setDraftPoints(value.points);setDraftEditedGeometry(value.geometry);setRedoPoints(value.redo);setHoverPoint(null);}
    else {parkedAnchorHistory.current.delete(id);setParkedPaths(paths=>paths.map(path=>path.id===id?{...path,anchorLevels:"anchorLevels" in value?value.anchorLevels:path.anchorLevels,redoAnchorLevels:"redoAnchorLevels" in value?value.redoAnchorLevels:path.redoAnchorLevels,startLevel:"startLevel" in value?value.startLevel:path.startLevel,infrastructure:value.infrastructure??path.infrastructure,extension:"extension" in value?value.extension??null:path.extension,destination:value.destination,redoDestination:value.redoDestination,points:value.points,editedGeometry:value.geometry,line:path.ofmPath?value.points:plannedGeometry(value.points,value.geometry,extensionPrevious("extension" in value?value.extension??null:path.extension)).points,redo:value.redo}:path));}
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
    if(!path||path.reference||path.ofmPath||path.roadFollowing||path.points.length<2||(index===0&&(path.extension||path.junctionSource)))return;
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
    ordinaryBus.clearHover();
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
    const busTarget=event.button===0&&ordinaryBusPicking&&ordinaryBus.waypoints&&event.target instanceof Element?event.target:undefined;
    const busControlId=busTarget?.closest('[data-bus-control]')?.getAttribute('data-bus-control')??undefined;
    const busLegAfter=busTarget?.closest('[data-bus-leg-after]')?.getAttribute('data-bus-leg-after')??undefined;
    gestureRef.current = { busControlId,busLegAfter, pointerId: event.pointerId, startX: event.clientX, startY: event.clientY, camera: cameraRef.current, scale, dragged: false, mayPlace: event.button===0 && (ordinaryBusPicking || ordinaryBusStopsPicking || (((mapMode === "place" && activeTool === "stations") || (mapMode === "draw" && activeTool === "tracks")) && !existingEquipment)) };
    suppressMapClick.current = false;
    suppressSelectionClick.current = false;
  }
  function pointerMove(event: ReactPointerEvent<SVGSVGElement>) {
    const selecting=boxGesture.current;
    if(selecting&&selecting.pointerId===event.pointerId){
      const rect=event.currentTarget.getBoundingClientRect(),x=Math.max(rect.left,Math.min(rect.right,event.clientX)),y=Math.max(rect.top,Math.min(rect.bottom,event.clientY));
      const p=coordinatesFromClient(x,y,rect,selecting.camera);if(p){selecting.box={...selecting.box,end:project(p.latitude,p.longitude,selecting.camera)};setSelectionBox(selecting.box);}return;
    }
    if(anchorGesture.current){moveAnchorGesture(event);return;}
    if(ordinaryBusPicking&&ordinaryBus.waypoints&&ordinaryBus.extending&&!gestureRef.current&&!event.buttons){
      const rect=event.currentTarget.getBoundingClientRect(),point=coordinatesFromClient(event.clientX,event.clientY,rect,cameraRef.current);
      if(point)ordinaryBus.offerHover(point,cameraRef.current,mapViewport(rect.width,rect.height).scale);
      else ordinaryBus.clearHover();
    }
    if(enabled&&referenceCopy.active&&referenceCopy.roadFollow&&mapMode==="draw"&&!gestureRef.current){const p=coordinatesFromClient(event.clientX,event.clientY,event.currentTarget.getBoundingClientRect(),camera);if(p)void referenceCopy.hover(p);}

    if(enabled&&activeTool==="stations"&&mapMode==="place"&&!gestureRef.current){const rect=event.currentTarget.getBoundingClientRect();setStationCursor(coordinatesFromClient(event.clientX,event.clientY,rect,camera));setCursorPosition({x:event.clientX-rect.left,y:event.clientY-rect.top,width:rect.width,height:rect.height});}
    if (enabled && !referenceCopy.active && mapMode === "draw" && activeTool === "tracks" && !gestureRef.current) {
      const rect=event.currentTarget.getBoundingClientRect(),point=coordinatesFromClient(event.clientX,event.clientY,rect,camera);
      setCursorPosition(point?{x:event.clientX-rect.left,y:event.clientY-rect.top,width:rect.width,height:rect.height}:null);
      drawingCursor.current=point?{point,width:rect.width,height:rect.height}:null;
      if(tramDrawing&&tramCtrl!==event.ctrlKey)updateTramCtrl(event.ctrlKey);
      if(usesRoadSnap(event.ctrlKey)&&point&&!parallelMode){roadDrawing.hover({point,start:!draftFinished?draftPoints.at(-1):undefined,radiusM:20,authoredTramCue:nativeTramGuide});}
      else if(tramDrawing)roadDrawing.clearHover();
      else roadDrawing.cancel();
      setHoverPoint(parallelMode?point:nativeTramGuide&&usesRoadSnap(event.ctrlKey)?null:point && !snappedEndpoint(point,rect.width,rect.height)?drawingPoint(point,rect.width,rect.height,event.ctrlKey):null);
    }
    const gesture = gestureRef.current;
    if (!gesture || gesture.pointerId !== event.pointerId) return;
    const dx = event.clientX - gesture.startX, dy = event.clientY - gesture.startY;
    if (!gesture.dragged && Math.hypot(dx, dy) < 5) return;
    if (!gesture.dragged && event.type === "pointermove") event.currentTarget.setPointerCapture?.(event.pointerId);
    gesture.dragged = true; suppressMapClick.current = true; suppressSelectionClick.current = true; setDragging(true);
    if(!gesture.busControlId&&!gesture.busLegAfter)changeCamera(panCamera(gesture.camera, -dx / gesture.scale, -dy / gesture.scale));
  }
  function pointerEnd(event: ReactPointerEvent<SVGSVGElement>, canceled = false) {
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
    if(!canceled&&gesture.dragged&&(gesture.busControlId||gesture.busLegAfter)&&ordinaryBusPicking){
      const rect=event.currentTarget.getBoundingClientRect();
      const at=coordinatesFromClient(event.clientX,event.clientY,rect,cameraRef.current);
      if(at)void ordinaryBus.pick(at,cameraRef.current,gesture.scale,1,undefined,gesture.busControlId,gesture.busLegAfter);
    }
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
    setParkedPaths(paths=>[...paths,{id,planId:activePlanId,name:planName,points:build.points,redo:[],line:build.points,infrastructure:referenceExtension?endpointInfrastructure(referenceExtension.alignment,referenceExtension.endpoint):infrastructure,designSpeedKmh:referenceExtension?(referenceExtension.alignment.design_speed_kmh??defaultDesignSpeed(referenceExtension.alignment.track_type??"normal")):designSpeedKmh,trackType:referenceExtension?.alignment.track_type??trackType,roadFollowing:false,extension:referenceExtension?{alignmentId:referenceExtension.alignment.id,endpoint:referenceExtension.endpoint}:null,reference:build}]);
    setPlannedStations(items=>items.map(station=>station.pathId===-1?{...station,pathId:id}:station));
    const moveHistory=(entries:StationHistoryEntry[])=>entries.map(entry=>entry.planned?.pathId===-1?{...entry,planned:{...entry.planned,pathId:id}}:entry);
    setStationUndo(moveHistory);setStationRedo(moveHistory);
    setNotice("実在形状に沿う経路を選択中の計画へ追加しました。『計画を着工』で他の経路・駅とまとめて建設します。");
  }
  function destinationTarget(point:GeoPoint,width:number,height:number) {
    if(draftFinished||!draftPoints.length)return null;
    if(activeBranch){
      if(!draftExtension||endpointKey(draftExtension)!==endpointKey(activeBranch.source))return null;
      const target=branchEnds.find(end=>end.key===endpointKey(activeBranch.destination));
      if(!target)return null;
      const distance=endpointDistance(point,target.point,width,height);
      return distance<=8?{...target,distance}:null;
    }
    return alignments.flatMap(alignment=>[
      {alignment,endpoint:"start" as const,point:alignment.points[0]},
      {alignment,endpoint:"end" as const,point:alignment.points.at(-1)!},
    ]).filter(e=>e.point&&!(e.alignment.id===draftExtension?.alignmentId&&e.endpoint===draftExtension?.endpoint)).map(e=>({...e,distance:endpointDistance(point,e.point,width,height)}))
      .filter(e=>e.distance<=8).sort((a,b)=>a.distance-b.distance)[0]??null;
  }
  function joinError(line:GeoPoint[],source:Extension|null,destination:Extension|undefined,pathTrackType=trackType,branch=activeBranch){
    return destinationJoinError(line,source,destination,alignments,world?.alignment_connections??[],world?.companies[0]?.id,pathTrackType,branch??undefined);
  }
  function beginBranchDrawing(){
    if(!world||!branchPlacementAvailable||!branchDraftAvailable||anchorPlacementPending.current||!selectedBranchStart||!selectedBranchEnd||selectedBranchStart.alignment.id===selectedBranchEnd.alignment.id||
       distanceM(selectedBranchStart.point,selectedBranchEnd.point)<.1)return;
    const ref=(end:typeof selectedBranchStart):Extension=>({alignmentId:end.alignment.id,endpoint:end.endpoint});
    const source=ref(selectedBranchStart),destination=ref(selectedBranchEnd);
    const sourceAnchor=branchResumes?draftPoints[0]:selectedBranchStart.point;
    if(!sourceAnchor)return;
    setBranchPermit({epoch:worldEpoch,basis:world,pathId:activePathId,source,destination,sourceAnchor});setBranchPicker(null);
    if(!branchResumes){
      setDraftExtension(source);setDraftDestination(undefined);setDraftRedoDestination(undefined);
      setTrackType("normal");setInfrastructure(DEFAULT_INFRASTRUCTURE);setStartLevel("ground");
      setDesignSpeedKmh(selectedBranchStart.alignment.design_speed_kmh??defaultDesignSpeed("normal"));
      setDraftAnchorLevels(["ground"]);setRedoAnchorLevels([]);setDraftPoints([sourceAnchor]);
      setDraftJunctionSource(undefined);setDraftEditedGeometry(undefined);setDraftFinished(false);
    }
    setHoverPoint(null);setError("");setNotice("分岐線を描き、選んだ終点で接続してください。既設線路は着工まで変更されません。");
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
    ?nearestJunctionSource(alignments as JunctionSource['expectedAlignment'][],hoverPoint,camera,viewport.scale,world?.companies[0]?.id,infrastructure.level):null;
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
    newPath(true);setDraftPoints(geometry.anchors);setDraftEditedGeometry(geometry);setDraftFinished(true);setRedoPoints([]);setAnchorUndo([]);setAnchorRedo([]);setParallelStart(null);setHoverPoint(null);
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

  function storeTramRoadSnap(point:GeoPoint,proof:AcceptedTramRoadSnap|undefined,path=activePathId){
    if(proof&&distanceM(point,proof.pick.point)<=.01)
      tramRoadSnaps.current.set(point,{proof,world,epoch:worldEpoch,plan:activePlanId,path});
  }
  async function placeTrackPoint(coordinates:GeoPoint,rect:{width:number;height:number},event:{detail:number;ctrlKey:boolean;clientX?:number;clientY?:number},roadSnap?:AcceptedTramRoadSnap) {
      const offered=coordinates;
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
            setDraftFinished(false);setHoverPoint(null);
          }else selectPath(target.id,true,target.endpoint);
          return;
        }
        if(target?.kind==="built"){
          if(target.alignment.owner!==world?.companies[0]?.id){setError("他社の線路は延伸できません。");return;}
          if(occupiedEndpoint(world?.alignment_connections??[],{alignmentId:target.alignment.id,endpoint:target.endpoint})){setError("この端点は接続済みです");return;}
          if(draftFinished)newPath(true);
          setDraftDestination(undefined);setDraftRedoDestination(undefined);setDraftExtension({alignmentId:target.alignment.id,endpoint:target.endpoint});
          const sourceType=target.alignment.track_type??"normal",sourceInfrastructure=endpointInfrastructure(target.alignment,target.endpoint);
          // A deliberate tram/rail choice builds a separate span. An ordinary
          // extension still inherits the existing type and speed.
          if(selectedTrackType.current!==trackType||!mixedTramTracks(sourceType,trackType)){
            if(sourceType!==trackType)selectedTrackType.current=null;
            setTrackType(sourceType);setDesignSpeedKmh(target.alignment.design_speed_kmh??defaultDesignSpeed(sourceType));
          }
          setDraftJunctionSource(undefined);setStartLevel(sourceInfrastructure.level);setInfrastructure(sourceInfrastructure);
          setDraftAnchorLevels([sourceInfrastructure.level]);setRedoAnchorLevels([]);setDraftPoints([target.point]);setRedoPoints([]);setHoverPoint(null);setDraftFinished(false);
          setNotice("建設済み線路の"+(target.endpoint==="start"?"始点":"終点")+"から延伸を計画しています。着工まで既存線路は変更されません。");return;
        }
        const junction=nearestJunctionSource(alignments as JunctionSource['expectedAlignment'][],coordinates,camera,mapViewport(rect.width,rect.height).scale,world?.companies[0]?.id,infrastructure.level);
        if(junction){if(draftFinished)newPath(true);setDraftJunctionSource(junction.source);setStartLevel(infrastructure.level);setDraftExtension(null);setDraftDestination(undefined);setDraftRedoDestination(undefined);setDraftAnchorLevels([infrastructure.level]);setRedoAnchorLevels([]);setDraftPoints([junction.point]);setRedoPoints([]);setDraftFinished(false);setHoverPoint(null);return;}
        setDraftJunctionSource(undefined);setDraftExtension(null);setDraftDestination(undefined);setDraftRedoDestination(undefined);
        if(draftFinished){const newId=newPath(true);storeTramRoadSnap(coordinates,roadSnap,newId);setStartLevel(infrastructure.level);setDraftAnchorLevels([infrastructure.level]);setRedoAnchorLevels([]);setDraftPoints([coordinates]);return;}
      }
      const branchDestination=activeBranch?destinationTarget(coordinates,rect.width,rect.height):null;
      if(!branchDestination&&snappedEndpoint(coordinates,rect.width,rect.height)){
        if(activeBranch&&!draftDestination){setError("選んだ終点まで分岐線を描いてください。");return;}
        if(draftPoints.length>=2&&!draftInvalid){setDraftFinished(true);setHoverPoint(null);}
        return;
      }
      const destination=activeBranch?branchDestination:destinationTarget(coordinates,rect.width,rect.height);
      coordinates=destination?.point??drawingPoint(coordinates,rect.width,rect.height,event.ctrlKey);
      let editedForAppend=draftEditedGeometry&&redoPoints.length?(draftPoints.length>=2?{version:1 as const,anchors:draftPoints,line:draftLine}:undefined):draftPoints.length?draftEditedGeometry:undefined;
      if(destination)editedForAppend=fittedDestination([...draftPoints,coordinates],editedForAppend,destination);
      const candidate=plannedGeometry([...draftPoints,coordinates],editedForAppend,previousPoint);if(candidate.error){setHoverPoint(coordinates);return;}
      const targetRef=destination?{alignmentId:destination.alignment.id,endpoint:destination.endpoint}:undefined;
      const targetError=joinError(candidate.points,draftExtension,targetRef);if(targetError){setError(targetError);return;}
      if (draftPoints.length && distanceM(draftPoints[draftPoints.length - 1], coordinates) < .1) return;
      if (draftPoints.length >= 128) { setError("線形点は128点までです。一度建設を確定してください。"); return; }
      if(!draftPoints.length)setStartLevel(infrastructure.level);
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
      if((roadFollowing&&roadFinishAfterPlacement.current)||finishPendingPlacement.current){setDraftFinished(true);roadFinishAfterPlacement.current=false;finishPendingPlacement.current=false;}
      if(!destination&&coordinates===offered)storeTramRoadSnap(coordinates,roadSnap);
      setDraftAnchorLevels(nextLevels);setRedoAnchorLevels([]);setHeightAnchor(null);
      setAnchorUndo(items=>[...items,{kind:"point" as const,id:activePathId,before:{anchorLevels:priorLevels,redoAnchorLevels:[...redoAnchorLevels,nextLevel],points:draftPoints,geometry:draftEditedGeometry,redo:[...redoPoints,coordinates],destination:draftDestination,redoDestination:targetRef??draftRedoDestination},after:{anchorLevels:nextLevels,redoAnchorLevels:[],points:[...draftPoints,coordinates],geometry:editedForAppend,redo:[],destination:targetRef}}].slice(-50));setAnchorRedo([]);setDraftDestination(targetRef);setDraftRedoDestination(undefined);if(targetRef)setDraftFinished(true);setDraftEditedGeometry(editedForAppend);setDraftPoints((points) => [...points, coordinates]); setRedoPoints([]); setHoverPoint(null); return;
  }
  async function placeRoadDrawing(point:GeoPoint,rect:{width:number;height:number},event:{detail:number;ctrlKey:boolean;clientX?:number;clientY?:number}) {
    if(legacyRoadDraft){setError('以前の道路追従下書きです。新しい計画から敷設してください');return;}
    if(event.detail>=2){
      if(roadDrawing.clickPending()){
        const at=pendingPlacementPoint.current;
        if(at&&Math.hypot((event.clientX??0)-at.x,(event.clientY??0)-at.y)<=8)roadFinishAfterPlacement.current=true;
        return;
      }
      if(draftPoints.length>1)setDraftFinished(true);roadDrawing.cancel();return;
    }
    if(roadDrawing.clickPending())return;roadFinishAfterPlacement.current=false;
    setStreetTramPreview(null);
    // Existing endpoints, planned endpoints, junction selection and finishing are
    // owned by normal construction, not inferred from road geometry.
    if(((!draftPoints.length||draftFinished)&&startTarget(point,rect.width,rect.height))||snappedEndpoint(point,rect.width,rect.height)) {placeTrackPoint(point,rect,event);roadDrawing.cancel();return;}
    const captured=anchorPlacementContext.current;
    const nearEnd=(!draftFinished&&draftPoints.length)?startTarget(point,rect.width,rect.height):null;
    const destination=nearEnd?.kind==='built'?nearEnd:(!draftFinished&&draftPoints.length)?destinationTarget(point,rect.width,rect.height):null;
    pendingPlacementPoint.current={x:event.clientX??0,y:event.clientY??0};
    const continuation=info?.authored_chain_street_tram_enabled&&draftExtension?.endpoint==="end"?draftExtension:undefined;
    const built=continuation&&alignments.find(a=>a.id===continuation.alignmentId&&a.owner===world?.companies[0]?.id&&a.track_type==="tram");
    const endSource=built&&world?.road_path_provenance?.find(r=>r.alignment_id===built.id&&r.path.method==="ofm_authored_street_tram_v1"&&
      r.end_point_index===built.points.length-1&&r.to_position===1);
    const retainedEnd=endSource&&built&&bindTramRoadSnap(automaticRoad.snapshot,{...endSource.path.end,point:built.points.at(-1)!});
    const endProof=endSource?.path.tramChainGuide?.tiles.at(-1)??endSource?.path.tramGuide?.adjacentEnd??endSource?.path.tramGuide;
    const certifiedEnd=retainedEnd&&endProof&&endProof.tileKey===`https://tiles.openfreemap.org/planet/${retainedEnd.tile.revision}/14/${retainedEnd.tile.x}/${retainedEnd.tile.y}.pbf`&&
      endProof.tileRawSha256===retainedEnd.tile.rawSha256?retainedEnd:undefined;
    if(endSource&&!certifiedEnd){setError("着工済みの終点と同じ版の道路データを表示してから、延伸先を選んでください。");roadDrawing.cancel();return;}
    // Only the native proof is eligible for street tram. A leased two-click
    // view is never parked as ordinary geometry by autosave/plan switching.
    const streetIntent=info?.authored_street_tram_enabled===true&&!!world&&roadFollowing&&
      !draftFinished&&draftPoints.length===1&&(!draftExtension||!!certifiedEnd)&&!destination&&
      !parkedPaths.some(p=>p.planId===activePlanId)&&!plannedStations.some(p=>stationPlanId(p)===activePlanId)&&
      infrastructure.level==="ground"&&(startLevel===undefined||startLevel==="ground")&&
      !(draftAnchorLevels??[]).some(level=>level!=="ground")&&
      roadDrawingBundle===automaticRoad.snapshot?.bundle;
    const allowAdjacent=streetIntent&&info?.authored_adjacent_street_tram_enabled===true;
    const allowChain=streetIntent&&info?.authored_chain_street_tram_enabled===true;
    let result:RoadDrawingResult|null=await roadDrawing.click({
      point:destination?.point??point,start:!draftFinished?draftPoints.at(-1):undefined,radiusM:20,authoredTramCue:nativeTramGuide,resolveAuthoredSeam:nativeTramGuide,nativeAdjacentTram:allowAdjacent,nativeChainTram:allowChain
    });
    const stillCurrent=()=>captured.world===anchorPlacementContext.current.world&&captured.key===anchorPlacementContext.current.key;
    const reject=(message:string)=>{
      finishPendingPlacement.current=false;roadFinishAfterPlacement.current=false;setError(message);
    };
    if(!stillCurrent())return;
    if(!result){finishPendingPlacement.current=false;roadFinishAfterPlacement.current=false;return;}
    // A confirmed click can recover one old source cell that disappeared
    // from the current viewport; hover and known disconnections never fetch.
    const previousRoadPoint=!draftFinished&&draftPoints.at(-1);
    const remembered=previousRoadPoint?tramRoadSnaps.current.get(previousRoadPoint):undefined;
    if(result.kind==="free"&&nativeTramGuide&&previousRoadPoint&&remembered&&
       remembered.world===world&&remembered.epoch===worldEpoch&&remembered.plan===activePlanId&&remembered.path===activePathId&&
       roadDrawingBundle===automaticRoad.snapshot?.bundle){
      const task=++anchorPlacementToken.current;
      const controller=new AbortController();tramReacquireAbort.current=controller;
      anchorPlacementPending.current=true;setAnchorPlacementInProgress(true);
      const waiting=setTimeout(()=>{if(anchorPlacementToken.current===task)setAnchorPlacementWaiting(true);},300);
      const currentTask=()=>anchorPlacementToken.current===task&&stillCurrent()&&!controller.signal.aborted;
      try{
        const recovered=await reacquireTramRoadSeam(automaticRoad.snapshot,previousRoadPoint,remembered.proof,destination?.point??point,!!destination,automaticRoad,currentTask,controller.signal);
        if(!currentTask())return;
        if(recovered)result=recovered;
      }catch(e){
        if(!currentTask())return;
        const message=detail(e);
        result={kind:"error",error:/^[\x00-\x7f]*$/.test(message)?"道路のつながりを確認できません。区間を短くして選び直してください。":message};
      }finally{
        clearTimeout(waiting);
        if(tramReacquireAbort.current===controller)tramReacquireAbort.current=null;
        if(anchorPlacementToken.current===task){anchorPlacementPending.current=false;setAnchorPlacementInProgress(false);setAnchorPlacementWaiting(false);}
      }
    }
    if(streetIntent&&certifiedEnd&&result.kind==="route"&&result.roadSnap){
      result={kind:"junction",from:certifiedEnd.pick,to:result.roadSnap};
    }
    if(streetIntent&&result.kind==="route"&&result.roadSnap&&remembered&&
       remembered.world===world&&remembered.epoch===worldEpoch&&remembered.plan===activePlanId&&remembered.path===activePathId){
      // Same-fragment fast path still obtains a native proof. Recovery
      // from a non-visible adjacent cell remains a visual guide only.
      const sameCell=bindTramRoadSnap(automaticRoad.snapshot,result.roadSnap);
      if(sameCell?.tile.x===remembered.proof.tile.x&&sameCell.tile.y===remembered.proof.tile.y&&
         sameCell.tile.revision===remembered.proof.tile.revision&&sameCell.tile.rawSha256===remembered.proof.tile.rawSha256){
        result={kind:"junction",from:remembered.proof.pick,to:result.roadSnap};
      }
    }
    if(result.kind==="junction"){
      const task=++anchorPlacementToken.current;
      anchorPlacementPending.current=true;setAnchorPlacementInProgress(true);
      pendingPlacementPoint.current={x:event.clientX??0,y:event.clientY??0};
      const waiting=setTimeout(()=>{if(anchorPlacementToken.current===task)setAnchorPlacementWaiting(true);},300);
      const currentTask=()=>anchorPlacementToken.current===task&&stillCurrent();
      try{
        const snapshot=automaticRoad.snapshot;
        const from=observedTramRoadAnchor(snapshot,certifiedEnd?.pick??result.from),to=observedTramRoadAnchor(snapshot,result.to);
        const start=draftPoints.at(-1);
        if(!start||distanceM(from.anchor.point,start)>.01)throw Error("起点を道路上の点に置き直すか、Ctrlで自由に敷設してください。");
        if(distanceM(destination?.point??point,to.anchor.point)>20.01)throw Error("道路上の点を選び直してください。");
        const expected=await invoke<string>("ofm_world_token");
        if(!currentTask())return;
        const hops=Math.abs(from.anchor.tile.x-to.anchor.tile.x)+Math.abs(from.anchor.tile.y-to.anchor.tile.y);
        let args:StreetTramPreview["args"];
        if(hops>1){
          const pick=remembered?.proof.pick;
          if(!allowChain||!certifiedEnd&&(!remembered||remembered.world!==world||remembered.epoch!==worldEpoch||
             remembered.plan!==activePlanId||remembered.path!==activePathId||
             remembered.proof.tile.rawSha256!==from.anchor.tile.rawSha256||
             pick?.fragmentId!==result.from.fragmentId||pick.segmentIndex!==result.from.segmentIndex||
             Math.abs(pick.fraction-result.from.fraction)>1e-6))
            throw Error("起点を道路上の点に置き直してください。");
          args=await prepareAuthoredTramChainQuery(snapshot,from,to,expected,automaticRoad.readTileEvidenceBatch,world?.road_evidence_descriptors??[]);
        }else{
          const adjacent=hops===1;
          if(adjacent&&!allowAdjacent)throw Error("道路の隣り合う範囲を確認できません。");
          const q=await prepareOrdinaryBusQuery(from.anchor,to.anchor,"軌道の参考経路","authored-tram-guide",expected,
            automaticRoad.readTileEvidenceBatch,world?.road_evidence_descriptors??[],undefined,
            adjacent?async()=>[from.anchor.tile,to.anchor.tile]:undefined);
          const adjacentEnd=adjacent&&q.path.rule==='ofm_ground_bus_rectangle_z14_v2'?q.path.additionalTiles[0]:undefined;
          if(adjacent&&!adjacentEnd)throw Error("隣り合う道路の元データが揃いません。");
          args={expectedWorldToken:expected,guide:{
            tileKey:q.path.tileKey,tileRawSha256:q.path.tileRawSha256,start:q.path.start,end:q.path.end,startFractionQ:from.fractionQ,endFractionQ:to.fractionQ,
            ...(adjacentEnd?{adjacentEnd}:{})
          },rawEvidence:q.rawEvidence,tileBinding:q.tileBinding,...(q.contextTiles?{contextTiles:q.contextTiles}:{})};
        }
        if(!currentTask())return;
        const points=await invoke<GeoPoint[]>("preview_authored_tram_road_guide",args);
        if(!currentTask())return;
        if(points.length<2||points.length>4096||points.some(p=>!Number.isFinite(p.latitude)||!Number.isFinite(p.longitude))
           ||distanceM(points[0],from.anchor.point)>.01||distanceM(points[points.length-1],to.anchor.point)>.01)throw Error("道路に沿う軌道の経路を確認できません。");
        if(streetIntent&&world){
          setStreetTramPreview({world,basis:draftPoints,plan:activePlanId,path:activePathId,args,points,
            ...(certifiedEnd&&continuation?{continuation}:{})});
          finishPendingPlacement.current=false;roadFinishAfterPlacement.current=false;
          setHoverPoint(null);setError("");roadDrawing.cancel();return;
        }
        // Preserve the same approved displayed endpoints across quantization.
        points[0]=start;points[points.length-1]=to.anchor.point;
        const line=fitRoadFollow(points);
        // Only the road portion was proved above. An explicitly chosen built
        // endpoint can have a short authored lead, independently checked as
        // part of the full construction candidate below.
        if(destination&&distanceM(to.anchor.point,destination.point)>.01)line.push(destination.point);
        result={kind:"route",point:to.anchor.point,line,roadSnap:result.to};
      }catch(e){if(!currentTask())return;result={kind:"error",error:String(e)};}
      finally{clearTimeout(waiting);if(anchorPlacementToken.current===task){anchorPlacementPending.current=false;setAnchorPlacementInProgress(false);setAnchorPlacementWaiting(false);}}
    }
    if(!stillCurrent())return;
    if(result.kind==='error'){reject(result.error);return;}
    if(result.kind==='seam'){reject('道路のつながりを確認できません。');return;}
    if(certifiedEnd){reject("同じ道路に沿って延伸できる範囲を選んでください。");return;}
    if(result.kind==='free'){
      if(nativeTramGuide){reject("道路の位置を確認できません。Ctrlで自由に敷設するか、道路に合わせる設定をオフにしてください。");return;}
      placeTrackPoint(point,rect,{...event,ctrlKey:true});return;
    }
    if(result.kind==='point'){placeTrackPoint(result.point,rect,{...event,ctrlKey:true},nativeTramGuide&&roadDrawingBundle===automaticRoad.snapshot?.bundle?bindTramRoadSnap(automaticRoad.snapshot,result.roadSnap):undefined);return;}
    const end=destination?.point??result.point,line=[...draftLine,...result.line.slice(1)];line[line.length-1]=end;
    const points=[...draftPoints,end],geometry:EditedGeometry={version:1,anchors:points,line};
    const targetRef=destination?{alignmentId:destination.alignment.id,endpoint:destination.endpoint}:undefined;
    const invalid=plannedGeometry(points,geometry,previousPoint).error??joinError(line,draftExtension,targetRef);
    if(invalid){reject(invalid);return;}
    if(points.length>128){reject('アンカーが多いため区間を分けてください');return;}
    const levels=anchorHeights(draftPoints.length,draftAnchorLevels,startLevel,infrastructure.level),nextLevel=destination?endpointInfrastructure(destination.alignment,destination.endpoint).level:infrastructure.level;
    const activeRequest=requests.find((_request,index)=>buildPaths[index].id===activePathId);
    if(!activeRequest){reject('敷設する計画を選び直してください。');return;}
    const request={...activeRequest,points:line,anchorIndices:anchorIndices(points,line),anchorLevels:[...levels,nextLevel],destination:targetRef,junctionSource:draftJunctionSource&&draftPoints[0]?junctionDirection(draftJunctionSource,draftPoints[0],draftPoints[1]??end):undefined,stations:plannedPlatforms.filter(item=>item.station.pathId===activePathId).map(item=>({name:stationDisplayGroup(item.station.id)?.name??item.station.name,position:nearestOnAlignment(line,item.station.point).position,lengthM:item.station.lengthM,widthM:12}))};
    pendingPlacementPoint.current={x:event.clientX??0,y:event.clientY??0};
    const accepted=await validatePlannedPlacement(request,finishPendingPlacement.current);
    if(!stillCurrent())return;
    if(!accepted){finishPendingPlacement.current=false;roadFinishAfterPlacement.current=false;return;}
    const before={points:draftPoints,geometry:draftEditedGeometry,redo:redoPoints,anchorLevels:draftAnchorLevels,redoAnchorLevels,extension:draftExtension,destination:draftDestination,redoDestination:draftRedoDestination};
    const after={points,geometry,redo:[],anchorLevels:[...levels,nextLevel],redoAnchorLevels:[],extension:draftExtension,destination:targetRef,redoDestination:undefined};
    if(!destination&&nativeTramGuide&&roadDrawingBundle===automaticRoad.snapshot?.bundle)storeTramRoadSnap(end,bindTramRoadSnap(automaticRoad.snapshot,result.roadSnap));
    applyAnchorEdit(activePathId,after);setAnchorUndo(history=>[...history,{kind:'point' as const,id:activePathId,before,after}].slice(-50));setAnchorRedo([]);setDraftFinished(!!targetRef||roadFinishAfterPlacement.current||finishPendingPlacement.current);roadFinishAfterPlacement.current=false;finishPendingPlacement.current=false;setHoverPoint(null);setError('');roadDrawing.cancel();
  }

  function clearTrackSelection() {
    setAlignmentId("");setSelectedPlanPath(null);setTrackDeleteConfirm(false);setJunctionSelection(null);
  }

  function clearStationSelection() {
    if(activeTool!=="station-management"||selected===null||!enabled||busyRef.current||stationSettingsDirty||stationResizePending||pendingStation||pendingDelete)return false;
    setSelected(null);setEditStationName("");
    return true;
  }

  function mapClick(event: MouseEvent<SVGSVGElement>) {
    if(anchorPlacementPending.current){const at=pendingPlacementPoint.current;if(event.detail>=2&&at&&Math.hypot(event.clientX-at.x,event.clientY-at.y)<=8){finishPendingPlacement.current=true;if(roadFollowing)roadFinishAfterPlacement.current=true;}return;}
    smoothZoom.cancel();
    if (!enabled || suppressSelectionClick.current || roadMapFrom!==null) return;
    // Track hit targets stop propagation in selection mode. A background click
    // clears only the highlight, without consuming drawing/placement clicks.
    clearTrackSelection();
    if (suppressMapClick.current) return;
    if (mapMode === "move") {clearStationSelection();return;}
    const rect = event.currentTarget.getBoundingClientRect();
    const rawCoordinates = coordinatesFromClient(event.clientX, event.clientY, rect, camera);
    if (!rawCoordinates) return;
    let coordinates:GeoPoint=rawCoordinates;
    if (activeTool === "tracks" && mapMode === "draw") {
      if(tramDrawing&&tramCtrl!==event.ctrlKey)updateTramCtrl(event.ctrlKey);
      if(usesRoadSnap(event.ctrlKey)&&!parallelMode){void placeRoadDrawing(coordinates,rect,{detail:event.detail,ctrlKey:event.ctrlKey,clientX:event.clientX,clientY:event.clientY});return;}
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
  function branchPathBuildable(path:PlannedPath){
    const branch=branchForDraft(path.id,path.points,path.extension,path.destination);
    if(!branch)return !path.extension||!!path.destination||!occupiedEndpoint(world?.alignment_connections??[],path.extension);
    return !!path.extension&&!!path.destination&&endpointKey(path.extension)===endpointKey(branch.source)&&
      endpointKey(path.destination)===endpointKey(branch.destination)&&path.trackType==="normal"&&
      path.infrastructure?.level==="ground"&&path.infrastructure.track_count===1&&
      !!path.anchorLevels?.length&&path.anchorLevels.every(level=>level==="ground")&&
      !path.reference&&!path.ofmPath&&!path.junctionSource;
  }
  const planBuildable=useMemo(()=>(buildPaths.length>0||currentStations.some(item=>item.station.alignmentId))&&currentStations.every(item=>item.station.pathId===undefined||buildPaths.some(path=>path.id===item.station.pathId))&&buildPaths.every(path=>path.line.length>=2&&branchPathBuildable(path)&&!((path.reference||path.ofmPath)?extensionJoinError(path.points,extensionPrevious(path.extension)):plannedGeometry(path.points,path.editedGeometry,extensionPrevious(path.extension)).error)&&!joinError(path.line,path.extension,path.destination,path.trackType,branchForDraft(path.id,path.points,path.extension,path.destination))&&!(path.roadFollowing)),[buildPaths,currentStations,extensionPrevious,world,trackType,currentBranchPermit]);
  const requests=buildPaths.map(path=>({name:alignmentName,points:path.line,anchorIndices:anchorIndices(path.points,path.line),trackType:path.trackType,designSpeedKmh:path.designSpeedKmh,infrastructure:path.infrastructure??DEFAULT_INFRASTRUCTURE,startLevel:path.startLevel,anchorLevels:path.anchorLevels,junctionSource:path.junctionSource,extension:path.extension??undefined,destination:path.destination,reference:path.reference?{sourceRecord:path.reference.sourceRecord,provenance:path.reference.provenance}:undefined,ofmPath:path.ofmPath,stations:plannedPlatforms.filter(item=>item.station.pathId===path.id).map(item=>({name:stationDisplayGroup(item.station.id)?.name??item.station.name,position:item.position,lengthM:item.station.lengthM,widthM:12}))}));
  const stations=currentStations.filter(item=>item.station.alignmentId).map(item=>({alignmentId:item.station.alignmentId!,name:stationDisplayGroup(item.station.id)?.name??item.station.name,position:item.position,lengthM:item.station.lengthM,widthM:12}));
  const constructionRequest={alignments:requests,stations};
  const roadCheckAbort=useRef<AbortController|null>(null);
  const [roadCheckPending,setRoadCheckPending]=useState(false);
  const buildIdentity=JSON.stringify(constructionRequest);
  const latestBuild=useRef({world,key:buildIdentity});latestBuild.current={world,key:buildIdentity};
  const constructionRoad=useAutomaticRoad(constructionRoadBounds(buildPaths,bounds),status==="ready"&&buildPaths.length>0,worldEpoch);
  const roadAttachment=useMemo(()=>constructionRoadAttachment(constructionRoad.snapshot,world),[constructionRoad.snapshot,world]);
  const roadChecks=useRoadAssessments(world,status==="ready");
  const roadCoverageUnknown=roadChecks.value.filter(([,r])=>!r.roads_ready).length;
  const roadHeightUnknown=roadChecks.value.filter(([,r])=>r.unknown_road_height).length;
  const constructionRoadReason=buildPaths.length>0&&(constructionRoad.status!=='ready'||roadAttachment.roads.some(r=>!r.coverage_verified))?(constructionRoad.status==='ready'?roadAttachment.reason:constructionRoad.message):'';
  const crossingCount=roadChecks.value.reduce((n,[,r])=>n+r.crossings.length,0);
  const roadStatusReason=constructionRoadReason||(roadChecks.error?`踏切判定未取得：${roadChecks.error}`:roadHeightUnknown?'道路の高さを確認できない区間があります':roadCoverageUnknown?'道路範囲を確認できない線路があります':alignments.length&&!world?.road_snapshots?.length?'道路参照が未保存のため踏切未確認です':'');
  const roadCursorPreview=roadSnapShown&&!draftFinished&&draftPoints.length>0&&roadDrawing.preview?.kind==='route'?roadDrawing.preview:null;
  const cursorLine=roadCursorPreview?[...draftLine,...roadCursorPreview.line.slice(1)]:previewLine;
  const cursorAnchors=roadCursorPreview?[...draftPoints,roadCursorPreview.point]:previewAnchors;
  const roadPreviewGeometry=useMemo(()=>{if(!roadCursorPreview)return null;const anchors=[...draftPoints,roadCursorPreview.point],line=[...draftLine,...roadCursorPreview.line.slice(1)];return plannedGeometry(anchors,{version:1,anchors,line},previousPoint);},[roadCursorPreview,draftPoints,draftLine,previousPoint]);
  const drawingPreviewError=roadPreviewGeometry?.error??(roadCursorPreview?null:previewError);
  const drawingPreviewSpeed=roadPreviewGeometry?Math.min(designSpeedKmh,...roadPreviewGeometry.arcs.map(arc=>provisionalCurveSpeed(arc.radiusM)??0)):Math.min(designSpeedKmh,circleGuide?(provisionalCurveSpeed(circleGuide.radiusM)??designSpeedKmh):designSpeedKmh);
  const cursorPreviewActive=!!roadCursorPreview||!planHover&&activeTool==="tracks"&&mapMode==="draw"&&!parallelMode&&!referenceCopy.active&&previewAnchors!==draftPoints&&previewAnchors.length>=2&&!previewError;
  const cursorDestination=cursorPreviewActive&&hoverPoint&&cursorPosition?destinationTarget(hoverPoint,cursorPosition.width,cursorPosition.height):null;
  const previewRequests=cursorPreviewActive?requests.map((request,index)=>buildPaths[index].id!==activePathId?request:{...request,points:cursorLine,anchorIndices:anchorIndices(cursorAnchors,cursorLine),anchorLevels:[...anchorHeights(draftPoints.length,draftAnchorLevels,startLevel,infrastructure.level),cursorDestination?endpointInfrastructure(cursorDestination.alignment,cursorDestination.endpoint).level:infrastructure.level],destination:cursorDestination?{alignmentId:cursorDestination.alignment.id,endpoint:cursorDestination.endpoint}:undefined,stations:plannedPlatforms.filter(item=>item.station.pathId===activePathId).map(item=>({name:stationDisplayGroup(item.station.id)?.name??item.station.name,position:nearestOnAlignment(cursorLine,item.station.point).position,lengthM:item.station.lengthM,widthM:12}))}):requests;
  const previewRequestValid=cursorPreviewActive?previewRequests.every(request=>request.points.length>=2):planBuildable;
  const waterModelRevision=useWaterModelAcquisition(status==="ready",world,waterAcquisitionBounds(previewRequests.flatMap(r=>r.points),camera));
  const nativeConstructionPreview=useConstructionPreview(world,status==="ready"&&previewRequestValid?{alignments:previewRequests,stations,roads:roadAttachment.roads}:null,waterModelRevision);
  const nativeValidationError=constructionValidationError(nativeConstructionPreview?.value);
  const [anchorPlacementWaiting,setAnchorPlacementWaiting]=useState(false);
  const [anchorPlacementInProgress,setAnchorPlacementInProgress]=useState(false);
  const anchorPlacementPending=useRef(false);
  const branchPlacementAvailable=enabled&&activeTool==="tracks"&&mapMode==="draw"&&!roadFollowing&&!parallelMode&&!referenceCopy.active&&
    !anchorPlacementInProgress;

  const anchorPlacementToken=useRef(0);
  const tramReacquireAbort=useRef<AbortController|null>(null);
  // Undo and Redo restore the original point objects; keep receipts only for
  // live in-memory anchors, never for a new click at identical coordinates.
  const tramRoadSnaps=useRef(new WeakMap<GeoPoint,{proof:AcceptedTramRoadSnap;world:unknown;epoch:number;plan:number;path:number}>());
  const finishPendingPlacement=useRef(false);
  const pendingPlacementPoint=useRef<{x:number;y:number}|null>(null);
  // Preserve intent by reference, without serializing full geometry on every hover.
  const placementContextKey=useMemo(()=>({}),[enabled,activePlanId,activePathId,activeTool,mapMode,draftPoints,draftEditedGeometry,draftAnchorLevels,draftFinished,draftExtension,draftDestination,activeBranch,infrastructure,trackType,designSpeedKmh,roadFollowing,parallelMode,parallelStart,referenceCopy.active,plannedStations,alignmentName,nativeTramGuide?automaticRoad.snapshot:null,nativeTramGuide?roadDrawingBundle:null]);
  const anchorPlacementContext=useRef<{world:unknown;key:unknown}>({world,key:placementContextKey});
  anchorPlacementContext.current={world,key:placementContextKey};
  useEffect(()=>{anchorPlacementPending.current=false;anchorPlacementToken.current++;finishPendingPlacement.current=false;roadFinishAfterPlacement.current=false;setAnchorPlacementInProgress(false);setAnchorPlacementWaiting(false);},[world,placementContextKey]);
  useEffect(()=>()=>{tramReacquireAbort.current?.abort();},[world,placementContextKey]);
  useEffect(()=>()=>{anchorPlacementContext.current={world:null,key:'unmounted'};},[]);

  const streetTramTileIsCurrent=(proof:{tileKey:string;tileRawSha256:string})=>
    proof.tileRawSha256===automaticRoad.snapshot?.tileSnapshots?.find(t=>t.bundle.source.source===proof.tileKey)?.bundle.sourceSha256;
  const visibleStreetTram=streetTramPreview?.world===world&&streetTramPreview.basis===draftPoints&&
    streetTramPreview.plan===activePlanId&&streetTramPreview.path===activePathId&&streetTramPreview.continuation===(draftExtension??undefined)&&activeTool==="tracks"&&
    mapMode==="draw"&&trackType==="tram"&&roadFollowing&&!tramCtrl&&infrastructure.level==="ground"&&
    (startLevel===undefined||startLevel==="ground")&&!(draftAnchorLevels??[]).some(level=>level!=="ground")&&
    !parkedPaths.some(p=>p.planId===activePlanId)&&!plannedStations.some(p=>stationPlanId(p)===activePlanId)&&
    ("chainGuide" in streetTramPreview.args?streetTramPreview.args.chainGuide.tiles.every(streetTramTileIsCurrent):
      streetTramTileIsCurrent(streetTramPreview.args.guide)&&
      (!streetTramPreview.args.guide.adjacentEnd||streetTramTileIsCurrent(streetTramPreview.args.guide.adjacentEnd)))
    ?streetTramPreview:null;
  useEffect(()=>{if(streetTramPreview&&!visibleStreetTram)setStreetTramPreview(null);},[streetTramPreview,visibleStreetTram]);

  function commitStreetTram() {
    const preview=visibleStreetTram;
    if(!enabled||!preview||busyRef.current||anchorPlacementPending.current)return;
    void perform(async()=>{
      const roads=constructionRoadAttachment(automaticRoad.snapshot,world).roads;
      const snapshot=await invoke<World>("build_authored_street_tram",{
        ...preview.args,roads,alignment:{name:alignmentName,points:preview.points,
          trackType:"tram",designSpeedKmh,infrastructure,stations:[],
          ...(preview.continuation?{extension:preview.continuation}:{})}
      });
      setStreetTramPreview(null);setWorld(snapshot);setDirty(true);setConfirmLoad(false);
      clearCurrentPlan();changeMapMode("draw");setNotice("道路に沿う併用軌道を建設しました。");
    });
  }

  function commitAlignment() {
    if (!enabled || !planBuildable || anchorGesture.current || anchorPlacementPending.current) return;
    void perform(async () => {
      const ids=new Set(buildPaths.map(path=>path.id));
      if(currentStations.some(item=>!item.valid))throw Error("計画駅が線路に収まりません。線形を元に戻すか、計画駅を取り消してください。");
      const {alignments:requests,stations}=constructionRequest;
      const first=requests[0];
      const batch=stations.length>0||requests.length>1||requests.some(request=>request.extension||request.destination||request.reference||request.ofmPath||request.junctionSource);
      let attachment=roadAttachment;
      let roadConfirmation='';
      if(requests.length){
        setNotice("道路を確認中（最大25秒）。未取得の場合も通常建設を続行します。");
        const controller=new AbortController();roadCheckAbort.current=controller;setRoadCheckPending(true);
        let loaded:Awaited<ReturnType<typeof constructionRoad.ensureCurrent>>;
        try{loaded=await constructionRoad.ensureCurrent(controller.signal);}
        finally{roadCheckAbort.current=null;setRoadCheckPending(false);}
        if(controller.signal.aborted){setNotice("着工を取り消しました。下書きは保持しています。");return;}
        if(loaded.status==='stale'||latestBuild.current.world!==world||latestBuild.current.key!==buildIdentity)throw Error("計画またはワールドが変更されたため着工を中止しました。");
        attachment=constructionRoadAttachment(loaded.status==='ready'?loaded.snapshot:undefined,world);
        roadConfirmation=loaded.status==='unavailable'?`踏切未確認：${loaded.reason.replace(/^道路未確認：/,'')}。後から道路を取得しただけでは既設線路へ反映されません。`:attachment.reason;
      }
      const roads=attachment.roads;
      const snapshot=await invoke<World>(roads.length?"build_plan_with_roads":batch?"build_plan":first.stations.length?"build_alignment_with_stations":"build_alignment",roads.length?{alignments:requests,stations,roads}:batch?{alignments:requests,stations}:first.stations.length?first:{name:first.name,points:first.points,anchorIndices:first.anchorIndices,trackType:first.trackType,designSpeedKmh:first.designSpeedKmh,infrastructure:first.infrastructure,startLevel:first.startLevel,anchorLevels:first.anchorLevels});
      ids.forEach(discardPlanStations);setPlannedStations(items=>items.filter(station=>stationPlanId(station)!==activePlanId));setParkedPaths(paths=>paths.filter(path=>path.planId!==activePlanId));
      const created=snapshot.alignments?.find(a=>!alignments.some(old=>old.id===a.id));
      setWorld(snapshot);setDirty(true);setConfirmLoad(false);setDraftFinished(false);setDraftAnchorLevels(undefined);setRedoAnchorLevels([]);setHeightAnchor(null);setDraftPoints([]);setDraftEditedGeometry(undefined);setAnchorUndo([]);setAnchorRedo([]);setRedoPoints([]);setHoverPoint(null);setDraftExtension(null);setDraftDestination(undefined);setDraftRedoDestination(undefined);
      const saved=savedPlanFingerprint,remaining=removeConstructionPlan(constructionPlans,activePlanId);
      const nextPlans:ConstructionPlans={...remaining,redoPoints:[],redoAnchorLevels:[],draftRedoDestination:undefined,parkedPaths:remaining.parkedPaths.map(path=>({...path,redo:[],redoAnchorLevels:[],redoDestination:undefined}))};
      restoreConstructionPlans(nextPlans);setSavedPlanFingerprint(saved);
      constructionUndoFloor.current=new Map([[nextPlans.activePathId,nextPlans.draftPoints.length],...nextPlans.parkedPaths.map(path=>[path.id,path.points.length] as [number,number])]);
      pendingPathHistory.current=null;clearStationHistory();setTrackDeleteUndo(null);setTrackDeleteRedo(null);
      if(created)setAlignmentId(created.id);
      changeMapMode(activeTool==="stations"?"place":"draw");setNotice((requests.length?"線路を建設しました。計画駅も反映しました。":"計画駅を建設しました。")+(roadConfirmation?` ${roadConfirmation}`:""));
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
    const placement=selected===null?undefined:stationPlacementById.get(selected);
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
      if(event.key==="Escape"&&ordinaryBusStops.active&&!ordinaryBusStops.mutating()&&!event.isComposing&&event.keyCode!==229&&!event.defaultPrevented){
        event.preventDefault();if(ordinaryBusStops.choices.length)ordinaryBusStops.cancelChoice();else if(ordinaryBusStops.movingStopId)ordinaryBusStops.cancelMove();else ordinaryBusStops.cancel();return;
      }
      if(event.key==="Escape"&&ordinaryBusActive&&(ordinaryBus.draft||ordinaryBus.deleting)&&!ordinaryBus.mutating()&&!event.isComposing&&event.keyCode!==229&&!event.defaultPrevented){event.preventDefault();if(!ordinaryBus.exitExtension())ordinaryBus.cancel();return;}
      if(event.key==="Escape"&&(roadMapFrom!==null||roadPlayerMapPick)&&!event.isComposing&&event.keyCode!==229&&!event.defaultPrevented){event.preventDefault();cancelRoadMapPick();return;}
      if (event.key === "Escape" && routeMapSelect.current && !event.defaultPrevented && !event.isComposing && event.keyCode!==229) {event.preventDefault();cancelRouteMapPick();return;}
      if(deleteTarget||confirmRangeDelete||document.querySelector('[aria-label="系統削除の確認"]'))return;
      if (status==="ready" && (event.ctrlKey||event.metaKey) && !event.altKey && !event.shiftKey &&
          event.key.toLowerCase()==="s" && !event.defaultPrevented && !event.isComposing && event.keyCode!==229) {
        event.preventDefault();
        if (event.repeat||!enabled||busyRef.current||saveBlocked||pendingDelete||closeGuard.confirmClose||
            gestureRef.current||anchorGesture.current||boxGesture.current||
            document.querySelector('dialog[open], [role="dialog"]')) return;
        if (!slotsReady||!activeSlotId) {
          setNotice("上書きするセーブがありません。最初は「セーブデータ」から保存してください。");
          return;
        }
        saveWorld(activeSlotId);
        return;
      }
      if (event.target instanceof HTMLElement && (event.target.isContentEditable || /INPUT|SELECT|TEXTAREA/.test(event.target.tagName))) return;
      if (mainTab==="construction" && enabled && !busyRef.current && !event.defaultPrevented &&
        !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey && !event.repeat &&
        !event.isComposing && event.keyCode!==229 && !gestureRef.current && !anchorGesture.current &&
        !boxGesture.current && !ordinaryBus.mutating() && !ordinaryBusStops.mutating() &&
        !document.querySelector('dialog[open], [role="dialog"]') &&
        !(event.target instanceof Element && event.target.closest('[contenteditable]:not([contenteditable="false"]), [role="textbox"]'))) {
        const match=constructionTools.find(([, ,shortcut])=>shortcut===event.key.toUpperCase());
        if (match) {
          event.preventDefault();
          if (match[0]!==activeTool||mapMode!==constructionMapMode(match[0])) {
            chooseConstructionTool(match[0]);
            if(document.activeElement?.closest(".construction-tabs"))document.getElementById("tab-"+match[0])?.focus();
          }
          return;
        }
      }
      if(ordinaryBusStops.active&&(event.ctrlKey||event.metaKey)&&!event.altKey&&/^[zy]$/i.test(event.key)){
        event.preventDefault();return; // a draft must not undo the saved world.
      }
      if(ordinaryBusActive&&ordinaryBus.draft&&ordinaryBus.waypoints&&!event.isComposing&&event.keyCode!==229&&!event.defaultPrevented&&!event.altKey){
        const key=event.key.toLowerCase(),ctrl=event.ctrlKey||event.metaKey;
        if(ctrl&&(key==="z"||key==="y")){
          event.preventDefault();if(!event.repeat)ordinaryBus.history(key==="y"||event.shiftKey?'redo':'undo');return;
        }
        if(!ctrl&&!event.shiftKey&&key==="enter"&&!(event.target instanceof HTMLElement&&event.target.closest('button, a, [role="button"]'))){
          event.preventDefault();if(!event.repeat)void ordinaryBus.commit();return;
        }
      }
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
      if (event.key === "Escape" && (event.defaultPrevented || event.isComposing || event.keyCode === 229 || document.querySelector('dialog[open], [role="dialog"]'))) return;
      if(event.key==="Escape"&&clearStationSelection()){event.preventDefault();return;}
      if (event.key === "Escape" && visibleStreetTram) {event.preventDefault();setStreetTramPreview(null);roadDrawing.cancel();return;}
      if (event.key === "Escape") {clearTrackSelection();roadDrawing.cancel();cancelParallel(); if(activeTool==="tracks")setActiveTool("inactive"); changeMapMode("move"); setStationCursor(null); }
    };
    window.addEventListener("keydown", key); return () => window.removeEventListener("keydown", key);
  });
  useEffect(() => {
    const refresh=(disableStraightSnap:boolean)=>{
      const cursor=drawingCursor.current;
      if(!cursor||!enabled||referenceCopy.active||mapMode!=="draw"||activeTool!=="tracks"||gestureRef.current||anchorGesture.current)return;
      const {point,width,height}=cursor;
      setHoverPoint(nativeTramGuide&&usesRoadSnap(disableStraightSnap)?null:!snappedEndpoint(point,width,height)?drawingPoint(point,width,height,disableStraightSnap):null);
    };
    const modifier=(event:KeyboardEvent)=>{
      if(event.key!=="Control"||event.repeat)return;
      const held=event.type==="keydown";
      const typing=event.target instanceof HTMLElement&&(event.target.isContentEditable||/INPUT|SELECT|TEXTAREA/.test(event.target.tagName));
      if(typing&&(!tramDrawing||held))return;
      if(tramDrawing)updateTramCtrl(held);
      else{
        if(roadFollowing&&held)roadDrawing.cancel();
        if(roadFollowing&&!held&&drawingCursor.current)roadDrawing.hover({point:drawingCursor.current.point,start:!draftFinished?draftPoints.at(-1):undefined,radiusM:20,authoredTramCue:nativeTramGuide});
      }
      refresh(held);
    };
    const blur=()=>{drawingCursor.current=null;setHoverPoint(null);if(tramDrawing){setTramCtrl(false);roadDrawing.clearHover();}};
    window.addEventListener("keydown",modifier);window.addEventListener("keyup",modifier);window.addEventListener("blur",blur);
    return ()=>{window.removeEventListener("keydown",modifier);window.removeEventListener("keyup",modifier);window.removeEventListener("blur",blur);};
  });
  useEffect(()=>{
    if(!tramDrawing)return;
    const cursor=drawingCursor.current;
    if(!roadSnapShown||!cursor||!enabled||parallelMode||referenceCopy.active){
      roadDrawing.clearHover();return;
    }
    // Overlay and worker may only become available after pressing Ctrl with
    // the mouse motionless. Re-run the current pick as soon as the source is ready.
    roadDrawing.hover({point:cursor.point,start:!draftFinished?draftPoints.at(-1):undefined,radiusM:20,authoredTramCue:nativeTramGuide});
  },[tramDrawing,roadSnapShown,roadDrawingBundle,roadDrawingTiles,enabled,parallelMode,referenceCopy.active]);
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
      setNotice("路線名を適用しました。保存でファイルに反映します。");
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
      setWorld(snapshot);setDirty(true);setConfirmLoad(false);resetRoute(created);setNotice("路線と所属駅を適用しました。系統の経路は変更していません。保存でファイルに反映します。");
    } finally {busyRef.current=false;setBusy(false);}
  }
  function resetService(service?: OperatingService) {
    setExtensionServiceDirty(false);
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
    busyRef.current=true;setBusy(true);try{const snapshot=await invoke<World>("create_service_path",draft);setWorld(snapshot);setDirty(true);setPathServiceDirty(false);setNotice("系統を作成しました。保存でファイルに反映します。");}finally{busyRef.current=false;setBusy(false);}
  }
  async function duplicateSelectedService() {
    if(!enabled||serviceActionHasEdits||!world||!serviceId||!selectedServicePath)return;
    const source=world.services.find(s=>s.id===serviceId);
    if(!source)return;
    await perform(async()=>{
      const snapshot=await invoke<World>("duplicate_service",{serviceId,expectedPath:selectedServicePath,expectedName:source.name});
      const created=snapshot.services.find(s=>!world.services.some(old=>old.id===s.id));
      setWorld(snapshot);setDirty(true);setPathServiceDirty(false);resetService(created);
      setNotice("別の系統として複製しました。元の系統は変更していません。保存でファイルに反映します。");
    });
  }
  function completeSave() {
    setSavedViewFingerprint(JSON.stringify(camera));setSavedPlanFingerprint(planFingerprint);
    setDirty(false);setStationUndo(history=>history.filter(entry=>entry.planned));setStationRedo(history=>history.filter(entry=>entry.planned));
    setNotice(referenceCopy.active?"ワールドと未着工計画を保存しました。選択中の参照候補は計画に追加するまで保存されません。":"ワールドと未着工計画を保存しました。");
  }
  async function refreshSaveSlots(){
    const generation=++slotGeneration.current;setSlotsLoading(true);setSlotsError("");
    try{
      const result=await invoke<{active_slot_id:string|null;slots:SaveSlot[];deleted_slots?:ArchivedSaveSlot[]}>("list_save_slots");
      if(!Array.isArray(result?.slots)||!(result.active_slot_id===null||typeof result.active_slot_id==="string"))throw Error("保存一覧を確認できません。");
      if(generation!==slotGeneration.current)return;
      setSaveSlots(result.slots);setDeletedSaveSlots(result.deleted_slots??[]);setActiveSlotId(result.active_slot_id);setSlotsReady(true);
    }catch(error){if(generation===slotGeneration.current){setSlotsError(detail(error));setSlotsReady(false);}}
    finally{if(generation===slotGeneration.current)setSlotsLoading(false);}
  }
  useEffect(()=>{if(status==="ready")void refreshSaveSlots();return()=>{slotGeneration.current++;};},[status]);
  async function renameSavedWorld(id:string,expectedName:string,name:string){
    if(busyRef.current||status!=="ready")throw Error("ほかの処理が完了してから操作してください。");
    busyRef.current=true;setBusy(true);
    try{
      await invoke("rename_save_slot",{slotId:id,expectedName,name});
      await refreshSaveSlots();
      setNotice("セーブデータの名前を変更しました。");
    }finally{busyRef.current=false;setBusy(false);}
  }
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
  function save() { saveWorld(); }
  function saveWorld(slotId?:string) {
    if (saveBlocked) return;
    void perform(async () => {
      const result=await invoke<{saved:boolean;replacement_required?:{token:string;company_name:string}}>("save_world",{constructionPlans:persistedPlans,viewState:camera,...(slotId===undefined?{}:{slotId})}).catch(error=>{if(detail(error).includes("save file changed since it was observed"))throw Error("保存先が読み込み時から変更されています。編集中の内容は保持しています。未保存のまま読込や終了をせず、保存先を確認してください。");throw error;});
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

  const showPlanHover=!!planHover&&!!hoveredPlan&&!referenceCopy.active&&!dragging&&(activeTool==="tracks"||activeTool==="inactive");
  const mapMetrics=mapRenderMetrics(camera.zoom,lineWidth,viewport.scale);
  const roadTransportAvailable=info?.road_transport_enabled===true;
  const constructionTools=[["tracks","線路","1"],["stations","駅設置","2"],["delete","削除","3"],["select","選択","4"],...(ordinaryBusAvailable?[["ordinary-bus","バス","U"]] as const:[]),...(roadTransportAvailable?[["road-transport","道路交通（検証）",undefined]] as const:[])] as const;
  const qaRoadEdges=useMemo(()=>{
    const edges=world?.road_transport?.edges??[];
    return edges.length<=150&&edges.every(edge=>edge.source==="synthetic-qa-not-real-roads")
      ? edges.filter(edge=>(edge.bus_access==="allowed"||edge.tram_access==="allowed")&&Array.isArray(edge.geometry))
      : [];
  },[world?.road_transport]);
  const qaRoadNodes=useMemo(()=>{
    const ids=new Set(qaRoadEdges.flatMap(edge=>[edge.from,edge.to]));
    return (world?.road_transport?.nodes??[]).filter(node=>ids.has(node.id)&&Number.isFinite(node.point.latitude)&&Number.isFinite(node.point.longitude));
  },[qaRoadEdges,world?.road_transport]);
  const ordinaryBusMultiple=info?.ordinary_ofm_bus_multiple_enabled===true;
  const [ordinarySelection,setOrdinarySelection]=useState<{epoch:number;id:string}>();
  const ordinaryProofs=world?.ofm_gameplay_bus_routes;
  const seamProofs=world?.ofm_installed_seam_bus_routes;
  const installedBusEditingLocked=!!(seamProofs&&Object.keys(seamProofs).length);
  const ordinaryRoutes=useMemo(()=>{
    const routes=world?.road_transport?.bus_routes??[],owner=world?.companies[0]?.id;
    if(ordinaryBusMultiple)return routes.filter(r=>r.owner===owner&&(ordinaryProofs&&Object.hasOwn(ordinaryProofs,r.id)||seamProofs&&Object.hasOwn(seamProofs,r.id)));
    return world?.ofm_gameplay_bus?routes.filter(r=>r.owner===owner).slice(0,1):[];
  },[world,ordinaryProofs,ordinaryBusMultiple,seamProofs]);
  const ordinaryRoute=(ordinarySelection?.epoch===worldEpoch&&ordinaryRoutes.find(r=>r.id===ordinarySelection.id))||ordinaryRoutes[0];
  const ordinaryRouteProof=ordinaryRoute?(ordinaryBusMultiple?ordinaryProofs?.[ordinaryRoute.id]:world?.ofm_gameplay_bus)??undefined:undefined;
  const ordinaryBuiltPath=useMemo(()=>{
    if(!ordinaryBusActive||!ordinaryRoute||!world?.road_transport)return '';
    const edges=firstByKey(world.road_transport.edges,edge=>edge.id);
    const selected=ordinaryRoute.edges.map(id=>edges.get(id));
    if(selected.some(e=>!e||e.geometry.length<2))return '';
    if(ordinaryRouteProof?.sameAtomStops){
      const [edge]=selected,cuts=ordinaryRouteProof.sameAtomStops;
      if(selected.length!==1||edge?.geometry.length!==2||cuts.length!==2||
         !cuts.every(q=>Number.isSafeInteger(q)&&q>0&&q<1_000_000)||cuts[0]>=cuts[1])return '';
      const [a,b]=edge.geometry;
      return mapPath(cuts.map(q=>({
        latitude:a.latitude+(b.latitude-a.latitude)*q/1_000_000,
        longitude:a.longitude+(b.longitude-a.longitude)*q/1_000_000,
      })),camera);
    }
    if(ordinaryRouteProof?.fractionalTerminals){
      const first=ordinaryRoute.stops[0],last=ordinaryRoute.stops.at(-1);
      if(ordinaryRoute.stops.length<2||!first||!last||first.edge_index!==0||last.edge_index!==selected.length-1)return '';
      const at=(row:typeof first)=>{
        const stop=world.road_transport!.bus_stops.find(s=>s.id===row.stop_id);
        const edge=selected[row.edge_index];
        if(!stop||!edge||stop.edge_id!==edge.id||edge.geometry.length!==2||
           !Number.isFinite(stop.position)||stop.position<=0||stop.position>=1)return null;
        const [a,b]=edge.geometry;
        return {latitude:a.latitude+(b.latitude-a.latitude)*stop.position,longitude:a.longitude+(b.longitude-a.longitude)*stop.position};
      };
      const a=at(first),z=at(last);
      if(!a||!z)return '';
      return mapPath([a,...selected.slice(0,-1).map(edge=>edge!.geometry.at(-1)!),z],camera);
    }
    return selected.map(e=>mapPath(e!.geometry,camera)).join(' ');
  },[ordinaryBusActive,ordinaryRoute,ordinaryRouteProof,world?.road_transport,camera]);
  const ordinaryBuiltStops=useMemo(()=>{
    if(!ordinaryBusActive||!ordinaryRoute||!world?.road_transport||info?.ordinary_ofm_bus_interior_stops_enabled!==true)return [];
    const graph=world.road_transport,stops=new Map(graph.bus_stops.map(stop=>[stop.id,stop]));
    const edges=new Map(graph.edges.map(edge=>[edge.id,edge]));
    const seen=new Set<string>();
    return ordinaryRoute.stops.flatMap(item=>{
      if(seen.has(item.stop_id))return [];
      seen.add(item.stop_id);
      const stop=stops.get(item.stop_id),edge=stop&&edges.get(stop.edge_id);
      if(!stop||!edge||edge.id!==ordinaryRoute.edges[item.edge_index]||edge.geometry.length!==2||!Number.isFinite(stop.position)||stop.position<0||stop.position>1)return [];
      const [a,b]=edge.geometry;
      return [{id:stop.id,name:stop.name,point:{
        latitude:a.latitude+(b.latitude-a.latitude)*stop.position,longitude:a.longitude+(b.longitude-a.longitude)*stop.position,
      }}];
    });
  },[ordinaryBusActive,ordinaryRoute,world?.road_transport,info?.ordinary_ofm_bus_interior_stops_enabled]);
  const ordinaryBusBlocked=!ordinaryRoute&&!!(world?.ofm_gameplay_bus||world?.road_transport&&
    (world.road_transport.nodes.length||world.road_transport.edges.length||world.road_transport.bus_stops.length||
      world.road_transport.bus_routes.length||world.road_transport.evidence?.length));
  const ordinaryBusEnabled=enabled&&!unapplied&&ordinaryBusActive;
  async function mutateOrdinaryBus(command:'put_gameplay_ofm_bus'|'remove_gameplay_ofm_bus',args:OrdinaryBusQuery|{routeId:string}){
    if(!ordinaryBusEnabled||busyRef.current)throw Error("別の操作を完了してからバス系統を変更してください。");
    busyRef.current=true;setBusy(true);
    try{
      const expectedWorldToken='expectedWorldToken' in args?args.expectedWorldToken:await invoke<string>('ofm_world_token');
      const snapshot=await invoke<World>(command,{...args,expectedWorldToken});
      setWorld(snapshot);setDirty(true);setConfirmLoad(false);
      if(command==='put_gameplay_ofm_bus')setOrdinarySelection({epoch:worldEpoch,id:args.routeId});
    }finally{busyRef.current=false;setBusy(false);}
  }
  async function installSeamBus({routeId,...request}:InstallSeamBusArgs){
    if(!ordinaryBusEnabled||busyRef.current||installedBusEditingLocked)throw Error("別の操作を完了してから確認済み区間を作成してください。");
    busyRef.current=true;setBusy(true);
    try{
      const snapshot=await invoke<World>("put_installed_seam_bus",request);
      setWorld(snapshot);setDirty(true);setConfirmLoad(false);setOrdinarySelection({epoch:worldEpoch,id:routeId});
    }finally{busyRef.current=false;setBusy(false);}
  }
  async function acquireSeamBusRoad({expectedWorldToken,catalogKey,missing}:AcquireSeamBusArgs){
    if(!ordinaryBusEnabled||busyRef.current||installedBusEditingLocked||missing.length<1||missing.length>2)
      throw Error("別の操作を完了してから確認済み区間を開き直してください。");
    const revision=missing[0].revision;
    if(missing.some(tile=>tile.revision!==revision||tile.z!==14||tile.decoder_rule!==OFM_EVIDENCE_RULE))
      throw Error("確認済みの道路データを開き直してください。");
    const refs:OpenFreeMapEvidenceRequest[]=missing.map(t=>({revision,z:14,x:t.x,y:t.y,rawSha256:t.raw_sha256,decoderRule:OFM_EVIDENCE_RULE}));
    busyRef.current=true;setBusy(true);
    try{
      const pairs=await readObservedRoadTilePairs(refs,automaticRoad.readPinnedSeamTiles,[]);
      const raw=[...new Map(pairs.map(p=>[p.rawEvidence.sha256,p.rawEvidence])).values()];
      const snapshot=await invoke<World>("retain_installed_seam_tiles",{expectedWorldToken,catalogKey,raw});
      setWorld(snapshot);setDirty(true);setConfirmLoad(false);
    }finally{busyRef.current=false;setBusy(false);}
  }
  const ordinaryRoutePattern=ordinaryRouteProof?.pattern;
  const ordinaryBus=useOrdinaryBusDraft({on:ordinaryBusActive,allow:ordinaryBusEnabled,scope:world,
    snapshot:automaticRoad.snapshot,loading:automaticRoad.status==='loading',route:ordinaryRoute?{...ordinaryRoute,pattern:ordinaryRoutePattern,path:ordinaryRouteProof}:undefined,multiple:ordinaryBusMultiple,roundTrip:info?.ordinary_ofm_bus_round_trip_enabled===true,fractionalAvailable:info?.ordinary_ofm_bus_fractional_enabled===true,generalFractionalAvailable:info?.ordinary_ofm_bus_ground_fractional_enabled===true,waypoints:info?.ordinary_ofm_bus_waypoints_enabled===true,autoStopsAvailable:info?.ordinary_ofm_bus_auto_stops_enabled===true,closedCapable:info?.ordinary_ofm_bus_closed_loop_enabled===true,observedGrade:info?.ordinary_ofm_bus_observed_grade_enabled===true,conflictingGraph:ordinaryBusBlocked,
    descriptors:world?.road_evidence_descriptors??[],read:automaticRoad.readTileEvidenceBatch,
    observe:info?.ordinary_ofm_bus_rectangle_enabled===true?automaticRoad.observeRouteTiles:undefined,
    token:()=>invoke<string>('ofm_world_token'),preview:query=>invoke<OrdinaryBusPreviewReply>('preview_gameplay_ofm_bus',query),
    commit:query=>mutateOrdinaryBus('put_gameplay_ofm_bus',query),remove:routeId=>mutateOrdinaryBus('remove_gameplay_ofm_bus',{routeId})});
  const busStopsAvailable=ordinaryBusActive&&info?.ordinary_ofm_bus_interior_stops_enabled===true;
  const routeAllowsOrdinaryBusStops=!!ordinaryRouteProof&&!ordinaryRouteProof.sameAtomStops&&
    (!ordinaryRouteProof.fractionalTerminals||info?.ordinary_ofm_bus_ground_fractional_stops_enabled===true);
  const ordinaryBusStops=useOrdinaryBusStops({active:busStopsAvailable,enabled:ordinaryBusEnabled&&busStopsAvailable,scope:world,
    route:ordinaryRoute&&ordinaryRouteProof&&routeAllowsOrdinaryBusStops?{id:ordinaryRoute.id,name:ordinaryRoute.name,path:ordinaryRouteProof}:undefined,
    token:()=>invoke<string>('ofm_world_token'),preview:query=>invoke<OrdinaryBusPreviewReply>('preview_gameplay_ofm_bus',query),
    commit:query=>mutateOrdinaryBus('put_gameplay_ofm_bus',query)});
  const ordinaryBusStopsPicking=ordinaryBusEnabled&&ordinaryBusStops.active&&!!ordinaryBusStops.draft&&!ordinaryBusStops.busy;
  function pickOrdinaryBusStop(event:MouseEvent<SVGSVGElement>){
    event.stopPropagation();smoothZoom.cancel();
    if(!ordinaryBusStopsPicking||event.detail>1||busyRef.current||suppressMapClick.current||suppressSelectionClick.current)return;
    const rect=event.currentTarget.getBoundingClientRect();
    const at=coordinatesFromClient(event.clientX,event.clientY,rect,cameraRef.current);
    if(at)ordinaryBusStops.pick(at,cameraRef.current,mapViewport(rect.width,rect.height).scale);
  }
  const ordinaryBusPicking=ordinaryBusEnabled&&!!ordinaryBus.draft&&(ordinaryBus.waypoints?ordinaryBus.canPlace:!ordinaryBus.draft.preview&&!ordinaryBus.waiting);
  function pickOrdinaryBusPoint(event:MouseEvent<SVGSVGElement>){
    event.stopPropagation();smoothZoom.cancel();
    if(!ordinaryBusPicking||busyRef.current||suppressMapClick.current||suppressSelectionClick.current)return;
    const rect=event.currentTarget.getBoundingClientRect();
    const at=coordinatesFromClient(event.clientX,event.clientY,rect,camera);
    const handle=event.target instanceof Element?event.target.closest('[data-bus-control]')?.getAttribute('data-bus-control')??undefined:undefined;
    if(at)void ordinaryBus.pick(at,camera,mapViewport(rect.width,rect.height).scale,event.detail,handle);
  }
  const roadMapScope=roadTransportAvailable&&mainTab==="construction"&&activeTool==="road-transport";
  const qaRoadMapPicking=roadMapScope&&roadMapFrom!==null;
  const playerMapTarget=roadMapScope&&roadPlayerMapPick&&roadPlayerMapPick.epoch===worldEpoch&&roadPlayerMapPick.graph===world?.road_transport?roadPlayerMapPick:undefined;
  const playerMapPath=useMemo(()=>playerMapTarget?mapPath(playerMapTarget.edge.geometry,camera):undefined,[playerMapTarget,camera]);
  const roadMapPicking=qaRoadMapPicking||!!playerMapTarget;
  function startPlayerRoadMapPick(request:RoadPlayerMapRequest){
    const graph=world?.road_transport;
    if(!roadMapScope||!enabled||unapplied||busyRef.current||roadMapPicking||!graph)return;
    const edge=graph.edges.find(item=>item.id===request.edgeId&&item.source.startsWith("native-pilot:")&&item.bus_access==="allowed"&&item.level==="ground");
    if(!edge||edge.geometry.length<2||!(request.position>0&&request.position<1))return;
    const at=pointOnAlignment(edge.geometry,request.position);
    if(!(at.latitude>=BOUNDS.south&&at.latitude<=BOUNDS.north&&at.longitude>=BOUNDS.west&&at.longitude<=BOUNDS.east))return;
    cancelMapGesture();cancelRouteMapPick();clearTrackSelection();changeMapMode("move");
    changeCamera(cameraAt(at.latitude,at.longitude,Math.max(cameraRef.current.zoom,16)));
    setRoadPlayerMapSelection(undefined);
    setRoadPlayerMapPick({request,graph,epoch:worldEpoch,edge});
  }
  function pickPlayerRoadMapPoint(event:MouseEvent<SVGSVGElement>){
    if(!playerMapTarget)return;
    event.stopPropagation();smoothZoom.cancel();
    if(!enabled||unapplied||busyRef.current||suppressSelectionClick.current||suppressMapClick.current)return;
    const rect=event.currentTarget.getBoundingClientRect();
    const point=coordinatesFromClient(event.clientX,event.clientY,rect,camera);
    if(!point)return;
    // A click is evaluated only against the selected directed edge, never nearby graph edges.
    const snapped=nearestOnAlignment(playerMapTarget.edge.geometry,point);
    const a=project(point.latitude,point.longitude,camera),b=project(snapped.point.latitude,snapped.point.longitude,camera);
    const position=Math.round(snapped.position*100)/100;
    if(Math.hypot(a.x-b.x,a.y-b.y)*mapViewport(rect.width,rect.height).scale>10||!(position>0&&position<1))return;
    setRoadPlayerMapSelection({selection:{...playerMapTarget.request,position,revision:++roadMapRevision.current},graph:playerMapTarget.graph,epoch:playerMapTarget.epoch});
    cancelRoadMapPick();
  }
  function startRoadMapPick(){
    if(!roadTransportAvailable||!enabled||unapplied||busyRef.current||activeTool!=="road-transport"||qaRoadNodes.length<2)return;
    cancelMapGesture();cancelRouteMapPick();clearTrackSelection();changeMapMode("move");
    // The QA fixture may be outside the current viewport. Center its actual
    // coordinates rather than moving the fixture to the current map.
    let next={...cameraRef.current,latitude:qaRoadNodes.reduce((sum,node)=>sum+node.point.latitude,0)/qaRoadNodes.length,longitude:qaRoadNodes.reduce((sum,node)=>sum+node.point.longitude,0)/qaRoadNodes.length,zoom:16};
    const fits=()=>qaRoadNodes.every(({point})=>{const p=project(point.latitude,point.longitude,next);return p.x>=viewport.x+20&&p.y>=viewport.y+20&&p.x<=viewport.x+viewport.width-20&&p.y<=viewport.y+viewport.height-20;});
    while(next.zoom>6&&!fits())next={...next,zoom:next.zoom-1};
    changeCamera(next);setRoadMapFrom("");
  }
  function pickRoadMapNode(id:string){
    if(!qaRoadMapPicking||!enabled||unapplied||busyRef.current||!qaRoadNodes.some(node=>node.id===id))return;
    if(!roadMapFrom){setRoadMapFrom(id);return;}
    if(id===roadMapFrom)return;
    setRoadMapSelection({from:roadMapFrom,to:id,revision:++roadMapRevision.current});setRoadMapFrom(null);
  }
  async function runRoadTransport(command:RoadTransportMutation,args:RoadTransportArgs){
    if(!roadTransportAvailable||command==="edit_road_tram"&&info?.road_tram_enabled!==true||!enabled||unapplied||busyRef.current||activeTool!=="road-transport"||roadMapPicking)throw Error("別の操作を完了してから、道路交通を変更してください。");
    busyRef.current=true;setBusy(true);
    try{
      const expectedWorldToken=await invoke<string>("ofm_world_token");
      const snapshot=await invoke<World>(command,{...args,expectedWorldToken});
      setWorld(snapshot);setDirty(true);setConfirmLoad(false);return snapshot;
    }finally{busyRef.current=false;setBusy(false);}
  }
  return (
    <OperationProvider available={status==="ready"&&!companySetupRequired&&info?.operation_enabled===true} blocked={busy} epoch={worldEpoch} worldVersion={world} vehicleViewport={markerViewport} onChange={()=>{setDirty(true);setOperationRevision(n=>n+1);}} onRunningChange={setOperationRunning} acquireMutation={()=>{if(busyRef.current)return null;busyRef.current=true;setBusy(true);return ()=>{busyRef.current=false;setBusy(false);};}}>
    <main className="workbench" data-theme={theme} style={{"--rail-width":lineWidth,"--map-track-width":mapMetrics.track+"px","--map-track-casing":mapMetrics.trackCasing+"px","--station-label-size":mapMetrics.stationLabelSize+"px","--map-contrast":contrast/100,"--panel-width":panelWidth+"px","--ui-font":textSize+"px"} as CSSProperties}>
      <header className="app-header">
        <div className="session-status"><span className={`connection-dot ${status}`} aria-hidden="true" />{status === "ready" ? `${info?.version} / エンジン接続済み` : status === "preview" ? "ブラウザプレビュー" : status === "loading" ? "接続中" : "接続エラー"}</div>
      </header>
      <div className="toolbar">
        <div><span className="eyebrow">CONSTRUCTION LAB</span><h2>線路を描き、街をつなぐ。</h2></div>
        <OperationClock key={worldEpoch}/>
        <div className="save-controls"><span className="save-state" aria-live="polite">{unapplied&&<span>未適用の下書きあり</span>}{unapplied&&(dirty||plansDirty)&&"・"}{(dirty||plansDirty)&&<span>未保存の変更あり</span>}{!unapplied&&!dirty&&!plansDirty&&"ローカルワールド"}</span><button className="secondary" disabled={!enabled} onClick={()=>activeSlotId?requestSlotAction({kind:"load",id:activeSlotId}):openSlotPicker()}>読込</button><button disabled={!enabled || saveBlocked} aria-keyshortcuts="Control+S Meta+S" onClick={save}>保存</button></div>
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
      {hasPlans&&plansDirty&&<p className="draft-warning">未着工計画はワールドと一緒に保存されます。建設するには「計画を着工」を選んでください。</p>}
      {referenceCopy.active&&<p className="draft-warning">参照候補は計画に追加するまで保存されません。</p>}
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
          {isTauri()&&<label><input type="checkbox" checked={retainedRoadVisible} onChange={event=>setRetainedRoadVisible(event.target.checked)}/>ワールド内の参考道路を表示</label>}
          {retainedRoad.message&&<small role="status">{retainedRoad.message}</small>}
          <ReferenceStationControls pack={referencePack} visible={referenceVisible} onPack={setReferencePack} onVisible={setReferenceVisible} total={referenceResult.total} shown={referenceResult.stations.length}/>
          </details>
            <div className="map-surface">
            <EditingMapFrame enabled={status==="ready"&&!companySetupRequired} tab={mainTab} tool={activeTool} mapMode={mapMode} pickingStations={routeMapPick!==null}/>
            {roadMapPicking&&<div className="construction-toolbar" role="group" aria-label="道路交通の地図選択"><span role="status">{playerMapTarget?`強調された道路上で${playerMapTarget.request.end==="from"?"乗車":"降車"}位置を選択`:`検証用道路の${roadMapFrom?"終点":"起点"}を選択`}</span> <button type="button" className="secondary" onClick={cancelRoadMapPick}>選択をやめる</button></div>}
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
            {activeTool === "tracks" && <div className="construction-toolbar track-construction-toolbar" role="group" aria-label="線路敷設設定"><HeightControls value={selectedHeight??infrastructure.level} disabled={!enabled||!!activeBranch||referenceCopy.active||(!heightAnchor&&!!draftDestination)} onChange={changeHeight}/><div className="construction-mode-row"><TrackTypeControls value={trackType} disabled={!enabled||referenceCopy.active||!!draftExtension} onChange={chooseTrackType}/><div className="track-follow-options" role="group" aria-label="線路の追従"><button type="button" className="secondary follow-icon" aria-label="実在の路線網に合わせる" aria-describedby="follow-tip-rail" aria-pressed={referencePanel&&!roadFollowing} disabled={!enabled||!!draftExtension} onClick={()=>{cancelParallel();setParallelMode(false);referenceCopy.clear();referenceCopy.setMode("rail");setRoadFollowing(false);setReferencePanel(value=>!value||roadFollowing);}}><TrackFollowIcon kind="rail"/><span id="follow-tip-rail" role="tooltip" className="follow-tip">実在の路線網に合わせる<br/>実在の線路に沿って、線路の経路を描きます</span></button><button type="button" className="secondary follow-icon" aria-label="道路に合わせる" aria-describedby="follow-tip-road" aria-pressed={roadFollowing} disabled={!enabled||!!activeBranch} onClick={()=>{cancelParallel();setParallelMode(false);referenceCopy.clear();roadDrawing.cancel();setRoadFollowing(value=>!value);setReferencePanel(!roadFollowing);setHoverPoint(null);}}><TrackFollowIcon kind="road"/><span id="follow-tip-road" role="tooltip" className="follow-tip">道路に合わせる<br/>道路に沿って、線路の経路を描きます{trackType==="tram"&&<><br/>Ctrlで一時切替{info?.authored_street_tram_enabled===true&&<><br/>地上で確認できた一区間は併用軌道として着工できます</>}</>}</span></button><button type="button" className="secondary follow-icon" aria-label="線路の横に沿わせる" aria-describedby="follow-tip-parallel" aria-pressed={parallelMode} disabled={!enabled||!!activeBranch||!copyCanStart} onClick={()=>{cancelParallel();referenceCopy.clear();setReferencePanel(false);setRoadFollowing(false);setParallelMode(value=>!value);setHoverPoint(null);}}><TrackFollowIcon kind="parallel"/><span id="follow-tip-parallel" role="tooltip" className="follow-tip">線路の横に沿わせる<br/>青い範囲にカーソルを合わせると、別の線路と平行に線路を描きます</span></button></div>{(world?.alignment_connections?.length??0)>=2&&!activeBranch&&<button type="button" className="secondary" aria-expanded={!!branchSelection} disabled={!branchPlacementAvailable}
              onClick={()=>{if(world)setBranchPicker(branchSelection?null:{epoch:worldEpoch,basis:world,source:"",destination:""});}}>分岐として接続</button>}</div>
            <div className="construction-options-row"><label>線路数<select aria-label="敷設する線路数" value={infrastructure.track_count} disabled={!enabled||!!draftExtension||draftPoints.length>0||referenceCopy.active} onChange={e=>setInfrastructure(value=>({...value,track_count:Number(e.target.value) as 1|2|4}))}>{([1,2,4] as const).map(count=><option key={count} value={count}>{COUNT_LABELS[count]}</option>)}</select></label><details className="track-advanced-settings"><summary>詳細設定</summary><div className="target-speed-control"><label htmlFor="track-target-speed">目標最高速度</label><span className="speed-input"><button type="button" className="secondary" aria-label="目標最高速度を20 km/h下げる" disabled={!enabled||!!draftExtension||referenceCopy.active||designSpeedKmh<=1} onClick={()=>setDesignSpeedKmh(value=>Math.max(1,value-20))}>−</button><input id="track-target-speed" aria-label="線路の目標最高速度" type="number" min="1" max="600" step="1" value={designSpeedKmh} disabled={!enabled||!!draftExtension||referenceCopy.active} onChange={event=>{const value=Number(event.target.value);if(Number.isFinite(value)&&value>=1&&value<=600)setDesignSpeedKmh(value);}}/><button type="button" className="secondary" aria-label="目標最高速度を20 km/h上げる" disabled={!enabled||!!draftExtension||referenceCopy.active||designSpeedKmh>=600} onClick={()=>setDesignSpeedKmh(value=>Math.min(600,value+20))}>＋</button><span>km/h</span></span></div></details></div>
</div>}
            {visibleStreetTram&&<div className="construction-toolbar" role="group" aria-label="併用軌道の確認">
              <span role="status">道路に沿う併用軌道 {Math.round(alignmentLength(visibleStreetTram.points))} m</span>
              <button type="button" disabled={!enabled} onClick={commitStreetTram}>併用軌道を建設</button>
              <button type="button" className="secondary" disabled={!enabled} onClick={()=>setStreetTramPreview(null)}>終点を選び直す</button>
            </div>}
            {activeTool==="tracks"&&mapMode==="draw"&&(activeBranch||branchSelection&&branchPlacementAvailable)&&<div className="construction-toolbar" role="group" aria-label="既設線の分岐接続">
              {branchSelection&&branchPlacementAvailable&&!activeBranch&&<>
                {([["source","分岐の起点"],["destination","分岐の終点"]] as const).map(([key,label])=><label key={key}>{label}
                  <select value={branchSelection[key]} onChange={e=>setBranchPicker({...branchSelection,[key]:e.target.value})}>
                    <option value="">端点を選ぶ</option>{branchEnds.map(end=><option key={end.key} value={end.key}>
                      {branchEndLabel(end)}
                    </option>)}
                  </select>
                </label>)}
                <small>{branchEnds.length?"自社の地上単線で、既に1本だけ接続している端点を選びます。線路名と方角で確認してください。":"分岐を追加できる端点がありません。"}</small>
                {!branchDraftAvailable&&<small>進行中の線路計画を終えるか、対象の分岐計画に切り替えてください。</small>}
                <button type="button" className="secondary" disabled={!branchDraftAvailable||!selectedBranchStart||!selectedBranchEnd||
                  selectedBranchStart.alignment.id===selectedBranchEnd.alignment.id||
                  distanceM(selectedBranchStart.point,selectedBranchEnd.point)<.1} onClick={beginBranchDrawing}>選んだ端点で分岐を計画</button>
              </>}
              {activeBranch&&<small>分岐の終点: {branchEnds.find(end=>end.key===endpointKey(activeBranch.destination))?.alignment.name??"接続先を確認してください"}</small>}
            </div>}
            {activeTool === "tracks" && mapMode==="draw" && !parallelMode && !referenceCopy.active && !dragging && !anchorPreview && cursorPosition && !endpointCandidate && !draftFinished && !showPlanHover && (draftPoints.length>0 || (roadSnapShown&&!roadDrawingBundle) || (roadSnapShown&&roadDrawing.preview?.kind==='error') || !!drawingPreviewError) && <div className="construction-tooltip drawing-tooltip drawing-metrics" role="status" style={{left:Math.max(8,Math.min(mapSize.width-210,cursorPosition.x+18)),top:Math.max(8,Math.min(mapSize.height-90,cursorPosition.y+18))}}>
              {((roadSnapShown&&!roadDrawingBundle) || (roadSnapShown&&roadDrawing.preview?.kind==='error') || drawingPreviewError || nativeValidationError)&&<strong>{roadSnapShown&&!roadDrawingBundle?automaticRoad.message:roadSnapShown&&roadDrawing.preview?.kind==='error'?roadDrawing.preview.error:`敷設不可：${drawingPreviewError??nativeValidationError}`}</strong>}
              {roadSnapShown&&(roadDrawing.preview?.kind==='junction'||roadDrawing.preview?.kind==='seam')&&<small>{roadDrawing.preview.kind==='junction'?'交差点を含む区間はクリック時に確認します':'道路のつながりはクリック時に確認します'}</small>}
              {draftPoints.length>0&&!roadSnapShown&&constructionRoadReason&&<small>{constructionRoadReason}</small>}
              {draftPoints.length>0&&!roadSnapShown&&!drawingPreviewError&&!nativeConstructionPreview?.error&&<CrossingEstimate value={nativeConstructionPreview?.value} pending={!nativeConstructionPreview||constructionRoad.status==="loading"}/>}
              {draftPoints.length>0&&<><div>長さ {Math.round(alignmentLength(draftLine))} m{candidateLine.length>1&&!drawingPreviewError?` ＋ ${Math.round(alignmentLength(roadCursorPreview?.line??candidateLine))} m`:""}</div>{!drawingPreviewError&&<div>速度上限 <span className="drawing-speed">{drawingPreviewSpeed} km/h</span>{(circleGuide||!!roadPreviewGeometry?.arcs.length)&&<small>（暫定）</small>}</div>}</>}
            </div>}
            {anchorPreview?.preview&&(anchorPreview.error || anchorPreview.snapped)&&cursorPosition&&<div className="construction-tooltip drawing-tooltip" role="status" style={{left:Math.max(8,Math.min(mapSize.width-256,cursorPosition.x+18)),top:Math.max(8,Math.min(mapSize.height-80,cursorPosition.y+18))}}>{anchorPreview.error??"位置合わせ（接続は変更しません）"}</div>}
            {destinationCandidate&&destinationError&&cursorPosition&&<div className="construction-tooltip drawing-tooltip" role="status" style={{left:cursorPosition.x+18,top:cursorPosition.y+18}}>{destinationError}</div>}
            {endpointCandidate&&!endpointCandidateAllowed&&cursorPosition&&<div className="construction-tooltip drawing-tooltip" role="status" style={{left:Math.max(8,Math.min(mapSize.width-256,cursorPosition.x+18)),top:Math.max(8,Math.min(mapSize.height-100,cursorPosition.y+18))}}>
              <strong>{endpointCandidateError}</strong>
            </div>}
            {activeTool==="tracks"&&mapMode==="draw"&&draftExtension&&draftPoints.length>0&&!referenceCopy.active&&<div className="extension-status" role="status">{activeBranch?"分岐元を選択済み：":"延伸元を選択済み："}{alignments.find(a=>a.id===draftExtension.alignmentId)?.name||"既存線路"}の{draftExtension.endpoint==="start"?"始点":"終点"}（着工前）</div>}
            {showPlanHover&&!endpointCandidate&&planHover&&hoveredPlan&&<div className="construction-tooltip plan-hover-tooltip" role="status" style={{left:Math.max(8,Math.min(mapSize.width-240,planHover.x+18)),top:Math.max(8,Math.min(mapSize.height-80,planHover.y+18))}}><strong>{hoveredPlan.name.trim()||"線路計画"}</strong>{hoveredGeometry?.error?<div>敷設不可：{hoveredGeometry.error}</div>:!cursorPreviewActive&&buildPaths.some(path=>path.id===planHover.id)&&nativeValidationError&&<div>この計画は敷設不可：{nativeValidationError}</div>}<div>{plannedTrackHeight(hoveredPlan,hoveredGeometry?.points??hoveredPlan.line,planHover.point)} · {hoveredSpeed===null?"制限速度：未算定":`${hoveredSpeed} km/h（暫定）`}</div></div>}
            <svg ref={svgRef} tabIndex={0} onKeyDown={event=>{
              if(activeTool!=="stations"||mapMode!=="place"||!enabled||event.ctrlKey||event.metaKey||event.altKey)return;
              if(event.key==="Enter"){event.preventDefault();if(!event.repeat){const rect=event.currentTarget.getBoundingClientRect();placePlatformAt(stationCursor??{latitude:camera.latitude,longitude:camera.longitude},rect.width,rect.height);}return;}
              if(!["ArrowLeft","ArrowRight","ArrowUp","ArrowDown"].includes(event.key))return;
              event.preventDefault();
              const p=stationCursor??{latitude:camera.latitude,longitude:camera.longitude};
              const moved=panCamera({...camera,latitude:p.latitude,longitude:p.longitude},event.key==="ArrowLeft"?-5:event.key==="ArrowRight"?5:0,event.key==="ArrowUp"?-5:event.key==="ArrowDown"?5:0);
              setStationCursor({latitude:moved.latitude,longitude:moved.longitude});
              const rect=event.currentTarget.getBoundingClientRect();if(rect.width>0&&rect.height>0){const viewport=mapViewport(rect.width,rect.height),screenPoint=project(moved.latitude,moved.longitude,camera);setCursorPosition({x:(screenPoint.x-viewport.x)*viewport.scale,y:(screenPoint.y-viewport.y)*viewport.scale,width:rect.width,height:rect.height});}
            }} onAuxClick={event=>{if(event.button===1)event.preventDefault();}} onPointerDown={pointerDown} onPointerMove={pointerMove} onPointerUp={(event) => pointerEnd(event)} onPointerCancel={(event) => pointerEnd(event, true)} onPointerLeave={() => { drawingCursor.current=null;ordinaryBus.clearHover();if(anchorGesture.current)return;referenceCopy.clearHover();setCursorPosition(null);setDeleteHover(null);setPlanHover(null);setBuiltHover(null); setHoverPoint(null);setStationCursor(null); if (gestureRef.current && !gestureRef.current.dragged) cancelMapGesture(); }} onLostPointerCapture={(event) => { if (event.target === event.currentTarget) pointerEnd(event, true); }} role="img" aria-label={background === "sample" ? "駅配置用サンプル地図" : background === "osm" ? "駅配置用OpenStreetMap" : "駅配置用地理院地図"} viewBox={`0 0 ${MAP_VIEWBOX.width} ${MAP_VIEWBOX.height}`} preserveAspectRatio="xMidYMid slice" onClickCapture={ordinaryBusStops.active?pickOrdinaryBusStop:ordinaryBusActive&&ordinaryBus.draft?pickOrdinaryBusPoint:playerMapTarget?pickPlayerRoadMapPoint:undefined} onClick={mapClick} className={(enabled && (playerMapTarget||ordinaryBusPicking||ordinaryBusStopsPicking||((mainTab==="routes"||mainTab==="services")&&routeMapPick)) ? "route-station-select" : dragging ? "map-dragging" : enabled && mapMode !== "move" ? "placement-enabled" : "map-move-mode")+(activeTool==="select"?" map-selection-mode":activeTool==="delete"?" map-delete-mode":"")}>
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
                {background !== "sample" ? <GsiBackground key={background} tiles={requestedTiles} camera={camera} width={MAP_VIEWBOX.width} height={MAP_VIEWBOX.height} onStatus={setTileStatus} /> : <g className="sample-background">
                  <rect width={MAP_VIEWBOX.width} height={MAP_VIEWBOX.height} fill="url(#sample-scene)" />
                  <rect width={MAP_VIEWBOX.width} height={MAP_VIEWBOX.height} fill="url(#sample-grid)" />
                </g>}
              <RetainedRoadLayer view={retainedRoad.value} camera={camera}/>
              <ReferenceGraphLayer edges={graphDisplay.edges} camera={camera} tramRoadGuideIds={tramRoadGuideIds}/>
              {automaticRailSelected&&automaticRail.view.chains.map(chain=><path key={chain.id} d={drawPath(chain.points)} className="automatic-rail-reference" stroke="#777" strokeWidth="1.5" strokeDasharray="4 4" fill="none" pointerEvents="none"/>)}
              {visibleAlignmentPaths.map(({alignment,hit}) => <g key={alignment.id} className="alignment route-base" data-alignment-id={alignment.id}>
                {!(mainTab==="construction"&&activeTool==="inactive")&&<title>{alignment.name}</title>}

                {displayInfrastructureSections(alignment).map((section,index)=><g key={index} data-track-count={section.infrastructure.track_count} data-structure-level={section.infrastructure.level} pointerEvents="none">
                  {section.infrastructure.level.startsWith('elevated')&&<path d={drawPath(section.points)} fill="none" stroke="#8095a7" strokeWidth={section.infrastructure.track_count*3+6} opacity=".4"/>}
                  {!trackRender.value&&schematicTrackPaths(section.points.map(point=>project(point.latitude,point.longitude,camera)),section.infrastructure.track_count).map((path,rail)=><TrackStroke key={rail} d={path} className={(referenceCopy.active?(manualReferenceCopyState.seamPlan?seamFocusedEndpoint?.alignment.id:referenceExtension?.alignment.id):alignmentId)===alignment.id?"alignment-path alignment-selected":"alignment-path"} style={{strokeDasharray:section.infrastructure.level.startsWith('underground')?'5 4':undefined,opacity:1}}/>)}
                </g>)}
                <path d={hit} className="alignment-hit" onPointerEnter={event=>hoverBuilt(event,alignment.id)} onPointerMove={event=>hoverBuilt(event,alignment.id)} onPointerLeave={()=>setBuiltHover(null)} onClick={(event) => { if (mapMode === "move" && activeTool!=="select" && !suppressSelectionClick.current && !roadMapPicking) { event.stopPropagation(); setAlignmentId(alignment.id); setSelectedPlanPath(null);setTrackDeleteConfirm(false); } }} />
              </g>)}
              {trackRender.value&&<TrackGeometryLayer geometry={trackRender.value} camera={camera} mapSize={mapSize} lines={alignments} selectedId={referenceCopy.active?((manualReferenceCopyState.seamPlan?seamFocusedEndpoint?.alignment.id:referenceExtension?.alignment.id)??""):alignmentId}/>}
              <g className="route-assignment-overlays" pointerEvents="none">{visibleAlignmentPaths.flatMap(({alignment})=>routeVisual.alignmentSections.get(alignment.id)?.map((points,index)=><path key={`${alignment.id}-${index}`} d={drawPath(points)} className="alignment-path route-assigned-section" />)??[])}</g>
              {(mainTab==="routes"||mainTab==="services")&&serviceMapSections.map((section,index)=><g key={section.alignmentIds.join(":")+":"+index} className="service-map-section" data-alignment-id={section.alignmentIds[0]} data-service-count={section.bands.length} pointerEvents="none">
                <title>{section.bands.map(b=>b.name).join(" / ")}</title>
                <path d={drawPath(section.points)} fill="none" stroke="white" strokeWidth={mapMetrics.serviceCasing} strokeLinecap="butt" strokeLinejoin="round"/>
                {serviceBandPaths(section.points.map(p=>project(p.latitude,p.longitude,camera)),section.bands,mapMetrics.service).map(band=><path key={band.serviceId} data-service-id={band.serviceId} d={band.d} fill="none" stroke={band.color} strokeWidth={band.width} strokeLinecap="round" strokeLinejoin="round"/>)}
              </g>)}
              {((mainTab==="routes"&&(routeCreating||routeMapEdit))||(mainTab==="services"&&(serviceCreating||extensionServiceDirty)))&&<g className="route-candidate-selection" pointerEvents="none" aria-label="選択した経路">{[...selectedRouteSections].flatMap(([id,sections])=>sections.map((points:GeoPoint[],index:number)=><path key={id+":"+index} d={drawPath(points)} fill="none" stroke="#18a6a6" strokeWidth={7/viewport.scale} strokeOpacity=".65" strokeLinecap="round" strokeLinejoin="round"/>))}</g>}
              {referenceCopy.active&&referenceCopy.roadFollow&&(referenceCopy.candidates.length?referenceCopy.candidates:referenceCopy.hoverCandidates).map(candidate=>{const a=graphNodesById.get(candidate.edge.from),b=graphNodesById.get(candidate.edge.to);return a&&b?<path key={candidate.edge.id} d={drawPath([a,b])} className="road-follow-candidate" stroke="#298746" strokeWidth="4" opacity=".7" fill="none" pointerEvents="none"/>:null;})}
              {roadSnapShown&&roadDrawing.preview?.kind==='junction'&&focusMarker(roadDrawing.preview.to.point,'anchor',roadSnapMarkerClass)}
              {roadSnapShown&&(roadDrawing.preview?.kind==='point'||roadDrawing.preview?.kind==='route'||roadDrawing.preview?.kind==='seam')&&focusMarker(roadDrawing.preview.point,'anchor',roadSnapMarkerClass)}
              {roadSnapShown&&roadDrawing.preview?.kind==='route'&&<TrackStroke d={drawInfrastructurePath(roadDrawing.preview.line,infrastructure)} className={'draft-path road-drawing-preview'+(tramRoadGuide?' tram-road-guide-preview':'')+(constructionValidationError(nativeConstructionPreview?.value)?' invalid-candidate':'')} pointerEvents="none"/>}
              {referenceCopy.active&&referencePreviewPoints&&<TrackStroke d={drawInfrastructurePath(referencePreviewPoints,referenceExtension?endpointInfrastructure(referenceExtension.alignment,referenceExtension.endpoint):infrastructure)} data-track-count={(referenceExtension?endpointInfrastructure(referenceExtension.alignment,referenceExtension.endpoint):infrastructure).track_count} className="draft-path reference-copy-preview" style={referenceCopy.roadFollow?{stroke:"#298746"}:undefined} pointerEvents="none"/>}
              {parallelMode&&activeTool==="tracks"&&parallelSources.map(source=>{const p=source.points[0],a=project(p.latitude,p.longitude,camera),b=project(p.latitude+PARALLEL_RANGE_M/111195,p.longitude,camera);return <path key={source.id} d={drawPath(source.points)} fill="none" stroke="#61afcf" strokeOpacity=".28" strokeWidth={2*Math.abs(a.y-b.y)} strokeLinecap="round" pointerEvents="none"/>;})}
              {parallelMode&&parallelStart&&(()=>{const q=project(parallelStart.point.latitude,parallelStart.point.longitude,camera);return <circle cx={q.x} cy={q.y} r={5/viewport.scale} fill="white" stroke="#287c9e" strokeWidth={1.5/viewport.scale} pointerEvents="none"/>;})()}
              {parallelMode&&parallelPreview&&<TrackStroke d={drawInfrastructurePath(parallelPreview.line,infrastructure)} data-track-count={infrastructure.track_count} className="draft-path candidate-draft" pointerEvents="none"/>}
              {parallelMode&&parallelCandidate&&!parallelStart&&(()=>{const line=offsetTrack(parallelCandidate.source.points,parallelCandidate.projection!.offset),p=line&&hoverPoint&&parallelProjection(line,hoverPoint)?.point;if(!p)return null;const q=project(p.latitude,p.longitude,camera);return <circle cx={q.x} cy={q.y} r={5/viewport.scale} fill="white" stroke="#287c9e" strokeWidth={1.5/viewport.scale} pointerEvents="none"/>;})()}
              <OperationVehicleLayer camera={camera} scale={viewport.scale} crop={viewport}/>
              <JunctionPoints mapSize={mapSize} selected={junctionSelection?.epoch===worldEpoch?junctionSelection.id:null} onSelectionChange={id=>setJunctionSelection(id===null?null:{epoch:worldEpoch,id})} isPointerSelectionSuppressed={()=>suppressSelectionClick.current} lines={alignments} connections={world?.alignment_connections??[]} camera={camera} selectable={enabled&&mapMode==="move"&&activeTool!=="delete"&&!roadMapPicking}/>
              {parkedPaths.map(plan=><g key={plan.id} className={plan.planId===activePlanId?undefined:"inactive-plan"}><TrackStroke d={drawInfrastructurePath(plan.line,plan.infrastructure)} data-track-count={plan.infrastructure?.track_count??1} className={"draft-path parked-plan"+(mapMode==="move"&&selectedPlanPath===plan.id?" track-explicit-selection":"")} data-plan-id={plan.id} pointerEvents="none" /><path d={drawPath(plan.line)} className="plan-hover-hit" data-plan-hover-id={plan.id} onClick={event=>{if(mapMode==="move"&&!roadMapPicking){event.stopPropagation();selectTrackPath(plan.id);}}} fill="none" stroke="transparent" strokeWidth="14" pointerEvents="stroke" onPointerEnter={event=>hoverPlan(event,plan.id)} onPointerMove={event=>hoverPlan(event,plan.id)} onPointerLeave={()=>setPlanHover(null)} />{drawAnchors(plan.points,true,plan.id)}</g>)}
              {tangentGuide&&<line className="construction-tangent-guide" aria-label="円弧・直線の先端延長ガイド" x1={tangentGuide.start.x} y1={tangentGuide.start.y} x2={tangentGuide.end.x} y2={tangentGuide.end.y} stroke="#65716b" strokeWidth="1" strokeDasharray="6 4" opacity=".8" vectorEffect="non-scaling-stroke" pointerEvents="none"/>}

              {!roadCursorPreview && !parallelMode && !referenceCopy.active && guideCenter && guideEdge && <circle className="curve-circle-guide" cx={guideCenter.x} cy={guideCenter.y} r={Math.hypot(guideEdge.x-guideCenter.x,guideEdge.y-guideCenter.y)} fill="none" stroke={previewError ? "#b34c4c" : "#65716b"} strokeWidth="1" opacity=".6" vectorEffect="non-scaling-stroke" pointerEvents="none" />}
              {visibleStreetTram&&<TrackStroke d={drawInfrastructurePath(visibleStreetTram.points,infrastructure)} data-track-count={infrastructure.track_count} className="draft-path candidate-draft" pointerEvents="none"/>}
              {!referenceCopy.active&&draftLine.length > 1 && <TrackStroke d={drawInfrastructurePath(draftLine,infrastructure)} data-track-count={infrastructure.track_count} className={"draft-path confirmed-draft"+(mapMode==="move"&&selectedPlanPath===activePathId?" track-explicit-selection":"")} pointerEvents="none" />}
              {!roadCursorPreview&&!parallelMode&&!referenceCopy.active&&candidateLine.length > 1 && <TrackStroke d={drawInfrastructurePath(candidateLine,infrastructure)} data-track-count={infrastructure.track_count} className={"draft-path candidate-draft"+(previewError||nativeValidationError?" invalid-candidate":"")} pointerEvents="none" />}
              {!referenceCopy.active && draftLine.length > 1 && <path d={drawPath(draftLine)} className="plan-hover-hit" data-plan-hover-id={activePathId} onClick={event=>{if(mapMode==="move"&&!roadMapPicking){event.stopPropagation();selectTrackPath(activePathId);}}} fill="none" stroke="transparent" strokeWidth="14" pointerEvents="stroke" onPointerEnter={event=>hoverPlan(event,activePathId)} onPointerMove={event=>hoverPlan(event,activePathId)} onPointerLeave={()=>setPlanHover(null)} />}
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
              {selectedStation&&selectedStationWalkLinks.length>0&&<StationWalkMapLayer selected={selectedStation} camera={camera}
                viewport={viewport} passages={selectedStationWalkLinks}/>}
              {detailedStations&&<><defs><filter id="station-group-outline" x="-10%" y="-10%" width="120%" height="120%"><feMorphology in="SourceAlpha" operator="dilate" radius={1/viewport.scale} result="expanded"/><feFlood floodColor="#333"/><feComposite in2="expanded" operator="in"/><feMerge><feMergeNode/><feMergeNode in="SourceGraphic"/></feMerge></filter></defs>{stationGroupDisplay.filter(g=>g.members.length>1).map(group=><g key={group.id} className="station-group-footprint" data-station-group={group.id} pointerEvents="none"><title>{group.name}</title><g filter="url(#station-group-outline)">{group.members.map(member=>{const planned=plannedStations.find(s=>s.id===member.id),inactive=planned&&stationPlanId(planned)!==activePlanId;return <path key={member.id} data-group-platform={member.id} d={drawPath(member.polygon)+"Z"} fill={planned?(inactive?"#b1bac3":"#c4e6f3"):"white"} stroke={planned?(inactive?"#78838e":"#287c9e"):"none"} strokeWidth="1.5" strokeDasharray={planned?"4 3":undefined} vectorEffect="non-scaling-stroke"><title>{group.name}{planned?"（未着工）":""}</title></path>;})}</g></g>)}</>}
              {plannedPlatforms.map(({station,polygon,valid})=>{const p=project(station.point.latitude,station.point.longitude,camera);return <g key={station.id} className={"planned-station"+(stationPlanId(station)===activePlanId?"":" inactive-plan")} pointerEvents="none" opacity=".7"><title>{station.name}（未着工）{valid?"":"・線形の修正が必要"}</title>{detailedStations&&valid?<path d={drawPath(polygon)+"Z"} className="station-platform" visibility={mergedStationIds.has(station.id)?"hidden":undefined}/>:<circle visibility={stationDisplayGroup(station.id)&&stationDisplayGroup(station.id)?.id!==station.id?"hidden":undefined} cx={p.x} cy={p.y} r={mapMetrics.stationRadius} fill={valid?"#777":"#b34c4c"}/>}{stationNamesVisible&&stationDisplayGroup(station.id)?.id===station.id&&<text x={p.x+mapMetrics.stationLabelOffset} y={p.y-mapMetrics.stationLabelBaseline}>{stationDisplayGroup(station.id)?.name??station.name}（計画）</text>}</g>;})}
              {stationPreview.length >= 4 && <path d={drawPath(stationPreview) + "Z"} className="station-platform station-placement-preview" opacity=".65" pointerEvents="none" />}
              {referenceVisible && referenceResult.stations.map(station=>{const p=project(station.latitude,station.longitude,camera);return <g key={station.osmType+"/"+station.osmId} className="reference-station" pointerEvents="none"><title>参考駅: {station.name} ({station.kind})</title><path d={`M${p.x},${p.y-5} l5,5 -5,5 -5,-5 Z`}/><text x={p.x+9} y={p.y-7}>参考: {station.name}</text></g>;})}
              {world?.stations.map((station) => {
                const { x, y } = project(station.latitude, station.longitude, camera);
                const placement = stationPlacementById.get(station.id);
                const polygon = stationPolygons.get(station.id);
                return <g key={station.id} pointerEvents={activeTool==="stations"&&mapMode==="place"?"none":undefined} className={"station-marker" + (selected === station.id ? " selected" : "") + (routeVisual.stations.has(station.id) ? "" : " route-unassigned")} onClick={(event) => { event.stopPropagation(); if (!suppressSelectionClick.current) selectStation(station.id); }}>
                  <title>{station.name}{placement ? ` / ホーム ${placement.length_m}m` : ""}</title>{!detailedStations&&<circle className="station-hit-target" aria-hidden="true" visibility={stationDisplayGroup(station.id)&&stationDisplayGroup(station.id)?.id!==station.id?"hidden":undefined} cx={x} cy={y} r={Math.max(5,8/viewport.scale)} fill="transparent" stroke="none"/>}{!detailedStations ? <circle visibility={stationDisplayGroup(station.id)&&stationDisplayGroup(station.id)?.id!==station.id?"hidden":undefined} cx={x} cy={y} r={mapMetrics.stationRadius} className="station-dot" /> : polygon ? <path d={drawPath(polygon) + "Z"} className="station-platform" style={mergedStationIds.has(station.id)?{fill:"transparent",stroke:"none"}:undefined} /> : <rect x={x - 13} y={y - 5} width="26" height="10" className="station-platform" />}{stationNamesVisible&&(!stationDisplayGroup(station.id)||stationDisplayGroup(station.id)?.id===station.id)&&<text x={x + mapMetrics.stationLabelOffset} y={y + mapMetrics.stationLabelBaseline}>{station.name}</text>}
                </g>;
              })}
              {((mainTab==="routes"&&(routeCreating||routeMapEdit))||(mainTab==="services"&&(serviceCreating||extensionServiceDirty)))&&<RouteStationSelection stations={world?.stations??[]} ordered={routeSelectedStations} camera={camera} scale={viewport.scale} entityLabel={mainTab==="services"?"系統":"路線"} endpoint={routeMapPick==="start"||routeMapPick==="end"?routeMapPick:null}/>}
               {ordinaryBuiltPath&&<path d={ordinaryBuiltPath} aria-label="作成済みのバス経路" fill="none" stroke="#007383" strokeOpacity={ordinaryBus.draft?.preview?0.4:1} strokeWidth="4" vectorEffect="non-scaling-stroke" pointerEvents="none"/>}
               {!ordinaryBusStops.active&&!ordinaryBus.draft&&ordinaryBuiltStops.map(stop=>{
                 const p=project(stop.point.latitude,stop.point.longitude,camera);
                 return <circle key={stop.id} aria-label={stop.name} cx={p.x} cy={p.y} r={4/viewport.scale} fill="#fef8dd"
                   stroke="#005666" strokeWidth="2" vectorEffect="non-scaling-stroke" pointerEvents="none"/>;
               })}
               {ordinaryBusActive&&ordinaryBusStops.draft&&<g aria-label="停留所を置くバス経路" pointerEvents="none">
                 <path d={mapPath(ordinaryBusStops.draft.preview.points,camera)} fill="none" stroke="#006b79" strokeWidth="5" vectorEffect="non-scaling-stroke"/>
                 {[...(ordinaryBusStops.draft.preview.interiorStops??[]),...(ordinaryBusStops.draft.preview.loopFallbackStop?[ordinaryBusStops.draft.preview.loopFallbackStop]:[])].map(view=>{
                  const p=project(view.point.latitude,view.point.longitude,camera);
                  return <circle key={view.id} aria-label={view.name} cx={p.x} cy={p.y} r={5/viewport.scale} fill="#fef8dd" stroke="#005666" strokeWidth="2" vectorEffect="non-scaling-stroke"/>;
                 })}
               </g>}
               {ordinaryBusActive&&ordinaryBus.hoverPreview&&<path d={mapPath(ordinaryBus.hoverPreview.points,camera)} aria-label="カーソル位置までのバス経路" fill="none"
                stroke="#006b79" strokeOpacity="0.7" strokeDasharray="6 5" strokeWidth="5" vectorEffect="non-scaling-stroke" pointerEvents="none"/>}
               {ordinaryBusActive&&ordinaryBus.draft?.preview&&<path d={mapPath(ordinaryBus.draft.preview.points,camera)} aria-label="バス系統のプレビュー" fill="none" stroke="#006b79" strokeWidth="5" vectorEffect="non-scaling-stroke" pointerEvents="none"/>}
               {ordinaryBusActive&&ordinaryBus.draft?.preview?.returnPath&&<path d={mapPath(ordinaryBus.draft.preview.returnPath.points,camera)} aria-label="バス系統の復路プレビュー" fill="none" stroke="#7f3c8d" strokeWidth="4" strokeDasharray="7 4" vectorEffect="non-scaling-stroke" pointerEvents="none"/>}
               {ordinaryBusActive&&!ordinaryBus.waypoints&&[ordinaryBus.draft?.start,ordinaryBus.draft?.end].map((anchor,i)=>{
                 if(!anchor)return null;const point=project(anchor.point.latitude,anchor.point.longitude,camera);
                 return <circle key={i} aria-label={i?"バスの降車位置":"バスの乗車位置"} cx={point.x} cy={point.y} r="5" fill="#006b79" stroke="white" strokeWidth="2" vectorEffect="non-scaling-stroke" pointerEvents="none"/>;
               })}
               {ordinaryBusActive&&ordinaryBus.waypoints&&!ordinaryBus.extending&&ordinaryBus.draft?.preview&&ordinaryBus.controls.slice(0,-1).map((control,i)=>{
                 const {points,viaPointIndices}=ordinaryBus.draft!.preview!;
                 const start=i===0?0:viaPointIndices?.[i-1],end=i===ordinaryBus.controls.length-2?points.length-1:viaPointIndices?.[i];
                 if(start===undefined||end===undefined||end<=start)return null;
                 return <path key={control.id} data-bus-leg-after={control.id} aria-label={`バス経路の編集区間 ${i+1}`}
                  d={mapPath(points.slice(start,end+1),camera)} stroke="transparent" strokeWidth="18" fill="none"
                  vectorEffect="non-scaling-stroke" pointerEvents="stroke" style={{cursor:"move"}}/>;
               })}
               {ordinaryBusActive&&ordinaryBus.draft?.preview?.loopFallbackStop&&(()=>{
                 const {point,name}=ordinaryBus.draft.preview.loopFallbackStop,p=project(point.latitude,point.longitude,camera);
                 return <circle aria-label={name} cx={p.x} cy={p.y} r={4/viewport.scale} stroke="#005666" fill="#fef8dd" strokeWidth="2" vectorEffect="non-scaling-stroke" pointerEvents="none"/>;
               })()}
               {ordinaryBusActive&&ordinaryBus.waypoints&&ordinaryBus.controls.map(({id,anchor},i)=>{
                 if(ordinaryBus.closed&&i===ordinaryBus.controls.length-1)return null;
                 const p=project(anchor.point.latitude,anchor.point.longitude,camera);
                 const position=i===0?(ordinaryBus.closed?"起終点":"起点"):i===ordinaryBus.controls.length-1?"終点":`経由点 ${i}`;
                 return <circle key={id} data-bus-control={id} aria-label={`バスの${position}`} cx={p.x} cy={p.y} r={7/viewport.scale}
                  fill={ordinaryBus.activeControl===id?"#cb751c":"#006b79"} stroke="white" strokeWidth="2" vectorEffect="non-scaling-stroke" style={{cursor:"move"}}/>;
               })}
               {ordinaryBusActive&&ordinaryBus.failedLeg!==undefined&&ordinaryBus.refusedControls?.slice(ordinaryBus.failedLeg,ordinaryBus.failedLeg+2).map(({id,anchor})=>{
                 const p=project(anchor.point.latitude,anchor.point.longitude,camera);
                 return <circle key={id} aria-label="接続できないバス区間の端" cx={p.x} cy={p.y} r={11/viewport.scale}
                  fill="none" stroke="#b42318" strokeWidth="3" vectorEffect="non-scaling-stroke" pointerEvents="none"/>;
               })}
               {playerMapPath&&<path d={playerMapPath} aria-label="停留所を置く道路" fill="none" stroke="#185d75" strokeWidth="5" vectorEffect="non-scaling-stroke" pointerEvents="none"/>}
              {qaRoadMapPicking&&<g className="road-qa-map-selection" aria-label="検証用道路の点">
                {qaRoadEdges.map(edge=><path key={edge.id} d={drawPath(edge.geometry)} fill="none" stroke="#185d75" strokeWidth="3" strokeDasharray="5 3" vectorEffect="non-scaling-stroke" pointerEvents="none"/>)}
                {qaRoadNodes.map(({id,point})=>{const p=project(point.latitude,point.longitude,camera);return <circle key={id} role="button" aria-label={`地図上の道路点 ${id}`} aria-pressed={id===roadMapFrom} tabIndex={enabled?0:-1}
                  cx={p.x} cy={p.y} r={9/viewport.scale} fill={id===roadMapFrom?"#c56b13":"#185d75"} stroke="white" strokeWidth="2" vectorEffect="non-scaling-stroke" style={{cursor:"pointer"}}
                  onClick={event=>{event.stopPropagation();if(!suppressSelectionClick.current)pickRoadMapNode(id);}}
                  onKeyDown={event=>{if(event.key==="Enter"||event.key===" "){event.preventDefault();event.stopPropagation();pickRoadMapNode(id);}}}/>;})}
              </g>}
              {activeTool==="select"&&<g className="range-selection-layer" pointerEvents="none">{rangeSelection.flatMap(row=>{const line=alignments.find(line=>line.id===row.alignmentId);return line?deletionSections(line).filter(section=>row.sectionIndices.includes(section.index)).map(section=><path key={row.alignmentId+":"+section.index} data-selected-alignment={row.alignmentId} data-selected-section={section.index} className="selected-built-section" strokeWidth={8/viewport.scale} d={drawPath(section.points)}/>):[];})}{selectionBox&&(()=>{const b=selectionBounds(selectionBox);return <rect className="selection-box" x={b.left} y={b.top} width={b.right-b.left} height={b.bottom-b.top}/>;})()}</g>}
              {activeTool==="delete"&&enabled&&<g className="deletion-layer">
                {visibleDeletionSections.map(({alignmentId,sectionIndex,path})=><path key={alignmentId+":"+sectionIndex} data-delete-alignment={alignmentId} data-delete-section={sectionIndex} d={path} {...deleteHitProps({kind:"built-section",alignmentId,sectionIndex})}/>)}
                {[...parkedPaths.map(path=>({id:path.id,points:path.points,line:path.line,whole:!!path.reference||!!path.ofmPath||path.ofmPath||path.roadFollowing})),{id:activePathId,points:draftPoints,line:draftLine,whole:legacyRoadDraft}].map(path=><g key={path.id}>
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
          <MapCredits waterModelVisible={status==="ready"} openFreeMapVisible={ordinaryBusActive||!!world?.road_evidence_descriptors?.length||parkedPaths.some(p=>!!p.ofmPath)||!!graphBundle?.graph.dataset.id.startsWith("ofm-tiles/")||!!constructionRoad.snapshot?.tileSnapshots?.length||!!world?.road_snapshots?.some(s=>s.source_url.startsWith("https://tiles.openfreemap.org/planet/"))||!!world?.source_records?.some(s=>s.dataset_id.startsWith("ofm-tiles/"))||roadFollowing||!!draftEditedGeometry||parkedPaths.some(p=>!!p.editedGeometry)||!!world?.alignments?.length} derivedOsmVisible={!!world?.road_evidence_descriptors?.length||crossingCount>0||(roadSnapShown&&!!roadDrawingBundle)||alignments.some(line=>line.points.length>1&&world?.alignment_provenance?.some(origin=>origin.alignment_id===line.id))||parkedPaths.some(path=>(!!path.reference||!!path.ofmPath)&&path.line.length>1)||!!(referenceCopy.active&&referencePreviewPoints&&referencePreviewPoints.length>1)} background={background} reference={referenceVisible && referencePack ? referencePack.dataset : null} graphVisible={(!!graphBundle && graphVisible)||(automaticRailSelected&&automaticRail.view.chains.length>0)}/>
        </section>
        <aside className={panelCollapsed ? "side-panel collapsed" : "side-panel"}>
          <WorkspaceTabs value={mainTab} onChange={chooseMainTab}/>
          <div className="inspector-title"><span>RAILWAY WORKSPACE</span><button className="secondary" aria-label={panelCollapsed ? "操作パネルを開く" : "操作パネルを畳む"} onClick={() => setPanelCollapsed(!panelCollapsed)}>{panelCollapsed ? "+" : "−"}</button></div>
          <nav className="editor-tabs construction-tabs" role="tablist" aria-label="建設ツール" hidden={mainTab!=="construction"}>
            {constructionTools.map(([tool,label,shortcut],index)=><button key={tool} disabled={seamStageSending} role="tab" id={"tab-"+tool} aria-controls={"panel-"+tool} aria-selected={activeTool===tool} aria-keyshortcuts={shortcut} tabIndex={activeTool===tool?0:-1} onClick={()=>chooseConstructionTool(tool)} onKeyDown={event=>{if(seamMutationPending.current)return;if(!["ArrowLeft","ArrowRight","Home","End"].includes(event.key))return;event.preventDefault();const length=constructionTools.length;const next=constructionTools[event.key==="Home"?0:event.key==="End"?length-1:(index+(event.key==="ArrowLeft"?length-1:1))%length][0];chooseConstructionTool(next);document.getElementById("tab-"+next)?.focus();}}>{label}{shortcut&&<kbd aria-hidden="true">{shortcut}</kbd>}</button>)}
          </nav>
          {ordinaryBusActive&&world?.companies[0]&&<section id="panel-ordinary-bus" role="tabpanel" aria-labelledby="tab-ordinary-bus">
            {ordinaryBusStops.active?<OrdinaryBusStopsPanel state={ordinaryBusStops}/>:<OrdinaryBusPanel state={{...ordinaryBus,start:kind=>{
              if(cameraRef.current.zoom<16)changeCamera(cameraAt(cameraRef.current.latitude,cameraRef.current.longitude,16));
              ordinaryBus.start(kind);
            }}} route={ordinaryRoute} routes={ordinaryRoutes} multiple={ordinaryBusMultiple} roadSource={automaticRoad}
             onSelect={id=>setOrdinarySelection({epoch:worldEpoch,id})} blocked={ordinaryBusBlocked} enabled={ordinaryBusEnabled} editingLocked={installedBusEditingLocked}/>}
            {info?.installed_seam_bus_enabled===true&&!installedBusEditingLocked&&!ordinaryBus.draft&&!ordinaryBus.deleting&&!ordinaryBusStops.active&&
              <InstalledSeamBusPanel scope={world} enabled={ordinaryBusEnabled} commit={installSeamBus} acquire={acquireSeamBusRoad}/>}
            {!installedBusEditingLocked&&busStopsAvailable&&routeAllowsOrdinaryBusStops&&!ordinaryBusStops.active&&!ordinaryBus.draft&&!ordinaryBus.deleting&&
             <button type="button" disabled={!ordinaryBusEnabled||ordinaryBusBlocked} onClick={()=>void ordinaryBusStops.start()}>停留所を編集</button>}
            {ordinaryRoute&&info?.bus_operation_enabled===true&&<BusOperations key={`${worldEpoch}:${ordinaryRoute.id}`} routes={[{...ordinaryRoute,pattern:ordinaryRoutePattern}]} enabled={ordinaryBusEnabled&&!ordinaryBus.draft&&!ordinaryBusStops.active}/>}
          </section>}
          {roadTransportAvailable&&world?.companies[0]&&<section id="panel-road-transport" role="tabpanel" aria-labelledby="tab-road-transport" hidden={mainTab!=="construction"||activeTool!=="road-transport"}>
            <RoadTransportPanel key={worldEpoch} data={world.road_transport??{nodes:[],edges:[],bus_stops:[],bus_routes:[],tram_sections:[]}} companyId={world.companies[0].id} enabled={enabled&&!unapplied&&!roadMapPicking&&mainTab==="construction"&&activeTool==="road-transport"} tramEnabled={info?.road_tram_enabled===true} mapSelection={roadMapSelection} onPickOnMap={qaRoadNodes.length>=2?startRoadMapPick:undefined} playerMapSelection={roadPlayerMapSelection?.graph===world.road_transport&&roadPlayerMapSelection?.epoch===worldEpoch?roadPlayerMapSelection.selection:undefined} onPickPlayerOnMap={startPlayerRoadMapPick} run={runRoadTransport}/>
            {info?.bus_operation_enabled===true&&activeTool==="road-transport"&&mainTab==="construction"&&<BusOperations key={worldEpoch} routes={world.road_transport?.bus_routes.filter(route=>route.owner===world.companies[0].id)??[]} enabled={enabled&&!unapplied&&!roadMapPicking}/> }
          </section>}
          <section id="workspace-panel-finance" role="tabpanel" aria-labelledby="workspace-tab-finance" hidden={mainTab!=="finance"}><h3>財務</h3><p>未実装です。収支・運賃・経費の画面は今後追加します。</p></section>
          <section id="workspace-panel-city" role="tabpanel" aria-labelledby="workspace-tab-city" hidden={mainTab!=="city"}><h3>都市</h3>{info?.fixed_od_enabled===true&&world?<FixedOdPanel key={worldEpoch} stations={world.stations} services={world.services} busStops={info.bus_operation_enabled===true?world.road_transport?.bus_stops:undefined} busRoutes={info.bus_operation_enabled===true?world.road_transport?.bus_routes:undefined} busTransferRoad={info.bus_operation_enabled===true?world.road_transport:undefined} enabled={enabled&&mainTab==="city"} worldVersion={demandVersion}
            saveScenario={info.fixed_od_scenario_enabled===true?async(expectedToken:string,scenario:FixedOdScenario|null)=>{
              if(!enabled||unapplied||busyRef.current||mainTab!=="city")throw Error("別の操作を完了してから、需要条件を記録してください。");
              busyRef.current=true;setBusy(true);
              try{
                const result=await invoke<FixedOdScenarioView>("set_fixed_od_scenario",{expectedToken,scenario});
                setDirty(true);setOperationRevision(n=>n+1);setConfirmLoad(false);return result;
              }finally{busyRef.current=false;setBusy(false);}
            }:undefined}/>:<p>人口・需要・都市発展の画面は今後追加します。</p>}{info?.rail_journey_replay_enabled===true&&world&&<RailJourneyReplayPanel key={worldEpoch} stations={world.stations} services={world.services} enabled={enabled&&mainTab==="city"} worldVersion={demandVersion}/>}</section>
          <section id="workspace-panel-saves" role="tabpanel" aria-labelledby="workspace-tab-saves" hidden={mainTab!=="saves"}><h3>セーブデータ</h3>
            <SaveSlots onRename={renameSavedWorld} deletedSlots={deletedSaveSlots} onDelete={id=>moveSavedWorld(id,false)} onRestore={id=>moveSavedWorld(id,true)} onSave={save} saveDisabled={!enabled||saveBlocked} slots={saveSlots} current={activeSlotId} loading={slotsLoading} error={slotsError} busy={busy||status!=="ready"} canSave={!saveBlocked&&!companySetupRequired} onRefresh={()=>void refreshSaveSlots()} onLoad={id=>requestSlotAction({kind:"load",id})} onSaveAs={()=>{setSlotName(companyName);setSlotDialog("save-as");}} onNew={()=>{setSlotName("");setSlotDialog("new");}}/>
            {!activeSlotId&&<p>初回保存後に自動保存を開始します。</p>}
            <h4>オートセーブ</h4><p>手動保存とは別に、ワールドと建設計画を保存します。</p><p role="status">{!slotsReady?"保存先の確認が完了するまで自動保存は停止しています。":activeSlotId?autosave.message:"初回保存待ち"}</p>{autosave.permission==="failed"&&<button className="secondary" disabled={status!=="ready"||busy} onClick={autosave.retryStatus}>自動保存データを再確認</button>}<button className="secondary" disabled={!enabled||!autosave.available} onClick={()=>requestLoad("auto")}>自動保存から復元</button></section>
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

            <details className="reference-tools" hidden={!referencePanel&&!referenceCopy.active} open><summary>実在線路・道路に沿う</summary>{roadFollowing&&<div role="status" aria-label="道路データ状態">{automaticRoad.message}<button type="button" className="secondary" disabled={referenceCopy.active} onClick={automaticRoad.retry}>再取得</button></div>}{roadFollowing?<p role="status">{roadDrawing.preview?.kind==="error"?roadDrawing.preview.error:"道路上で始点と次の地点をクリック。Ctrl を押す間は自由に敷設"}</p>:automaticRailSelected?<AutomaticRailControls copy={{...referenceCopy,canUndo:referenceHistoryAvailable.undo,canRedo:referenceHistoryAvailable.redo,undo:()=>stepReferenceHistory(false),redoPick:()=>stepReferenceHistory(true),begin:()=>{if(!copyCanStart)return;if(roadFollowing){setFrozenRoadBundle(graphBundle);setFrozenRoadTiles((automaticRoad.snapshot?.tileSnapshots??[]).filter(t=>"tileLines" in t&&Array.isArray(t.tileLines)) as unknown as readonly DraftTileSnapshot[]);}newPath();referenceCopy.begin();changeMapMode("draw");setHoverPoint(null);}}} status={automaticRail.status} retry={automaticRail.retry} enabled={enabled} canStart={copyCanStart} zoom={camera.zoom} truncated={automaticRail.view.truncated}/>:<ReferenceCopyControls cancelDisabled={seamStageSending} extensionError={referenceExtensionError} extensionControls={manualReferenceCopyState.seamPlan?<>{([0,1] as const).map(index=><label key={index}>{index===0?"道路追従の始点接続":"道路追従の終点接続"}<select aria-label={index===0?"道路追従の始点接続":"道路追従の終点接続"} value={index===0?seamExtensionKey:seamDestinationKey} onChange={e=>{(index===0?setSeamExtensionKey:setSeamDestinationKey)(e.target.value);setSeamFocusKey(e.target.value);referenceCopy.setConfirmed(false);}}><option value="">接続しない</option>{seamConnectionCandidates(index).map(candidate=><option key={candidate.key} value={candidate.key}>{seamCandidateLabel(index,candidate)}</option>)}</select></label>)}<small>元の道路形状上に正確に一致する既設端点だけ選択できます。形状は移動しません。</small></>:<label>敷設方法<select aria-label="敷設方法" value={referenceExtensionKey} onChange={event=>{setReferenceExtensionKey(event.target.value);referenceCopy.setConfirmed(false);}}><option value="">独立した新しい線路</option>{referenceExtensionCandidates.map(candidate=><option key={candidate.key} value={candidate.key}>{referenceExtensionLabel(candidate)}</option>)}</select><small>起点から8px以内で、参照形状から正確に再現できる既存端点を選択できます。形状を平行移動せず参照区間の起点を補正します。既存の線路種別を引き継ぎます。複数候補から自動接続はしません。</small></label>} copy={{...referenceCopy,canUndo:referenceHistoryAvailable.undo,canRedo:referenceHistoryAvailable.redo,undo:()=>stepReferenceHistory(false),redoPick:()=>stepReferenceHistory(true),begin:()=>{if(!copyCanStart)return;if(roadFollowing){setFrozenRoadBundle(graphBundle);setFrozenRoadTiles((automaticRoad.snapshot?.tileSnapshots??[]).filter(t=>"tileLines" in t&&Array.isArray(t.tileLines)) as unknown as readonly DraftTileSnapshot[]);}newPath();referenceCopy.begin();changeMapMode("draw");setHoverPoint(null);}}} enabled={enabled} canStart={copyCanStart} hasBundle={!!graphBundle}/> }</details>

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
              {world&&<StationWalkLinks key={`${worldEpoch}:${selectedStation.id}`} station={selectedStation} stations={world.stations} groups={world.station_groups}
                links={world.station_walk_links??[]} enabled={enabled&&!unapplied&&!stationSettingsDirty} onEdit={editStationWalk}/>}
              <button className="secondary danger" disabled={!enabled || unapplied || !!stationReferences(selectedStation.id)} onClick={() => setPendingDelete({ kind: "station", id: selectedStation.id })}>{world?.station_groups?.some(g=>g.members.includes(selectedStation.id))?"選択ホームを撤去":"選択駅を削除"}</button>
              {stationReferences(selectedStation.id) && <p className="field-hint">削除不可: {stationReferences(selectedStation.id)}</p>}
            </div>}
            {pendingStation && <div className="inline-confirm" role="alert"><p>{platformDirty ? "駅名・ホーム長の下書きを破棄して別駅を選びますか。" : "駅名の下書きを破棄して別駅を選びますか。"}</p><button className="secondary" disabled={busy} onClick={() => setPendingStation(null)}>駅名編集を続ける</button><button disabled={busy} onClick={() => { const placement = stationPlacementById.get(pendingStation.id); setEditLength(String(placement?.length_m ?? 200));setEditWidth(String(placement?.width_m ?? 12));setMainTab("stations"); setActiveTool("station-management");setSelected(pendingStation.id); setEditStationName(stationsById.get(pendingStation.id)?.name ?? ""); setPendingStation(null); setPendingDelete(null); }}>{platformDirty ? "駅設定下書きを破棄して切替" : "駅名下書きを破棄して切替"}</button></div>}
          </section>
          <section className="route-editor" role="tabpanel" id="workspace-panel-routes" aria-labelledby="workspace-tab-routes" hidden={activeTool !== "routes"}>
            <h3>路線</h3>
            <RouteDirectory key={`routes-${worldEpoch}`} onRename={renameSavedRoute} onDirty={setRouteRenameDirty} world={world??previewWorld} enabled={enabled&&!candidateDirty&&!routeCreating&&!routeMapEdit} selected={routeId} onChoose={id=>{if(candidateDirty||routeMapEdit){chooseRoute(id);return;}chooseRoute(id===routeId?null:id);cancelRouteMapPick();setRouteCreating(id===null);if(id===null)setRouteSequenceKey(key=>key+1);}}/>
            {(routeCreating||routeMapEdit)&&<RouteCandidatePreview stationSearchDescription={stationSearchDescription} onLocateStation={locateSearchStation} candidateLength={candidateLength} onCandidateChange={setRouteSelectedCandidate} onSelectionChange={setRouteSelectedStations} key={routeMapEdit?.id??"new-route"} editRoute={routeMapEdit??undefined} defaultName={(()=>{let n=1;while(world?.routes.some(r=>r.name===`路線${n}`))n++;return `路線${n}`;})()} onCancelEdit={()=>{cancelRouteMapPick();setRouteMapEdit(null);setRouteSequenceKey(0);setRouteCreating(false);setCandidateDirty(false);}} startSequenceKey={routeSequenceKey} mapPickTarget={routeMapPick} onMapPick={(target,select)=>{if(!target){cancelRouteMapPick();return;}if(!enabled||mainTab!=="routes")return;routeMapSelect.current=select??null;setRouteMapPick(target);changeMapMode("move");}} resetKey={worldEpoch} onDirty={setCandidateDirty} stations={world?.stations ?? []} worldVersion={world} enabled={enabled} canApply={true} onSave={applyPlannedRoute} />}

            {routeId && <button type="button" className="secondary" disabled={!enabled||candidateDirty||routeRenameDirty||!!routeMapEdit} onClick={()=>{const route=world?.routes.find(r=>r.id===routeId);if(!route)return;cancelRouteMapPick();const path=world?.route_paths?.find(p=>p.route_id===route.id);const physical=path?resolveServicePath(route.start,path.segments.map((segment,i)=>({key:String(i+1),segment})),world!).map(o=>o.station_id):[];setRouteMapEdit({id:route.id,name:route.name,...(path?{path:{stations:physical,segments:path.segments},memberStations:path.member_stations,excludedStations:physical.filter(id=>!path.member_stations.includes(id))}:{})});setRouteSequenceKey(key=>key+1);}}>地図で路線を編集</button>}
            {selectedRoutePath && <section aria-label="保存済み路線の経路"><h4>{routeName}</h4><ol className="saved-route-stations" aria-label="所属駅">{selectedRoutePath.member_stations.map((id,index)=><li key={`${id}-${index}`}>{stationsById.get(id)?.name ?? "不明な駅"}</li>)}</ol></section>}
          </section>
          <section className="service-editor" role="tabpanel" id="workspace-panel-services" aria-labelledby="workspace-tab-services" hidden={activeTool !== "services"}>
            <h3>系統</h3>
            {status==="preview"&&<p className="field-hint">閲覧専用のサンプルです。種別・ダイヤ・列車運行は未実装です。</p>}
            <ServiceDirectory key={`services-${worldEpoch}`} showLiveOperation={mainTab==="services"} services={world?.services??[]} appearances={world?.service_appearances} routes={world?.routes} bindings={world?.service_route_bindings} selected={serviceId} enabled={enabled} onChoose={chooseService} onCreate={()=>{if(serviceDirty||pathServiceDirty||bindingDirty||serviceAppearanceDirty){chooseService(null);return;}chooseService(null);cancelRouteMapPick();setServiceCreating(true);setServiceSequenceKey(key=>key+1);}}/>
            {serviceCreating&&serviceId===null&&<ServiceMapCreator stationSearchDescription={stationSearchDescription} onLocateStation={locateSearchStation} onCandidateChange={setRouteSelectedCandidate} onSelectionChange={setRouteSelectedStations} key={`map-service-${pathDraftEpoch}`} world={world??previewWorld} stations={world?.stations??[]} enabled={enabled&&mainTab==="services"} defaultName={(()=>{let n=1;while(world?.services.some(row=>row.name===`系統${n}`))n++;return `系統${n}`;})()} onDirty={setPathServiceDirty} onSave={async draft=>{await createPathService(draft);setServiceCreating(false);cancelRouteMapPick();}} onCancel={()=>{setServiceCreating(false);setPathServiceDirty(false);cancelRouteMapPick();}} onMapPick={(target,select)=>{if(!target){cancelRouteMapPick();return;}if(!enabled||mainTab!=="services")return;routeMapSelect.current=select??null;setRouteMapPick(target);changeMapMode("move");}} mapPickTarget={routeMapPick} startSequenceKey={serviceSequenceKey}/>}
            {!serviceCreating&&serviceId===null&&<ServiceFromRoute key={`route-service-${worldEpoch}-${pathDraftEpoch}`} routes={world?.routes??[]} paths={world?.route_paths??[]} stations={world?.stations??[]} world={world??previewWorld} enabled={enabled&&!serviceDirty&&(!pathServiceDirty||routeServiceDirty)} onDirty={dirty=>{setRouteServiceDirty(dirty);setPathServiceDirty(dirty);}} onSave={createPathService}/>}
            <details><summary>区間・直通を指定して作成</summary>
            <ServicePathCreator key={pathDraftEpoch} routes={world?.routes??[]} routePaths={world?.route_paths??[]} stations={world?.stations??[]} world={world??previewWorld} enabled={enabled} canStart={!serviceDirty&&serviceId===null&&!serviceCreating&&!routeServiceDirty} onDirty={setPathServiceDirty} onSave={createPathService}/></details>

            {pendingServiceChoice && <div className="inline-confirm" role="alert"><p>未適用の系統下書きを破棄して切り替えますか。</p><button className="secondary" disabled={busy} onClick={() => setPendingServiceChoice(null)}>系統編集を続ける</button><button disabled={busy} onClick={() => resetService(world?.services.find((service) => service.id === pendingServiceChoice.id))}>系統下書きを破棄して切替</button></div>}
            {serviceId&&selectedServicePath&&world&&<div role="group" aria-label="系統の操作" key={`actions-${serviceId}-${worldEpoch}`}>
              <button type="button" className="secondary" disabled={!enabled||serviceActionHasEdits} onClick={()=>void duplicateSelectedService()}>系統を複製</button>
              <ServiceDelete key={`delete-${serviceId}-${worldEpoch}`} name={world.services.find(s=>s.id===serviceId)?.name??""} enabled={enabled} hasEdits={serviceActionHasEdits} onDelete={async()=>{
                if(!enabled||busyRef.current||serviceActionHasEdits)throw Error("編集中の変更を適用または取り消してください。");
                const source=world.services.find(s=>s.id===serviceId);if(!source)throw Error("系統が見つかりません。");
                const args={serviceId,expectedName:source.name,expectedPath:selectedServicePath};
                busyRef.current=true;setBusy(true);
                try{const snapshot=await invoke<World>("delete_service",args);setWorld(snapshot);setDirty(true);setConfirmLoad(false);setTrackDeleteRedo(null);setTrackDeleteUndo({kind:"built",world:snapshot,plans:planFingerprint,command:"delete_service",args});resetService();setNotice("系統を削除しました。Ctrl+Zで元に戻せます。保存でファイルに反映します。");}
                finally{busyRef.current=false;setBusy(false);}
              }}/>
            </div>}
            {serviceId&&selectedServicePath&&<ServiceOperations key={`${worldEpoch}-${serviceId}`} serviceId={serviceId} enabled={enabled&&!unapplied} standardEnabled={info?.standard_schedule_enabled===true}/>}
            {info?.crossing_planning_enabled===true&&mainTab==="services"&&<CrossingOperations key={worldEpoch} enabled={enabled&&!unapplied}/>}
            {serviceHint&&<p role="status">{serviceHint}</p>}
            {serviceId&&world&&<ServiceRouteBindingEditor key={`binding-${serviceId}-${worldEpoch}`} binding={world.service_route_bindings?.find(b=>b.service_id===serviceId)?.binding} routes={world.routes.filter(r=>r.owner===world.services.find(s=>s.id===serviceId)?.owner)} enabled={enabled&&!serviceDirty&&!pathServiceDirty&&!serviceAppearanceDirty} onDirty={setBindingDirty} onSave={async binding=>{if(!enabled||busyRef.current||serviceDirty||pathServiceDirty)throw Error("別の系統編集を適用または破棄してください。");busyRef.current=true;setBusy(true);try{const snapshot=await invoke<World>("set_service_route_binding",{serviceId,binding});const saved=snapshot.service_route_bindings?.find(b=>b.service_id===serviceId)?.binding;if(!sameRouteBinding(saved,binding))throw Error("関連付けの適用結果を確認できません。");setWorld(snapshot);setDirty(true);setConfirmLoad(false);setBindingDirty(false);setNotice("系統の関連付けを適用しました。保存でファイルに反映します。");}finally{busyRef.current=false;setBusy(false);}}}/>}
            {selectedServicePath&&serviceId&&<ServicePathChangeNotice serviceId={serviceId}/>}
            {selectedServicePath&&serviceId&&world&&<ServiceMapEditor stationSearchDescription={stationSearchDescription} onLocateStation={locateSearchStation} onCandidateChange={setRouteSelectedCandidate} onSelectionChange={setRouteSelectedStations} key={`map-edit-${serviceId}-${pathDraftEpoch}`} service={world.services.find(s=>s.id===serviceId)!} path={selectedServicePath} binding={world.service_route_bindings?.find(b=>b.service_id===serviceId)?.binding} world={world} stations={world.stations} candidateLength={candidateLength} enabled={enabled&&!serviceDirty&&!bindingDirty&&!serviceAppearanceDirty&&!pathServiceEditingDirty} onDirty={setExtensionServiceDirty} mapPickTarget={routeMapPick} onMapPick={(target,select)=>{if(!target){cancelRouteMapPick();return;}if(!enabled||mainTab!=="services")return;routeMapSelect.current=select??null;setRouteMapPick(target);changeMapMode("move");}} onSave={async draft=>{if(!enabled||busyRef.current||serviceDirty)throw Error("別の系統編集を適用または破棄してください");busyRef.current=true;setBusy(true);try{const snapshot=await invoke<World>("replace_service_path",{serviceId,expectedPath:selectedServicePath,...draft});setWorld(snapshot);setDirty(true);setPathServiceDirty(false);setConfirmLoad(false);resetService(snapshot.services.find(s=>s.id===serviceId));setNotice("経路と停車設定を更新しました。保存でファイルに反映します。");}finally{busyRef.current=false;setBusy(false);}}}/>}

            {selectedServicePath&&world&&<ServicePathCreator key={`replace-${serviceId}-${pathDraftEpoch}`} routes={world.routes} routePaths={world.route_paths??[]} stations={world.stations} world={world} enabled={enabled} canStart={!extensionServiceDirty&&!serviceDirty&&!bindingDirty&&!serviceAppearanceDirty} onDirty={setPathServiceDirty} replaceService={{id:serviceId!,name:world.services.find(s=>s.id===serviceId)!.name,path:selectedServicePath}} onSave={async draft=>{if(!enabled||busyRef.current||serviceDirty)throw Error("別の系統編集を適用または破棄してください");busyRef.current=true;setBusy(true);try{const snapshot=await invoke<World>("replace_service_path",{serviceId,expectedPath:selectedServicePath,...draft});setWorld(snapshot);setDirty(true);setPathServiceDirty(false);setConfirmLoad(false);resetService(snapshot.services.find(s=>s.id===serviceId));setNotice("経路と停車設定を更新しました。保存でファイルに反映します。");}finally{busyRef.current=false;setBusy(false);}}}/>}
            {serviceId&&selectedServicePath&&world&&<ServiceAppearanceControls key={`appearance-${serviceId}`} color={world.service_appearances?.find(row=>row.service_id===serviceId)?.color??null} serviceName={world.services.find(s=>s.id===serviceId)?.name??''} appearance={world.service_appearances?.find(row=>row.service_id===serviceId)} onSaveAppearance={async(color,shape,symbol)=>{if(!enabled||busyRef.current||serviceDirty||pathServiceDirty||bindingDirty)throw Error("別の系統編集を適用または取り消してください。");busyRef.current=true;setBusy(true);try{const snapshot=await invoke<World>("set_service_appearance",{serviceId,color,shape,symbol,expectedAppearance:world.service_appearances?.find(row=>row.service_id===serviceId)??null});setWorld(snapshot);setDirty(true);setServiceAppearanceDirty(false);setNotice("系統の色・記号・形を適用しました。保存でファイルに反映します。");}finally{busyRef.current=false;setBusy(false);}}} enabled={enabled&&!serviceDirty&&!pathServiceDirty&&!bindingDirty} onDirty={setServiceAppearanceDirty}/>}
            {selectedServicePath&&world&&<ServicePathEditor key={serviceId!} service={{id:serviceId!,name:world.services.find(s=>s.id===serviceId)!.name,start:serviceStart}} path={selectedServicePath} world={world} stations={world.stations} enabled={enabled&&!pathServiceDirty&&!bindingDirty&&!serviceAppearanceDirty} onDirty={setServiceDirty} onSaved={()=>setNotice("系統名と停車設定を適用しました。保存でファイルに反映します。")} onSave={async(name,stops)=>{if(!enabled||busyRef.current)throw Error("処理中です");busyRef.current=true;setBusy(true);try{const snapshot=await invoke<World>("update_service_path",{serviceId,name,stops});setWorld(snapshot);setDirty(true);setConfirmLoad(false);resetService(snapshot.services.find(s=>s.id===serviceId));}finally{busyRef.current=false;setBusy(false);}}}/> }
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
      {slotPicker&&<div className="company-setup-backdrop"><section className="company-setup save-slot-dialog" role="dialog" aria-modal="true" aria-label="ワールドを選択"><h2>セーブデータ</h2><SaveSlots onRename={renameSavedWorld} deletedSlots={deletedSaveSlots} onDelete={id=>moveSavedWorld(id,false)} onRestore={id=>moveSavedWorld(id,true)} slots={saveSlots} current={activeSlotId} loading={slotsLoading} error={slotsError} busy={busy} canSave={false} onRefresh={()=>void refreshSaveSlots()} onLoad={id=>requestSlotAction({kind:"load",id})} onSaveAs={()=>{}} onNew={()=>{setSlotPicker(false);setSlotName("");setSlotDialog("new");}}/><button disabled={busy} onClick={()=>setSlotPicker(false)}>閉じる</button>{error&&<p role="alert">{error}</p>}</section></div>}
      {slotDialog&&<div className="company-setup-backdrop"><section className="company-setup" role="dialog" aria-modal="true" aria-label={slotDialog==="new"?"新しいワールド":"別のセーブとして保存"}><h2>{slotDialog==="new"?"新しいワールド":"別のセーブとして保存"}</h2><form autoComplete="off" onSubmit={submitSlotName}><label>名前<input autoComplete="off" autoFocus maxLength={80} value={slotName} disabled={busy} onChange={event=>setSlotName(event.target.value)}/></label><button type="submit" disabled={busy||!slotName.trim()||confirmLoad}>{slotDialog==="new"?"作成":"保存"}</button><button type="button" disabled={busy} onClick={()=>{setSlotDialog(null);setSlotAction(null);setConfirmLoad(false);}}>キャンセル</button></form>{error&&<p role="alert">{error}</p>}</section></div>}
      {confirmLoad && <div className="banner load-confirm" style={{position:"fixed",zIndex:2000,left:"20%",right:"20%",top:"20%"}} role="alert"><p>{slotAction?.kind==="new"?"新しいワールドを作成すると、現在の未保存の変更と未適用の下書きは破棄されます。保存済みのワールドは変更しません。":"読込すると、現在の未保存の変更と未適用の下書きが破棄され、未着工計画を含め保存時点の状態に置き換わります。"}</p><button className="secondary" disabled={busy} onClick={() => {setConfirmLoad(false);setSlotAction(null);}}>戻る</button><button disabled={busy} onClick={()=>slotAction?applySlotAction():load()}>{slotAction?.kind==="new"?"変更を破棄して作成":"読込を続ける"}</button></div>}
      <footer className="app-footer"><p role="status" aria-live="polite">{busy ? (roadCheckPending ? "道路を確認中（最大25秒）。未取得の場合も通常建設を続行します。" : "ワールドを更新しています。") : notice || "線路 → 駅 → 運行経路の順に建設"}</p><span>{info?.fixed_od_enabled===true?"固定ODの需要試算は「都市」から":info?.operation_enabled===true?"旅客流動は未接続":"列車種別・ダイヤ・列車運行・旅客流動は未実装"}</span></footer>
    </main>
    </OperationProvider>
  );
}
