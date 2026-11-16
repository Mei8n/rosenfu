# Pinned water-only zone reader: experimental, offline only

This stage is **not nationwide water support**. It does not change the construction model,
fetch network data, install a session cache, or change the production provider. No third-party
code or mixed city pack is copied into the implementation. Test payloads are synthetic.

## Evidence and rights, checked 2026-11-16

The reference is [TW commit 6b8ef917](https://github.com/circularsquare/trainsworld/tree/6b8ef91729e1e5812f3dcbaedd6c0a49fed6a84b).
Its README permits hosted forks to read published versioned data, asks busy forks to host their
own copy, and names data-version `2026-11-13`. Its terms prohibit unreasonable server load.
This is not an explicit assurance that an unrelated independent game may indefinitely rely
on that service. Provider permission/operational suitability for that use remains unresolved.
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
reading Content-Range; future acquisition should use a bounded native fixed-URL client.
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
header and checks the exact origin, grid, row/run dtype, water category, counts and contiguous
offsets. Duplicate consumed JSON fields or duplicate water arrays reject. Unrelated mixed-pack
fields are discarded. This binds the specification to pinned bytes, not real-world completeness.
`urls` cannot form arbitrary hosts/paths but does not assert a zone is published: the caller
must select an audited allowlist entry. No caller-supplied pin should be exposed as IPC trust.

`ModelDry` means successfully decoded adopted-model dry. `OutsideGrid` does not mean dry
or provider nonprovision. Missing, damaged or unacquired data is an error, never an empty mask.
The observational API remains separate and may retain Unknown for incomplete coverage.

## Next stage, not implemented

Native streaming fetch with redirect rejection, deadlines/cancellation, aggregate budget,
cache retention on failure, fixed audited zone allowlist and independent header extraction.
Generate water-only SHA pins with a bounded opt-in tool, not production trust-on-first-use.
The fixed reference registry lists 45 Japanese zones, not proof every Japanese location is
covered. Keep registry boxes and actual raster footprints separate. Unchecked/missing zones
must not be promoted to normal nonprovision. Overlap resolution needs an explicit same-version
policy. Save-model revision/migration and full path construction integration remain separate.


## Acquisition state machine checkpoint

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
