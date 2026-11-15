# Water evidence delivery contract (proposed; not wired into the app)

`loadWaterEvidenceComposite({directoryUrl, compositeSha256}, {signal})`
returns the existing `WaterEvidenceComposite` inspection view. The caller must
obtain the SHA-256 from a trusted, separately pinned release descriptor. Fetching
an untrusted index and trusting the hash printed inside it is not pinning.

Delivery directories contain `composite.json`, `inland/{manifest,evidence}.json`
and `marine/{manifest,evidence}.json`. Use immutable, content-addressed directory
names and HTTPS with CORS allowed for the app origin. No credentials are sent;
redirects are rejected. Root pin is checked before any child request. Each child
manifest and payload is verified by the existing strict reader. Requests are
sequential, bounded (64 KiB index, 5 MiB per child, 20 MiB child total), cancellable
and limited to 30 seconds for the operation. No failed-request or stale fallback
cache is kept. Transport/schema/hash errors reject the entire operation; the
caller must display unavailable evidence rather than an empty water result.

## Contract to agree with the road/app owner

- Owner may display `observedAreas` as inspection overlays with source attribution,
  source issues and separate per-part revisions. Preserve holes and overlapping
  polygons. Do not sum overlapping area or crossing distance.
- Successful delivery still returns `status: incomplete` and `coverage: []`.
  Do not insert its requested bounds into `EnvironmentSnapshot.waterCoverage`,
  convert it to ready, or use missing observations to certify dry land.
- `hasMixedRevisionClaims` reports unequal strings, not verified time alignment.
  Current inland source is 2026-11-14T21:20:41Z and marine source is
  2026-11-13T15:00:00Z. Pins establish byte identity, not same-date truth.
- Waterway centerlines retain their source tags. Unknown width has no polygon;
  numeric width is a claim, not authorization to buffer a line. Culvert/tunnel/
  covered tags do not establish a six-metre construction elevation.
- Relation issues remain visible. A source with no reported issues can still
  omit an unassembled relation or unmapped river. Empty marine results do not
  prove inland water absence. No coverage promotion is supported by this API.
- Owner cancels the previous operation when route/region changes and discards
  stale results. Existing validated construction data must not be silently
  replaced by this inspection-only result.

This change touches no App, hook, session, shared environment type or physics.
Connection of the inspection view requires agreement with that owner. Nationwide
regional observations are staged, but nationwide delivery cells, hosting and
end-user app integration are not complete. Ten regional trial composites are
available in the cloud artifact directory; their existence is not nationwide
coverage certification. OSM data attribution and ODbL obligations are separate
from this project's code license; expose each part's source metadata in the UI
and retain the release provenance on redistribution.

## Static trial release

Run `python scripts/water-pack/delivery.py selection.json --output NEW_DIRECTORY`.
The selection is an array of `{key, directory, composite_sha256}` records. Input
paths are operator-local paths. The generator revalidates both parts and copies
exact bytes beneath `<composite_sha256>/`. `delivery.json` is written last with
relative directories, bounds, separate revisions and empty coverage. It makes no
blanket license or attribution claim: retain and display each part's source
metadata, and verify its redistribution conditions before hosting.
Pin its printed SHA-256 separately when distributing the release. Its cells are
selected query windows, not coverage polygons. Limit: 256 cells and 512 MiB of
copied pack bytes per trial release; this is not a nationwide cell generator.
A failed run leaves partial output without the completion marker. Do not publish
or consume that directory. No hosting or deployment is performed by this tool.
