# Kiln authority adapter

`basalt::kiln` is the verified holder-invocation boundary for Kiln implementation and
source-view operations. It owns the reviewed operation mapping, the canonical resource
grammar, the delegation chain rule and the verified adapter. Kiln retains work
admission, current attempts and fences, independent verification, budget accounting,
grant issuance and lineage, view restriction, publication and every external effect.

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

Each implementation operation has one reviewed contract and one ability. Its contract
admits both the `kiln://implementation/v1/` and `kiln://implementation/v2/` prefixes; the
request's `framing` selects one. No operation implies another. A dispatch grant cannot
admit submission, verification, publication, integration or lifecycle completion.

## Source-view operations

| `SourceOperation` | Serialized | Contract | Ability | Scope |
| --- | --- | --- | --- | --- |
| `Discover` | `discover` | `kiln-source-discover` | `kiln/source/discover` | entry or subtree |
| `ReadContent` | `read` | `kiln-source-read` | `kiln/source/read` | entry only |
| `ReadHistory` | `history` | `kiln-source-history` | `kiln/source/history` | entry or subtree |
| `PrepareWorkspace` | `prepare` | `kiln-source-prepare` | `kiln/source/prepare` | entry or subtree |
| `ModifySource` | `modify` | `kiln-source-modify` | `kiln/source/modify` | entry only |
| `CreateSource` | `create` | `kiln-source-create` | `kiln/source/create` | entry only |
| `Delegate` | `delegate` | `kiln-source-delegate` | `kiln/source/delegate` | entry or subtree |
| `RetrieveResult` | `result` | `kiln-source-result` | `kiln/source/result` | entry or subtree |

Each source operation has one reviewed contract, one ability and the `kiln://source/v1/`
prefix. The variants mirror Kiln's `SourceViewOperation` one to one. Candidate submission
and publication of source changes reuse the implementation operations. An unknown
operation name does not deserialize, has no contract and therefore denies.
`Verifier::verify_source`, `Verifier::revalidate_source` and `SourceSession::require`
mirror the implementation methods.

## Resource grammar

Segments are written in order. Each one ends with `/`:

```text
text    = <decimal byte length> "/" <lowercase hex of the bytes> "/"
number  = <canonical decimal u64> "/"
digest  = <64 lowercase hex characters> "/"
```

Text fields are non-empty, at most 512 bytes, ASCII, equal to their trimmed form and
free of control bytes. Every resource is at most 4096 bytes.

### `kiln://implementation/v1/` (frozen)

```text
kiln://implementation/v1/ text(subject) text(audience) text(project) text(work)
  text(attempt) text(target) number(work_version) number(fence) number(generation)
  <64 hex request_digest> "/" <64 hex candidate>
```

These bytes are unchanged; a golden fixture pins them. The holder comes first, so a
byte-prefix grant cannot cover another holder's request.

### `kiln://implementation/v2/` (attenuable)

```text
kiln://implementation/v2/ text(audience) number(generation) text(project) text(work)
  number(work_version) text(attempt) number(fence) text(target)
  digest(request_digest) digest(candidate)
```

The holder is not part of the resource. UCAN audience equality and the holder signature
bind it. A grant that stops after `project`, `work`, `work_version`, `attempt` or `fence`
is a ceiling over every request below it, whoever holds it. Build those prefixes with
`ImplementationScope::prefix`; a later field without every earlier one denies.

### `kiln://source/v1/`

```text
kiln://source/v1/ text(audience) number(generation) text(repository) context
  component* terminal text(snapshot) text(view) digest(request_digest)
context   = "n/"                                  ; no attempt
          | "a/" text(attempt) number(fence)      ; acting for one attempt
component = <decimal byte length> "/" <lowercase hex of the component bytes> "/"
terminal  = "e/"                                  ; exactly this entry
          | "s/"                                  ; this subtree node
```

