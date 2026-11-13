// @vitest-environment node
import { describe, expect, it, vi } from "vitest";
// Test the actual producer contract rather than a hand-maintained sidecar fixture.
// @ts-expect-error Node-only CLI has no declaration file.
import { convertGraph } from "../scripts/convert-osm-graph.mjs";
import { GRAPH_BUNDLE_LIMITS, readReferenceGraphBundle } from "./referenceGraphBundle";
const xml = '<osm version="0.6"><node id="1" lat="35" lon="135"/><node id="2" lat="35.001" lon="135.001"/><way id="3"><nd ref="1"/><nd ref="2"/><tag k="railway" v="rail"/></way></osm>';
const encode = (s: string) => new TextEncoder().encode(s);
const options = { id: "synthetic", revision: "1", source: "https://example.invalid/<script>alert(1)</script>", sourceDate: "2026-11-12", bounds: { west: 134, south: 34, east: 136, north: 36 } };
function bundle(overrides: Partial<typeof options> = {}) {
  const result = convertGraph(encode(xml), { ...options, ...overrides });
  return [new File([result.graphJson], "graph.json"), new File([result.sourceJson], "source.json"), new File([result.manifest], "complete.json")];
}
async function hash(bytes: Uint8Array) {
  return Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes as Uint8Array<ArrayBuffer>)), v => v.toString(16).padStart(2, "0")).join("");
}
async function replace(files: File[], index: number, bytes: Uint8Array) {
  files[index] = new File([bytes as Uint8Array<ArrayBuffer>], files[index].name);
  const manifest = JSON.parse(await files[2].text());
  manifest[index === 0 ? "graphSha256" : "sourceSha256"] = await hash(bytes);
  files[2] = new File([JSON.stringify(manifest)], "complete.json");
}
describe("local three-file reference graph bundle", () => {
  it("reads actual CLI output and retains provenance as inert text", async () => {
    const files = bundle();
    const result = await readReferenceGraphBundle(files);
    expect(result.graphSha256).toBe(await hash(new Uint8Array(await files[0].arrayBuffer())));
    expect(result.sourceSha256).toBe(await hash(new Uint8Array(await files[1].arrayBuffer())));
    expect(result.graphSha256).toMatch(/^[a-f0-9]{64}$/);
    expect(result.sourceSha256).toMatch(/^[a-f0-9]{64}$/);
    expect(result.graph.edges).toHaveLength(1);
    expect(result.source).toEqual({ source: options.source, sourceDate: options.sourceDate, license: "ODbL-1.0", attribution: "© OpenStreetMap contributors" });
  });
  it("accepts long fractional-second source dates emitted by the existing CLI", async () => {
    const sourceDate = "2026-11-12T00:00:00." + "0".repeat(150) + "Z";
    expect((await readReferenceGraphBundle(bundle({ sourceDate }))).source.sourceDate).toBe(sourceDate);
  });
  it.each([0, 1, 2])("rejects missing file %i", async index => {
    const files = bundle(); files.splice(index, 1); await expect(readReferenceGraphBundle(files)).rejects.toThrow();
  });
  it("rejects duplicates, aliases, extra files and directory names", async () => {
    const f = bundle();
    for (const files of [[f[0], f[0], f[2]], [...f, f[0]], [new File(["{}"], "GRAPH.JSON"), f[1], f[2]], [new File(["{}"], "path/graph.json"), f[1], f[2]]]) await expect(readReferenceGraphBundle(files)).rejects.toThrow();
  });
  it.each(Object.entries(GRAPH_BUNDLE_LIMITS))("checks %s size before any reading", async (name, limit) => {
    const files = bundle(), read = vi.fn();
    for (const f of files) Object.defineProperty(f, "arrayBuffer", { value: read });
    Object.defineProperty(files.find(f => f.name === name), "size", { value: limit + 1 });
    await expect(readReferenceGraphBundle(files)).rejects.toThrow(); expect(read).not.toHaveBeenCalled();
  });
  it("rejects declared/actual byte mismatch and read failures", async () => {
    const files = bundle(); Object.defineProperty(files[2], "arrayBuffer", { value: async () => new ArrayBuffer(1) });
    await expect(readReferenceGraphBundle(files)).rejects.toThrow();
    Object.defineProperty(files[0], "arrayBuffer", { value: async () => { throw Error("read failed"); } });
    await expect(readReferenceGraphBundle([files[0], bundle()[1], bundle()[2]])).rejects.toThrow("read failed");
  });
  it.each([0, 1])("rejects tampered file %i even with completion marker", async index => {
    const files = bundle(); files[index] = new File([await files[index].text(), " "], files[index].name);
    await expect(readReferenceGraphBundle(files)).rejects.toThrow();
  });
  it.each([0, 1])("rejects hashed invalid UTF-8 or JSON file %i", async index => {
    for (const bytes of [new Uint8Array([0xc3, 0x28]), encode("{")]) {
      const files = bundle(); await replace(files, index, bytes); await expect(readReferenceGraphBundle(files)).rejects.toThrow();
    }
  });
  it.each(["{", '{"format":"rosenfu-osm-graph-complete","version":2}', '[]'])("rejects invalid completion marker", async json => {
    const files = bundle(); files[2] = new File([json], "complete.json"); await expect(readReferenceGraphBundle(files)).rejects.toThrow();
  });
  it.each([
    (s: any) => { s.graphSha256 = "0".repeat(64); }, (s: any) => { s.coverageGuaranteed = true; },
    (s: any) => { s.source = "https://other.invalid"; }, (s: any) => { s.license = "CC0"; },
    (s: any) => { s.requestedBounds.west = 133; }, (s: any) => { s.nodes[0].id = "999"; },
    (s: any) => { s.ways[0].nd.reverse(); }, (s: any) => { s.sourceDate = "not a date"; },
    (s: any) => { s.extra = true; }, (s: any) => { s.counts.refs = -1; },
  ])("rejects inconsistent source even when hashes recomputed", async mutate => {
    const files = bundle(), source = JSON.parse(await files[1].text()); mutate(source); await replace(files, 1, encode(JSON.stringify(source)));
    await expect(readReferenceGraphBundle(files)).rejects.toThrow();
  });
  it("reuses graph validation before sidecar handling", async () => {
    const files = bundle(), graph = JSON.parse(await files[0].text()); graph.nodes[0].latitude = 91;
    await replace(files, 0, encode(JSON.stringify(graph))); await expect(readReferenceGraphBundle(files)).rejects.toThrow();
  });
  it("fails closed without WebCrypto", async () => {
    vi.stubGlobal("crypto", undefined);
    try { await expect(readReferenceGraphBundle(bundle())).rejects.toThrow("SHA-256"); } finally { vi.unstubAllGlobals(); }
  });
});
