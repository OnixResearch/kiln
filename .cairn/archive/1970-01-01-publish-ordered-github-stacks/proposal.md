## Why

Issue [#89](https://github.com/OnixResearch/kiln/issues/89) asks Kiln to publish one Cairn change as an ordered stack of small, dependent pull requests instead of one large pull request. The stack must stay off by default. Today `PublicationPlan` binds one candidate, one branch and one pull request, and #48's landing observer reads one pull request. GitHub stacked pull requests became generally available on 6 October 2026, with REST endpoints to list, create, extend and dissolve stacks, and a `stack` object on every stacked pull request. Kiln has no Origin collaboration adapter.

## What Changes

- **Core (pure, `kiln_core::implementation::stack`):**
  - A publication route that defaults to the existing single pull request, keeps a host without stacked pull requests on single pull-request publication, and refuses Origin as unsupported.
  - A layer verification receipt that admits canonical CI for one exact lower-layer candidate under the same rules as #46. The rules are shared, not copied.
  - A stack plan that binds the attempt's handoff and verification, the frozen base and trunk, and 2–16 layers. Each layer binds its exact candidate, tree, parent, verification, deterministic branch and the branch below as its base. The top layer is the verified candidate.
  - A bottom-up gate: a refused or unknown lower layer blocks every layer above it with that same reason.
  - Integration admission for an exact contiguous prefix of layers, plus a stack-level completion flag.
- **GitHub adapter:**
  - A stack capability probe before any effect.
  - Tree and ancestry checks on every layer.
  - Per-layer branch and pull-request publication through the existing #47 functions.
  - Creating, or reconciling once, one exact GitHub stack.
  - Read-only per-layer merge recovery under the exact stacked mapping.
  - A helper that turns per-layer observations into gate inputs.
- **Documentation:** `docs/github-publication.md` and `docs/github-landing-observation.md` document GitHub's stack publication and merge behaviour and the guards Kiln keeps.

## Impact

Consumers: operators and the later Cairn consumers #50, #62, #83 and #84 that may select this route for an already layered change. Owners: Kiln implementation-publication and GitHub adapter maintainers. Files: a new core module, a small refactor that shares the CI verification rules, stack publication and recovery in the existing adapter publication and landing modules with fixtures, the guides, the changelog and a new `implementation-stack` capability spec.

## Non-goals

This change does not merge, rebase, retarget, unstack or land, and it does not replace #47 or #48. It does not decompose a Cairn change into layers or drive a campaign, and it does not gate contributions (#46). It does not wire the route into the operator CLI or persist stack records; those, and live GitHub qualification, remain on #89. It does not require one commit per pull request, and GitHub's 9 percent merged-code and 5 percent time-to-merge claims are not targets.
