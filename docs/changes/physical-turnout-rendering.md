# Physical lane and turnout rendering

Native worlds with explicit endpoint connections request `preview_track_render_geometry` for one atomic snapshot. The core returns trimmed lane centre lines and curved turnout movements in geographic coordinates using a 4 m gameplay spacing. The UI projects those coordinates at the current zoom instead of mixing screen-pixel-offset straight lines with metre-offset connectors. Reverse movement pairs are rendered once.

Late responses from a previous world are ignored. A query error is shown explicitly; no synthetic Y curve is invented. Original selection/hit geometry remains the alignment centreline. Selection markers are informational, not switch-control simulation. Worlds without explicit connections continue to use schematic parallel rendering. Train dispatch, collision avoidance and dynamic switch operation are not implemented by this rendering change.

Depends on PR118's complete render query. Focused component tests verify exact shared joins across zoom levels, reverse deduplication and stale/error handling; native GUI and integrated backend testing remain required.

Internal track-count changes within a single alignment are not yet lane-topology connections. They must not be presented as completed Y movements. The current Y curves describe explicit connections between separate alignments only.
