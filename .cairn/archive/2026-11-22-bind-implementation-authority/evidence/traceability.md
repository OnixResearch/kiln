# Traceability coverage of the implementation-authority spec

The change was synced and archived on 2026-11-22 with `cairn 0.1.0` (the PATH
cairn). `cairn validate` and the proposal, design and tasks gates passed before
sync; `cairn validate` passed again after archive
(`evidence/post-archive-validate.txt`).

## Dangling references

Before archive, coverage reported nine dangling
`kiln.implementation.authority.*` IDs: the code from commit `24fb3ef4` cited
requirement IDs that only existed in this change's delta spec. Sync merged
`specs/implementation-authority/spec.md` into `.cairn/specs/implementation-authority/spec.md`,
and archive closed the change; coverage now reports zero dangling references
(`evidence/post-archive-traceability-coverage.txt`).

## Missing references owned by this change

Sync promoted all seven requirement blocks, so the nine accepted requirement
IDs that the implementation cited only in prose (`binding.scenario.substitution`,
`binding.scenario.candidate`, `currentness.scenario.blocked`,
`currentness.scenario.unavailable`, `separation`,
`separation.scenario.widen`, `compatibility.scenario.role`, `cohort`,
`cohort.scenario.import`) would have become missing entries. They are now bound
to exact evidence in the same commit:

- `crates/kiln-core/src/implementation/authority.rs` — request-identity,
  candidate-exactness, role-only and projection denial markers, plus
  `dispatch_session_denies_widened_and_non_current_effects`: a session admitted
  for `Dispatch` denies verification, publication, integration and lifecycle
  requests, and a cancelled, superseded, fence-replaced, lease-expired or
  unavailable attempt denies a later effect.
- `crates/kiln-adapter-basalt/src/lib.rs` — the adapter denies a substituted
  bound field before any effect.
- `crates/kiln-adapter-basalt/tests/cohort.rs` — the recorded Basalt import
  resolves to Kiln's single vendored UCAN cohort: the record names the
  repository-owned importer, the UCAN revision, both archive identities, and the
  three literal manifest rewrites that are present in the vendored
  `vendor/basalt/Cargo.toml` and absent as nested paths.

No requirement of this change is missing after those markers.

## Missing references this change does not own

`.cairn/specs/convergence-campaign/spec.md` and
`.cairn/specs/mantle-foreign-realize-adapter/spec.md` contribute 33 accepted
requirements with no `impl` or `verify` marker in any evidence root: every
`kiln.convergence.*.scenario.*` requirement, `kiln.convergence.nonclaims`,
`kiln.convergence.realize`, `kiln.mantle.foreign_realize.dispatch` and every
`kiln.mantle.foreign_realize.*.scenario.*` requirement. They were accepted by the
archived change `.cairn/archive/2026-11-21-realize-convergence-evidence-through-mantle`,
whose evidence records the gates but no coverage run.

That is a defect of that change, not of this one: the implementations exist —
`crates/kiln-core/src/convergence.rs`, `crates/kiln-core/src/convergence/tests.rs`
(27 tests), `crates/kiln-adapter-mantle/src/realize.rs`,
`crates/kiln-adapter-mantle/src/realize/tests.rs` (20 tests) and
`crates/kiln-adapter-cairn/src/convergence.rs` — but not one of those tests
carries an `r[verify ...]` marker and the modules carry nine `r[impl ...]`
markers, all on base requirements. Closing it means binding those 47 tests to the
33 requirements in the convergence and mantle-foreign-realize change scope; it is
reported rather than silently absorbed here.
