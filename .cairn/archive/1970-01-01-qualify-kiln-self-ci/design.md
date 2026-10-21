# Design: fork-based Kiln self-CI

## Context

Reuse the qualified distinct-host publisher and remote status observer. Replace
only the synthetic source/build selection with the exact Kiln source and the
Kiln product. The canonical repository cannot be used: fresh canary identities
are not delegates, and adding a delegate would mutate governance. A dedicated
fork created from the exact local Kiln commit preserves the source content and
build behavior while keeping all effects inside canary-owned state.

## Decisions

### Decision: Dedicated fork, not the canonical repository

**Choice:** Clone the local Kiln Git repository at a recorded base commit, create
a fresh Radicle repository from that clone under the canary publisher identity,
and run all source/status refs in the fork. Never push to the canonical RID.

**Rationale:** The canonical repo's delegates, threshold and governance are
operator authority. A fork proves the same source-to-product path without
claiming canonical publication. The evidence must call it a fork-based self-CI
qualification.

### Decision: Exact base commit and full-source bounds

**Choice:** The clone is detached at the recorded Kiln commit with no
uncommitted working-copy content. The Radicle source selection raises only
`max_entries`, `max_file_bytes` and `max_total_bytes` to typed values sufficient
for the measured tree (for example 8192 entries, 4 MiB per file and 64 MiB
total); identity, reference and path bounds stay unchanged. The measured entry
count, total bytes, largest file and base commit are retained.

**Rationale:** The synthetic bounds (512 entries, 16 MiB) reject Kiln's 4133-file,
roughly 39 MiB tree. The raised bounds remain finite, are admitted by the typed
profile and are exercised by the real source acquisition. Cloning committed
history only prevents accidental publication of local changes.

### Decision: Real package selection and product check

**Choice:** Set the existing discovery root selector to
`packages.x86_64-linux` and the selected task to `kiln`. Success requires the
canonical run to succeed, a rooted build output whose target contains the
executable `bin/kiln`, exact root discharge and a matching remote status.
Failure injects a source-controlled Rust syntax error and requires the genuine
Nix compiler diagnostic; the unavailable case remains nonterminal.

**Rationale:** The existing evaluation/build adapters already accept arbitrary
supported selectors and tasks. A docs-only success commit changes the real
source revision; a syntax error in `kiln-provider-api` forces a real package
build failure. No synthetic `jobs.app`, `value.txt` or output-content check is
used.

**Alias note:** the Kiln flake exposes `packages.default` as an alias of
`packages.kiln`. The pure discovery mapper admits distinct attributes that
share one derivation and still rejects duplicate attributes and dependency
cycles; the selected `kiln` task resolves uniquely. Without this admission the
`packages.x86_64-linux` selector is unreachable, as the rejected attempt
recorded.

### Decision: Tailored case loop over the qualified rails

**Choice:** Keep the qualified broker, controller, worker, Aspen/Lattice,
publisher/observer placement and status readback. Replace the synthetic case
loop with real source commits and product/root checks. The success case retains
the controller host kill/reconcile and the remote publisher restart; duplicate
fetch and the two-invocation assertion remain.

**Rationale:** The source/build boundary is the only synthetic part. Reusing the
rest preserves the qualified recovery and status evidence while changing the
source content and selected derivation.

### Decision: Source commits are made on the remote publisher

The local clone supplies the fork's initial history. Each case writes a small
docs or Rust change into the remote source worktree, commits it on `ci-canary`
and pushes it to the fork. The local worktree remains at the base commit; the
expected product is identified from the actual build root and the worker result,
not a precomputed synthetic output.

## Risks / Trade-offs

- The Kiln package build is much heavier than the synthetic sleep. Dependencies
  must already be in the local Nix store; the existing build deadline and memory
  bounds must admit the build. A timeout is an explicit prerequisite failure,
  not permission to substitute a lighter fixture.
- The source snapshot is roughly 39 MiB and 4133 entries. The raised bounds must
  cover it with margin; exceeding a bound fails closed.
- The fork RID and fresh identities prove the path, not canonical publication,
  delegate authority or governance.
- The publisher host and observer host are trusted canary machines; this does
  not prove hostile-node isolation, WAN behavior or production rollout.
- The remote coordinator status binary must match the exercised source and be
  present on the observer host.

## Evidence and lifecycle

Retain the base commit, clone identity, fork RID, measured source shape, raised
bounds, selector/task, real source revisions, broker ingress, canonical
journals, build products and compiler diagnostics, remote status readback,
duplicate/restart/host-kill observations, root discharge, retirement and gates.
Exclude private keys and seed values. Gate proposal, design and tasks before
implementation; synchronize and archive only after the actual self-CI
prerequisites pass.

Kiln maintainers own the repeatable self-CI rail. The operator owns the host,
SSH destination, tailnet, fork state and cleanup.
