# Seven-level construction toolbar

The supplied reference image (2026-11-15 09:54:11) shows a compact horizontal +3/+2/+1/0/-1/-2/-3 height row above track type controls, tinted level buttons, a green selected level, and a small hover panel with the level name and cost information.

The construction toolbar now uses that layout. Names follow the agreed structural levels. Hover and keyboard focus expose user-specified construction and maintenance multipliers from `heightCostModel.ts` (one shared definition). These are configuration metadata, not implemented economic deductions. Cost per kilometre explicitly says **未実装**; no reference-image amount is treated as a game base price. No unimplemented keyboard shortcut is advertised.

Existing restrictions on changing infrastructure during an active draft, extension or reference-copy operation are preserved. Range editing is a separate control. Native visual testing remains outstanding; DOM tests check order, names, selection state, tooltip content, focus and disabled actions.
