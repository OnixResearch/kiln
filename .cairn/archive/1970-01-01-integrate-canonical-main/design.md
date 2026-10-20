## Context

GitHub main `1867f611a7968211e29830f4f1a6d5c4ff568138` contains the checked GitHub CI/public-input work. Canonical Radicle main `5d99f4e9bb954743443e340baece31256b4c53f1` additionally contains Aspen provider and hosting functionality. Neither tip contains the other. The operator chose preservation through integration. Lattice main is independently published at `6339864d3c4e5de72ba4f0899b11b5d5b76704b6`.

## Decisions

### Decision: Preserve both parent histories and capabilities

**Choice:** Create an explicit merge descendant of both exact tips. Resolve conflicts by retaining both supported provider paths and their tests rather than selecting one entire tree. Keep core decisions provider-neutral and external effects in existing shells.

**Rationale:** A non-fast-forward replacement would discard canonical-only functionality from main. A merge preserves ancestry and permits guarded fast-forward publication.

### Decision: Preserve accepted semantics while reconciling source

**Choice:** Treat the accepted requirements in both parents as the review input. Preserve stable IDs, requirement/scenario meaning, capability admission and non-claims. No-spec classification covers integration of existing contracts, not authority to invent or weaken them. Reclassify before a necessary normative change.

**Rationale:** Restoring a canonical capability is not permission to hide it, weaken checks or silently accept a new contract.

### Decision: Validate the integrated candidate before publication

**Choice:** Reuse repository-owned import/generation paths; do not hand-edit imported snapshots or lockfiles. Run the integrated workspace, provider, policy, traceability, lifecycle and Nix gates and actual local shell smokes. Bind evidence to exact candidate, archive and hosting inputs. Publish only the selected main refs and independently read back their exact revisions.

**Rationale:** Results for either pre-merge tree do not validate an integrated tree. Source-ref success is not live CI deployment or distributed availability.

## Risks / Trade-offs

- Shared core/provider interfaces or dependency cohorts can conflict; preserve both consumers and reject unsupported requests before effects.
- Historical receipts remain bound to historical revisions; retain them unchanged and create scoped new observations.
- Remote main may advance during integration; re-fetch and require fast-forward ancestry rather than forcing publication.
- Local Radicle storage acceptance is not replica availability. Record missing seeds or acquisition evidence explicitly.
- Operator credentials and production isolation remain outside this source integration.
