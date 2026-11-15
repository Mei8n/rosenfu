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

Native `preview_construction_plan` now receives the same complete requests used by
build. The active cursor proposal includes its sampled geometry, anchor indices,
heights, destination and recalculated planned-station positions. A response only
applies to its exact world and request;
stale results are ignored. The
preview never mutates the world; build performs authoritative validation again.

This checkpoint awaits multi-profile backend integration and independent review.
Active and parked anchor edits, Undo/Redo, save/reload, plan-switch and start-reversal
regressions pass. Explicit array deletion needs further focused boundary tests.
Native GUI and 3 OS CI are not verified. No train operation or financial effect
is implied by changing height.

## 保存形式の統合ゲート

アンカー区間別の `section_profiles` と計画高さ配列を含む一体候補は schema21 専用とする。旧1〜20はデコード前に拒否し、元のmanual/autosave/backupを変更しない。PR123までのschema20版と混同しない。この候補の3 OS CI・GUI受入は別途必要。
