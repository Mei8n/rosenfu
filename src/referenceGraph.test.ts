import { describe, expect, it } from "vitest";
import { createReferenceGraphRepository, GRAPH_LIMITS, parseReferenceGraph } from "./referenceGraph";
import { syntheticReferenceGraph } from "./fixtures/referenceGraph";
const parse = (value: unknown) => parseReferenceGraph(JSON.stringify(value));
const query = { point: { longitude: 135.0005, latitude: 35.0001 }, mode: "rail" as const, radiusM: 50, limit: 10 };
describe("synthetic offline reference graph", () => {
  it("roundtrips raw coordinates, string source IDs and unknown elevation", () => {
    const pack = syntheticReferenceGraph(); pack.nodes[0].osmId = "99999999999999999999"; pack.edges[0].from = pack.nodes[0].osmId;
    expect(parse(pack)).toEqual(pack); expect(parse(pack).edges[0].layer).toBeNull();
  });
  it("connects explicit shared-node branches, but not road transfers or coincident nodes", async () => {
    const repo = createReferenceGraphRepository(syntheticReferenceGraph());
    expect((await repo.connected("101", "2", 10)).edges.map(e => e.id)).toEqual(["102", "103"]);
    expect((await repo.connected("104", "5", 10)).edges).toEqual([]);
    expect((await repo.connected("105", "2", 10)).edges).toEqual([]);
  });
  it("does not join incompatible vertical metadata even at a shared node", async () => {
    const pack = syntheticReferenceGraph(); pack.edges[1].layer = 0;
    expect((await createReferenceGraphRepository(pack).connected("101", "2", 10)).edges.map(e => e.id)).toEqual(["103"]);
  });
  it("projects to original segments and returns bounded, deterministic candidates", async () => {
    const repo = createReferenceGraphRepository(syntheticReferenceGraph());
    const result = await repo.nearest(query);
    expect(result.candidates[0].edge.id).toBe("104");
    const ground = result.candidates.find(c => c.edge.id === "102")!;
    expect(ground.point.longitude).toBeCloseTo(135.0005, 10); expect(ground.point.latitude).toBe(35);
    expect(ground.fraction).toBeCloseTo(0.5); expect(ground.distanceM).toBeCloseTo(11.11949, 3);
    const bounded = await repo.nearest({ ...query, limit: 1 });
    expect(bounded.candidates).toHaveLength(1); expect(bounded.truncated).toBe(true); expect(bounded.total).toBe(result.total);
    const junction = await repo.connected("101", "2", 1);
    expect(junction).toMatchObject({ total: 2, truncated: true });
  });
  it("clamps projection to endpoints and separates query modes", async () => {
    const repo = createReferenceGraphRepository(syntheticReferenceGraph());
    const result = await repo.nearest({ ...query, point: { longitude: 134.9989, latitude: 35 }, radiusM: 20 });
    expect(result.candidates[0].fraction).toBe(0);
    const roads = await repo.nearest({ ...query, point: { longitude: 135, latitude: 35 }, mode: "road" });
    expect(roads.candidates.map(c => c.edge.id)).toEqual(["105"]);
  });
  it("distinguishes outside/partial coverage from no candidates", async () => {
    const repo = createReferenceGraphRepository(syntheticReferenceGraph());
    expect(await repo.nearest({ ...query, point: { longitude: 136, latitude: 35 } })).toMatchObject({ status: "outside", coverage: "outside", candidates: [] });
    expect(await repo.nearest({ ...query, point: { longitude: 135.01, latitude: 35 } })).toMatchObject({ status: "none", coverage: "boundary" });
    expect(await repo.nearest({ ...query, point: { longitude: 135.005, latitude: 35 } })).toMatchObject({ status: "none", coverage: "inside" });
  });
  it("detaches source and result mutation", async () => {
    const pack = syntheticReferenceGraph(), repo = createReferenceGraphRepository(pack);
    pack.nodes[0].longitude = 0; pack.edges[0].mode = "road";
    const a = await repo.connected("101", "2", 10); a.edges[0].id = "changed";
    expect((await repo.connected("101", "2", 10)).edges[0].id).toBe("102");
    const q = await repo.nearest(query); q.candidates[0].edge.id = "changed";
    expect((await repo.nearest(query)).candidates[0].edge.id).toBe("104");
  });
  it.each([
    (p: any) => { p.version = 2; }, (p: any) => { p.extra = true; },
    (p: any) => { p.nodes[0].osmId = 1; }, (p: any) => { p.nodes.push(p.nodes[0]); },
    (p: any) => { p.nodes[0].longitude = 136; }, (p: any) => { p.edges[0].to = "999"; },
    (p: any) => { p.edges[0].to = p.edges[0].from; }, (p: any) => { p.edges.push(p.edges[0]); },
    (p: any) => { p.edges[1].osmWayId = "101"; }, (p: any) => { p.edges[0].mode = "tram"; },
    (p: any) => { p.edges[0].layer = 0.5; }, (p: any) => { delete p.edges[0].layer; },
    (p: any) => { p.edges[0].bridge = true; p.edges[0].tunnel = true; },
    (p: any) => { p.dataset.bounds.west = 180; }, (p: any) => { p.dataset.license = ""; },
    (p: any) => { p.edges[0].segmentIndex = -1; },
  ])("rejects malformed packs without repair", mutate => { const p = syntheticReferenceGraph(); mutate(p); expect(() => parse(p)).toThrow(); });
  it("rejects sub-centimetre segments rather than silently losing NaN projections", () => {
    const pack = syntheticReferenceGraph();
    pack.dataset.bounds = { west: -1, east: 1, south: -1, north: 1 };
    pack.nodes = [{ osmId: "1", longitude: 0, latitude: 0 }, { osmId: "2", longitude: 1e-200, latitude: 0 }];
    pack.edges = [pack.edges[0]];
    expect(() => parse(pack)).toThrow("1 cm");
    expect(() => createReferenceGraphRepository(pack)).toThrow("1 cm");
  });
  it.each([NaN, Infinity, -Infinity])("rejects nonfinite object metadata before serialization: %s", value => {
    const pack = syntheticReferenceGraph(); pack.edges[0].layer = value;
    expect(() => createReferenceGraphRepository(pack)).toThrow("Invalid graph number");
  });
  it("rejects excessive input and invalid query budgets", async () => {
    expect(() => parseReferenceGraph(" ".repeat(GRAPH_LIMITS.bytes + 1))).toThrow();
    const pack = syntheticReferenceGraph(); pack.nodes = Array(GRAPH_LIMITS.nodes + 1).fill(pack.nodes[0]); expect(() => parse(pack)).toThrow();
    const repo = createReferenceGraphRepository(syntheticReferenceGraph());
    for (const overrides of [{ limit: 0 }, { limit: 101 }, { limit: 1.5 }, { radiusM: Infinity }, { radiusM: -1 }, { radiusM: 1001 }, { point: { longitude: NaN, latitude: 35 } }]) await expect(repo.nearest({ ...query, ...overrides })).rejects.toThrow();
    await expect(repo.connected("missing", "2", 10)).rejects.toThrow();
    await expect(repo.connected("101", "3", 10)).rejects.toThrow();
  });
});