A component is non-empty, at most 255 bytes, contains no `/` or control character and is
never `.` or `..`. A path has at most 64 components. UTF-8 is framed by bytes, and no
normalization is applied. An `e/` path needs at least one component; an empty `s/` path is
the repository root. Read, modify and create require `e/`.

Because lengths are decimal and markers are letters, grants line up with paths only at
component boundaries:

| Grant prefix (`SourceScope::prefix`) | Covers | Never covers |
| --- | --- | --- |
| `… a b` (`terminal: None`) | `a/b` entry, `a/b` subtree, every descendant | `a/bc`, `ab`, `a` |
| `… a b e/` (`Some(Entry)`) | exactly the `a/b` entry | `a/b/c`, the `a/b` subtree, `a/bc` |
| `… a b s/` (`Some(Subtree)`) | subtree-scoped requests at `a/b` | the `a/b` entry, `a/b/c` |

An operator-context (`n/`) grant never covers an attempt-context (`a/…`) request, and the
reverse also holds.

## Delegation chain rule

`Verifier::verify` and `Verifier::revalidate` (and their source variants) walk every
proof path to a configured trusted root. A token with no proofs must be issued by a
trusted root. The compact profile has no Delegation `sub` field, so this is its
counterpart of UCAN's rule that the root Delegation issuer is the chain subject
(`ucan.v1_authorization.chain`, UCAN 417f7466): an untrusted issuer, or a delegator that
holds the delegate ability, never becomes a root. For `kiln://implementation/v2/` and
`kiln://source/v1/` requests, the walk
also enforces these rules, where *covering grant* means any capability whose resource is
a byte prefix of the request resource:

1. **Explicit delegate.** Each covering grant in a token with proofs was issued by an
   intermediate delegator. Some parent proof must give that delegator a capability whose
   ability is exactly the family's delegate ability and whose resource is a byte prefix
   of the delegated grant. The capability must have no caveats.
   - Implementation v2 uses `kiln/delegate`.
   - Source v1 uses `kiln/source/delegate`.

   A wildcard ability, the other family's delegate ability, or a delegate capability over
   a sibling prefix does not count. A trusted root needs no delegate capability. A token
   with several proofs passes if any of its parents holds the delegate.
2. **Pinned, aligned grants.** Every covering grant in the chain, including root-issued
   ones, must be inside the family, pin the audience and generation, and stop on a
   segment boundary of the family grammar. A grant such as `kiln://`, the bare family
   prefix, or an audience-only prefix denies, so no grant survives a generation bump. A
   grant cut inside a segment would also cover unrelated siblings, so it denies too.
3. **Depth.** The number of tokens with proofs on any path from the leaf to a root must
   not exceed `Verifier::max_delegation_depth`. With `0`, only root-issued leaves are
   admitted. Revalidation applies the verifier's current limit.

UCAN still enforces signatures, audience linkage, time and revocation of every proof, and
capability attenuation. A child can only narrow the resource and keep the ability.

`kiln://implementation/v1/` keeps its compatibility rule: any verified chain reaching a
trusted root, with no alignment or depth requirement. However, if any covering grant in a
v1 chain is wider than `kiln://implementation/v1/` (for example `kiln://`), every
delegator in that chain must hold explicit `kiln/delegate` over the grants it issued.
`Verifier::implementation_v1 = LegacyFraming::Refuse` closes the compatibility window:
v1 requests deny. The default is `Accept`. A v1 grant never covers a v2 or source
request, because the prefixes differ.

## Effective capabilities

`Session::effective_capabilities` and `SourceSession::effective_capabilities` contain two
kinds of leaf capability:

1. **Covering grants**: every leaf grant whose resource is a byte prefix of the admitted
   request. These authorized the request and passed the chain rule, so they are always
   kept, including wide v1 grants such as `kiln://`. An admitted request therefore never
   has an empty set.
