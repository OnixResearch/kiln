# Verification evidence

## Result

Kiln appends one declared Radicle patch `review` action through the existing preview-first collaboration port.

Each plan binds one RID, patch type, object, expected head, actor, policy, operation identity, and BLAKE3 payload identity.

The shell cannot create, close, archive, merge, or revise patches. It cannot rewrite Git history or mutate canonical Kiln CI state.

## Compatibility evidence

The transport reuses the SQLite-free Radicle cohort from `publish-radicle-ci-status`:

- `radicle-cob` `0.21.1`;
- `radicle-crypto` `0.19.0`;
- `radicle-oid` `0.2.2`;
- `radicle-git-ref-format` `0.2.0`;
- `git2` `0.21.0`; and
- `x86_64-linux` support only.

The review action schema is bound to Radicle `0.25.1`. Its source SHA-256 is `fc23b44b518df749486358fef29c4375daa47299eeec32b3659781c1bfd207e4`.

The full Radicle runtime remains outside `Cargo.lock`. This preserves Kiln's existing SQLite cohort.

The production signer uses the pinned SecretSpec `0.19.0` executable and BLAKE3 identity from the accepted status profile.

## Authority evidence

The application keeps its existing order:

```text
observe -> fresh-plan admission -> resolve signer -> preview -> dispatch
```

Dispatch reloads the exact head immediately before signing. The Git reference update also uses an atomic expected-target comparison.

The resolved signer must equal the planned actor. The current patch head must have the same signer as its author.

The profile stores typed review actions under their exact BLAKE3 identities. The transport renders only the pinned `review` JSON shape.

The bounded history loader rejects multiple heads, malformed actions, unknown action types, missing revisions, inconsistent identity resources, and exceeded bounds.

Operation and payload identities are recorded in the change summary. A repeat with equal facts returns the current observation without another write.

Reconciliation resolves the declared signer and adopts only an exact operation, payload, and author match.

## Validation evidence

The following checks passed:

- baseline `kiln-adapter-cob` and `kiln-adapter-radicle` tests before implementation;
- 253 final workspace tests;
- 35 Radicle adapter tests in the normal workspace run;
- 5 focused mutation unit tests;
- 6 native mutation repository tests;
- strict workspace Clippy with warnings denied;
- focused Radicle Clippy with native fixtures enabled;
- Rust formatting;
- `kiln-core` for `wasm32-unknown-unknown`;
- the positive Nickel mutation profile;
- 5 negative Nickel mutation profiles;
- core and standalone runtime-independence checks;
- the native mutation Nix fixture check;
- strict Cairn validation;
- proposal, design, and task gates; and
- all `x86_64-linux` flake checks.

The native cases cover authorized review, repeat dispatch, moved head, foreign head, missing signer, reference failure, deadline expiry, and reconciliation.

The proposal gate receipt is `e0bddda7d00b8959c5b7601c205202ac4e181f009f3adde9d8c0caa137228b7c`.

The design gate receipt is `42d694cce2ac7da450753710ac46cbd4ecd92ad58073cfad7ba8f932f31ecc7f`.

The task gate receipt is `cb4e528e8cba26cc5873dd2bdc6c7027e42712d63048e37cfa16e6619e6698ce`.

The specification sync receipt is `5e6c6d623936b4f15010cff4db934bfb46d19d70d9daa294b3e8bb627660d84a`.

The archive receipt is `d56b77d00b6224819d85912b9fe1fa715e10a6bb49594c7af6befe5f530ee9f4`.

Tracey references all 493 accepted requirements. Its receipt is `1a02f25ec962d32ccb7afcaecf359ba1b660951d41a1c35470bc0c198b8df84f`.

The deterministic release-evidence identity is `6ba830d9926030b8567b9cd17b2adf2c5ceb1dc4cf237224b572b51f049bc394`.

Kiln has no root Octet metadata profile. This change does not claim an Octet result.

## Claim boundary

These results prove bounded local patch-review mutation under one declared profile. They do not prove peer convergence, remote visibility, actor trust, patch correctness, or merge correctness.
