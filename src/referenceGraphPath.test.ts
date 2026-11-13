import { describe, expect, it } from "vitest";
import { syntheticReferenceGraph } from "./fixtures/referenceGraph";
import { extractReferenceWayPath, type ReferenceGraphPick } from "./referenceGraphPath";
function fixture() {
  const pack = syntheticReferenceGraph();
  pack.edges[1].osmWayId = "101"; pack.edges[1].segmentIndex = 1;
  // Bent same-way polyline, plus an unrelated branch at node 2.
  pack.nodes[2].latitude = 35.0005;
  return pack;
}
const pick = (edgeId: string, fraction: number): ReferenceGraphPick => ({ datasetId: "synthetic-test", revision: "1", edgeId, fraction });
describe("read-only same-way source extraction", () => {
  it("preserves intermediate original vertices without smoothing or crossing branches", () => {
    const pack = fixture(), before = JSON.stringify(pack);
    const result = extractReferenceWayPath(pack, pick("101", 0.5), pick("102", 0.5));
    expect(result).toMatchObject({ edgeIds: ["101", "102"], direction: "forward", osmWayId: "101", mode: "rail" });
    expect(result.points).toEqual([{ longitude: 134.9995, latitude: 35 }, { longitude: 135, latitude: 35 }, { longitude: 135.0005, latitude: 35.00025 }]);
    result.points[1].longitude = 0;
    expect(JSON.stringify(pack)).toBe(before);
  });
  it("reverses the same raw path and edge order", () => {
    const pack = fixture(), a = pick("101", 0.25), b = pick("102", 0.75);
    const forward = extractReferenceWayPath(pack, a, b), reverse = extractReferenceWayPath(pack, b, a);
    expect(reverse.direction).toBe("reverse"); expect(reverse.points).toEqual([...forward.points].reverse());
    expect(reverse.edgeIds).toEqual(["102", "101"]);
  });
  it("supports a subsegment in both directions and retains exact endpoints", () => {
    const pack = fixture();
    const result = extractReferenceWayPath(pack, pick("102", 1), pick("102", 0));
    expect(result.points).toEqual([ { longitude: pack.nodes[2].longitude, latitude: pack.nodes[2].latitude }, { longitude: 135, latitude: 35 } ]);
    expect(result.direction).toBe("reverse");
  });
  it("deduplicates exact consecutive boundary points and rejects zero-length selections", () => {
    const pack = fixture();
    expect(extractReferenceWayPath(pack, pick("101", 1), pick("102", 1)).points).toHaveLength(2);
    expect(() => extractReferenceWayPath(pack, pick("101", 1), pick("102", 0))).toThrow("empty-path");
    expect(() => extractReferenceWayPath(pack, pick("101", 0.5), pick("101", 0.5))).toThrow("empty-path");
  });
  it("rejects stale or foreign dataset identities", () => {
    const pack = fixture();
    for (const extra of [{ datasetId: "other" }, { revision: "2" }]) expect(() => extractReferenceWayPath(pack, { ...pick("101", 0), ...extra }, pick("102", 1))).toThrow("dataset-mismatch");
  });
  it("rejects invalid picks and does not substitute nearby edges", () => {
    for (const fraction of [-1, 2, NaN, Infinity]) expect(() => extractReferenceWayPath(fixture(), pick("101", fraction), pick("102", 1))).toThrow("invalid-pick");
    expect(() => extractReferenceWayPath(fixture(), pick("missing", 0), pick("102", 1))).toThrow("invalid-pick");
  });
  it("rejects other ways and modes even at a shared node", () => {
    expect(() => extractReferenceWayPath(fixture(), pick("101", 0), pick("103", 1))).toThrow("way-or-mode-mismatch");
    const pack = fixture(); pack.edges[1].mode = "road";
    expect(() => extractReferenceWayPath(pack, pick("101", 0), pick("102", 1))).toThrow("way-or-mode-mismatch");
  });
  it("rejects missing source indices and coincident-but-distinct source nodes", () => {
    const pack = fixture(); pack.edges[1].segmentIndex = 2;
    expect(() => extractReferenceWayPath(pack, pick("101", 0), pick("102", 1))).toThrow("missing-segment");
    pack.edges[1].segmentIndex = 1; pack.edges[1].from = "8";
    expect(() => extractReferenceWayPath(pack, pick("101", 0), pick("102", 1))).toThrow("disconnected-segment");
  });
  it("rejects duplicate source segments rather than choosing one", () => {
    const pack = fixture(); pack.edges.push({ ...pack.edges[1], id: "duplicate" });
    expect(() => extractReferenceWayPath(pack, pick("101", 0), pick("102", 1))).toThrow("Duplicate");
  });
  it.each([
    { layer: 0 }, { level: "0" }, { bridge: true }, { tunnel: true },
  ])("rejects contradictory same-way vertical metadata: %j", metadata => {
    const pack = fixture(); Object.assign(pack.edges[1], metadata);
    expect(() => extractReferenceWayPath(pack, pick("101", 0), pick("102", 1))).toThrow("inconsistent-way");
    // A short selection cannot conceal conflicting metadata elsewhere on its way.
    expect(() => extractReferenceWayPath(pack, pick("101", 0), pick("101", 1))).toThrow("inconsistent-way");
  });
  it("enforces the serialized 5 MiB limit on typed object input", () => {
    const pack = fixture(), original = pack.edges[0];
    pack.edges = Array.from({ length: 20000 }, (_, i) => ({ ...original, id: `${i}`.padStart(200, "x"), osmWayId: String(i + 1) }));
    expect(() => extractReferenceWayPath(pack, pick(pack.edges[0].id, 0), pick(pack.edges[0].id, 1))).toThrow("5 MiB");
  });
  it("retains a closed loop in source order instead of shortening it", () => {
    const pack = fixture();
    pack.edges = [pack.edges[0], pack.edges[1], { ...pack.edges[1], id: "loop", segmentIndex: 2, from: "3", to: "1" }];
    const result = extractReferenceWayPath(pack, pick("101", 0), pick("loop", 1));
    expect(result.points).toHaveLength(4);
    expect(result.points[0]).toEqual(result.points[3]);
    expect(result.edgeIds).toEqual(["101", "102", "loop"]);
  });
  it("accepts exactly 256 segments and rejects 257", () => {
    const pack = fixture(), original = pack.edges[0];
    pack.nodes = Array.from({ length: 258 }, (_, i) => ({ osmId: String(i + 1), longitude: 135 + i * 0.00001, latitude: 35 }));
    pack.edges = Array.from({ length: 257 }, (_, i) => ({ ...original, id: String(i), segmentIndex: i, from: String(i + 1), to: String(i + 2) }));
    expect(extractReferenceWayPath(pack, pick("0", 0), pick("255", 1)).points).toHaveLength(257);
    expect(() => extractReferenceWayPath(pack, pick("0", 0), pick("256", 1))).toThrow("segment-limit");
  });
  it("rejects selections whose projected endpoints round to the same coordinate", () => {
    expect(() => extractReferenceWayPath(fixture(), pick("101", 0), pick("101", 1e-12))).toThrow("empty-path");
  });
  it("rejects budgets/large ranges before expanding them", () => {
    const pack = fixture();
    for (const budget of [0, 1, 257, 1.5, Infinity]) expect(() => extractReferenceWayPath(pack, pick("101", 0), pick("102", 1), budget)).toThrow("segment-limit");
    pack.edges[1].segmentIndex = Number.MAX_SAFE_INTEGER;
    expect(() => extractReferenceWayPath(pack, pick("101", 0), pick("102", 1))).toThrow("segment-limit");
  });
});
