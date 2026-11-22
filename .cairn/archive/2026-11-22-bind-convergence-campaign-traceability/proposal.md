## Why

`cairn traceability coverage` reports 33 missing requirement IDs for the accepted
`convergence-campaign` and `mantle-foreign-realize-adapter` specs. Both specs
were archived from `2026-11-20-admit-cairn-convergence-campaigns` and
`2026-11-21-realize-convergence-evidence-through-mantle`. Their implementations
and tests exist, but only nine `r[impl ...]` markers were ever written and no
test carries an `r[verify ...]` marker. Coverage therefore cannot connect the
accepted requirements to the code that implements them or the tests that verify
them, and the repository gate fails.

## What Changes

- Add `r[impl ...]` markers to the exact functions that implement the two
  remaining unbound requirements, the preserve-route realization lowering and
  profile resolution (`kiln.convergence.realize`) and the bounded realize
  dispatch (`kiln.mantle.foreign_realize.dispatch`).
- Add `r[verify ...]` markers above the existing tests that exercise each
  accepted scenario, using the exact accepted requirement IDs.
- Bind `kiln.convergence.nonclaims` to `EvidenceBuildRequest::into_build_effect_plan`,
  the only lowering through which a convergence record reaches an effect, and
  which yields only a typed build effect. The specs' non-claims remain
  statements about what Kiln does not prove; no new admission behaviour is
  added.
- Add small tests that cover scenarios no existing test exercised: a moved or
  unbound targeted row stays selected, a complete receipt plus a planned Verify
  request grants no merge, publication or release consequence, the pinned
  realize profile is admitted, and a complete realization grants no release
  authority.

No specification delta. The accepted requirement and scenario text, stable IDs
and non-claims stay as they are.

## Impact

- **Core**: `crates/kiln-core/src/convergence.rs` and its tests gain the
  non-claims constant and the missing scenario tests.
- **Adapters**: `kiln-adapter-cairn` and `kiln-adapter-mantle` test modules gain
  verification markers; the Mantle realize module gains the dispatch and
  realize markers plus two tests.
- **Lifecycle**: this change binds accepted IDs to evidence and closes with
  `cairn sync` and `cairn archive`.
- **Testing**: `cargo fmt`, `cargo clippy`, the three crate test suites,
  `cairn validate`, `cairn traceability coverage` and `cairn release-readiness`.

## Specification Effect

No normative change. This change only adds implementation and verification
markers for accepted requirements and records their non-claims in the module
that already carries them in prose. Any need to change requirement meaning would
require a delta-spec package instead.

## Out of Scope

No change to the convergence receipt schema, the campaign identity, the Verify
request lowering, the Mantle realize cohort, the realization receipt, or any
provider effect. No publication, deployment or release claim.
