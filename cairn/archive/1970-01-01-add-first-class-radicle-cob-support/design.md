## Context

Kiln separates source, collaboration, triggers, status, execution, evidence,
and gates. The existing design mentions Radicle patches, broker events, and job
COBs, but it lacks a shared collaborative-object contract.

Cairn revision `837d730799faffe6a088cfd6dc49e08c7e0f1822` provides six custom
COB types through direct `radicle-cob` storage:

- `xyz.onixresearch.cairn-change`
- `xyz.onixresearch.rfd`
- `xyz.onixresearch.adr`
- `xyz.onixresearch.verification-obligation`
- `xyz.onixresearch.release-decision`
- `xyz.onixresearch.policy-exception`

Cairn pins `radicle-cob` `0.20.0` for its current product adapter. Its
interoperability producer also records Radicle `0.24.0`. These versions are
review inputs, not automatic Kiln dependencies.

## Goal and Success Contract

The exact goal is first-class, profile-driven support for Radicle COB
observations, events, projections, and bounded mutation requests.

Completion requires these results:

- The core represents provider identities without importing provider types.
- A typed profile registry admits every supported type and capability.
- The initial set covers native Radicle issues, patches, CI jobs, and all six
  Cairn profiles.
- Unknown or mismatched formats fail before semantic mapping or mutation.
- Observation, event intake, status projection, and mutation stay separate.
- Every mutation is preview-first and binds expected object heads.
- Concurrent heads and semantic conflicts stay visible.
- Candidate, claim, policy, and semantic-state changes invalidate prior use.
- Positive and negative fixtures cover each enabled profile.
- Exact source archives still build and test without Radicle or Cairn runtime
  software.

False completion includes these results:

- a stringly typed `type_name -> JSON` map with no profile contract;
- importing Cairn or Radicle implementations into core crates;
- treating provider IDs as Kiln IDs;
- treating any signature as sufficient authority;
- using last-writer-wins conflict handling;
- treating ADR acceptance, obligation discharge, policy approval, or release
  approval as an automatic Kiln command;
- treating a job COB as the Kiln journal;
- claiming network publication or peer convergence from local fixtures; or
- supporting only the existing job COB status projection.

## Approach Registry

These serial analysis passes are correlated. Deterministic repository checks
remain the completion authority.

| Family | Mechanism | State | Reason |
|---|---|---|---|
| opaque envelope | Keep all COBs as type names plus untyped payload bytes | rejected | It preserves transport but cannot safely expose lifecycle, conflict, candidate, or action semantics. |
| imported reducers | Reuse Cairn and Radicle reducers inside the Kiln core | rejected | It breaks provider neutrality and moves Git, signatures, and product semantics across the core boundary. |
| profile registry | Use a neutral envelope plus versioned profile-specific pure mappings and optional shells | selected | It preserves common identity and capability rules without claiming universal COB semantics. |
| per-type adapters only | Build unrelated adapters without a shared registry | rejected | It duplicates admission, identity, conflict, effect, and non-claim behavior. |

The adversarial audit targets type confusion, stale heads, actor promotion,
semantic drift, mixed schemas, conflict loss, candidate substitution, and status
authority promotion.

## Decisions

### Decision: Use a neutral envelope and typed profiles

**Choice:** The core owns a bounded `CobReference`, `CobObservation`,
`CobEventObservation`, `CobConflict`, and `CobEffectPlan` model.

A profile binds one exact provider, type name, schema line, mapper version,
supported actions, limits, conflict rules, applicability rules, and non-claims.
The registry uses stable profile IDs and typed Nickel configuration.

An unknown type can enter only as an explicit bounded opaque read observation.
It cannot produce semantic state, triggers, status, gates, or mutations.

**Rationale:** COBs share storage mechanics but not lifecycle meaning. The
profile is the smallest boundary that preserves both facts.

### Decision: Keep initial profile semantics explicit

**Choice:** The initial registry includes these semantic families:

| Profile family | Kiln projection | Authority limit |
|---|---|---|
| Radicle issue | proposed work, discussion, labels, lifecycle, conflicts | not a task definition, gate, or execution command |
| Radicle patch | proposed change, revisions, review facts, merge observations | not source truth, merge authority, or candidate identity |
| Radicle CI job | run-status and log-link projection | not journal or execution authority |
| Cairn change | lifecycle artifacts, heads, evidence references, conflicts | not accepted requirements or implementation proof |
| Cairn RFD | document lifecycle, discussion, links, and numbering facts | not specification authority or global number truth |
| Cairn ADR | decision lifecycle, review, lineage, and numbering facts | not an automatic Kiln architecture or command change |
| Cairn verification obligation | claim root, evidence roles, findings, and discharge facts | not proof truth or verifier soundness |
| Cairn release decision | immutable candidate, evidence, blockers, votes, and verdict | not release execution or Kiln readiness by itself |
| Cairn policy exception | policy scope, controls, decisions, and validity observations | not ambient authority or automatic Kiln policy bypass |

Each profile has its own mapping tests. A generic reducer cannot infer these
meanings from field names.

**Rationale:** Explicit mappings prevent false equivalence across native and
custom object types.

### Decision: Preserve provider and Kiln identities separately

**Choice:** A COB observation preserves the repository RID or declared local
repository identity, type name, object ID, operation heads, actor and signature
observations, provider schema, and reducer identity.

Kiln derives a separate BLAKE3 identity from the canonical admitted projection.
Equal projections do not imply equal operation history or authority.

**Rationale:** Provider history and CI meaning need stable linkage without
identity aliasing.

### Decision: Split read, event, projection, and mutation paths

