import { validateReferenceGraph, type GraphPoint, type ReferenceGraphPack } from "./referenceGraph";

/** Projection identity rather than a trusted cached point. Fractions are measured
 * in original way direction. Dataset revision is required to reject stale picks. */
export type ReferenceGraphPick = {
  datasetId: string; revision: string; edgeId: string; fraction: number;
};
export const REFERENCE_PATH_MAX_SEGMENTS = 256;
export type ReferenceGraphPath = {
  datasetId: string; revision: string; osmWayId: string; mode: "rail" | "road";
  direction: "forward" | "reverse"; edgeIds: string[]; points: GraphPoint[];
};
export type ReferencePathFailure = "dataset-mismatch" | "invalid-pick" | "way-or-mode-mismatch" | "inconsistent-way" | "segment-limit" | "missing-segment" | "disconnected-segment" | "empty-path";
export class ReferencePathError extends Error {
  constructor(public readonly code: ReferencePathFailure) { super(code); this.name = "ReferencePathError"; }
}

/** Read-only same-way extraction, not network routing or an Alignment creation API.
 * Input contract validation rejects duplicate source segments before extraction.
 * Source IDs are traceability hints, not proof of license or legal provenance. */
export function extractReferenceWayPath(input: ReferenceGraphPack, start: ReferenceGraphPick, end: ReferenceGraphPick, maxSegments = REFERENCE_PATH_MAX_SEGMENTS): ReferenceGraphPath {
  const pack = validateReferenceGraph(input);
  if (!Number.isInteger(maxSegments) || maxSegments < 1 || maxSegments > REFERENCE_PATH_MAX_SEGMENTS) throw new ReferencePathError("segment-limit");
  for (const pick of [start, end]) {
    if (pick.datasetId !== pack.dataset.id || pick.revision !== pack.dataset.revision) throw new ReferencePathError("dataset-mismatch");
    if (!Number.isFinite(pick.fraction) || pick.fraction < 0 || pick.fraction > 1) throw new ReferencePathError("invalid-pick");
  }
  const first = pack.edges.find(e => e.id === start.edgeId), last = pack.edges.find(e => e.id === end.edgeId);
  if (!first || !last) throw new ReferencePathError("invalid-pick");
  if (first.osmWayId !== last.osmWayId || first.mode !== last.mode) throw new ReferencePathError("way-or-mode-mismatch");
  const low = Math.min(first.segmentIndex, last.segmentIndex), high = Math.max(first.segmentIndex, last.segmentIndex);
  // Check budget before enumerating the requested range (indices may be huge).
  if (high - low >= maxSegments) throw new ReferencePathError("segment-limit");
  const wayEdges = pack.edges.filter(e => e.osmWayId === first.osmWayId && e.mode === first.mode);
  if (wayEdges.some(e => e.layer !== first.layer || e.level !== first.level || e.bridge !== first.bridge || e.tunnel !== first.tunnel)) throw new ReferencePathError("inconsistent-way");
  const byIndex = new Map(wayEdges.map(e => [e.segmentIndex, e]));
  const segments = [];
  for (let i = low; i <= high; i++) {
    const edge = byIndex.get(i);
    if (!edge) throw new ReferencePathError("missing-segment");
    if (segments.length && segments[segments.length - 1].to !== edge.from) throw new ReferencePathError("disconnected-segment");
    segments.push(edge);
  }
  const forward = first.segmentIndex < last.segmentIndex || (first.segmentIndex === last.segmentIndex && start.fraction <= end.fraction);
  const nodes = new Map(pack.nodes.map(n => [n.osmId, n]));
  const projected = (edge: typeof first, fraction: number): GraphPoint => {
    const a = nodes.get(edge.from)!, b = nodes.get(edge.to)!;
    // Preserve exact endpoint coordinates, including tiny floating-point details.
    if (fraction === 0) return { longitude: a.longitude, latitude: a.latitude };
    if (fraction === 1) return { longitude: b.longitude, latitude: b.latitude };
    return { longitude: a.longitude + fraction * (b.longitude - a.longitude), latitude: a.latitude + fraction * (b.latitude - a.latitude) };
  };
  const ordered = forward ? segments : [...segments].reverse();
  const points: GraphPoint[] = [projected(first, start.fraction)];
  for (const edge of ordered.slice(0, -1)) {
    const node = nodes.get(forward ? edge.to : edge.from)!;
    points.push({ longitude: node.longitude, latitude: node.latitude });
  }
  points.push(projected(last, end.fraction));
  // Remove only exact consecutive endpoint duplicates; never smooth or simplify.
  const unique = points.filter((p, i) => i === 0 || p.longitude !== points[i - 1].longitude || p.latitude !== points[i - 1].latitude);
  if (unique.length < 2) throw new ReferencePathError("empty-path");
  return { datasetId: pack.dataset.id, revision: pack.dataset.revision, osmWayId: first.osmWayId, mode: first.mode, direction: forward ? "forward" : "reverse", edgeIds: ordered.map(e => e.id), points: unique };
}
