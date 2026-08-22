# Verification evidence

## Result

Kiln now provides separate concrete Radicle source and trigger shells.

The source shell binds one local bare repository, RID, signer namespace, branch, expected signed-refs commit, exact revision, and bounded canonical snapshot.

The trigger shell retains durable replay and recovery-only redispatch behavior through Lattice.

## Contract and compatibility evidence

The source shell consumes `kiln-radicle-source-contract` at revision `0a5924dd2092243a0fe5df94d43008f0402e46f6`.

The source verifier uses this SQLite-free cohort:

- Radicle protocol source `0.25.1`, SHA-256 `fc23b44b518df749486358fef29c4375daa47299eeec32b3659781c1bfd207e4`;
- `radicle-core 0.4.1` without its SQLite feature;
- `radicle-crypto 0.19.0`;
- `radicle-oid 0.2.2`;
- `radicle-git-ref-format 0.2.0`; and
- Git2 `0.21.0`.

The full `radicle` runtime remains absent from `Cargo.lock`.

The trigger shell retains its published Radicle broker and Lattice compatibility cohorts.

The deployed broker schema does not carry an actor signature. Kiln records the local broker boundary and does not infer authenticated actor evidence.

## Source evidence

The source shell performs this ordered read:

```text
profile admission
  -> bare repository admission
  -> exact sigrefs lookup
  -> canonical refs parsing
  -> signer verification
  -> RID identity-root verification
  -> parent linkage verification
  -> signed and namespaced branch comparison
  -> bounded exact-revision snapshot
  -> final sigrefs comparison
  -> contract receipt admission
```

Native repository tests cover repeat acquisition, provider-port resolution, invalid signatures, RID substitution, namespace substitution, stale signed refs, and snapshot bounds.

Git errors are redacted. Diagnostics contain no repository path, object content, signature, credential, or provider error text.

## Validation evidence

The following checks passed:

- 35 baseline Radicle adapter tests before source implementation;
- 38 final Radicle adapter unit tests;
- 3 native signed-source repository tests;
- all workspace tests and all targets;
- strict workspace Clippy with warnings denied;
- focused Radicle Clippy with native source fixtures;
- Rust formatting;
- the `no_std` wasm checks for Kiln core and the source contract;
- the positive Nickel source profile;
- 5 negative Nickel source profiles;
- source contract and cohort pin checks;
- source native Nix fixtures;
- core, standalone, and contract dependency boundaries;
- strict Cairn validation;
- proposal, design, and task gates;
- Tracey coverage for all 524 accepted requirements; and
- all compatible `x86_64-linux` flake checks.

The flake omitted `aarch64-linux` because the repository marks that system incompatible. This change makes no result claim for that system.

The proposal gate receipt is `54a36b787b5c647cd0518b9f40223fb769253cbfe4d746bab5cfc3f434401b35`.

The design gate receipt is `9c3abad06fdf547c17d122d8eb9b99b5811a2c0c28a926f29b1a9c3d55f3f89c`.

The final task gate receipt is `fb5a641e68493aef84217c9bb6544bf938cd0c24a00fd86814573f2779912c65`.

The specification sync receipt is `69041bc1bbfdf4fe3cccdf84c80b305f235976ef3623c494afb28742ecda4d97`.

The Tracey receipt is `c23733b937e6310c5789c882af7eb5f3965320a2ac4e38804b96b32cfe9fdce9`.

The archive receipt is `4d4168a80b4818eed93cfc0ca073a22aff461b5459cadb304e8373a0e16e6362`.

The deterministic release-evidence identity is `e7ef3769bd39c953c9525efecdc003444916149cb54a2d41120f0358c2efa19d`.

Kiln has no root Octet metadata profile. This change does not claim an Octet result.

## Claim boundary

These checks prove bounded local observations for the selected profiles. They do not prove signer trust, filesystem-path stability, global freshness, history completeness, peer convergence, remote availability, workflow correctness, build correctness, or release readiness.
