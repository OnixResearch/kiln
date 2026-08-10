# Add a Git source adapter

## Why

Kiln models Git source identities, but it has no direct Git source adapter. Jujutsu support requires a Jujutsu repository and cannot replace a plain Git provider.

The first Git profile can provide bounded read-only local repository access without forge or network authority.

## What Changes

- Add optional `kiln-adapter-git` support for the `SourcePort`.
- Pin one exact Git library, object-format, repository-format, and platform cohort.
- Support local discovery, exact revision resolution, snapshot acquisition, and identity observation.
- Preserve repository, reference, object-format, commit, tree, submodule, and signature-observation facts.
- Compute the canonical BLAKE3 tree identity from admitted snapshot bytes.
- Reject moved references, missing objects, unsafe paths, unsupported entry kinds, and exceeded bounds.
- Exclude fetch, push, merge, signing, credentials, and forge collaboration from the first profile.
- Add typed Nickel profiles and real positive and negative repositories.

## Dependencies

This change depends on `establish-neutral-ci-control-plane` and the accepted source-collaboration contract.

## Impact

- **Adapters**: adds `kiln-adapter-git`.
- **Core**: adds bounded provider-neutral Git facts when required.
- **Configuration**: adds read-only local Git profiles and named bounds.
- **Testing**: adds real SHA-1 and supported alternate-format fixtures where the pinned cohort permits them.
