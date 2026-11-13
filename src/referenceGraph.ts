import { createReferenceSpatialIndex } from "./referenceSpatialIndex.ts";
/** Offline, regional reference geometry only. Never mutates World or infers a junction
 * from crossing/proximity. Version 1 stores original consecutive OSM node segments;
 * it is not a PBF importer, route finder, or finished construction snap system. */
export type GraphPoint = { longitude: number; latitude: number };
export type GraphBounds = { west: number; south: number; east: number; north: number };
export type GraphMode = "rail" | "road";
export type GraphNode = GraphPoint & { osmId: string };
export type GraphEdge = {
  id: string; osmWayId: string; segmentIndex: number; from: string; to: string;
  mode: GraphMode;
  // Null means unknown, not ground. OSM layer/level are not physical heights.
  layer: number | null; level: string | null; bridge: boolean | null; tunnel: boolean | null;
};
export type ReferenceGraphPack = {
  format: "rosenfu-reference-graph"; version: 1;
  dataset: { id: string; revision: string; source: string; license: string; bounds: GraphBounds };
  nodes: GraphNode[]; edges: GraphEdge[];
};
export const GRAPH_LIMITS = { bytes: 5 * 1024 * 1024, nodes: 20000, edges: 40000, results: 100, radiusM: 1000, minimumSegmentM: 0.01 } as const;
const LAT_LIMIT = 85.05112878;
function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw Error("Invalid graph object");
  return value as Record<string, unknown>;
}
function exact(value: unknown, keys: string[]): Record<string, unknown> {
  const r = record(value);
  if (Object.keys(r).length !== keys.length || keys.some(k => !Object.hasOwn(r, k))) throw Error("Unexpected or missing graph fields");
  return r;
}
function text(value: unknown, max = 200): string {
  if (typeof value !== "string" || !value.trim() || value.length > max || /[\p{Cc}\p{Cf}]/u.test(value)) throw Error("Invalid graph text");
  return value;
}
function osmId(value: unknown): string {
  if (typeof value !== "string" || !/^[1-9]\d{0,19}$/.test(value)) throw Error("OSM IDs must be positive integer strings");
  return value;
}
function number(value: unknown, min: number, max: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < min || value > max) throw Error("Invalid graph number");
  return value;
}
function nullableBoolean(value: unknown): boolean | null {
  if (value !== null && typeof value !== "boolean") throw Error("Invalid graph structure flag");
  return value;
}
function inside(p: GraphPoint, b: GraphBounds): boolean {
  return p.longitude >= b.west && p.longitude <= b.east && p.latitude >= b.south && p.latitude <= b.north;
}
/** Reject unsupported fields/versions instead of silently discarding future semantics. */
export function parseReferenceGraph(json: string): ReferenceGraphPack {
  if (new TextEncoder().encode(json).byteLength > GRAPH_LIMITS.bytes) throw Error("Reference graph exceeds 5 MiB");
  return validateReferenceGraph(JSON.parse(json));
}
/** Validate unknown object input before serialization can coerce NaN/Infinity to null. */
export function validateReferenceGraph(input: unknown): ReferenceGraphPack {
  const r = exact(input, ["format", "version", "dataset", "nodes", "edges"]);
  if (r.format !== "rosenfu-reference-graph" || r.version !== 1) throw Error("Unsupported reference graph version");
  const d = exact(r.dataset, ["id", "revision", "source", "license", "bounds"]);
  const b = exact(d.bounds, ["west", "south", "east", "north"]);
  const bounds = { west: number(b.west, -180, 180), east: number(b.east, -180, 180), south: number(b.south, -LAT_LIMIT, LAT_LIMIT), north: number(b.north, -LAT_LIMIT, LAT_LIMIT) };
  if (bounds.west >= bounds.east || bounds.south >= bounds.north || bounds.east - bounds.west > 180) throw Error("Invalid or unsupported regional bounds");
  const dataset = { id: text(d.id), revision: text(d.revision), source: text(d.source, 1000), license: text(d.license, 1000), bounds };
  if (!Array.isArray(r.nodes) || r.nodes.length > GRAPH_LIMITS.nodes || !Array.isArray(r.edges) || r.edges.length > GRAPH_LIMITS.edges) throw Error("Graph count limit exceeded");
  const byId = new Map<string, GraphNode>();
  const nodes = r.nodes.map(raw => {
    const n = exact(raw, ["osmId", "longitude", "latitude"]);
    const node = { osmId: osmId(n.osmId), longitude: number(n.longitude, -180, 180), latitude: number(n.latitude, -LAT_LIMIT, LAT_LIMIT) };
    if (!inside(node, bounds) || byId.has(node.osmId)) throw Error("Duplicate or out-of-region graph node");
    byId.set(node.osmId, node); return node;
  });
  const ids = new Set<string>(), segments = new Set<string>();
  const edges: GraphEdge[] = r.edges.map(raw => {
    const e = exact(raw, ["id", "osmWayId", "segmentIndex", "from", "to", "mode", "layer", "level", "bridge", "tunnel"]);
    const id = text(e.id), way = osmId(e.osmWayId), index = number(e.segmentIndex, 0, Number.MAX_SAFE_INTEGER);
    const from = osmId(e.from), to = osmId(e.to), a = byId.get(from), z = byId.get(to);
    if (!Number.isInteger(index) || !a || !z || from === to || (a.longitude === z.longitude && a.latitude === z.latitude)) throw Error("Invalid graph segment");
    const metres = scale({ longitude: a.longitude, latitude: (a.latitude + z.latitude) / 2 });
    if (Math.hypot((z.longitude - a.longitude) * metres.x, (z.latitude - a.latitude) * metres.y) < GRAPH_LIMITS.minimumSegmentM) throw Error("Graph segment is below the supported 1 cm minimum");
    if (e.mode !== "rail" && e.mode !== "road") throw Error("Unsupported graph mode");
    const key = `${way}/${index}/${e.mode}`;
    if (ids.has(id) || segments.has(key)) throw Error("Duplicate graph edge or source segment");
    ids.add(id); segments.add(key);
    const layer = e.layer === null ? null : number(e.layer, -100, 100);
    if (layer !== null && !Number.isInteger(layer)) throw Error("Invalid OSM layer");
    const bridge = nullableBoolean(e.bridge), tunnel = nullableBoolean(e.tunnel);
    if (bridge === true && tunnel === true) throw Error("Conflicting bridge/tunnel flags");
    return { id, osmWayId: way, segmentIndex: index, from, to, mode: e.mode, layer, level: e.level === null ? null : text(e.level, 100), bridge, tunnel };
  });
  const pack: ReferenceGraphPack = { format: "rosenfu-reference-graph", version: 1, dataset, nodes, edges };
  if (new TextEncoder().encode(JSON.stringify(pack)).byteLength > GRAPH_LIMITS.bytes) throw Error("Reference graph exceeds 5 MiB");
  return pack;
}
export type GraphProjection = { edge: GraphEdge; point: GraphPoint; fraction: number; distanceM: number };
export type GraphQuery = { point: GraphPoint; mode: GraphMode; radiusM: number; limit: number };
export type GraphQueryResult = {
  status: "outside" | "available" | "none";
  /** Query disk touches regional boundary; absence is not evidence of no real-world features. */
  coverage: "inside" | "boundary" | "outside";
  candidates: GraphProjection[]; total: number; truncated: boolean;
};
/** Async contract also permits a later SQLite/RTree adapter. */
export interface ReferenceGraphRepository {
  nearest(query: GraphQuery): Promise<GraphQueryResult>;
  connected(edgeId: string, atNodeId: string, limit: number): Promise<{ edges: GraphEdge[]; total: number; truncated: boolean }>;
}
function limit(value: number): number {
  number(value, 1, GRAPH_LIMITS.results);
  if (!Number.isInteger(value)) throw Error("Invalid result limit");
  return value;
}
/** Small-region tangent-plane approximation in meters, not a routing/geodesic engine. */
function scale(p: GraphPoint) { return { x: 6371000 * Math.PI / 180 * Math.cos(p.latitude * Math.PI / 180), y: 6371000 * Math.PI / 180 }; }
function project(p: GraphPoint, a: GraphPoint, b: GraphPoint): Omit<GraphProjection, "edge"> {
  const s = scale(p), ax = (a.longitude - p.longitude) * s.x, ay = (a.latitude - p.latitude) * s.y;
  const dx = (b.longitude - a.longitude) * s.x, dy = (b.latitude - a.latitude) * s.y;
  const length = Math.hypot(dx, dy);
  if (!Number.isFinite(length) || length <= 0) throw Error("Unprojectable graph segment");
  const fraction = Math.max(0, Math.min(1, -(ax * (dx / length) + ay * (dy / length)) / length));
  return { fraction, point: { longitude: a.longitude + fraction * (b.longitude - a.longitude), latitude: a.latitude + fraction * (b.latitude - a.latitude) }, distanceM: Math.hypot(ax + fraction * dx, ay + fraction * dy) };
}
export function createReferenceGraphRepository(input: ReferenceGraphPack): ReferenceGraphRepository {
  // Validate and detach caller-owned state. Return detached records as well.
  const pack = validateReferenceGraph(input);
  const nodes = new Map(pack.nodes.map(n => [n.osmId, n]));
  const edges = new Map(pack.edges.map(e => [e.id, e]));
  const edgeBounds = pack.edges.map(e => {
    const a=nodes.get(e.from)!, b=nodes.get(e.to)!;
    return {west:Math.min(a.longitude,b.longitude),east:Math.max(a.longitude,b.longitude),south:Math.min(a.latitude,b.latitude),north:Math.max(a.latitude,b.latitude)};
  });
  const spatial = createReferenceSpatialIndex(edgeBounds);
  const incident = new Map<string, GraphEdge[]>();
  for(const edge of pack.edges) for(const id of [edge.from,edge.to]) {
    const list=incident.get(id) ?? []; list.push(edge); incident.set(id,list);
  }
  const order = (a: GraphEdge, b: GraphEdge) => a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
  return {
    async nearest(q) {
      limit(q.limit); number(q.radiusM, 0, GRAPH_LIMITS.radiusM);
      number(q.point.longitude, -180, 180); number(q.point.latitude, -LAT_LIMIT, LAT_LIMIT);
      if (q.mode !== "rail" && q.mode !== "road") throw Error("Unsupported query mode");
      const b = pack.dataset.bounds;
      if (!inside(q.point, b)) return { status: "outside", coverage: "outside", candidates: [], total: 0, truncated: false };
      const s = scale(q.point);
      const margin = Math.min((q.point.longitude - b.west) * s.x, (b.east - q.point.longitude) * s.x, (q.point.latitude - b.south) * s.y, (b.north - q.point.latitude) * s.y);
      // Padding protects exact-radius inclusion from degree/metre roundoff;
      // the original projection remains the authoritative narrow phase.
      const dx=q.radiusM/s.x+1e-10, dy=q.radiusM/s.y+1e-10;
      const nearby=spatial.query({west:q.point.longitude-dx,east:q.point.longitude+dx,south:q.point.latitude-dy,north:q.point.latitude+dy});
      const candidates = nearby.map(i=>pack.edges[i]).filter(e => e.mode === q.mode).map(edge => ({ edge: { ...edge }, ...project(q.point, nodes.get(edge.from)!, nodes.get(edge.to)!) })).filter(c => c.distanceM <= q.radiusM).sort((a, z) => a.distanceM - z.distanceM || order(a.edge, z.edge));
      return { status: candidates.length ? "available" : "none", coverage: margin <= q.radiusM ? "boundary" : "inside", candidates: candidates.slice(0, q.limit), total: candidates.length, truncated: candidates.length > q.limit };
    },
    async connected(edgeId, atNodeId, count) {
      limit(count);
      const edge = edges.get(edgeId);
      if (!edge || (edge.from !== atNodeId && edge.to !== atNodeId)) throw Error("Unknown edge or nonincident node");
      // Explicit source-node identity is required. Mode and vertical metadata remain
      // conservative: mismatched metadata blocks connectivity. Equal unknowns may
      // connect at an explicit shared source node, but never imply ground level.
      const matches = (incident.get(atNodeId) ?? []).filter(e => e.id !== edgeId && (e.from === atNodeId || e.to === atNodeId) && e.mode === edge.mode && e.layer === edge.layer && e.level === edge.level && e.bridge === edge.bridge && e.tunnel === edge.tunnel).sort(order);
      return { edges: matches.slice(0, count).map(e => ({ ...e })), total: matches.length, truncated: matches.length > count };
    },
  };
}
