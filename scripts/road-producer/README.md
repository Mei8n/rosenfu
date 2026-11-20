# Pinned national-road producer (not a public release)

Responsibilities are separate:
- PR152 `road-pack/national.py` extracts unchanged selected OSM ways with bounded disk use.
- PR154 `road-delivery/package.py` validates files and creates bounded deterministic ZIP shards.
- This producer retrieves pinned official input bytes, orchestrates regional jobs, then combines packages.
  It does not alter runtime release pointers, the user's PC, or coverage certification.

## Source contract and license

`sources.json` pins the eight official Geofabrik Japanese regional PBFs dated 261114,
all with replication timestamp `2026-11-14T21:20:41Z`, together with downloaded
polygon hashes and exact byte lengths. Hashes were computed from the actual shared
source files, not invented. URLs are restricted to the official HTTPS origin;
redirects, changed bytes and changed sizes fail rather than falling back to latest.
The undated `.poly` URL may change, and old dated PBFs may disappear. A failure then
requires a reviewed source-catalog revision; it must not silently accept replacements.

Geofabrik describes historical-file retention and polygon extraction at
https://download.geofabrik.de/technical.html . Public PBFs omit contributor user,
uid and changeset metadata. Only highway geometry and needed structure tags are
exported. No authenticated/internal extracts are used. The underlying OSM data is
ODbL-1.0; derived data retains that license and © OpenStreetMap contributors with
links to https://www.openstreetmap.org/copyright and
https://opendatacommons.org/licenses/odbl/1-0/ . Code license and data license are separate.
No TrainsWorld code or dataset is copied. Generation uses at most one regional job
at once, two source requests sequentially, no background retries or schedule.
No unlimited hosting/SLA is assumed from the open-data license.

The historical clipping-polygon/PBF binding is **not proven**. Every actual source
stays `coverage_verified:false`, `polygon_snapshot_match:"unverified"`. Hash
verification is transfer integrity, not proof of all real roads. Missing cells,
region borders, road classes outside the selected list and OSM omissions are not
inferred to be road-free. `publishable:true` in a prepared catalog means package
integrity only. UI/runtime decisions must preserve the separate coverage flag.

## Workflow

`road-producer.yml` runs only unit tests on PRs. Explicit `workflow_dispatch`
generates the selected region (default Shikoku) or all eight, with matrix
`max-parallel:1`. Each fresh Ubuntu runner downloads one source pair, generates and
packages it, then retains compressed artifacts for one day. Failed regions prevent
combination; an `all` run cannot silently publish a partial-country index.
The combined release preserves exactly the requested regions and rechecks source,
revision, coverage, index hash, archive hash/size, path/count and total budgets.

Limits: 8 GiB free preflight, 3 GiB global free floor, 4 GiB own generation allocation,
2 GiB raw output per region, 20 million source ways, 500,000 cells per region.
Packaging retains its 64 MiB compressed/256 MiB expanded per-shard and 2 GiB total
compressed/998-assets limits. These are rejection limits, not claims that every
region will fit. Source downloads have 30-second socket and 15-minute overall
limits; generation uses its two-hour extraction deadline and the Actions job is
bounded at 180 minutes. Cancellation publishes no completed raw index or release.

The producer's prepared artifacts directly use the PR154 catalog/ZIP contract;
it does not call PR154's separate historical-run staging consumer. They are two
alternative preparation entry points, not a circular same-run dependency.

Draft preparation defaults off. It requires an explicit input, execution from
`dev/construction-review-integration`, and an already-existing `data-road-*` tag.
It uses `gh release create --verify-tag --draft`; it never publishes, overwrites,
or creates a tag. Further human review/approval is required before publication.
`workflow_dispatch` may be unavailable until the workflow is present on the
repository's default branch. This PR does not change main to enable it. Actions
storage, runner quotas, all-region capacity, and a real draft run remain unverified.

## Local checks

```
python -B -m unittest discover -s scripts/road-producer -v
python scripts/road-producer/produce.py produce shikoku /some/new/output
python scripts/road-producer/produce.py combine /regional/packages /new/release kanto shikoku
```

Tests use tiny synthetic payloads with mocked HTTP. They exercise hash/size failure,
redirect refusal, source identity, partial-region rejection, unknown coverage,
corrupt ZIP bytes, path/duplicate rejection and atomic budget failure. They do not
claim that a national production run, Actions dispatch, or release upload occurred.

## Transport version 2

The original 1-degree ZIP can exceed the 256 MiB expanded safety limit in dense
regions (observed in Kanto). Catalog v2 preserves the original index and complete
shard manifest, but splits cell files into lexicographically contiguous parts.
Each part targets 32 MiB expanded, includes the same complete manifest, and records
`first_cell`, `last_cell`, and `cell_count`; `files = cell_count + 1`.
The hard limits remain 256 MiB expanded, 64 MiB compressed, 998 assets, and 2 GiB
aggregate compressed. These are refusal limits, not promises that all Japan fits.
Version 1 catalogs are rejected. Combining verifies manifest hashes, ZIP member
sets, and complete non-overlapping cell ranges before making output available.
All real data remains coverage-unverified; transport integrity is not proof of
complete roads or absence of roads. The workflow has not yet run nationally.
