# Japan-region coarse construction map model

© OpenStreetMap contributors. This derived database is available under the
[ODbL 1.0](https://opendatacommons.org/licenses/odbl/1-0/), separately from the
repository code license. Preserve the source notice and provide
[OpenStreetMap attribution](https://www.openstreetmap.org/copyright) in the application.
The lake input is public-domain Natural Earth data: Made with Natural Earth.

This is a coarse game map model of marine water and large lakes, not a complete
map of real water. Small streams, ponds, narrow channels and small islands can be
missed. Every 1/60 degree cell uses 16 subcell-centre membership samples and is
water if at least 8 samples are water. Split marine polygons are unioned, preserving
island holes. Approximate latitude spacing is 1.85 km, not 25 m.

The supported half-open scope is longitude [122,154), latitude [20,46), containing
Japan and surrounding areas. The TWW1 file uses a global grid container but its
outside-scope zeroes are UNSUPPORTED, NOT LAND. Always use the pinned catalog
scope via loadConstructionWaterAt; never apply this container as a world model.
The catalog has no construction fine masks. The separate three-window observation
trial is not a complete fine model and is not silently promoted or inserted here.

Sources and reproducibility:
- [OSM water polygons](https://osmdata.openstreetmap.de/data/water-polygons.html),
  source date 2026-11-13T15:00:00Z, downloaded ZIP SHA256
  22d97b206fd5ce4b9fc991e561513b5f3468ed1cfea8ae4410982336991623ed.
  [Download](https://osmdata.openstreetmap.de/download/water-polygons-split-4326.zip).
  The live download may change; manifest.json records exact component hashes.
- [Natural Earth 1:10m lakes](https://www.naturalearthdata.com/downloads/10m-physical-vectors/10m-lakes/),
  version 5.0.0, retrieved 2026-11-16. ZIP SHA256
  0803a06f9c3cb4671d89b68c48b142aad9366ba40f665245e12a913fbc61722a.
  [Download](https://naturalearth.s3.amazonaws.com/10m_physical/ne_10m_lakes.zip).
  [Public-domain terms](https://www.naturalearthdata.com/about/terms-of-use/).

The complete distributed derived database is the row-run file here; its format is
publicly documented in docs/water-mask-model.md and the independent decoder.
No encryption or additional restrictions apply. Reproduce using
scripts/water-mask/build_coarse.py with --bounds 122 20 154 46,
--marine-pin bfdd842a9436dff39b1e9565f8c9b62448a7ae3c1cf924428cca8403595210db and
--lakes-pin 0a333a393b604c7c793600358217e48e0b07d734179e6c333c94114284e3de2f.
These pins hash the canonical component inventories in manifest.json, not ZIPs.
Different source dates are retained, not claimed to be a simultaneous snapshot.
This data does not by itself implement track height, the 30 km limit, or build IPC.
