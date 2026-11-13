import type { GraphBounds } from './referenceGraph';
type Entry = { bounds: GraphBounds; index: number };
type Node = { bounds: GraphBounds; entries?: Entry[]; left?: Node; right?: Node };
export function boundsOverlap(a: GraphBounds, b: GraphBounds): boolean {
  return !(a.east < b.west || a.west > b.east || a.north < b.south || a.south > b.north);
}
/** Immutable packed bounding-volume tree. Each edge occurs once, including long
 * edges; memory cannot explode with cell coverage. Broad phase only. */
export function createReferenceSpatialIndex(bounds: readonly GraphBounds[]) {
  function build(entries: Entry[]): Node | undefined {
    if (!entries.length) return undefined;
    const b = { west: Infinity, south: Infinity, east: -Infinity, north: -Infinity };
    for (const e of entries) { b.west=Math.min(b.west,e.bounds.west); b.south=Math.min(b.south,e.bounds.south); b.east=Math.max(b.east,e.bounds.east); b.north=Math.max(b.north,e.bounds.north); }
    if (entries.length <= 16) return { bounds:b, entries };
    const horizontal = b.east-b.west >= b.north-b.south;
    entries.sort((a,z) => (horizontal ? a.bounds.west+a.bounds.east-z.bounds.west-z.bounds.east : a.bounds.south+a.bounds.north-z.bounds.south-z.bounds.north) || a.index-z.index);
    const mid=entries.length>>>1;
    return { bounds:b, left:build(entries.slice(0,mid)), right:build(entries.slice(mid)) };
  }
  const root=build(bounds.map((b,index)=>({bounds:{...b},index})));
  return { query(query:GraphBounds):number[] {
    const result:number[]=[];
    function visit(node:Node|undefined) {
      if (!node || !boundsOverlap(node.bounds,query)) return;
      if (node.entries) { for(const e of node.entries) if(boundsOverlap(e.bounds,query))result.push(e.index); }
      else { visit(node.left);visit(node.right); }
    }
    visit(root);
    // Stable source order is part of display truncation semantics.
    return result.sort((a,b)=>a-b);
  }};
}
