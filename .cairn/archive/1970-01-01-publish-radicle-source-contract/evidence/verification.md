# Verification evidence

## Result

Kiln publishes a standalone `no_std + alloc` contract for exact Radicle signed-reference source observations.

The contract binds one RID, signer namespace, branch, expected signed-refs commit, exact revision, snapshot bounds, native tree, and canonical BLAKE3 tree identity.

It performs no I/O and grants no source, network, credential, signing, mutation, collaboration, or release authority.

## Compatibility evidence

The contract records reviewed behavior from Radicle `0.25.1`:

- `src/storage/refs.rs` defines `Refs`, `FeatureLevel`, `RefsAt`, and `SignedRefs`;
- `src/storage/refs/sigrefs/read.rs` defines `SignedRefsReader` and verification rules; and
- the source SHA-256 is `fc23b44b518df749486358fef29c4375daa47299eeec32b3659781c1bfd207e4`.

The contract has no normal dependency on Radicle, Git, SQLite, Bounded Exec, Kiln core, or provider APIs.

The consumer shell cohort remains `radicle-crypto 0.19.0`, `radicle-oid 0.2.2`, `radicle-git-ref-format 0.2.0`, and Git2 `0.21.0`.

## Validation evidence

The following checks passed:

- 35 baseline Radicle adapter tests before implementation;
- 10 focused contract unit tests;
- 1 independent producer-fixture test;
- all workspace tests and all targets;
- strict workspace Clippy with warnings denied;
- focused strict contract Clippy;
- Rust formatting;
- `kiln-core` and `kiln-radicle-source-contract` for `wasm32-unknown-unknown`;
- the positive Nickel contract fixture;
- 6 negative Nickel contract fixtures;
- deterministic producer fixture regeneration;
- dependency-boundary checks;
- strict Cairn validation;
- proposal, design, and task gates;
- Tracey coverage for all 511 accepted requirements; and
- all compatible `x86_64-linux` flake checks.

The flake omitted `aarch64-linux` because the repository marks that system incompatible. This change makes no result claim for that system.

The proposal gate receipt is `0b0ab665c1e374c0f706a09fb3a8bff3767c9b5ab58be0d95f7a4e191dccdb87`.

The design gate receipt is `75d2e23d4c5b602ee0ada876de739b66319cbac34fd8c094ea78f64a5af2b3c8`.

The final task gate receipt is `2ab6327abd6c5bd988ef3e1de084a1910459a7d5e5133f45d2739b5e2f692001`.

The specification sync receipt is `02d2f9938838079dfefe30e4b493c627dceb365d7e09da872494a5422da8fdad`.

The Tracey receipt is `c7bc9e5ce831d0c83005a8b509cc74fe529fb0f9fdd1df3e8fc223153ba621a8`.

The archive receipt is `ea34a903e8b3d2affa0894122570646a73ae12cbf8d938de2ba7e47e2c67446e`.

Kiln has no root Octet metadata profile. This change does not claim an Octet result.

## Claim boundary

These checks prove structural admission over supplied local facts. They do not prove signature-library correctness, signer trust, global freshness, complete history, peer convergence, remote availability, build correctness, or release readiness.
