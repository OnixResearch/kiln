## Decisions

The snapshot is parsed as bounded JSON and cross-checked against its `changes`
array: selection digest, readiness, gates, unit presence and requirement
references must agree. A forged selection therefore cannot create work.

The caller supplies the monotonic sequence because Cairn snapshots carry
eligibility and a digest, not an ordered revision. A higher sequence with a
changed digest supersedes the same unit; the canonical key excludes the source
version so edits group and supersede rather than creating parallel work.

Requirement references become acceptance obligations with an explicit statement
that the referenced requirement is satisfied. Operator policy supplies the
remaining acceptance, paths, effects, verification profile, budget and terminal
policy.

## Non-claims

Cairn eligibility is not implementation completion, execution permission or
acceptance. Intake does not claim, dispatch, verify, publish or mutate Cairn.
