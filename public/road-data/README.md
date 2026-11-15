# Tokyo centre road snapshot — ODbL 1.0

© OpenStreetMap contributors. This directory contains a derived OpenStreetMap database,
licensed under the Open Database License 1.0:
https://opendatacommons.org/licenses/odbl/1-0/
Attribution and OSM licensing: https://www.openstreetmap.org/copyright
Source extracts provided by Geofabrik: https://download.geofabrik.de/technical.html

This is a **regional release**, not worldwide road coverage. The 36 explicit cells cover
longitude 139.68–139.80, latitude 35.60–35.72 (Tokyo centre). Requested generation bounds
were 139.70,35.62,139.80,35.72; conservative outward cell rounding accounts for the extra
western/southern cells. Cells outside the manifest are unavailable, not empty.

Source: https://download.geofabrik.de/asia/japan/kanto-261114.osm.pbf
Snapshot timestamp: 2026-11-14T21:20:41Z
Parent PBF SHA-256: 29967275fa5925a5dc224708e41869800471745d53db004b25104c6eddca52dc
The matching https://download.geofabrik.de/asia/japan/kanto.poly was retrieved in the same
publication window; selected outer ring 3 wholly contains all emitted cells. It has no
hole sections. The complete .poly digest and selected ring are recorded in manifest.json.

Generation: scripts/road-pack/from_pbf.py, pyosmium 4.3.2, explicit operator budgets
600000000 input bytes / 100000000 source nodes, file-backed sparse index. The source had
66893437 nodes. Selected roads: 35711 ways, 144640 distinct nodes. Cell files total
15337933 bytes. Geometry, original OSM IDs/node order and structure tags are preserved;
no line simplification, merging or height inference is performed.

`source.file_sha256`/`xml_sha256` identify the intermediate selected XML;
`source.parent_pbf_sha256` identifies the original linked PBF and is used as the reference
bundle source hash. `extraction_bounds`, source polygon, selected highway classes, cell
hashes and counts make the exact dataset contract explicit. It means complete selection
from this OSM snapshot, not a guarantee that OSM maps every real road.

The manifest is pinned by src/roadPackRelease.ts. The app verifies all requested cell
hashes before publishing coverage. Road/manifest failure never falls back to another
public server. Future regions require a verified release; users do not manually import.
