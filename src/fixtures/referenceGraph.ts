import type { GraphEdge, ReferenceGraphPack } from "../referenceGraph";
/** Entirely synthetic coordinates/IDs. Not an OSM extract or licensed map dataset. */
export function syntheticReferenceGraph(): ReferenceGraphPack {
  const edge = (id: string, from: string, to: string, overrides: Partial<GraphEdge> = {}): GraphEdge => ({ id, osmWayId: id, segmentIndex: 0, from, to, mode: "rail", layer: null, level: null, bridge: null, tunnel: null, ...overrides });
  return {
    format: "rosenfu-reference-graph", version: 1,
    dataset: { id: "synthetic-test", revision: "1", source: "Hand-authored test fixture; not OSM data", license: "Repository code license", bounds: { west: 134.99, east: 135.01, south: 34.99, north: 35.01 } },
    nodes: [
      { osmId: "1", longitude: 134.999, latitude: 35 },
      { osmId: "2", longitude: 135, latitude: 35 },
      { osmId: "3", longitude: 135.001, latitude: 35 },
      { osmId: "4", longitude: 135, latitude: 35.001 },
      // Bridge crosses east rail geometrically; no shared node.
      { osmId: "5", longitude: 135.0005, latitude: 34.999 },
      { osmId: "6", longitude: 135.0005, latitude: 35.001 },
      { osmId: "7", longitude: 135, latitude: 34.999 },
      // Same coordinate as junction but different source node is NOT connected.
      { osmId: "8", longitude: 135, latitude: 35 },
    ],
    edges: [edge("101", "1", "2"), edge("102", "2", "3"), edge("103", "2", "4"), edge("104", "5", "6", { layer: 1, bridge: true }), edge("105", "7", "2", { mode: "road" }), edge("106", "8", "7")],
  };
}
