# Built endpoint connections (work in progress)

This branch is not ready for integration or user testing.

## Verified cause
The UI only offers endpoint candidates before drawing or after finishing.
ConstructionPath/PlannedAlignment record a source extension, not a destination.
Core extend_alignment only appends/prepends one Alignment.
Route planning builds a station graph from adjacent stations within each Alignment.
Coincident rendered endpoints therefore do not provide cross-alignment connectivity.

## Model decision
Retain both Alignment IDs, names, owner, track type, design speed, geometry,
station placements, provenance, route paths and service legs.
Add an explicit pair of AlignmentEndpointRef values.
Never merge B into A or infer a connection merely from coordinate coincidence.
Each endpoint may belong to at most one pair: this feature does not create midline
junctions. Free endpoints may form a cycle, but repeated-station route rules remain.
A connection requires exact coordinates, equal owners/types and opposite inward
tangents within 5 degrees (same sampled-tangent tolerance as existing extensions).
Different speed caps remain distinct; a connecting segment must not overwrite them.
Existing occupied endpoints cannot subsequently be extended away from their link.

## Next implementation stages
1. Add an explicit cross-alignment corridor RouteSegment carrying its ordered
   endpoint links and station endpoints. Resolve it using persisted links, not
   preview geometry. Extend graph search to walk degree-two endpoint chains,
   including intervening alignments with no stations. Bound traversal and detect
   cycles. Preserve all existing RouteSegment and ServiceLeg keys.
2. Saved plan destination endpoint: persist for active and parked paths; detach
   deliberately when dragging its anchor away, never retain stale connectivity.
   Validate destination ownership, availability, geometry and current coordinates
   again at construction time. Commit extension and connection in one candidate.
3. Draw-time destination snap: distinct from resume-source selection. Snapping
   alone must not be reported as built connectivity; exact last point plus tangent
   feasibility determines whether construction can be committed.
4. Deletion must reject links involving the alignment (including plan references).
   Undo must atomically restore topology with geometry and preserve later edits.
5. Save schema 14, backward reads through schema 13, cross-endpoint route/service
   roundtrips, all four orientation combinations, invalid joins, late stale target,
   stations, speed caps, provenance and batch rollback.

## Current milestone
Only the core endpoint pair and validation API have been implemented. There is no
UI connect action or cross-alignment route traversal yet. Do not present this
milestone as a fixed connection bug.
