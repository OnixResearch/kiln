# Publish Radicle CI status

## Why

Kiln runs CI for Radicle repositories, but run outcomes never reach Radicle. Collaborators cannot see CI state where they work.

Radicle stores job status in the `xyz.radicle.ci-job` collaborative object. Kiln can publish a rebuildable projection there without giving up canonical CI authority.

The signer must never come from an ambient identity or a hard-coded path. SecretSpec resolves one declared signing secret through the operator's chosen provider.

## What Changes

- Pin the exact SQLite-free `radicle-cob` crate cohort and the SecretSpec CLI.
- Add a typed Nickel status profile with RID and COB-type allowlists, bounds, and non-claims.
- Resolve the signer secret through one explicit SecretSpec declaration at dispatch time.
- Write admitted run status to `xyz.radicle.ci-job` as an idempotent, single-writer projection.
- Keep every ambiguous write unknown until reconciliation.
- Add local positive and negative fixtures created by the pinned cohort.
- Keep canonical CI state, merge authority, source trust, and release decisions outside this adapter.

## Dependencies

This change depends on `establish-neutral-ci-control-plane` and uses the accepted `kiln.radicle.status` allowance.

Implementation must pin the exact cohort before it adds provider code. The pin records crate versions, source revisions, licenses, and the supported platform.

## Impact

- **Adapters**: extends `kiln-adapter-radicle` with a bounded status publication shell.
- **Configuration**: adds a typed Radicle status profile that names one external SecretSpec declaration.
- **Testing**: adds positive and negative native fixtures, redaction checks, drift, stale-head, and reconciliation cases.
- **Documentation**: records setup, SecretSpec resolution, limits, and non-claims.

## Out of Scope

- Do not mutate patch or issue objects; collaboration mutation has its own change.
- Do not read, clone, fetch from peers, or claim node convergence.
- Do not let a status write complete a Kiln attempt or change the journal.
