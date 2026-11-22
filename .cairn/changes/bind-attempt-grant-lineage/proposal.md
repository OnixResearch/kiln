## Why

Kiln #70 needs a durable relationship between standing policy, the admitted
attempt and any authorized child grant. #69 made every protected operation
require current authority, but Kiln has no grant lineage record: nothing binds a
grant to its ceiling, campaign revision, work version, attempt and fence,
holder, #61 reservation or parent, and nothing refuses a child that is broader
than its parent. A copied grant, a replacement worker or a delegated child could
therefore not be checked against the lineage it claims.

This change is slice S2 of the authority plan: the pure attenuation core and
the attempt record field. The grant-lifecycle shell (S6) persists, mints,
revokes and recovers through it later; verified cross-holder fixtures wait for
Basalt B1.

## What Changes

- Add `kiln-core::implementation::grant` with the
  `kiln.attempt-authority-binding.v1` contract: `AuthorityCeiling`,
  `AttemptAuthorityBinding`, `ChildGrantRequest`, the closed `LineageRefusal`
  set, `admit_root`, `admit_child` and `check_current`.
- Admission is a pure function of explicit owner facts: clock, authority and
  revocation generations, revoked proof references, the current attempt, the
  verified delegator and its delegation permission, the lineage chain and the
  owner-confirmed #61 reservation.
- Add `ImplementationAttempt.authority: Option<AttemptAuthorityBinding>`, absent
  from serialization when unset, and the `AttemptLedger::bind_authority` fold.
- Add `kiln-implementer::grant_issuance`: journal record types for intent,
  issuance and revocation and the pure reconciliation fold. It is not wired.
- Draft the Basalt B1 request for scoped cross-holder delegation.

## Impact

- **Core**: new `implementation::grant` module and tests; one optional attempt
  field and its fold. Existing ledgers round-trip byte-identically.
- **Implementer**: new unwired `grant_issuance` module.
- **Struct literals**: test and fixture constructors of `ImplementationAttempt`
  gain `authority: None`.
- **Not changed**: admission, host-owner and fair-admission code; `authority.rs`
  (S1); any shell, store, token or credential path.
- **Files:** `crates/kiln-core/src/implementation/{grant.rs,grant_tests.rs,mod.rs,attempt.rs,publication.rs,stack_tests.rs}`, `crates/kiln-core/src/fleet/prompt.rs`, `crates/kiln-implementer/src/{grant_issuance.rs,lib.rs}`, `crates/kiln-adapter-lattice/src/implementation.rs`, `crates/kiln-web-service/src/implementation_campaign/adoption_tests.rs`, `cairn-policy/development-coverage.ncl`, `CHANGELOG.md` and this package.
