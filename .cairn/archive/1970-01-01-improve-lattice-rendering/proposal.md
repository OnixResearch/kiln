## Why

The Pipeline view renders Lattice nodes as identical small boxes with no stage labels or status distinction, repeats them in an undifferentiated list, and buries dependency navigation in a raw facts block. Fixed card dimensions can overlap long labels. The existing cyclic-input relaxation draws an invalid graph as if it were a usable DAG.

## What Changes

Improve the existing workflow view with dependency stages, textual status/color badges, recorded-state counts, fit/zoom controls, selected-node relationship highlighting and a readable node inspector with prerequisite/dependent navigation and exact-node log links. Preserve a complete accessible node list, exact URL selection and scroll/focus across live refreshes. Missing or malformed dependency data stays explicit and does not invent topology or execution state.

## Impact

Kiln dashboard maintainers own this presentation for operators inspecting Lattice-backed runs. Use only the existing provider-neutral run/node/edge/timing fields; no provider API, schema, backend execution, authority or deployment changes. Current live runs have one node each; branching, long-label and negative cases require clearly identified browser fixtures.
