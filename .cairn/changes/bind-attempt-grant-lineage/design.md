## Design

### Layer split

- `kiln-core::implementation::grant` decides. It reads explicit facts and never
  mints, signs, stores, revokes or resolves a token. A binding holds proof
  references (BLAKE3 of the compact token), never token bytes.
- `kiln-core::implementation::attempt` retains one binding per attempt.
- `kiln-implementer::grant_issuance` folds journal records into the effects a
  live shell still owes. It is pure and not wired; S6 composes it with the #61
  `AdmissionCommand::Transfer` transaction, minting and revocation storage.
  Any durable store stays behind the shell's port, so the planned
  `durable-authority-state` adapter can back it without changing this core.

### Contract `kiln.attempt-authority-binding.v1`

`AuthorityCeiling`, `AttemptAuthorityBinding` and `LineageRefusal` follow the
plan's C5 exactly. Additional types:

- `CeilingTerms`: the requested ceiling before its generation and identity are
  fixed. A child's generation is always its parent's.
- `GrantBudget { parent_budget_identity, account }`: the C5 `budget` field.
- `AttemptLineage`: the current attempt facts a lineage must match.
- `AdmittedGrant`: an admitted, not yet issued grant (the issuance intent). It
  carries every binding field except `grant_ref` and `identity`, plus an
  `intent` identity. `issue(grant_ref)` yields the binding.
- `LineageError = Invalid(Diagnostic) | Refused(LineageRefusal)`: malformed
  input stays distinct from the closed refusal set.

`admit_child` therefore returns an `AdmittedGrant`, not a finished binding: the
proof reference is the BLAKE3 of a token that exists only after minting, and
recovery must re-derive the same intent identity whether or not a token was
minted. This is the one deviation from the plan's wording "returning a child
binding".

### Identities

All identities are BLAKE3 over a domain-separated encoding: text is
length-prefixed, integers are big-endian, options carry a presence byte, and
sets carry their count in canonical order. Operations hash as their Basalt ids.

- Ceiling: `kiln.authority-ceiling.v1` over every ceiling field.
- Intent: `kiln.attempt-authority-grant-intent.v1` over every binding field
  except `grant_ref` and `identity` (the ceiling by its identity).
- Binding: `kiln.attempt-authority-binding.v1` over the same fields plus
  `grant_ref`.

`validate()` recomputes each identity, so a copied DTO that was edited fails.

### Depth

`max_depth` is the number of further delegation hops below a binding. A root
has depth 0; a child has its parent's depth plus one and a `max_depth` strictly
below its parent's. `depth + max_depth` never exceeds `MAX_GRANT_DEPTH` (8),
which also bounds every chain walk.

### Roles

`GrantRole::of` partitions the nine operations into coding (claim, renew,
dispatch, submit, cancel), verification, and publication (publish, integrate,
lifecycle), matching the separate coordinator, verifier and publisher holder
keys. A root ceiling may hold several roles (the coordinator). A child holds
exactly one role, and only a role its parent holds; otherwise it is
`role_escalation`.

### Check order

`admit_child` checks in a fixed order, so one request yields one refusal:

1. `check_current(parent, ancestors)`:
   - the chain is at most `MAX_GRANT_LINEAGE` long (`depth_exceeded`);
   - every link recomputes, links root first by parent identity and depth,
     keeps the root policy, and is covered by its predecessor (`foreign_lineage`);
   - every link binds the current fence (`stale_generation`) and the current
     campaign revision, work, version and attempt (`foreign_lineage`);
   - every link carries the current authority generation, was admitted at or
     before the observed revocation generation, and has an unrevoked proof
     (`stale_generation`);
   - every link is unexpired (`outlives_parent`).
2. The verified delegator is the parent's holder (`foreign_lineage`).
3. The delegator holds explicit delegation permission and the parent ceiling
   permits another hop (`delegation_not_permitted`).
4. The child ceiling is well formed (`Invalid`).
5. Role (`role_escalation`), then a child work scope for other work
   (`foreign_lineage`).
6. Attenuation: audience (`widen_audience`), project, work scope and disclosure
   routes (`widen_resource`), operations (`widen_operation`), lifetime
   (`outlives_parent`), depth (`depth_exceeded`).
7. The child is valid now (`Invalid`).
8. Budget: the request equals the owner-confirmed reservation and either shares
   the parent's account or is a disjoint account transferred from it
   (`budget_unbound`).

Disclosure-route widening maps to `widen_resource`, because routes are
resources and C5's refusal set is closed. A predecessor fence maps to
`stale_generation` and is checked before the attempt identity, which also
changes with the fence.

`admit_root` checks the ceiling, generation, expiry, work scope and confirmed
reservation, and binds the current attempt with depth 0.

### Revocation and expiry propagation

`check_current` walks the whole chain, so revoking or expiring any ancestor
invalidates every descendant. A child's lifetime never exceeds its parent's.
The issuance fold applies the same rule to its journal: a revoked grant makes
each issued descendant owe a `Revoke` and each pending descendant an `Abandon`.

### Issuance fold

Records are `Intended { grant }`, `Issued { intent, grant_ref }` and
`Revoked { grant_ref, revocation_generation }`. Folding is idempotent. It
refuses:

- an intent that does not recompute;
- one account reserved from two parent budgets;
- an issuance for an unknown intent;
- a second proof for one intent;
- one proof for two intents.

`reconcile(now_s)` returns `Issue` for an unissued, current intent. The shell
first looks for a token already minted for that intent rather than minting
again. Replaying records issues nothing. Two grants sharing one reserved
account share its #61 allowance; they cannot multiply it, because #61 enforces
the account.

### Basalt B1 request (draft for the Basalt owner)

Not filed; for the owner's decision O1.

> **Attenuable Kiln implementation framing `kiln://implementation/v2/`.**
> Basalt v1 frames the holder first in the resource (`src/kiln.rs:223-230`),
> and UCAN coverage is a byte prefix (`verified-logic/src/scope.rs:62-66`), so
> a ceiling held by one holder can never cover another holder's request. The
> only cross-holder chain tested is root-wide `kiln://implementation/v1/` to an
> exact leaf (`tests/ucan_enforcement/kiln.rs:324-385`).
>
> Request:
>
> 1. Add a v2 framing without the holder segment. The holder stays bound by
>    UCAN audience equality (`kiln.rs:298`) and the holder signature. Order the
>    segments `audience/generation/project/work/work_version/attempt/fence/target/digest/candidate`,
>    each length-framed, so a byte-prefix grant expresses project, work and
>    attempt ceilings.
> 2. In `check_roots` (`kiln.rs:394-440`), require every intermediate delegator
>    to hold an explicit `kiln/delegate` capability over the delegated prefix,
>    and enforce a declared `max_depth`.
> 3. Keep v1 verifiable for a stated compatibility window.
> 4. Fixtures: scoped cross-holder delegation admits; widening, missing
>    delegate and depth overflow refuse.
>
> Kiln's matching contract is `kiln.attempt-authority-binding.v1`: the ceiling
> prefix maps to `project/work`, `max_depth` to the delegation depth, and
> `grant_ref` to the UCAN proof reference.

### Non-claims

This change proves the pure lineage decisions and the fold over local fixtures.
It does not prove verified UCAN delegation, durable revocation, crash recovery,
concurrent issuance against a shared #61 parent, or any live grant lifecycle.
Those belong to S1, S6, Basalt B1 and the #49 qualification.
