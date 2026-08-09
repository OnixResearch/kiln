## Why

Kiln models Pijul identities and channels, but it has no Pijul adapter crate.
The current conformance test proves only that a Pijul profile name can exist.
Kiln cannot resolve a channel or acquire an immutable Pijul snapshot.

A Pijul channel is not a Git branch. Its state and change identities need a
native mapping that does not invent commits, branches, or forge authority.

## What Changes

- Add `kiln-adapter-pijul` as an optional, read-only source adapter.
- Add bounded Pijul repository, channel, state, and change facts to the pure
  source model.
- Bind each resolution to one provider-produced immutable state identity.
- Acquire the snapshot and state facts from one consistent repository view.
- Compute the canonical BLAKE3 tree identity from the acquired bytes.
- Add a typed Nickel profile for the admitted Pijul format cohort and bounds.
- Add a concrete local repository shell backed by a pinned Pijul library.
- Add positive and negative fixtures that use real local Pijul repositories.
- Keep Pijul history, signatures, trust, mutation, and hosting outside Kiln.

## Dependencies

This change uses the accepted `source-collaboration` contracts from
`establish-neutral-ci-control-plane`.

Implementation must pin the exact Pijul and `libpijul` release cohort before it
adds provider code. The pin must record the source revision, crate versions,
repository format, licenses, and supported platform set.

## Impact

- **Core**: adds pure Pijul fact validation and exact-state admission.
- **Adapters**: adds the optional `kiln-adapter-pijul` crate.
- **Configuration**: adds a typed Pijul source-provider profile and named bounds.
- **Nix**: supplies the pinned adapter dependencies and local fixture tools.
- **Tests**: adds positive and negative Pijul repository fixtures.
- **Documentation**: records Pijul capabilities, limits, and non-claims.

## Out of Scope

- Do not record, apply, unrecord, push, pull, clone, fork, or delete changes.
- Do not create, rename, reset, or delete channels.
- Do not treat Pijul as a forge or collaboration provider.
- Do not claim signature trust, complete history, or remote availability.
- Do not compare Pijul and Git history from equal canonical tree identities.
- Do not add Pijul or `libpijul` dependencies to Kiln core crates.