2. **Scoped grants**: any other leaf grant that both
   - starts with the session's scope prefix, and
   - passes the same family's chain rule when evaluated with its own resource.

   The scope prefix is:
   - for an implementation v2 session, `kiln://implementation/v2/` + `text(audience)` +
     `number(generation)` of the verifier;
   - for a source session, `kiln://source/v1/` + `text(audience)` + `number(generation)`
     + `text(repository)` + the request's context (`n/`, or `a/` with attempt and fence).

   A v1 session has no scope prefix and exposes only covering grants.

So grants pinned to another generation or audience, another repository, another source
context, or another family never appear. A delegator holding `kiln/source/delegate` only
over `a` may name `secret` in a leaf token; that grant never appears either. The set is
computed before replay and is not transferable authority.

The consumer still owns these checks:
- mapping the set onto its own scope types;
- the ability of each grant;
- matching an implementation grant to the current project, work, attempt and fence;
- view ceilings;
- the snapshot a share is bound to.

A session stores the set from admission. Revalidation does not recompute it.
- Under `LegacyFraming::Refuse`, a v1 session fails revalidation outright.
- v2 and source sessions never contain v1 grants.

In admission and revalidation, per-grant chain checks share one verified proof graph, so
proof-verification work does not grow with the number of leaf grants. Admission first
runs the full UCAN enforcement path, including the holder signature, against a replay
port that refuses after recording success. A forged signature therefore stops before any
Basalt chain walk, and the single consuming replay remains the last step.

A subtree grant cannot pin a snapshot, because the snapshot is framed after the terminal
marker. Kiln must enforce a share's snapshot itself.

## Exact binding

Rules enforced before replay:

- the deployment generation matches the verifier and is non-zero;
- the request audience matches the configured audience;
- the subject and every framed text field obey the text rule;
- the selected contract admits the framing prefix and the operation's ability;
- the canonical resource is at most 4096 bytes;
- a candidate identity is non-zero exactly for `CandidateSubmit`, `VerificationAdmit`,
  `PublicationPublish`, `IntegrationLand` and `LifecycleComplete`, and zero for claim,
  dispatch and cancellation;
- a source request for read, modify or create names exactly one entry.

The caller's `request_digest` is the digest of the canonical payload it intends to
execute. Basalt binds it; the receiving owner recomputes it. For source requests the
snapshot and view binding are also framed after the path.

## Verified ingress

`Verifier::verify` mints a `Session`, and `Verifier::verify_source` a `SourceSession`, only
after signature, proof, holder, trusted-root, chain-rule, policy, time, revocation and
final replay checks. Neither session is `Clone` or serializable, and neither has a
constructor outside the module. A serialized request, a redacted receipt or a stored
session cannot construct live authority.

`Verifier::revalidate` and `Verifier::revalidate_source` recheck signatures, the full
proof chain including the chain rule, time, current revocations, caveat evaluation, the
loaded policy and the deployment generation before a later effect. They perform no second
replay commit; the session's single final replay admission remains the only one recorded.

## Caller responsibilities

Callers own keys, proof loading, revocation currentness, monotonic time, durable replay
storage and effect recovery. Kiln owns current work/fence/policy facts, claims, budgets,
grant issuance and lineage records, view restriction and handles, verification
acceptance, publication, integration and lifecycle completion. Kiln also configures
`max_delegation_depth` and `implementation_v1` from its reviewed authority
configuration. Lattice owns execution
admission and confinement.

## Non-claims

- A `Session`, receipt, policy artifact or redacted projection is not a transferable
  grant and cannot authorize another request.
- Revalidation does not refresh, extend or reissue authority.
- This adapter proves neither effect completion nor independent acceptance.
- Unknown operations, unknown policy contracts and generic document or process authority
  deny; there is no fallback mapping.
- The chain rule proves only that delegation was explicitly permitted along the proof
  chain. It does not prove budget lineage, issuance intent, worker incarnation or that a
  delegated scope matches a view ceiling; Kiln owns those checks.
- Source scope covers paths inside one repository and snapshot rule. It does not prove
  that a path exists, that a view restricts disclosure, or that two snapshots are
  related.
