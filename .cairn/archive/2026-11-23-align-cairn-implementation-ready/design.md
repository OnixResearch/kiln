## Context

Kiln consumes Cairn through three immutable source imports and one policy
snapshot:

| Input | Before | Consumer |
|---|---|---|
| `vendor/cairn` | `15f00875` | `cairn-gate-contract`, producer fixtures, Nickel contract |
| `vendor/cairn-verifier` | `8f6cd0b3` | `cairn-core`: selected-work admission, readiness, workflow obligations, authoritative completion |
| `vendor/cairn-kiln-core` | Kiln `5d99f4e9` | `cairn-core`'s exact `kiln-core` Git dependency |
| `cairn-policy/{upstream-default,contracts}.ncl` | `007ba519` | Kiln's `kiln-default` policy overlay and its generated JSON |

The gate provider profile (`config/cairn-gate-profile.ncl`,
`kiln_adapter_cairn::profile`) separately pins a reviewed `15f00875` executable,
its BLAKE3 and a Kiln policy identity.

## Decision

Import both Cairn source trees from `317d3ff6` with the existing importer.
Record the producer, full-archive and selected-archive identities in both
source records and importer pins. Remove each old snapshot before importing;
the importer refuses drift and has no force mode.

Keep the gate executable profile at `15f00875`. The wire contract that the
library and the executable share is unchanged:

- `contracts/cairn-gate-contract.ncl` and
  `fixtures/cairn-gate-contract/producer/fixtures-v1.json` are byte-identical;
- `identity.rs` became `binding.rs` with the same identity domains and framing
  order, and the evidence loop moved into a helper;
- the scalar types moved to `primitive.rs` and are re-exported, with the same
  validation predicates and errors;
- admission adds `debug_assert`s, splits the tool-cohort check into
  per-field returns of the same error, and qualifies paths.

Kiln uses only re-exported names, so no Kiln Rust source changes. Moving the
gate executable would need a new reviewed path, executable digest, policy
identity and gate-machine fixtures. This change needs none of them.

`cairn-core` from `8f6cd0b3` to `317d3ff6` changes convergence, delta-merge
and assurance targets, plus `assurance-targets.json` and `mcp-discovery.json`
in its generated policy. It also adds a test-only check of shipped workflow
command routes, bounds policy identifiers copied into generated-workflow
admission diagnostics to 64 bytes, and files `readiness` and
`drain-plan-readiness` under the `readiness` class in its own generated
`cairn-policy.json`. Kiln reads `selected_work`, `readiness`,
`generated_agent_workflow` lookups, the receipt hashes and canonical JSON;
their accepted inputs and decisions are unchanged. The vendored
`workflow-obligations.json` that Kiln tests include is unchanged, but the
vendored `cairn-policy.json` identity is not, so the retained selected-work
producer, admitted and contract fixtures are regenerated with
`KILN_UPDATE_SELECTED_WORK_FIXTURES=1`; every changed line is a 64-hex
identity. The Lattice intent normalized from that snapshot changes with it, so
`fixtures/lattice/workflow-intent/verify.intent.json` is regenerated with
`KILN_UPDATE_LATTICE_INTENT_FIXTURES=1`. Its request, plan and compare files
are regenerated with the Lattice CLI, as `docs/cairn-selected-work.md`
requires: a release `lattice` rebuilt from Lattice `661499d2` compiles the new
request (whose `compiler_artifact_identity` is that executable's measured
identity), compares it with one obligation dropped (`owner_readmission_required`),
and refuses it with `coverage = incomplete` (`IncompleteContext`). The
synthetic runtime profile identity is unchanged.

Cairn's root `Cargo.toml` still pins `kiln-core` at `5d99f4e9` and
`onix-artifact` at `e41340be`, so the existing root patches still resolve
them. Cairn's workspace version moved from 0.1.0 to 0.2.1, which Cargo
records for the path package.

The current CLI parses `generated_agent_workflow_policy` with two new
required fields, `command_catalog` and `entry`, and refuses Kiln's
`007ba519`-shaped export. Re-import the policy snapshot with
`scripts/import-cairn-policy.sh` (which verifies the source revision, NAR and
file BLAKE3 values, and accepts only the previous or the new local bytes), then
re-export with `cairn policy export --source cairn-policy/default.ncl`. Kiln's
overlay does not touch that section, so the export differs only there. The
`fixtures/agent-completion` contract and results bind the policy hash, which
the release-readiness MCP smoke checks; only that field is rebound, as in
earlier policy changes. The commands, registry, selection, evidence and
execution receipt hashes stay as they are.

## Specification classification

No accepted Kiln requirement changes. The imports and the policy section are
producer-owned shapes that Kiln consumes unchanged; Kiln's own requirements
for gate admission and selected-work admission keep their wording and IDs.
Semantic review inputs are this design, the proposal, `acceptance.md`,
`tasks.md`, the Cairn source diff between the old and new revisions for the
imported paths, and the generated-policy diff.

## Failure behavior

- Importer drift (archive digest, record or existing bytes) stops the import
  with no replacement.
- A gate fixture or Nickel contract that drifts from the frozen `15f00875`
  copies fails `checks.<system>.cairn-gate-cohort`.
- A policy source whose revision, NAR or file BLAKE3 differs from the pin, or
  a locally modified snapshot, stops `import-cairn-policy.sh`.
- A Kiln caller of a removed or changed contract name fails compilation; none
  exist.

## Verification

Byte equality of each import with `git archive 317d3ff6` for its recorded
paths, apart from the importer's pruned members and `rustfmt.toml`. Then
workspace fmt, clippy with and without all features, workspace tests, the
Nickel fixtures, `cairn validate`, traceability and release readiness with the
`kiln-default` policy and a `317d3ff6` `cairn`, the flake checks and packages
that build the imports, and a real `cairn implementation-ready` output admitted
through `kiln-cairn-intake`. Negative: the `317d3ff6` `cairn` refuses the old
policy export.

## Review

The independent review of this package was `KilnCairnRevendorReview`, over
the earlier `9082e1ae` re-import. Its one finding (P2) was that the imported
catalog filed `cairn readiness` under the `gate` command class while the
`apply` workflow permits only `readiness`. Cairn fixed the classification
upstream in a reviewed and published change, `317d3ff6`, and this re-import
follows that fix: Kiln's generated policy now files `readiness` and
`drain-plan-readiness` under `readiness`. The `no-spec-delta` review receipt
is recorded only from an independent reviewer's verdict on the final
artifacts; the author does not write it.

## Risks and non-claims

Wire compatibility is not executable qualification. A newer gate
executable is not admitted by this change. No onix-core pin, scripted VM run,
publication or release eligibility is established.
