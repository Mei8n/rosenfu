# Range selection checkpoint

- Construction's Selection tool uses left drag to select owned built anchor-to-anchor sections intersecting the screen rectangle. It does not cut geometry at rectangle edges. Planned lines, stations and junctions are not range-operation targets yet.
- Middle-button drag pans the map in every tool; it does not place/select/delete. Map panning uses the normal cursor; deletion keeps the crosshair.
- Batch demolition and infrastructure updates send each full original alignment as an expected snapshot. Backend rejection leaves selection and input intact. All selected sections commit atomically and share one Undo/Redo entry.
- Track count choices are 1/2/4. Height choices are ground, elevated 1/2/3 and underground 1/2/3. No metre values are implied. Mixed values are shown as mixed; a field left at “unchanged” sends null and preserves each section's value.
- Requires batch section backend and section-infrastructure model. No station/plan range operation, lane topology or train operation is claimed. Native GUI testing is still required; focused DOM and geometry tests are not a substitute.
