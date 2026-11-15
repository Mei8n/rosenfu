# Nationwide road data delivery (not deployed)

## Decision and evidence

TrainsWorld's worldwide coverage is **demand-driven**: its `roadFeed.ts` obtains
zoom-14 basemap vector tiles intersecting the proposed railway, rather than shipping
a planet database. `basemap.ts` selects OpenFreeMap Positron. This is useful as an
architecture reference, not code to copy and not proof that every road is present.

- https://github.com/circularsquare/trainsworld/blob/main/trainworld/app/src/workers/roadFeed.ts
- https://github.com/circularsquare/trainsworld/blob/main/trainworld/app/src/map/basemap.ts
- https://openfreemap.org/tos/ (reviewed 2026-11-15)

OpenFreeMap permits commercial map use, but its current terms restrict automated
collection outside the stated offline allowance: at most 3,000 tiles or 50 MB per
download, four parallel requests and an identifying User-Agent. Larger downloads
require contacting the provider. Its public instance has no availability guarantee.
Do not transplant TrainsWorld's six-request concurrency or infer bulk collection
permission from its source. Road display tiles are not certified unsimplified
construction input. No OpenFreeMap data is downloaded by this delivery script.

Use the independent OSM extract pipeline for construction data and copy the
**fetch only what is needed** architecture. This retains whole OSM ways, IDs and
structure tags; missing data stays unknown. ODbL notices and source/hash provenance
remain in the database. This is not a guarantee that OSM records every real road.

## Proposed hosting

The existing `Mei8n/rosenfu` repository was verified public through the GitHub API
on 2026-11-15. GitHub Releases supports up to 1,000 assets per release, each under
2 GiB; the documented release total size and bandwidth have no limit. These are
release limits, **not** Actions artifact limits or an SLA.

- https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases
- https://docs.github.com/en/rest/releases/assets
- https://docs.github.com/en/actions/reference/limits

Use a separate `data-road-*` tag, not a main-branch merge, with a pinned catalog,
index and compressed shard assets. Version paths are never overwritten. The
client must use fixed release download URLs, not unauthenticated REST discovery
for each cell (REST has a 60 requests/hour/IP limit). No token ships with the app.
No paid provider, billing change or repository visibility change is made here.

GitHub Actions artifacts expire and have plan-dependent storage quotas; they are
only a temporary producer-to-release handoff, never the application's data host.
The workflow keeps its handoff for one day. A national build's workspace/time and
artifact quota must be checked before running it; standard hosted jobs have a
six-hour limit. Do not automatically repeat a national build on every app commit.

## Implemented packaging

```
python scripts/road-delivery/package.py /path/to/generated-pack /new/output
python -B -m unittest discover -s scripts/road-delivery -v
```

The package operation verifies the index's manifest hashes and every named cell's
size/hash, rejects traversal/duplicates/symlinks, writes deterministic ZIP files
one per one-degree shard and a hash/size catalog. Failure leaves no final output.
Archives are limited to 64 MiB compressed and 256 MiB expanded, and 998 shard
assets; oversized shards need subdivision, not a silent budget increase. This is
a packaging integrity check, not a replacement for the source coverage audit.
Unverified polygon/snapshot coverage is retained as `publishable: false`.

Measured against the generated unverified Shikoku pack: 12,381 cells, 12 archives,
265.49 MiB expanded -> 60.14 MiB ZIP total, largest ZIP 17.54 MiB. These are local
measurements, **not** published files or nationwide runtime coverage.

## Runtime work still required

The current national reader accepts same-origin JSON only. It cannot be pointed
at these ZIP URLs without another implementation step. A native download/cache
adapter should:

1. Pin the exact catalog SHA and index SHA in the app release.
2. Select just the needed shard(s); never download Japan at startup.
3. Fetch an allowlisted GitHub release URL, validating redirect destinations,
   with bounded transfer/time, retries and cancellation. Do not forward credentials.
4. Check the compressed hash, file count, all entry paths, each entry size/hash
   and aggregate expanded bytes before atomic cache publication. Reject symlinks,
   duplicates, zip bombs, unexpected entries and traversal. No arbitrary extraction.
5. Expose requested JSON cells through IPC or a same-origin adapter. Use an LRU
   disk budget, source-revision keys and offline cached reuse. Never equate a 404
   or interrupted download with an empty road cell.
6. Merge coverage and deduplicate way identity across shard borders. The current
   reader explicitly rejects cross-shard requests; that remains incomplete.

A whole compressed shard is the first practical transport, not a whole region.
PMTiles or an indexed cell-range container could reduce individual downloads later,
but would need a new file format and verified HTTP Range/redirect behaviour. Those
properties have **not** been tested on a published asset; do not claim them here.

## Workflow boundaries

`road-release.yml` tests packaging on all three OSes. Its manual preparation path
consumes `road-pack-staging` from a successful, exact-SHA, same-repository push or
manual producer run. No producer or generated national data is created by that
workflow. Default preparation is read-only plus a short-lived artifact.
An explicit optional draft job is restricted to the development integration branch,
requires an already existing `data-road-*` tag and verified coverage, grants only
that job `contents: write`, and creates a **draft**, never a public release.

GitHub requires a `workflow_dispatch` definition on the default branch before it
can be called for another ref. The repository default is main, which must not be
changed without the owner's separate approval. Therefore the manual path is
**not yet runnable merely by merging to dev**. A `workflow_call` entry also allows
an explicitly configured producer/caller on an authorized development branch to
invoke preparation without changing main; no such producer is enabled here. The same packaging command can run
inside an authorized cloud producer and use authenticated release tooling once
publication is approved. Do not work around the main restriction or claim a draft
can be fetched anonymously. Final publication, anonymous download/hash verification,
coverage audit, native client integration and end-to-end GUI testing are outstanding.

https://docs.github.com/en/actions/how-tos/manage-workflow-runs/manually-run-a-workflow
