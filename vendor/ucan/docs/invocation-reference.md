# Invocation Reference: Replay Admission Identity

This reference records the replay admission contract for the audience-only
invocation surface. It explains the admission key, the default posture, and
the caller-owned store requirement. It does not redefine token, proof, or
authorization semantics.

## Admission-key contract

The replay admission key is the BLAKE3 digest of the complete bounded compact
token. `verify_invocation_with_replay` derives this digest from the presented
token bytes and passes it to the caller-supplied `ReplayAdmission` hook.

The compact token format carries no invocation-specific nonce claim. Two
invocations that present the same token bytes and the same resource/ability
pair are indistinguishable to the admission hook. The hook receives only the
token digest, the resource, and the ability.

The digests and policies are caller-owned observations. The hook does not
read clocks, storage, network state, or ambient application state.

## Default posture

The default audience-only hook is `AllowAllReplayAdmission`. It accepts
every verified invocation and never reports a duplicate. This posture is the
off-by-default replay protection.

A consumer that needs exactly-once invocation semantics must install a
caller-owned store. The store keys an invocation by the token digest plus the
resource and ability. The store must reject a repeat of an admitted key.

The in-repo baseline is `examples/replay_baseline.rs`. It issues one token,
admits the first invocation, and rejects the second as
`ReplayAdmissionError::Duplicate`. The example comments show where a durable
store plugs in.

## Indistinguishability example

Present the same token twice with the same resource/ability:

```text
token bytes              T
resource                 did:key:z6Mkresource/photos
ability                  store/add
admission key            BLAKE3(T)
```

First presentation: the store admits the key. Second presentation: the store
rejects the same key as a duplicate. A consumer that issues two logically
separate operations from one token sees one admission unless it adds a
distinguishing field to the resource or its request.

## Boundary

This reference records the measured admission contract only. It does not
claim nonce-store truth, durability, crash recovery, request provenance,
transport identity, or execution safety. Those remain caller-owned.
