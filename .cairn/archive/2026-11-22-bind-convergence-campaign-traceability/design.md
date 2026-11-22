## Context

Two accepted specs were promoted from archived changes on 2026-11-20 and
2026-11-21. Their implementation landed in commit `203296d4` and later, but the
traceability markers did not. `cairn traceability coverage` treats the accepted
requirements as arbitrary claims with no evidence and fails. The previous change
`bind-implementation-authority` established the remedy: bind accepted IDs to the
exact implementing function and the exact test, without touching the specs.

## Decisions

### Decision: Markers only where the code and test already exist

**Choice:** Read every candidate test body and every scenario before adding a
marker. A marker names a test only when the test's assertions check the
scenario's given/when/then facts. Where no test checks the scenario, write one
test that follows the scenario, positive and negative together where the
scenario states both.

**Rationale:** A wrong verification marker is worse than a missing one: it
asserts evidence that does not exist. Coverage is a claim about the relation
between accepted text and checked behaviour, not a checklist to fill.

### Decision: Bind requirements to their pure decision or effect boundary

**Choice:** Bind each requirement to the pure function that decides it or to the
adapter function that owns the effect boundary:

- intake to `kiln-adapter-cairn::convergence::admit_convergence_receipt`;
- selection, campaign minting and Verify planning to the `kiln-core::convergence`
  functions;
- realize lowering to `EvidenceBuildRequest::into_build_effect_plan` and to the
  Mantle profile's plan-keyed `binding_for`, plus the closed-world
  `admit_realization_receipt`;
- bounded dispatch to `MantleRealizeAdapter::dispatch_inner`.

**Rationale:** Each marker names the smallest function that enforces the
requirement's rule, so a reader can go from requirement to decision.

### Decision: Bind non-claims to the only effect a convergence record has

**Choice:** Mark `kiln.convergence.nonclaims` on
`EvidenceBuildRequest::into_build_effect_plan`, the single lowering through which
a receipt, selection, campaign revision or Verify request can reach an effect,
and which yields only a typed build effect for one exact plan. Add a test that
shows a complete receipt plus a planned request yields only typed build data and
that merge admission still needs a passing gate result and current approval.

**Rationale:** The requirement is a negative claim about every convergence
artifact, so it is enforced by what the artifacts can produce, not by a list of
strings. A non-claims constant that no record carries would be dead code bound
only to a marker. The Mantle side's `REALIZE_NON_CLAIMS` differs because the
realize profile and receipt admission use it.

### Decision: Close through the lifecycle, not a spec delta

**Choice:** Run the proposal, design and tasks gates, implement, run the full
check set, then `cairn sync` and `cairn archive` with
`CAIRN_ARCHIVE_DATE=2026-11-22`, recording transcripts under `evidence/`.

**Rationale:** The change binds accepted content; it must not rewrite it. `cairn sync`
with no delta leaves `.cairn/specs/` unchanged.

## Risks / Trade-offs

- A marker that names a test which only exercises an adjacent fact would inflate
  coverage. Every row is checked by reading the assertion, and the added tests
  are the ones the scenarios require.
- The non-claims marker binds the lowering boundary; it adds no field, so the
  campaign identity and every serialized campaign field are unchanged.
- The `moved` scenario has no bound-revision field on `EvidenceRow`; the test
  models a missing job binding and a changed `row_blake3` in `stale`/`unrealized`
  states, which are the facts selection can observe, and asserts they are
  selected rather than treated as satisfied.

## Non-claims

Coverage proves that an accepted ID has a named implementation marker and a
named test marker; it does not prove the test is exhaustive, the implementation
correct or the accepted requirement sufficient. These bindings add no release,
publication or merge authority.
