# Height-transition editor checkpoint

The first manually placed anchor captures `startLevel`; the toolbar can then change the ending infrastructure level. This is preserved in active/parked plans, save/load payloads and construction requests. Plan-direction reversal swaps endpoint levels along with its existing geometry Undo entry. Existing plans without a start level remain horizontal at their configured level.

Backend grade generation and validation are required before integration. Metre heights, grade limits, placement and endpoint compatibility are not simulated by this UI. Planned ramp subsection deletion is explicitly blocked until its vertical profile can be split without changing the surviving elevation; whole-plan deletion remains available. This is not a completed arbitrary junction UI.
