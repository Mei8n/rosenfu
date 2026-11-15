# National source query batches

`national_query.py CONFIG --output NEW_DIRECTORY` queries every selected window
against all eight sealed regional stages, then clips the pinned marine source.
It preserves nine independent parts (`inland_<region>`, `marine`) rather than
choosing one region based on a point or joining source polygons. This avoids
silently excluding the other side of an extract boundary. It does not prove
that the eight extracts contain every real-world water feature.

The config contains `source_set_sha256`, `stages` (exactly the eight known region
keys, each with local `directory` and stage-manifest `sha256`), `marine` (local
Shapefile `path`, canonical component inventory `bundle_sha256`, source `url` and
explicit source-date `revision`), and `cells` (`key`, four-number `bounds`). All
regional stage manifests must agree on source-set pin and source timestamp.
Each stage extraction rechecks its database hash and source metadata; the marine
input inventory is checked before and after extraction. The source-date string
is an operator-verified claim, not a date inferred from download time. Marine
and inland dates may differ and remain separate in the release.

Select 1..32 cells, each at most 0.2 degrees on each axis. Limits remain 5 MiB per
leaf file, 20 MiB/20,000 observations/200,000 coordinates per composite and
128 MiB aggregate leaf bytes per query batch. Oversized cells fail; split the
requested window explicitly and keep the resulting cells unknown. Working data
and copied composites consume additional disk beyond the leaf-byte limit.
Only `release/delivery.json` marks a completed release. A failed batch can leave
partial working data, which must not be treated as a completed release.

No polygon union, deduplication, width inference or coverage promotion occurs.
Cross-region observations may overlap or repeat. Source issues from all stages
remain attached even when their location is unknown; their absence does not
prove relation assembly completeness. Nine parts can contain zero observations
and still mean unknown. Line width, culvert/tunnel/covered tags and mixed source
dates retain the restrictions in `water-evidence-delivery.md`.

The unchanged TypeScript delivery loader accepts these pinned multi-part
composites for inspection only. App wiring requires agreement with its owner.
This is an on-demand batch builder over national source observations, not a
prebuilt nationwide tile service. Public HTTPS hosting, CORS in the actual app,
complete water coverage and construction decisions remain outside this result.
