# Proposal: Update the Mantle build cohort

## Why

Kiln's Mantle adapter admits one exact published cohort. The current pin
`0bbab77e23ab95caa4e4184b9945cfaa5c8040e2` predates the newest revision that
builds its own package by hundreds of commits. The newer `main` revisions cannot
be admitted:

- `43ccd68840b748e03773fc1bff5eb1cfca99dffa` (main tip) defines the same
  `#[test]` function twice in `tests/integration_build.rs` (lines 1408 and
  1509), so its check phase cannot compile;
- `d20459c6db6d60ed2b08533911e279cbeadb742e` omits
  `crates/crunch-build/audit/finish_dlopen_audit.c` from its own flake source
  filter, so its dependency build fails on the native loader-audit module.

The selected revision `d75158b00b080535f2ffea12430665ca3d57ea82` is the newest
revision whose canonical `nix build .#default` passes with its own test phase.
Its producer contract and fixtures are byte-identical to the current pin. The
adapter's wire contract is unchanged; the executable identity and the recorded
source revision no longer describe the current producer.

A cohort refresh keeps the adapter's exact-admission behavior while binding the
selected published source and its rebuilt executable. It must not silently widen
the admitted contract, and it must not substitute a build with its checks
disabled for a canonically verified one.

## What Changes

- Bind the adapter, typed profile, stack profile, vendored fixture record, Nix
  cohort check, and documentation to one new exact Mantle revision. r[kiln.mantle.cohort_refresh]
- Rebuild the pinned executable from a clean export of that revision and record
  its exact path and BLAKE3 identity; the plan, policy, worker, and store
  identities stay unchanged because the producer contract bytes are identical. r[kiln.mantle.cohort_refresh.identity]
- Prove that the producer contract and fixture bytes did not change, so the
  adapter's request, observation, receipt, and native report schemas remain the
  admitted ones. r[kiln.mantle.cohort_refresh.contract]
- Reject mixed cohorts: a profile whose revision, executable identity, fixture
  revision, or Nix cohort value disagrees with the others must fail admission or
  the cohort check. r[kiln.mantle.cohort_refresh.drift]
- Record the refresh in the adapter documentation, reference pins, license map,
  and importer pin without rewriting historical release or publication receipts. r[kiln.mantle.cohort_refresh.records]

## Impact

- **Core**: no change. Build-plan admission and observation admission are
  revision-agnostic and remain pure.
- **Adapter**: constant and profile-value changes only; dispatch, completion,
  reconciliation, and log behavior are unchanged.
- **Nix**: the `mantle-build-cohort` check binds the new revision and re-verifies
  the vendored contract bytes and fixture identities.
- **Claims**: a refreshed cohort proves bounded compatibility with one published
  producer revision, not build correctness, sandbox completeness, cache truth,
  reproducibility, or release readiness.

## Out of Scope

- Adopting new Mantle surfaces such as the evaluation stream, remote builders, or
  `mantle.build-*` renames; the compatibility report surface is retained.
- Changing the adapter's schemas, bounds, outcome classes, or non-claims.
- Publishing a new executable, changing Mantle governance, or rewriting
  historical receipts.
- Any claim about the producer's uncommitted working tree.
