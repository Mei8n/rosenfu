# Pinned water-only zone reader and registry

The initial PR193 checkpoint was an offline-only reader. The current runtime adds bounded
native acquisition, an audited 45-zone registry and construction-model integration; see
[national-water-runtime.md](national-water-runtime.md). Registered zones do not prove complete
national geographic coverage. No third-party code or mixed city pack is copied. Committed
test payloads are synthetic; real water-only evidence is verified externally, not redistributed.

## Evidence and rights, checked 2026-11-16

The reference is [TW commit 6b8ef917](https://github.com/circularsquare/trainsworld/tree/6b8ef91729e1e5812f3dcbaedd6c0a49fed6a84b).
Its README permits hosted forks to read published versioned data, asks busy forks to host their
own copy, and names data-version `2026-11-13`. Its terms prohibit unreasonable server load.
This is not an explicit assurance that an unrelated independent game may indefinitely rely
on that service. The follow-up public README statement “the data host allows any site to read it” supports
bounded on-demand reads, subject to its load restrictions. It does not grant a permanent
service commitment or an individual operating agreement.
No provider outreach has been performed.

[NOTICE](https://github.com/circularsquare/trainsworld/blob/6b8ef91729e1e5812f3dcbaedd6c0a49fed6a84b/NOTICE.md)
separates AGPL code from ODbL data, but notes additional restrictions in mixed packs.
The water pipeline and Tokyo header identify OSM coastline-derived sea polygons and Geofabrik
inland water as water sources. Keep OpenStreetMap attribution, ODbL terms and adaptation/source
information with any eventual water-only derivative. This is not clearance to redistribute
mixed packs or unrelated layers. No real water payload is committed in this stage.

A bounded inspection fetched only Tokyo's 23,984-byte JSON header and 16 water bytes.
The header SHA256 was `9120fa6c925372d9e4dfca76b4e9ba3e65a6efe186ff62872baab6b56cba8afd`.
The published URLs are `https://data.anita.garden/trainworld/2026-11-13/tyo.json` and
`https://data.anita.garden/trainworld/2026-11-13/tyo.bin`.
Water rows begin at 2,518,712 (9,262 u32 values), followed by 203,244 u16 run boundaries;
combined water-only range is 443,536 bytes. Full container length is 6,696,000 bytes.
Grid: 8,002 × 9,261 at 25 m; the source reports removal of water narrower than 50 m.
This is a deliberately simplified game model, not all real waterways.

Observed responses: exact 206 Content-Range for the 16-byte probe, ACAO `*`, immutable cache,
ETag. No Access-Control-Expose-Headers was observed. Browser code therefore cannot rely on
reading Content-Range; the runtime therefore uses a bounded native fixed-URL client.
An ETag is not a cryptographic payload pin. The follow-up fetched exactly the 443,536-byte water range (no other binary layers).
Payload SHA256: `588d24c63b44ebe4cb004583b2c22cdc3691dc7fec101966adffb605f2fcb0b4`.
This is a local integrity pin, not a provider signature. The payload is not committed.

## Pure API and limits

`water_zone::ZonePin` binds provider revision, zone, rule, grid, water offset/count/container
length and header/payload SHA256. It is an untrusted DTO, not an authenticated registry entry.
`WaterZone::from_range` requires exact status 206, canonical Content-Range including total,
body length and SHA. `decode` verifies offline water-only bytes. Rows and runs are allocated
only after length/hash checks; dense cells are never allocated. Limit: 16 MiB payload,
65,535 rows/columns, 1 MiB header. A large sparse grid does not require a dense-cell budget.
Rows must start at zero, end at run count, increase monotonically in even pairs; runs must
be ordered, non-overlapping and within width. Coordinates and bounds must be finite.

`verify_header` checks integrity only. `verify_header_spec` additionally parses the bounded
header and checks the exact origin, grid, row/run dtype, water category, counts and explicitly padded
offsets. Duplicate consumed JSON fields or duplicate water arrays reject. Unrelated mixed-pack
fields are discarded. This binds the specification to pinned bytes, not real-world completeness.
`urls` cannot form arbitrary hosts/paths but does not assert a zone is published: the caller
must select an audited allowlist entry. No caller-supplied pin should be exposed as IPC trust.

`ModelDry` means successfully decoded adopted-model dry. `OutsideGrid` does not mean dry
or provider nonprovision. Missing, damaged or unacquired data is an error, never an empty mask.
The observational API remains separate and may retain Unknown for incomplete coverage.

## Original pure-reader checkpoint scope

At that checkpoint, the following remained outside the pure reader: native streaming fetch
with redirect rejection, deadlines/cancellation, aggregate budget,
cache retention on failure, fixed audited zone allowlist and independent header extraction.
Generate water-only SHA pins with a bounded opt-in tool, not production trust-on-first-use.
The fixed reference registry lists 45 Japanese zones, not proof every Japanese location is
covered. Keep registry boxes and actual raster footprints separate. Unchecked/missing zones
must not be promoted to normal nonprovision. Overlap resolution needs an explicit same-version
policy. Save-model revision/migration and full path construction integration remain separate.


## Transport-independent acquisition state machine checkpoint

`water_zone::acquisition::Acquisition` is a native transport-independent adapter, **not an
HTTP client**. It emits fixed-host/version request URL, Range and expected byte count after
header hash/spec validation. Chunk accumulation is bounded; every push/finish checks a 30-second
monotonic deadline. Tokens reject canceled or replaced attempts. The successful immutable
Arc cache is replaced only after complete response/hash/decode verification; failures preserve it.
An audited registry must select the pin. Repeated begin invalidates older attempts, rather than
sharing network work. No background tasks or sockets are started by this module.

The future HTTP owner must bound the separate header request, disable redirects, apply socket
cancellation/deadlines and invoke finish with actual HTTP status and Content-Range. The adapter
cannot stop a transport that never returns. No Tauri IPC/runtime/provider switch is included.
Reqwest exists transitively in the desktop lock, but no direct HTTPS client dependency/configuration
is added in this checkpoint. This avoids coupling an experimental provider to the debug build.

External verification (opt-in, real payload stays outside git):

```sh
TW_WATER_HEADER=/path/to/tyo.json TW_WATER_PAYLOAD=/path/to/water-only.bin \
  cargo test -p rosenfu-core --lib water_zone -- --include-ignored --nocapture
```

The external test pins both hashes, checks header-to-grid/offset binding, decodes every RLE row
and compares the total wet-cell fraction against the provider header. It does not demonstrate
all 45 zones, browser behavior, provider permission for indefinite production dependence, or
correct real-world water boundaries.

## Fixed Japanese registry verification

The registry targets the 45 Japanese zones listed by the reference registry at
`6b8ef91729e1e5812f3dcbaedd6c0a49fed6a84b`, with delivery version `2026-11-13`.
This is a finite list, not nationwide geographic or real-world water completeness.
Only extracted water grid metadata and integrity pins belong in the manifest;
full mixed headers and binary packs must not be redistributed. The water source
identifies OpenStreetMap coastline polygons and Geofabrik inland water, under ODbL.
Keep OpenStreetMap attribution and license information visible. The distinct restricted
sources in mixed packs are not part of this water-only extraction.

The public README says “the data host allows any site to read it” and asks busy
forks to self-host. The live terms prohibit unreasonable server load. These support
bounded on-demand access, not a guarantee of permanent hosting, availability, or
an author-signed integrity assertion. Version pins are hashes of observed published
bytes. No provider signature is claimed. Do not silently change a pin after failure.

- README: https://github.com/circularsquare/trainsworld/blob/6b8ef91729e1e5812f3dcbaedd6c0a49fed6a84b/README.md
- Attribution: https://github.com/circularsquare/trainsworld/blob/6b8ef91729e1e5812f3dcbaedd6c0a49fed6a84b/NOTICE.md
- Terms: https://anita.garden/trainsworld/terms.html
- ODbL: https://opendatacommons.org/licenses/odbl/summary/

`registry::pins()` fails closed until the entire audited manifest validates. Do not
convert that error to an empty list or enable the production provider with a partial
list. `intersecting([west,south,east,north])` uses the same grid projection as `at`,
not the publisher's city-selection bounding boxes, and returns all touching grids.
Consumers must give wet cells priority in overlaps. Missing required grids remain
unknown even if another overlapping grid is dry. Only a valid registry query with
no applicable fine grid establishes normal fine non-provision; it does not establish
land. Outside-grid points and failed downloads never establish dry cells.

Some headers align water arrays with zero-filled padding. `runPadding` explicitly
binds 0–7 bytes between row offsets and runs. Padding participates in the payload
hash, is checked against header offsets, and must decode to all zero bytes. Existing
zero-gap pins remain valid. No mixed-layer bytes may be interpreted as padding.

### Registry measurement (2026-11-16)

All 45 versioned headers were retrieved with bounded retries: 1,096,113 bytes total.
The sum of water-only contiguous ranges, including alignment padding, is 8,150,112
bytes; the largest is 507,184 bytes. There are 24 four-byte gaps and 21 zero-byte
gaps, 43 grids at 25 metres and two at 50 metres. The largest grid dimension is
16,839. These are model resolutions, not a real-world accuracy guarantee.
Acquisition experienced transient connection timeouts, not confirmed HTTP 404s;
timeouts were never classified as normal non-provision. The proxy configuration
was not disabled or changed. No full mixed binary pack was fetched.

All 45 water-only ranges were subsequently acquired and validated: status 206,
exact Content-Range including container length, exact body size, independently
recorded SHA256, pinned-header field agreement, zero padding and every RLE row.
The extracted manifest is `crates/rosenfu-core/src/water_zone/registry.json`.
Neither binary payloads nor complete mixed headers are committed. This verification
is of the fixed published model, not the completeness of all Japanese waterways.

To recheck local evidence without downloading anything:

```sh
WATER_ZONE_EVIDENCE=/path/to/evidence \
  cargo test -p rosenfu-core --lib external_registry_headers_and_all_row_runs -- --ignored
```

The evidence directory must contain `<zone>.json` and `<zone>.water` for every
manifest entry. Reacquisition uses each pin's fixed JSON URL and binary URL with
`Range: bytes=rangeStart-(rangeStart+payload_length-1)`, never a full binary pack.
Reject redirects, non-206 status, a mismatched range/total, oversized response,
header mismatch or changed SHA. A failed recheck must not rewrite the committed
manifest or become a successful empty zone. Native/runtime integration tests are
separate from these pure registry/decoder tests.
