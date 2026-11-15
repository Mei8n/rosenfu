# Water inspection trial — three selected windows only

© OpenStreetMap contributors. This trial database is available under the
[Open Database License (ODbL) 1.0](https://opendatacommons.org/licenses/odbl/1-0/).
This data license is separate from the repository code license. Retain this
notice and the source attribution and license metadata in each part. Map
applications must display [OpenStreetMap attribution](https://www.openstreetmap.org/copyright).

The complete distributed trial database is the editable JSON in this directory.
It consists of selected OSM observations and clipped marine polygons for Fuji,
Kanmon and Takamatsu. Query and extraction source code is in
`scripts/water-pack/` in this same repository revision. No data is encrypted or
restricted by additional technical measures.

Sources: [Geofabrik Japan regional extracts](https://download.geofabrik.de/asia/japan.html)
(OSM timestamp 2026-11-14T21:20:41Z), and
[OSM water polygons](https://osmdata.openstreetmap.de/data/water-polygons.html)
(source date 2026-11-13T15:00:00Z). Each part retains source URLs, hashes, selection
rules and metadata. Marine geometry was intersected with each query rectangle;
regional geometry was selected by bounding box without clipping. Marine split
polygons can overlap. No union, deduplication or inferred river width was added.

All cells remain **incomplete / coverage=[]**. The requested bounds are query
windows, not coverage certification. Source errors and unknown widths remain;
no observations does not mean dry land. Original source times differ. This data
must not enable construction or certify dry land. Use for inspection only.

`delivery.json` SHA-256:
`348194210a1c47e06536f347f1b889dc777c5931e2eb7a1a7299412e3924bff9`

Use a commit-pinned HTTPS root, verify this catalog hash independently, and pass
its cell directory plus composite hash to `loadWaterEvidenceComposite`. Never
trust a hash obtained only from the same untrusted download as the data. GitHub
raw delivery here is a small trial; no nationwide availability, performance or
production hosting guarantee is made. Actual app CORS/CSP/WebView integration
still requires owner testing.
