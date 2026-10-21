# Compose Radicle source-to-build CI

## Why

The maintained OnixOS pilot proves authenticated full-closure construction but starts from a signed operator request and separately supplied immutable consumer. Radicle trigger intake, verified local source acquisition and status publication already have separate authority profiles. Their current source adapter returns a manifest rather than a worker checkout; their status projection carries run/outcome but no exact source revision. Connecting only the existing commands would therefore not establish source-event-to-build-to-revision-bound-status CI.

## What Changes

- Admit one configured Radicle repository, signer namespace and branch; capture exact signed refs once and materialize only the verified event revision under the existing bounds. r[kiln.radicle_ci.source]
- Add a narrow composition shell and a project mode in the existing worker. A fixed reviewed workflow receives admitted source context; its worker acquires and freezes that exact source, runs the existing Nix discovery/build adapters and retains a source-bound result. No per-event operator signature or generated workflow operation is required. r[kiln.radicle_ci.runtime]
- Reserve canonical CI start and native ingress before dispatch; complete only from matching actual source/worker and native-host observations. Preserve original-operation recovery without normal redispatch. r[kiln.radicle_ci.journal] r[kiln.radicle_ci.recovery]
- Extend the dedicated status contract with the exact revision and persist a publication intent/observation separately from canonical CI. Unknown publication is reconciled, not blindly repeated. r[kiln.radicle_ci.status]
- Retain real producer/worker commands, positive and negative native observations, exact identities, operator procedures and explicit authority limits. r[kiln.radicle_ci.evidence]

## Impact

Current consumer: one reviewed Radicle-hosted Nix project, using the maintained Aspen/Lattice runtime and existing rooted worker. Kiln maintainers own composition, admission and the repeatable proof rail. OnixOS retains service deployment ownership; operators retain real repository, signer, credential and mutation authority.

Immediate outcome: an actual source event drives canonical CI over its acquired source and a revision-bound status projection. Durable capability: a fixed per-project workflow can process new exact revisions without rewriting its operation or forging an operator request.

Files: the Radicle source/status shell modules and contracts, a small `kiln-radicle-ci` composition package, the existing worker, Nickel configuration, Nix packaging/checks, deterministic behavioral regressions, operator docs and native lifecycle evidence.

## Out of Scope

No production repository initialization/signing, push, synchronization, announcement, delegate/threshold change or mirror publication. No activation, production service rollout, merge/release authority, alternate executor, scheduler or generic pipeline framework. Native qualification uses isolated owned state and fixture-only credentials; it must distinguish genuine producer execution from synthetic protocol fixtures and may not relabel unavailable live evidence as a pass.
