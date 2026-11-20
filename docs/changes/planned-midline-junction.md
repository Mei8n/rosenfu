# Planned midline junction checkpoint

A click near an owned, level-matching built line can start a planned branch at its actual projected sample-segment fraction, not at a nearby unrelated anchor. The first outgoing direction selects the source tangent direction. Source geometry is sent as an expected snapshot; construction uses transactional `build_plan` and does not modify the built source while planning.

Saved plans retain the junction source. Source-anchor movement and partial deletion of such plans are blocked until they can preserve connection intent; whole plan deletion is supported. Sloped source lines are not eligible yet. Three-port connection components get a point marker with keyboard/mouse informational selection; this does not claim a switch-operation/train-routing UI.

Requires the pending arbitrary-fraction/reverse junction backend. Unit projection/direction tests pass. App compilation was interrupted with exit 137 under shared load and must be rerun. Integrated App tests, backend compatibility and native GUI are pending. Do not treat this checkpoint as complete or merge it without verification.
