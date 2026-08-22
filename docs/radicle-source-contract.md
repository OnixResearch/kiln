# Radicle source acquisition contract

Kiln publishes `kiln-radicle-source-contract` as a runtime-independent boundary for exact Radicle source observations.

The contract uses `no_std + alloc`. It does not read a repository or verify a signature. A provider shell supplies those facts.

## Selected protocol cohort

The reviewed protocol source is Radicle `0.25.1`, with these exact implementation files:

- `src/storage/refs.rs` defines `Refs`, `FeatureLevel`, `RefsAt`, and `SignedRefs`.
- `src/storage/refs/sigrefs/read.rs` defines `SignedRefsReader` and its verification rules.

The full Radicle crate has crates.io SHA-256 `fc23b44b518df749486358fef29c4375daa47299eeec32b3659781c1bfd207e4`.

The contract records behavior from that source. It does not depend on the full crate.

The later shell uses Kiln's existing SQLite-free cohort:

- `radicle-crypto 0.19.0`;
- `radicle-oid 0.2.2`;
- `radicle-git-ref-format 0.2.0`; and
- Git2 `0.21.0`.

These third-party components use their published licenses. Radicle uses `MIT OR Apache-2.0`.

## Signed-reference facts

Radicle stores one peer's signed references at this path:

```text
refs/namespaces/<node>/refs/rad/sigrefs
```

The target commit contains `refs` and `signature` blobs. The signature covers the canonical `refs` blob.

An accepted shell observation must establish these facts:

1. The declared node key verifies the signature.
2. `refs/rad/root` resolves to the declared RID.
3. `refs/rad/sigrefs-parent` matches the commit parent when present.
4. The recognized feature level is `parent`.
5. The signed branch maps to the requested revision.
6. The namespaced branch maps to the same revision.
7. The signed-refs reference equals the requested commit before and after acquisition.

The request carries `expected_sigrefs_commit`. A floating signed-reference tip is not an accepted request.

## Contract values

`SourceRequest` declares one exact operation and all snapshot bounds.

`SignedRefsObservation` preserves provider-native identities and explicit verification results.

`SnapshotEvidence` preserves the exact revision, native Git tree, BLAKE3 canonical tree identity, entry count, and byte count.

`SourceReceipt` links all three values. Pure admission rejects identity substitution, movement, feature downgrade, missing verification, malformed identities, and exceeded bounds.

## Consumer duties

A consumer shell must:

- open only the configured local repository;
- reject network, credential, signing, and mutation authority;
- parse and verify the selected signed-refs commit;
- compare the signed map with the namespaced branch target;
- acquire only the exact signed revision;
- build the canonical manifest under all named bounds;
- read the signed-refs reference again after acquisition; and
- pass the resulting receipt through contract admission.

The shell must keep repository paths, signatures, object contents, and provider errors out of diagnostics.

## Non-claims

r[impl kiln.radicle.source_contract.nonclaims]

An accepted receipt does not prove:

- that the signer or repository is trusted;
- that the selected signed-refs commit is globally fresh;
- that history is complete;
- that another peer has the same state;
- that a remote seed is available;
- that equal trees have equal history or authority;
- that a build is correct; or
- that a release is ready.
