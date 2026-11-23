## Why

Kiln #77, #78 and #79 have their pure contracts (build-input plans, materialization and pinned overlays, captured writeback), but no projected workspace has run anywhere except an example harness. Nothing derives what a restricted runtime may read and write from a pinned overlay, `kiln-lattice-dispatch` accepts any `workspace_root`, `read_paths` and `write_paths` an operator writes, the restricted-runtime negatives of #78 and the "undeclared read cannot escape to the host checkout" criterion of #77 are unqualified, writeback captures a rename as a delete and a create and has no fork, alias or membership outcome, a moved source is only a refusal with no overlap report, and verification has no facts binding a sealed candidate to its build plan and the projects it affects. This is the non-authority half of those issues (plan slice S4); current authority at retrieval and workspace effects stays with S7.

## What Changes

- `kiln.projected-workspace-route.v1` (C9): a pure route derived from the admitted definition, completeness witness, materialization plan and dispatch pin, naming the overlay as workspace root, its top-level entries as read paths, the overlay destinations of writable origins and writable creation destinations as write paths, the build plan and the `lattice-bwrap` runtime, under one identity. The worker materializer prepares an overlay for its route. `kiln-lattice-dispatch` and `kiln-implementation-fixture bind-execution` refuse, before any durable step, a pinned plan without its route, a route for another pin, and a request whose paths or network differ from the route; `bind-execution` emits the route it bound.
- Writeback v2 (C10): `kiln.projection.writeback-candidate.v2` adds declared `Rename` and `Fork` source operations and `AliasChange` and `MembershipChange` view-definition operations that never touch the source, and a `Conflict { base, current, overlapping }` outcome for a moved source instead of sealing; recorded v1 candidates still verify, read-only. The seal shell commits renames and forks.
- `ProjectedCandidateFacts` (C11, `kiln.projected-candidate-facts.v1`): binds a sealed candidate to its manifest, definition, build plan and the projects its changes affect through `monorepo::affected` over the full candidate, with evidence applicable only to the same facts.
- An off-by-default `projected-runtime` example that dispatches the two-project Cargo corpus and a new two-project Nix corpus through `kiln-lattice-dispatch` to the real Lattice binary under bubblewrap, runs each planned build in the restricted runtime, records the confinement negatives and captures the overlay into facts.

## Capabilities

### Modified Capabilities

- `source-projection`: projected workspace routes, the restricted projected runtime, structural writeback operations, writeback conflicts, recorded candidate compatibility and projected candidate facts.

## Impact

`kiln_core::projection::{materialization, writeback, build_inputs}`; `kiln-web-service` worker materializer and writeback shells, `implementation-fixture` and a new example; new `kiln_adapter_lattice::projected_workspace` and one route check in `worker::run`. `capture` takes declarations and returns `Captured`. Plans without a pin keep their behavior and identity.

## Non-goals

Authority at retrieval, workspace effects and per-operation writeback admission (S7, #69/#70), the verifier's handoff of the facts (`CandidateHandoff.projection`, owner decision O5), positive relocation (O6), generated-entry providers, live host confinement (#49) and bucket credentials (#80).
