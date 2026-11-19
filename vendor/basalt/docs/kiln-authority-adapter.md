# Kiln authority adapter

`basalt::kiln` is the verified holder-invocation boundary for Kiln implementation
operations. It owns the reviewed operation mapping and the verified adapter. Kiln retains
work admission, current attempts and fences, independent verification, budget accounting,
publication and every external effect.

## Operations

| Operation | Contract | Ability |
| --- | --- | --- |
| `ClaimAcquire` | `kiln-claim-acquire` | `kiln/claim/acquire` |
| `ClaimRenew` | `kiln-claim-renew` | `kiln/claim/renew` |
| `Dispatch` | `kiln-dispatch` | `kiln/dispatch` |
| `CandidateSubmit` | `kiln-candidate-submit` | `kiln/candidate/submit` |
| `VerificationAdmit` | `kiln-verification-admit` | `kiln/verification/admit` |
| `AttemptCancel` | `kiln-attempt-cancel` | `kiln/attempt/cancel` |
| `PublicationPublish` | `kiln-publication-publish` | `kiln/publication/publish` |
| `IntegrationLand` | `kiln-integration-land` | `kiln/integration/land` |
| `LifecycleComplete` | `kiln-lifecycle-complete` | `kiln/lifecycle/complete` |

Each operation has one reviewed contract, one ability and the
`kiln://implementation/v1/` resource prefix. No operation implies another. A dispatch
grant cannot admit submission, verification, publication, integration or lifecycle
completion.

## Exact binding

`Verifier::enforcement` frames the holder, deployment audience, project, work, work
version, attempt, fence, authority generation, payload digest, target and candidate into
one canonical resource. String fields are length-prefixed and hex-encoded so delimiters
and Unicode normalization cannot alias two requests.

Rules enforced before replay:

- the deployment generation matches the verifier and is non-zero;
- the request audience matches the configured audience;
- every string field is non-empty, at most 512 bytes, ASCII, untrimmed-equal and free of
  control bytes;
- the canonical resource is at most 4096 bytes;
- a candidate identity is non-zero exactly for `CandidateSubmit`, `VerificationAdmit`,
  `PublicationPublish`, `IntegrationLand` and `LifecycleComplete`, and zero for claim,
  dispatch and cancellation.

The caller's `request_digest` is the digest of the canonical payload it intends to
execute. Basalt binds it; the receiving owner recomputes it.

## Verified ingress

`Verifier::verify` mints a `Session` only after signature, proof, holder, trusted-root,
policy, time, revocation and final replay checks. `Session` is not `Clone`, not
serializable and has no constructor outside the module. A serialized request, a redacted
receipt or a stored session cannot construct live authority.

`Verifier::revalidate` rechecks signatures, the full proof chain, time, current
revocations, caveat evaluation, the loaded policy and the deployment generation before a
later dispatch. It performs no second replay commit; the session's single final replay
admission remains the only one recorded.

## Caller responsibilities

Callers own keys, proof loading, revocation currentness, monotonic time, durable replay
storage and effect recovery. Kiln owns current work/fence/policy facts, claims, budgets,
verification acceptance, publication, integration and lifecycle completion. Lattice owns
execution admission and confinement.

## Non-claims

- A `Session`, receipt, policy artifact or redacted projection is not a transferable
  grant and cannot authorize another request.
- Revalidation does not refresh, extend or reissue authority.
- This adapter proves neither effect completion nor independent acceptance.
- Unknown operations, unknown policy contracts and generic document or process authority
  deny; there is no fallback mapping.
