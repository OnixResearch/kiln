# Design: Radicle source acquisition contract

## Goal and Success Contract

Publish a small contract that can cross the Radicle shell boundary without carrying provider libraries into Kiln core.

Completion requires these observable results:

- the Rust crate builds for `wasm32-unknown-unknown` with `no_std + alloc`;
- request admission requires exact RID, signer, signed reference, signed-refs commit, and revision identities;
- observations require the same identities before and after acquisition;
- accepted observations require signed-reference feature level `parent`;
- snapshot evidence binds native tree identity, BLAKE3 canonical identity, entry count, and byte count;
- positive and negative Rust and Nickel fixtures agree; and
- dependency checks exclude Git, Radicle runtime, process, storage, network, clock, and credential crates.

False completion includes a floating branch, an unbound signer, a boolean-only signature claim, an unchecked moved reference, or a contract that performs I/O.

## Functional Core

The contract owns immutable values and pure admission:

- `SourceRequest` declares the exact consumer intent;
- `SignedRefsObservation` records the exact provider-native facts;
- `SnapshotEvidence` records bounded acquisition facts;
- `SourceReceipt` links the request, observation, and snapshot;
- `admit_request` and `admit_receipt` enforce equality, bounds, feature level, and digest shape.

The contract does not open repositories, parse Git objects, verify signatures, walk trees, or read clocks.

## Signed-Reference Boundary

The selected cohort is Radicle `0.25.1` with `radicle-crypto 0.19.0`, `radicle-oid 0.2.2`, `radicle-git-ref-format 0.2.0`, and Git2 `0.21.0`.

Radicle stores a peer's signed references at `refs/namespaces/<node>/refs/rad/sigrefs`. The signed commit contains `refs` and `signature` blobs.

The contract requires these verified shell facts:

- the signature covers the canonical `refs` blob and verifies with the declared signer;
- `refs/rad/root` resolves to the declared RID;
- `refs/rad/sigrefs-parent` matches the signed commit parent;
- the feature level is `parent`;
- the declared branch maps to one exact revision;
- the namespaced branch target equals that signed revision; and
- the signed-refs reference has the expected value before and after acquisition.

The request supplies `expected_sigrefs_commit`. This prevents provider substitution during one operation. It does not prove global freshness.

## Snapshot Identity

The shell builds the existing canonical Git snapshot manifest under named entry, path, file, and total byte bounds.

The receipt preserves the native Git tree OID and a BLAKE3 digest of the canonical manifest. Equal BLAKE3 tree identities do not prove equal history or authority.

## Errors

Admission returns typed, bounded error codes for invalid versions, empty identities, digest shape, zero bounds, feature downgrade, request substitution, moved signed refs, revision mismatch, namespace mismatch, and snapshot overflow.

Errors contain no repository paths, object contents, signatures, credentials, or provider output.

## Configuration

A typed Nickel contract mirrors the Rust wire model. Producer fixtures include one accepted receipt and negative cases for a moved signed-refs reference, signer substitution, feature downgrade, revision substitution, malformed digest, and exceeded bounds.

## Claim Boundary

Passing contract admission proves only that supplied facts are internally consistent with one exact request.

It does not prove cryptographic implementation correctness, repository trust, delegate authority, event freshness, complete history, peer convergence, remote availability, build correctness, or release readiness.
