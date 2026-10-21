# Qualify Kiln self-CI from its own source

## Why

The qualified canary builds a synthetic `jobs.app` fixture. The remaining
product question is whether Kiln's own source, published through the qualified
remote publisher and observer, can drive the unchanged broker, controller,
worker, Nix build and status path to a real Kiln product. A dedicated fork is
required because the fresh canary identities have no delegate authority over the
canonical repository, and canonical repository or governance mutation is not
authorized.

## What Changes

- Create a dedicated fork RID from the exact Kiln source at a recorded base
  commit. The canonical repository, its `main`, delegates and governance are not
  touched. Fresh canary identities, the declared SSH/tailnet topology and the
  existing retirement rules apply. r[kiln.self_ci.authority]
- Raise the typed Radicle source bounds to cover the full Kiln tree and retain
  the exact base commit, tree size, entry count and fork identity. The canary
  source is the real Kiln tree, not a synthetic flake or `value.txt`.
  r[kiln.self_ci.source]
- Select `packages.x86_64-linux` / `kiln` through the existing discovery and
  build adapters. Success requires the real package product and exact rooted
  output; failure is a source-controlled Rust syntax error with its genuine Nix
  diagnostic. No fixture build, hardcoded `jobs.app` or fabricated output is
  admitted. r[kiln.self_ci.build]
- Exercise duplicate delivery, controller host kill and remote restart, and
  unavailable-provider uncertainty with the same canonical and status rails.
  r[kiln.self_ci.recovery]
- Retain the fork RID, base commit, source bounds, real revisions, products,
  diagnostics, status, retirement and gates. State that this is a fork-based
  self-CI qualification, not canonical publication, production rollout or
  release authority. r[kiln.self_ci.evidence] r[kiln.self_ci.claims]

## Impact

Current consumer and immediate outcome: the trusted canary proves Kiln's CI path
on Kiln's own source and product. Durable capability: a repeatable self-CI rail
over the existing provider, host and status mechanisms.

Maintenance owner: Kiln maintainers own the runner, source bounds, selector and
evidence. The operator owns the host, SSH destination, tailnet, fork root and
cleanup.

Files: the existing live/distinct/status runners with a self-CI exercise and
typed profile; source-bound and selector hooks; positive/negative admission
fixtures; operator documentation and Cairn evidence.

Verification: actual fork creation from the exact base commit, a real Kiln
package build, a genuine Rust syntax failure, duplicate/restart/host-kill and
unavailable cases, exact remote status readback, and all required gates.

## Authority and exclusions

The operator's `begin` request authorizes this dedicated fork, fresh test
identities, a fresh remote `/var/tmp` root, source pushes confined to the fork's
`ci-canary`, status refs confined to the fork RID, and the pinned read-only
status reconcile on the remote observer. It does not authorize any change to the
canonical Kiln repository, its `main`, delegates, threshold, visibility,
mirrors, governance, existing Radicle homes or production services, nor
activation of built outputs or release approval. No untrusted repository is
admitted.
