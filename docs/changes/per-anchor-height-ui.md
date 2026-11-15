# Per-anchor height editor checkpoint

The editor stores explicit `draftAnchorLevels` / `redoAnchorLevels`, and each
parked path carries `anchorLevels` / `redoAnchorLevels`. Values follow their
corresponding anchor / redo point order. Build requests include `anchorLevels`
with `anchorIndices`; sampled polyline points are not mistaken for anchors.

Placing an anchor captures the currently selected height. In selection mode,
clicking a planned anchor exposes the upper height control. Connected endpoints
are locked; the backend remains responsible for their actual height and grade,
which may be between nominal levels. Changing a height uses anchor Undo/Redo.
Explicit malformed height array lengths are rejected rather than silently fitted.

This checkpoint is not ready for integration: native preview_construction_plan
connection and multi-profile backend integration are pending. Parked-anchor editing
history, plan switching and explicit array deletion need further focused tests.
Native GUI and 3 OS CI are not verified. No train operation or financial effect
is implied by changing height.
