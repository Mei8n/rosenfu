# Native road release cache (integration contract)

`src-tauri/src/road_delivery.rs` is independent of the editor and never changes a
world or save. Wire it through Tauri state; `RoadDelivery::read(path)` returns the
requested JSON bytes. No caller-supplied URL, filesystem path or hash is accepted.
The webview may request only `index.json` or a manifest/cell listed by the pinned
catalog. It must still run existing schema, coverage and geometry validations.

Construct the service outside an async runtime (or inside a blocking worker) with
an application-owned cache directory and an optional compiled `Release { tag,
catalog_sha256 }`. **None is the correct setting until data is actually published**.
Do not ship a fabricated release pin. Native initialization uses a blocking HTTP
client; wrap reads with `tauri::async_runtime::spawn_blocking` rather than blocking
the UI. Register a separate cancel command calling `cancel_all()`; cancellation
is checked before/after transfer and during reading/decompression. A blocked socket
may take up to the 30-second request timeout to return; immediate network cancellation
is not implemented. Requests queued behind the serialized cache mutex also cancel.

Only the fixed Mei8n/rosenfu GitHub release URL and HTTPS redirects to GitHub's
`release-assets.githubusercontent.com` host are accepted, with four redirects,
10-second connect/30-second total timeout, no credentials and no Referer forwarding.
No live release has been downloaded in tests because no road release is published.
Tests use an injected transfer function and test the production URL allowlist.

Catalog and index are SHA-pinned, then shard archives are checked by exact size and
SHA. ZIP entry paths, type, counts, advertised/actual lengths, CRC and content hashes
are checked; duplicates/extra files and symlinks are rejected. ZIP entries are never
extracted to disk. Read output is at most 5 MiB, archives at most 64 MiB compressed /
256 MiB expanded. All work is serialized per service. Cancellation/failure never
publishes a partial file. Only verified hash-named blobs are atomically cached.

The app-owned cache is bounded to 512 MiB. Eviction removes only regular files
named `<64 lowercase hex>.data` in that directory, oldest modification first;
it is FIFO-like, **not** true access-time LRU. Other files are untouched. A corrupt
or symlink cache entry errors instead of reading untrusted bytes. Catalog/index
and ZIP data can be reused offline after a successful fetch. The coverage flag
is preserved: successful delivery is not evidence that all roads are mapped.

Local tests use a standalone Rust harness because this cloud environment lacks
GTK/WebKit desktop libraries; normal three-OS CI also includes the module through
`src-tauri/tests/road_delivery.rs`. Main command registration, native GUI interaction,
a published release download, and actual user cancel wiring are separate checks.
