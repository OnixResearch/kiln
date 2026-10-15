# Single-writer Radicle patch review mutation

Kiln can append one declared `review` action to one declared `xyz.radicle.patch` object.

This shell uses the SQLite-free cohort from `docs/radicle-status.md`. The full Radicle runtime does not enter the product dependency graph.

Its Git COB storage is the same verified, repository-owned v0.2.0 shared source imported for CI status publication; the immutable commit, digest, and guarded update procedure are recorded in `docs/radicle-status.md`. The shared library supplies signed-ref admission, bounded discovery, and exact-reference CAS, not permission to sign, review, or mutate a patch. Kiln retains all patch-specific admission and reconciliation decisions.

The patch action schema comes from Radicle `0.25.1`. Its crate SHA-256 is `fc23b44b518df749486358fef29c4375daa47299eeec32b3659781c1bfd207e4`.

## Authority flow

The application calls the existing `CollaborationMutationPort` flow:

```text
observe -> fresh-plan admission -> resolve signer -> preview -> dispatch
```

The transport repeats the head check immediately before the write. The Git reference update also compares the exact old target atomically.

The profile declares each allowed RID, patch type, object ID, and review payload. A plan outside any declaration fails before repository access.

The resolved SecretSpec signer must equal the planned actor. The current patch head must also have that signer as its author.

The shell never uses ambient Radicle or Git identity. It supplies fixed Git display metadata through an ephemeral configuration.

## Supported action

The only supported action is this Radicle patch shape:

```json
{
  "type": "review",
  "revision": "<40-hex-entry-id>",
  "summary": "<bounded-text>",
  "verdict": "accept"
}
```

`verdict` can be `accept` or `reject`. Kiln requires a non-empty summary and a revision that exists in the bounded patch history.

The profile binds the exact serialized action to its BLAKE3 identity. The plan carries that identity, not unreviewed payload bytes.

The shell cannot create, close, archive, merge, or revise patches. It cannot rewrite Git history.

## Single-writer checks

The target must have exactly one head. The plan must name that exact head.

Before signing, the shell reloads a bounded history from local storage. It rejects multiple heads, moved heads, foreign authors, inconsistent identity resources, and malformed actions.

The write uses the same identity resource as the existing patch. It appends one signed change and updates only the declared namespace reference.

## Idempotency and recovery

The change summary records the Kiln operation identity and payload identity. Action JSON stays compatible with the pinned Radicle schema.

A repeated operation with equal payload facts returns the current observation without a second write. A reused operation identity with different payload facts fails closed.

A reference-update failure stays `Unknown`. A write that passes its deadline after possible acceptance also stays `Unknown`.

Reconciliation scans the bounded history for the exact operation marker and payload. It never signs or writes.

## Bounds

The default limits are:

- payload: 16,384 bytes;
- history: 1,024 entries;
- history bytes: 16,777,216 bytes;
- preflight: 5,000 milliseconds;
- signer resolution: 15,000 milliseconds; and
- write: 15,000 milliseconds.

The profile can reduce these values. Zero values and values above the absolute adapter bounds fail admission.

## SecretSpec setup

The signer declaration uses the same pinned SecretSpec executable as status publication. Configure a 64-character hexadecimal Ed25519 seed under the declared secret name.

Do not put the seed in Nickel, action JSON, command arguments, logs, observations, or receipts. The resolver zeroizes captured output after key construction.

## Evidence boundary

Six native patch-review fixtures cover authorized review, repeat dispatch, moved and authenticated foreign heads, missing signer, reference failure, timeout, and reconciliation. The foreign fixture uses a secondary signer in its own valid namespace, so strict shared signed-author checks remain enforced before the Kiln single-writer review policy rejects the action.

These checks prove bounded local patch-review mutation under one profile. They do not prove peer convergence, remote visibility, actor trust, patch correctness, or merge correctness.
