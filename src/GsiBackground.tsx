import { positionTile, MAX_TILES } from "./map";
import type { Camera } from "./map";
import { useEffect, useRef, useState } from "react";

export type BackgroundTile = { id: string; url: string; x: number; y: number; width: number; height: number };
export type TileStatus = { loaded: number; failed: number; total: number };

type Props = { camera?: Camera; tiles: readonly BackgroundTile[]; width: number; height: number; onStatus: (status: TileStatus) => void };

/** Provider-neutral raster layer. Browser HTTP cache is retained; no prefetch, application cache or automatic retries. */
export function GsiBackground({ tiles, camera, width, height, onStatus }: Props) {
  // Only already displayed, successfully loaded tiles are retained during replacement.
  // At most one bounded layer of metadata, no prefetch or persistent/offline cache.
  const loadedTiles=useRef(new Map<string,BackgroundTile>());
  const currentTiles=useRef(tiles);currentTiles.current=tiles;
  const currentTileIds = useRef(new Set<string>());
  currentTileIds.current = new Set(tiles.map((tile) => tile.id));
  const [results, setResults] = useState<Record<string, "loaded" | "failed">>({});
  useEffect(() => {
    const visible = new Set(tiles.map((tile) => tile.id));
    setResults((previous) => {
      const retained = Object.entries(previous).filter(([id]) => visible.has(id));
      return retained.length === Object.keys(previous).length ? previous : Object.fromEntries(retained);
    });
  }, [tiles]);
  const reported=useRef<TileStatus|null>(null);
  useEffect(() => {
    const next={
      loaded: tiles.filter((tile) => results[tile.id] === "loaded").length,
      failed: tiles.filter((tile) => results[tile.id] === "failed").length,
      total: tiles.length,
    };
    const previous=reported.current;
    if(!previous||previous.loaded!==next.loaded||previous.failed!==next.failed||previous.total!==next.total){reported.current=next;onStatus(next);}
  }, [results, tiles, onStatus]);
  const settled=tiles.every(tile=>results[tile.id]!==undefined);
  useEffect(()=>{
    if(settled)for(const id of loadedTiles.current.keys())if(!currentTileIds.current.has(id))loadedTiles.current.delete(id);
  },[settled,tiles]);
  const fallback=!settled && camera ? [...loadedTiles.current.values()].filter(tile=>!currentTileIds.current.has(tile.id)).map(tile=>positionTile(tile,camera)) : [];
  const rendered=[...fallback,...tiles];
  function finish(id: string, status: "loaded" | "failed") {
    if (!currentTileIds.current.has(id)) return;
    if(status==="loaded"){
      const tile=currentTiles.current.find(tile=>tile.id===id);
      if(tile){loadedTiles.current.delete(id);loadedTiles.current.set(id,tile);}
      while(loadedTiles.current.size>MAX_TILES)loadedTiles.current.delete(loadedTiles.current.keys().next().value!);
    }
    setResults((previous) => previous[id] ? previous : { ...previous, [id]: status });
  }
  return <g className="gsi-background">
    <rect width={width} height={height} fill="#dce6e5" />
    {rendered.map((tile) => results[tile.id] === "failed"
      ? <g key={tile.id} data-failed-tile={tile.id}>
          <rect x={tile.x} y={tile.y} width={tile.width} height={tile.height} fill="#e4d5cb" stroke="#bfa48f" />
          <text x={tile.x + tile.width / 2} y={tile.y + tile.height / 2} textAnchor="middle" fill="#654b3c" fontSize="14">読込失敗</text>
        </g>
      : <image key={tile.id} data-tile-id={tile.id} data-fallback-tile={!currentTileIds.current.has(tile.id) || undefined} href={tile.url} x={tile.x} y={tile.y} width={tile.width} height={tile.height}
          onLoad={() => finish(tile.id, "loaded")} onError={() => finish(tile.id, "failed")} />)}
  </g>;
}
