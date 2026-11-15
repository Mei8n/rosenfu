# Built anchor height selection

In the construction Selection tab, owned built alignments expose anchor handles.
Click or keyboard-select a handle to edit its height with the upper height buttons.
The toolbar explicitly says 建設済み, separate from the planned anchor editor.
Actual height is taken from section profiles (or the existing Hermite profile),
not rounded to a nominal structure label. Non-level heights have no pressed button.

The request includes alignment ID, anchor ordinal, requested level, exact expected
alignment snapshot and construction plans. Backend rejection leaves the selection
available and shows its reason. Success adopts the returned world, clears stale
selection and registers one world Undo with replayable Redo. No save file is written.
Connected endpoints and stations remain protected by backend validation.

Requires the backend update_alignment_anchor_height command and PR124. Focused UI
mock tests and TypeScript pass; native backend integration, independent review and
3 OS CI / native GUI have not yet been verified.
