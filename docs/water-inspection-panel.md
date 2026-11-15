# Minimal debug panel: integration owner handoff

`WaterEvidenceInspector` is a standalone, no-props React panel. It loads only the
three fixed, pinned trial cells (Fuji, Kanmon, Takamatsu) after a user click. It
shows source water-area polygons (including holes) and waterway-line records;
line stroke width is a display symbol, never a derived river width. SVG display
is clipped to the requested query window; original source geometry is unchanged. White is not
certified land. It shows source times, unknown-width counts, unlocated source
issues and OSM attribution. Every state distinguishes inspection from construction.

This is a useful morning data/debug view, not the pending route-intersection
preview. No IPC, world state, current route, assessment, EnvironmentSnapshot or
build callback is used. There is no ready/coverage promotion. Failure, refresh,
cell changes, cancel and unmount remove or suppress stale results.

## Exact shared integration surface (not edited by this branch)

1. In the App-owned inspector/sidebar area, import
   `WaterEvidenceInspector` from `./WaterEvidenceInspector` and mount
   `<WaterEvidenceInspector />` as an independent debug section. Keep it outside
   any code that computes `environmentReady`, `planBuildable`, build requests or
   saved environment. Do not label its result a current-route assessment.
2. In the owner-controlled `src-tauri/tauri.conf.json` connect-src list, allow only:
   `https://raw.githubusercontent.com/Mei8n/rosenfu/5b647a06d15605eceb2f9235bec6bc7e59ec0bb1/data/water-inspection-trial-20261114/`
   This exact directory was tested in cloud WebKit; main/other commits remain
   blocked. Do not disable CSP or allow the entire host. Retain existing sources.
3. Owner must verify the mounted panel in the actual packaged app and its origin,
   CSP, platform WebView and existing CSS. This branch tests the isolated panel.

New owned files are the component, its CSS/test, this document and a dedicated
three-OS UI workflow. App, shared CSP, session and physical engine are untouched.

## Remaining actual construction-preview boundary

The read-only water-intersection preview needs separate owner work: preserve
per-polygon query scope without making it certified coverage, do not merge saved
coverage, validate routes, and explicitly keep water_ready/buildable false. Use
the existing Hermite/profile engine, not JavaScript physics. Current
combineCorridor and preview_environment are not safe drop-in observation adapters.
Even that preview cannot certify absent water or enable construction with the
current incomplete data. The national query generator's eight-region input is
not nationwide published data or coverage. Public service remains three cells.
