# Three-window observed water mask trial

© OpenStreetMap contributors. This derived database is available under the
[Open Database License 1.0](https://opendatacommons.org/licenses/odbl/1-0/), separately
from the code license. Preserve source attribution and license metadata in each
manifest. Applications displaying the data must provide
[OpenStreetMap attribution](https://www.openstreetmap.org/copyright).

Only Fuji, Kanmon and Takamatsu windows are included. These are 25m cell-centre
observations, not national coverage or construction clearance. Empty cells remain
Unknown. Width-unknown streams and uncertain height/covered/basin areas are not
converted into water. Source issues and different source dates remain unresolved.

Sources: Geofabrik Japan regional OSM extracts dated 2026-11-14T21:20:41Z and
OSM water polygons dated 2026-11-13T15:00:00Z. See each manifest for original URLs,
source hashes, attribution, exclusions and issues. The editable input database is
[published at fixed revision 5b647a06](https://github.com/Mei8n/rosenfu/tree/5b647a06d15605eceb2f9235bec6bc7e59ec0bb1/data/water-inspection-trial-20261114).
Use scripts/water-mask/build.py with the composite hash in each manifest to
reproduce the raster. Each water.bin contains u32 little-endian row offsets then
u16 paired column edges; manifest payload.row_bytes marks the split. No technical
restriction or additional license is imposed on these files.

Use a commit-pinned HTTPS URL and independently pinned catalog length/SHA256.
The catalog registers observations_only masks; never promote these to
complete_model or interpret an absent file as dry. Small GitHub trial hosting does
not promise national availability or production performance. No App integration
or build permission is enabled by this dataset.
