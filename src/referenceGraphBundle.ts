import { GRAPH_LIMITS, parseReferenceGraph, type ReferenceGraphPack } from "./referenceGraph";

export type RoadWayMetadata = { highway: string | null; access: string | null; motorVehicle: string | null; structureUnsupported: boolean };
export type ReferenceGraphBundle = {
  graph: ReferenceGraphPack;
  /** Validated source tags for geometry filtering, not construction permission. Absent in old test/adaptor bundles: fail closed. */
  roadWays?: Record<string, RoadWayMetadata>;
  /** Verified raw-file digests for snapshot identity, not proof of authenticity. */
  graphSha256: string;
  sourceSha256: string;
  /** Untrusted display text, never a URL to fetch/open or HTML to execute. */
  source: { source: string; sourceDate: string | null; license: string; attribution: string };
};
export const GRAPH_BUNDLE_LIMITS = { "graph.json": GRAPH_LIMITS.bytes, "source.json": 20 * 1024 * 1024, "complete.json": 4096 } as const;
const fail = (): never => { throw Error("参照地図バンドルの形式または整合性が不正です。"); };
function record(v: unknown): Record<string, unknown> {
  if (!v || typeof v !== "object" || Array.isArray(v)) return fail();
  return v as Record<string, unknown>;
}
function exact(v: unknown, keys: string[]): Record<string, unknown> {
  const r = record(v);
  if (Object.keys(r).length !== keys.length || keys.some(k => !Object.hasOwn(r, k))) fail();
  return r;
}
function text(v: unknown, max = 1000): string {
  if (typeof v !== "string" || !v.trim() || v.length > max || /[\p{Cc}\p{Cf}]/u.test(v)) return fail();
  return v;
}
const hashText = (v: unknown) => typeof v === "string" && /^[a-f0-9]{64}$/.test(v) ? v : fail();
const osmId = (v: unknown) => typeof v === "string" && /^[1-9]\d{0,19}$/.test(v) ? v : fail();
async function sha256(bytes: ArrayBuffer): Promise<string> {
  if (!globalThis.crypto?.subtle) throw Error("SHA-256検証を利用できない環境では読み込めません。");
  return Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)), n => n.toString(16).padStart(2, "0")).join("");
}
function sourceMetadata(value: unknown, graph: ReferenceGraphPack, graphHash: string): {source:ReferenceGraphBundle["source"];roadWays:Record<string,RoadWayMetadata>} {
  const s = exact(value, ["format", "version", "converter", "inputSha256", "graphSha256", "source", "sourceDate", "attribution", "license", "licenseUrl", "requestedBounds", "sourceBounds", "geometryOnly", "directionsEnforced", "restrictionsEnforced", "relationsPreserved", "coverageGuaranteed", "boundaryPolicy", "originalInputRetention", "classification", "counts", "ways", "nodes"]);
  if (s.format !== "rosenfu-osm-graph-source" || s.version !== 1 || s.converter !== "osm-xml-v1" || hashText(s.graphSha256) !== graphHash) fail();
  hashText(s.inputSha256);
  if (s.geometryOnly !== true || [s.directionsEnforced, s.restrictionsEnforced, s.relationsPreserved, s.coverageGuaranteed].some(v => v !== false)) fail();
  if (s.license !== "ODbL-1.0" || s.license !== graph.dataset.license || s.licenseUrl !== "https://opendatacommons.org/licenses/odbl/1-0/" || s.attribution !== "© OpenStreetMap contributors") fail();
  const source = text(s.source);
  if (source !== graph.dataset.source || !/^https?:\/\//.test(source)) fail();
  const sourceDate = s.sourceDate === null ? null : text(s.sourceDate, GRAPH_BUNDLE_LIMITS["source.json"]);
  if (sourceDate !== null && (!/^\d{4}-\d{2}-\d{2}(?:T[^\s]+)?$/.test(sourceDate) || !Number.isFinite(Date.parse(sourceDate)))) fail();
  const b = exact(s.requestedBounds, ["west", "south", "east", "north"]);
  for (const key of ["west", "south", "east", "north"] as const) if (b[key] !== graph.dataset.bounds[key]) fail();
  if (s.sourceBounds !== null && Object.values(record(s.sourceBounds)).some(v => typeof v !== "string" || v.length > 4096)) fail();
  if (s.boundaryPolicy !== "original segments with both endpoints inside; outside-to-outside crossings omitted" || s.originalInputRetention !== "Retain the original XML; relation contents are not preserved here.") fail();
  const c = exact(s.classification, ["rail", "road", "areaYesExcluded"]);
  if (c.areaYesExcluded !== true || JSON.stringify(c.rail) !== JSON.stringify(["rail", "subway", "light_rail", "tram", "narrow_gauge"]) || JSON.stringify(c.road) !== JSON.stringify(["motorway", "motorway_link", "trunk", "trunk_link", "primary", "primary_link", "secondary", "secondary_link", "tertiary", "tertiary_link", "unclassified", "residential", "living_street", "service"])) fail();
  const counts = exact(s.counts, ["objects", "refs", "tags", "relationsIgnored", "excludedWays", "boundarySegments"]);
  for (const v of Object.values(counts)) if (typeof v !== "number" || !Number.isSafeInteger(v) || v < 0 || v > 500000) fail();
  const tags = (v: unknown) => { const t = record(v); if (Object.keys(t).length > 256 || Object.entries(t).some(([k, v]) => !k || k.length > 4096 || typeof v !== "string" || v.length > 4096)) fail(); };
  if (!Array.isArray(s.nodes) || s.nodes.length !== graph.nodes.length || !Array.isArray(s.ways) || s.ways.length > 40000) fail();
  const nodes = new Set(graph.nodes.map(n => n.osmId));
  for (const raw of s.nodes as unknown[]) { const n = exact(raw, ["id", "tags"]); if (!nodes.delete(osmId(n.id))) fail(); tags(n.tags); }
  const ways = new Map<string, string[]>(); const roadWays:Record<string,RoadWayMetadata>=Object.create(null); let refs = 0;
  for (const raw of s.ways as unknown[]) {
    const w = exact(raw, ["id", "nd", "tags"]), id = osmId(w.id);
    if (ways.has(id) || !Array.isArray(w.nd) || w.nd.length < 2 || (refs += w.nd.length) > 500000) fail();
    const nd = (w.nd as unknown[]).map(osmId); tags(w.tags); ways.set(id, nd);
    const t=record(w.tags);
    const tag=(key:string)=>typeof t[key]==="string"?t[key] as string:null;
    const nonzero=(key:string)=>t[key]!==undefined && (!/^[+-]?0(?:\.0+)?$/.test(String(t[key])));
    roadWays[id]={highway:tag("highway"),access:tag("access"),motorVehicle:tag("motor_vehicle"),structureUnsupported:(t.bridge!==undefined&&t.bridge!=="no")||(t.tunnel!==undefined&&t.tunnel!=="no")||nonzero("layer")||nonzero("level")};
  }
  const used = new Set<string>();
  for (const e of graph.edges) { const nd = ways.get(e.osmWayId); if (!nd || nd[e.segmentIndex] !== e.from || nd[e.segmentIndex + 1] !== e.to) fail(); used.add(e.osmWayId); }
  if (!graph.edges.length || used.size !== ways.size) fail();
  return { source:{ source, sourceDate, license: "ODbL-1.0", attribution: "© OpenStreetMap contributors" },roadWays };
}
/** Local-only integrity check. Not authenticity, geographic accuracy, completeness or routing validation. */
export async function readReferenceGraphBundle(files: File[]): Promise<ReferenceGraphBundle> {
  if (files.length !== 3) throw Error("graph.json・source.json・complete.json の3ファイルを選択してください。");
  const selected = new Map<string, File>();
  for (const file of files) {
    if (!Object.hasOwn(GRAPH_BUNDLE_LIMITS, file.name) || selected.has(file.name)) fail();
    const limit = GRAPH_BUNDLE_LIMITS[file.name as keyof typeof GRAPH_BUNDLE_LIMITS];
    if (!Number.isSafeInteger(file.size) || file.size <= 0 || file.size > limit) throw Error("参照地図ファイルのサイズ上限を超えています。");
    selected.set(file.name, file);
  }
  // All size/name checks precede every read. Recheck actual bytes before decoding/parsing.
  const read = async (name: keyof typeof GRAPH_BUNDLE_LIMITS) => {
    const file = selected.get(name)!; const bytes = await file.arrayBuffer();
    if (bytes.byteLength !== file.size || bytes.byteLength > GRAPH_BUNDLE_LIMITS[name]) fail();
    return bytes;
  };
  const manifest = exact(JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(await read("complete.json"))), ["format", "version", "graphSha256", "sourceSha256"]);
  if (manifest.format !== "rosenfu-osm-graph-complete" || manifest.version !== 1) fail();
  const gh = hashText(manifest.graphSha256), sh = hashText(manifest.sourceSha256);
  const graphBytes = await read("graph.json");
  if (await sha256(graphBytes) !== gh) fail();
  const graph = parseReferenceGraph(new TextDecoder("utf-8", { fatal: true }).decode(graphBytes));
  const sourceBytes = await read("source.json");
  if (await sha256(sourceBytes) !== sh) fail();
  const metadata = sourceMetadata(JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(sourceBytes)), graph, gh);
  return { graph, ...metadata, graphSha256: gh, sourceSha256: sh };
}
