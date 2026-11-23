## Why

Kiln #49's deterministic scripted-worker VM cannot build. Its fixture and
`kiln-cairn-intake` need `cairn implementation-ready`, which first appears on
Cairn `main` at `ba68144`. Kiln imports the gate contract at `15f00875` and
`cairn-core` at `8f6cd0b3`, so no single Cairn revision satisfies Kiln's
imports, Lattice's lock and the intake together.

Cairn `main` is now `9082e1aece4f04bcbead5eeea087e496bc48a675`. It has
`implementation-ready`, consumer-selected release gates and fail-closed refusal
of malformed gate lists. Lattice moves its `cairn` input to that revision.

## What Changes

- Re-import `vendor/cairn` (gate contract, producer fixtures and Nickel
  contract) and `vendor/cairn-verifier` (`cairn-core` and its generated
  policies) at `9082e1ae` through `scripts/import-pinned-source.sh`. Update both
  source records and importer pins together. The importer's `cairn` pin had a
  stale digest copied from another entry and now carries the record's digest.
- `vendor/cairn-kiln-core` stays at `5d99f4e9`, which `9082e1ae` still pins.
- A `9082e1ae` `cairn` refuses Kiln's generated policy because
  `generated_agent_workflow_policy` lacks the new required `command_catalog`
  and `entry` fields. Re-import `cairn-policy/upstream-default.ncl` and
  `cairn-policy/contracts.ncl` at `9082e1ae` through
  `scripts/import-cairn-policy.sh` and re-export
  `cairn-policy/generated/cairn-policy.json`. Only
  `generated_agent_workflow_policy` changes: it gains the command catalog, the
  entry guide, the `readiness` command class, and readiness evidence in the
  apply, archive, drain and verify descriptors.
- Rebind `fixtures/agent-completion/{contract,results}.json` to the new
  policy hash, as earlier policy changes did, so the release-readiness MCP
  smoke keeps validating them without `verification_contract.policy_drift`.
- The gate provider profile keeps its reviewed `15f00875` executable, policy
  identity and gate-machine fixtures. `cairn-gate-cohort` now requires the
  `9082e1ae` import and checks that its producer fixtures and Nickel contract
  are byte-identical to the frozen `15f00875` copies.
- `Cargo.lock` records `cairn-gate-contract` 0.2.1 (was 0.1.0).
- Reference pins, the Cairn gate guide, the policy provenance, the license map,
  README and changelog name the new revisions and the split between the
  imported revision and the gate executable.

## Impact

- **Consumers:** `kiln-adapter-cairn` (gate lowering and admission,
  `cairn.selected-work.v1` admission, workflow obligations, authoritative
  completion), every crate that links it, and maintainers running Cairn
  against Kiln's `kiln-default` policy.
- **Behavior:** Kiln's generated agent workflow guidance now routes through
  `cairn readiness`. Library code paths that Kiln calls are unchanged: the gate
  contract changed only module layout and lint annotations, and `cairn-core`
  changed only convergence, delta-merge and assurance-target code that Kiln
  does not call.
- **Authority:** none. No executable profile, remote or publication changes.

## Specification Effect

Accepted Kiln specifications do not change. The change re-imports pinned
producer sources and regenerates a producer-shaped policy section; it adds no
Kiln requirement and changes no accepted requirement's meaning or stable ID.
If adapting Kiln needs a normative change, this classification stops and a
delta-spec package replaces it.

## Out of Scope

Moving the gate executable profile to a `9082e1ae` binary, pinning onix-core
and running the scripted VM (Kiln #49), and the Lattice input bump.