**Choice:** The adapter declares separate capabilities for discovery, typed
load, event intake, status publication, creation, update, review action,
resolution, and export.

A read-only path does not acquire a signer. An event can request an admitted
Kiln command, but it cannot grant provider mutation. A mutation starts from a
pure plan and requires fresh shell preflight.

**Rationale:** Observation, event intake, and external mutation use different
authority and failure models.

### Decision: Bind mutations to exact current heads

**Choice:** Every effect plan contains the profile, repository, object,
expected heads, exact action, bounded payload identity, actor reference,
capability identity, policy identity, and idempotency key.

The shell reloads the object before execution. A head, profile, policy, actor,
or capability mismatch rejects the plan before signing.

An interrupted provider call returns success, failure, denial, cancellation,
timeout, provider error, or unknown outcome. The core never collapses unknown
outcome into success or safe retry.

**Rationale:** A preview is safe only when execution checks the same object and
authority facts.

### Decision: Preserve conflicts without a universal merge

**Choice:** The neutral model records current heads and bounded profile conflict
facts. It does not select a winner.

A profile can define a pure resolution request shape. Complete-head profiles
must name every current head. The shell signs only an admitted resolution plan.

**Rationale:** Last-writer-wins can discard signed review, decision, or policy
state.

### Decision: Bind semantic use to exact applicability facts

**Choice:** A COB-derived input used by Kiln binds the exact semantic-state
identity, provider observation, profile, policy, and related Kiln candidate or
claim.

A changed claim root, candidate root, policy identity, object state, or required
head makes the prior result inapplicable. History remains available for audit.

**Rationale:** A valid decision for one candidate or claim says nothing about a
replacement.

### Decision: Keep job COB status as a projection

**Choice:** The existing Radicle job COB path becomes one registry profile, but
its output remains a rebuildable external projection.

A disagreement with the Kiln journal becomes a typed discrepancy observation.
The status object cannot mutate canonical CI state.

**Rationale:** Shared profile mechanics do not transfer status authority.

### Decision: Pin producer contracts and keep runtime independence

**Choice:** Implementation records exact upstream revisions, crate versions,
type names, schemas, reducers, fixtures, and license facts for each profile.

Cairn is a producer and contract reference. Kiln can consume published format
fixtures or an optional adapter crate, but core and standalone builds cannot
require a Cairn checkout, Radicle node, profile, identity document, or network.

**Rationale:** A sibling checkout is useful for design review but is not a
portable product dependency.

## Functional Core and Imperative Shell

Pure core functions own:

- profile and capability admission;
- typed external-reference validation;
- canonical projection identity inputs;
- event-to-command mapping decisions;
- conflict and malformed-state classification;
- candidate and claim applicability;
- mutation-plan construction and stale-plan rejection; and
- bounded result and non-claim projection.

Thin shells own:

- Git and Radicle repository access;
- `radicle-cob` storage traits and provider reducers;
- nodes, subscriptions, polling, and transport;
- keystores, signatures, profiles, identity documents, and credentials;
- payload reads and BLAKE3 computation;
- fresh preflight, signing, mutation, and reconciliation; and
- provider-native encoding and diagnostics.

No shell can dispatch a COB mutation without a pure admission result.

## Data Flows

Read and trigger flow:

```text
provider repository or event
  -> typed load + provider reduction in shell
  -> bounded COB observation
  -> pure profile mapping and applicability
  -> optional Kiln command or external projection
```

Mutation flow:

```text
Kiln state + COB observation + requested action + capability facts
  -> pure effect plan with expected heads
  -> fresh shell preflight
  -> signed provider mutation
  -> bounded provider result or unknown outcome
  -> canonical Kiln completion command
```

## Configuration

Typed Nickel contracts define:

- profile ID, provider, type name, schema line, reducer, and upstream pin;
- enabled read, event, projection, and mutation capabilities;
- allowed actions and role requirements;
- object, operation, head, payload, field, collection, and text bounds;
- conflict and resolution behavior;
- applicability and invalidation rules;
- retry, timeout, reconciliation, and unknown-outcome policy;
- opaque-read behavior for unknown types; and
- required non-claims and fixture identities.

Missing, conflicting, unsupported, or drifting values fail before adapter
startup.

## Validation

Positive validation includes every initial profile, exact identity retention,
read-only observation, event mapping, status projection, fresh mutation plans,
complete conflict resolution, candidate binding, and fixture round trips.

Negative validation includes unknown types, wrong schemas, mixed object kinds,
malformed history, forged actors, invalid signatures, missing capabilities,
stale heads, partial resolutions, oversized payloads, candidate substitution,
status disagreement, dependency leakage, transport failure, and unknown outcome.

Independent interoperability evidence uses pinned producer fixtures and a
separate read-only consumer. Cairn fixtures remain producer evidence only.

## Risks and Trade-offs

- A profile registry adds configuration and conformance work. It avoids a false
  universal COB schema.
- Upstream formats can change. Exact pins and drift rejection convert change
  into a visible compatibility decision.
- Semantic mappings can overstate provider facts. Profile non-claims and
  negative authority tests limit this risk.
- Local signatures do not prove remote state or trust. Receipts retain this
  boundary.
- Direct event mode can require a node shell. The core remains independent and
  broker mode stays available.

## Claim Boundary

Passing this change proves only the checked profile admission, identity
mapping, pure decisions, fixture compatibility, and bounded local provider
observations.

It does not prove remote publication, synchronization, peer convergence, actor
trust, signature-policy sufficiency, semantic correctness, evidence truth,
provider execution, merge success, release eligibility, or future format
compatibility.
