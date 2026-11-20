# Fixed Japanese water-zone runtime

The desktop uses the audited 2026-11-13 water-only registry, not whole mixed packs.
Only regions intersecting the current camera, planned geometry, or existing track
segments are requested. Existing track regions are needed because a small endpoint
edit can trigger validation of its entire connected component. Separate segment
bounds avoid downloading unrelated regions between distant disconnected lines.
This is not an unconditional nationwide download.

Native HTTPS verifies the fixed host/version/path, header hash and grid layout,
206 Content-Range, payload length/hash, padding and every RLE row. No arbitrary
frontend URL is accepted. Two concurrent transfers, 30-second transfer deadlines,
64 MiB bounded memory caches and existing IPC limits apply. Successful bytes are
immutable; late installation can refresh current previews only after a fresh query.
A hook round waits at most 30 seconds. Progress resets its failure counter; retries
rotate the queue so a failed first pair cannot permanently starve later regions.
At most 48 rounds occur per stable context, with 30-second backoff. Network failures
are still unknown/refused, not empty land. Native cancellation is not wired to user
cancellation; the native deadline bounds work after the UI stops waiting.

Every overlapping registered grid must be available. If any valid overlapping grid
is wet, the model is wet. Valid fine dry cells override coarse water. Outside fine
grids the bundled coarse model applies; outside its supported area validation fails.
Neither missing required data nor corrupt/changed provider bytes means land.

Fine cell sizes are 25 or 50 metres. Narrow or omitted water features are not
represented; this is a game model, not complete surveyed reality. Data attribution
remains available even with a sample/offline map background. The public data-access
basis, ODbL sources and hosting limitations are documented in
[pinned-water-zone-reader.md](pinned-water-zone-reader.md). No TrainsWorld code or
non-water mixed-pack layers are copied into this implementation.

The save schema remains 25. Raw water bytes and provider revision are not embedded
in saves. The app uses a fixed revision, but a future revision change may change
validation of existing geometry; it must be disclosed. Actual GUI and 3 OS results
must be checked for the published final commit before claiming readiness.
