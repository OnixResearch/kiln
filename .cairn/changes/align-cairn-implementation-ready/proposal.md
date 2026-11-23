## Why

Kiln #49's deterministic scripted-worker VM cannot build. Its fixture and
`kiln-cairn-intake` need `cairn implementation-ready`, which first appears on
Cairn `main` at `ba68144`. Kiln imports the gate contract at `15f00875` and
`cairn-core` at `8f6cd0b3`, so no single Cairn revision satisfies Kiln's
imports, Lattice's lock and the intake together.

Cairn `main` is now `317d3ff648f01a5ea1572e6126f4dd71bc7afa99`. It has
`implementation-ready`, consumer-selected release gates, fail-closed refusal of
malformed gate lists, and files `cairn readiness` under the `readiness` command
class. Lattice moves its `cairn` input to that revision.

## What Changes

- Re-import `vendor/cairn` (gate contract, producer fixtures and Nickel
  contract) and `vendor/cairn-verifier` (`cairn-core` and its generated
  policies) at `317d3ff6` through `scripts/import-pinned-source.sh`. Update both
  source records and importer pins together. The importer's `cairn` pin had a
  stale digest copied from another entry and now carries the record's digest.
- `vendor/cairn-kiln-core` stays at `5d99f4e9`, which `317d3ff6` still pins.
- A current `cairn` refuses Kiln's generated policy because
  `generated_agent_workflow_policy` lacks the new required `command_catalog`
  and `entry` fields. Re-import `cairn-policy/upstream-default.ncl` and
  `cairn-policy/contracts.ncl` at `317d3ff6` through
  `scripts/import-cairn-policy.sh` and re-export
  `cairn-policy/generated/cairn-policy.json`. Only
  `generated_agent_workflow_policy` changes: it gains the command catalog
  (with `readiness` and `drain-plan-readiness` in the `readiness` class), the
  entry guide, the `readiness` command class, and readiness evidence in the
  apply, archive, drain and verify descriptors.
- Rebind `fixtures/agent-completion/{contract,results}.json` to the new
  policy hash, as earlier policy changes did, so the release-readiness MCP
  smoke keeps validating them without `verification_contract.policy_drift`.
- The gate provider profile keeps its reviewed `15f00875` executable, policy
  identity and gate-machine fixtures. `cairn-gate-cohort` now requires the
  `317d3ff6` import and checks that its producer fixtures and Nickel contract
  are byte-identical to the frozen `15f00875` copies.
- `Cargo.lock` records `cairn-gate-contract` 0.2.1 (was 0.1.0).
- Regenerate the retained `fixtures/cairn/selected-work` snapshots with
  `KILN_UPDATE_SELECTED_WORK_FIXTURES=1`, the documented step after a producer
  pin change. The vendored generated policy's readiness reclassification
  changes its identity, so only the 64-hex identities derived from it change.
  The Lattice workflow-intent fixtures follow: `verify.intent.json` is
  regenerated with `KILN_UPDATE_LATTICE_INTENT_FIXTURES=1`, and the request,
  plan and compare files come from a release `lattice` rebuilt from the same
  Lattice `661499d2`, whose measured compiler identity the request now binds.
- Reference pins, the Cairn gate guide, the policy provenance, the license map,
  README and changelog name the new revisions and the split between the
  imported revision and the gate executable.

## Impact

- **Consumers:** `kiln-adapter-cairn` (gate lowering and admission,
  `cairn.selected-work.v1` admission, workflow obligations, authoritative
  completion), every crate that links it, and maintainers running Cairn
  against Kiln's `kiln-default` policy.
- **Behavior:** Kiln's generated agent workflow guidance now routes through
  `cairn readiness`, and a `cairn` older than the new policy shape refuses the
  export. Library code paths that Kiln calls are unchanged: the gate contract
  changed only module layout and lint annotations, and `cairn-core` changed
  only convergence, delta-merge, assurance-target and workflow command-route
  code that Kiln does not call.
- **Authority:** none. No executable profile, remote or publication changes.

## Specification Effect

Accepted Kiln specifications do not change. The change re-imports pinned
producer sources and regenerates a producer-shaped policy section; it adds no
Kiln requirement and changes no accepted requirement's meaning or stable ID.
If adapting Kiln needs a normative change, this classification stops and a
delta-spec package replaces it.

## Out of Scope

Moving the gate executable profile to a newer binary, pinning onix-core and
running the scripted VM (Kiln #49), and the Lattice input bump.
