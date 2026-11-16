# Shared OFM evidence draft v2 (isolated, not save-ready)

`prepareSharedOfmEvidenceDraft` adapts the independently validated draft v1 into:

- `persistentProposal.newRawEvidence`: SHA-256 addressed raw PBF bytes, canonical RFC4648 base64 and exact byte length. One content value may have multiple tile bindings. Existing SHA hints omit transmission only; the receiver must resolve and verify a matching stored value or reject.
- `tileBindings`: canonical provider URL, revision and z/x/y, raw SHA, decoder rule, license and attribution. Binding identity is the canonical URL; identical raw SHA does **not** imply identical geographic tiles. Reuse must compare all immutable binding fields, never replace an existing binding silently.
- `alignment`: ordered fragment IDs and fractional source spans, join algorithm and ranges, start/end picks, complete required binding keys. No original coordinates, graph JSON, raw bytes, derived points or full decoded context are stored per alignment.
- `temporaryValidationContext`: full graph JSON and decoded tile lines solely for comparison with backend re-decoding. Discard after validation; never persist in World. Backend decoded raw evidence is authoritative, not this caller-provided context. It can eventually replace this redundant IPC field entirely.

This proposal is **not connected** to App, IPC, World or storage. `persistentProposal` means a proposed data representation, not authorization to write it or proof it has passed a backend decoder. The current raw bytes fixtures are deliberately opaque and do not demonstrate MVT validity. Existing v1 hashes prove byte integrity but not completeness or correspondence of decoded context. Rust must independently decode the complete relevant tiles, re-run candidate uniqueness and regenerate geometry, then validate curvature/height/grade atomically before accepting an alignment. Branching competitors cannot be dropped from candidate context.

## Limits

The v1 adapter enforces raw input <=8MiB including evidence requested for reuse, <=5MiB per tile, <=16 tiles, decoded point/line limits and snapshot JSON <=16MiB. The v2 envelope also has a 16MiB IPC JSON limit. Base64 has exact `4*ceil(bytes/3)` length, strict alphabet/padding and re-encoding equality, bounding allocation and rejecting nonzero unused padding bits. Digest comparison happens in v1 before conversion; the exported base64 decoder alone is not a digest verifier.

`assertProposedWorldBudget` is a separate exact UTF-8 JSON size check for the **entire proposed World** at 16MiB; consumers must include existing evidence, all alignments and unrelated saved data. A small incremental payload is not proof that a World will fit. No caller/storage integration exists yet. The backend must enforce its own limits before allocation. A reused SHA is not permission to omit source verification after load.

Two observed dense Tokyo tiles total 1,509,801 raw bytes (~2.01MB base64); the previous full-context envelope was ~11.27MB. Most of that is temporary decoded graph/context and selected source-coordinate duplication, not necessary stored bytes. v2 removes decoded context and source coordinates from its persistent proposal and deduplicates raw bytes by SHA across alignments. A larger World can still exhaust 16MiB. Compression or an external content-addressed source store with atomic lifecycle/recovery requires a separate design; limits are not raised here.

## Backend contract coordination

The isolated Rust decoder takes raw bytes plus canonical URL/revision/z/x/y/raw SHA/decoder rule. It will produce canonical decoded content hashes from domain-separated, length-prefixed typed integer-coordinate geometry and raw attributes. JavaScript graph JSON hashes are **not** substitutes for that canonical digest. The draft retains a decoder rule name but does not invent a backend canonical hash. Introduce that digest only after independently verified decoder parity. Coordinate binding aliases and same-SHA tile reuse explicitly.

`verifyRawTileEvidence` separately verifies SHA-256 after canonical decoding. `reconcileSharedEvidence` snapshots inputs, rehashes unique evidence, rejects same-SHA differing content and same-URL conflicting bindings, resolves bindings only to present raw evidence, and returns a new table without mutating existing state. It does not validate MVT semantics or prove a successful save. The whole-world budget check remains mandatory after reconciliation, not just a subtraction of an estimated evidence increment.
