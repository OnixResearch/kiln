## ADDED Requirements

### Requirement: Immutable four-role evaluation request

r[kiln.semantic_merge.request]
A semantic merge check MUST be requested by one immutable request with four role-tagged sources (base O, variant A, variant B, candidate M). Each source MUST carry its exact source and canonical tree identity and its native provider observation. The request MUST also carry:
- the semantic subject, property and profile;
- the property assumptions;
- the dependency and model identities;
- the checker and toolchain identities;
- the policy revision;
- named resource bounds.

Admission MUST compute a role-tagged BLAKE3 binding over every component. Any change MUST produce a different identity.

#### Scenario: Frozen quartet admission

r[kiln.semantic_merge.request.scenario.frozen]
- GIVEN four exact Pijul channel states acquired through the read-only profile and a complete request
- WHEN the request is admitted twice
- THEN both admissions MUST produce the same binding identity, and every role component MUST be distinct and role-tagged

#### Scenario: Swapped roles

r[kiln.semantic_merge.request.scenario.swapped]
- GIVEN an admitted request and the same request with A and B exchanged, or a source placed in a slot whose role tag differs
- WHEN the bindings are compared or the mislabeled request is admitted
- THEN the comparison MUST name the variant component as the failed binding, and the mislabeled request MUST be refused

#### Scenario: Malformed request

r[kiln.semantic_merge.request.scenario.malformed]
- GIVEN a request with an unsupported contract version, an empty or unsorted coverage set, unsorted assumptions, a zero bound, or a bound above the policy ceiling
- WHEN it is admitted
- THEN admission MUST fail before any effect, with a diagnostic naming the field

### Requirement: Explicit lineage selection

r[kiln.semantic_merge.lineage]
Base and variant selection MUST be explicit. Kiln MUST NOT infer or substitute a merge base. Under the Pijul change-containment policy:
- every role MUST carry Pijul native channel-state facts;
- O's change set MUST be contained in both A and B;
- A ∪ B MUST be contained in M, within a named bound on extra candidate changes;
- when the policy requires the greatest common base, O MUST equal A ∩ B.

Ambiguous or unavailable lineage MUST be refused. A single change identity MUST NOT stand for an entire state.

#### Scenario: Contained lineage

r[kiln.semantic_merge.lineage.scenario.contained]
- GIVEN Pijul states where O ⊆ A, O ⊆ B, O = A ∩ B and A ∪ B ⊆ M
- WHEN the request is admitted under containment
- THEN admission MUST succeed and preserve each role's channel, state and ordered change sequence

#### Scenario: Ambiguous or unavailable lineage

r[kiln.semantic_merge.lineage.scenario.refused]
- GIVEN a base not contained in a variant, variants sharing a change outside the base, a candidate missing a variant change, a role without Pijul facts, or a change identity used as the exact revision
- WHEN the request is admitted under containment
- THEN admission MUST fail with a distinct lineage diagnostic and request no effect

### Requirement: Authorized frozen candidate acquisition

r[kiln.semantic_merge.candidate]
Candidate M MUST be supplied by a supplier that the policy admits and frozen as an exact resolution through an admitted read-only acquisition profile. The supplied state MUST equal M's exact revision. Every role's acquisition profile MUST hold only discover, resolve-exact, acquire and observe-identity capabilities, with immutable resolution. The existing Pijul source adapter MUST NOT gain apply, record, reset, push, merge or any other mutation operation. Its profile MUST continue to refuse every forbidden capability.

#### Scenario: Unauthorized candidate

r[kiln.semantic_merge.candidate.scenario.unauthorized]
- GIVEN a candidate from a supplier the policy does not admit, a supplied state different from M, or an acquisition profile with mutation, collaboration, trigger, status or credential capability
- WHEN the request is admitted
- THEN admission MUST fail with a diagnostic naming the candidate supply or acquisition authority

#### Scenario: Read-only adapter refusal

r[kiln.semantic_merge.candidate.scenario.adapter]
- GIVEN the pinned Pijul source profile
- WHEN each forbidden capability is requested, or a profile enabling network, credentials, collaboration, triggers, status publication or mutation is admitted
- THEN each request and admission MUST be refused, and the repository port MUST expose only channel discovery and exact read acquisition

### Requirement: Provider-port dispatch

r[kiln.semantic_merge.dispatch]
Each checker attempt MUST be expressible as a versioned Lattice workflow plan whose operation identity is the request identity and whose idempotency key is the attempt identity. The plan MUST pass existing workflow-plan validation. The checker producer MUST be a provider port with bounded, cancellable checks. This change MUST admit only a fixture producer.

#### Scenario: Frozen positive fixture

r[kiln.semantic_merge.dispatch.scenario.e2e]
- GIVEN the Nickel-authored frozen quartet fixture, resolved through the Pijul adapter mapping
- WHEN Kiln admits the request, dispatches a checker request to the fixture producer, admits the result evidence, re-observes inputs and decides the gate
- THEN the required gate MUST be satisfied by a fresh verified result bound to all four roles

### Requirement: Modes and outcome lattice

r[kiln.semantic_merge.outcomes]
Checks MUST declare an advisory or required mode. Kiln MUST preserve these outcomes: verified, counterexample, unknown, unsupported, exhausted, cancelled, stale and tool-error. In required mode, only a fresh, applicable, fully covering verified result from the admitted checker MUST satisfy the gate. Advisory mode MUST neither block nor satisfy. Unknown MUST NOT count as success or as a demonstrated conflict.

#### Scenario: Non-verified outcomes

r[kiln.semantic_merge.outcomes.scenario.nonverified]
- GIVEN results that are counterexample, unknown, unsupported, exhausted, cancelled, stale or tool-error
- WHEN a required gate is decided
- THEN the gate MUST NOT be satisfied, the outcome MUST be preserved, and only counterexample MUST be marked as a demonstrated conflict

#### Scenario: Advisory mode

r[kiln.semantic_merge.outcomes.scenario.advisory]
- GIVEN an advisory check with any outcome
- WHEN the gate is decided
- THEN the verdict MUST report the outcome without blocking or satisfying any requirement

### Requirement: Bound attempts, evidence, retries and caches

r[kiln.semantic_merge.binding]
Attempts, logs, products, retries and caches MUST be bound to all four role-tagged identities and to the property, assumptions, dependencies, model, checker and policy. Result admission MUST reject any mismatch with a diagnostic naming the failed binding. Admission MUST also reject:
- missing coverage for verified;
- substitute evidence;
- invalid artifact bindings.

Partial output MUST record tool-error. Retry MUST be classified by policy. A cache entry MUST be reused only for the identical request and checker, and only for deterministic outcomes.

#### Scenario: Changed inputs

r[kiln.semantic_merge.binding.scenario.changed]
- GIVEN a result bound to a different base, candidate, dependency set, policy, checker or property
- WHEN it is observed
- THEN it MUST be rejected with a diagnostic naming that component

#### Scenario: Coverage and partial output

r[kiln.semantic_merge.binding.scenario.coverage]
- GIVEN a verified result missing a required coverage unit, or a result with incomplete output or a product bound to another attempt
- WHEN it is observed
- THEN missing coverage and invalid artifacts MUST be rejected, and incomplete output MUST record tool-error, not verified

#### Scenario: Cache and retry

r[kiln.semantic_merge.binding.scenario.cache]
- GIVEN a cached verified result for the same request and checker, a cached result for another request, a cached non-deterministic outcome, and tool-error and exhausted outcomes under a bounded retry policy
- WHEN cache reuse and retry are decided
- THEN only the identical deterministic entry MUST be reused, the mismatched entry MUST name its failed binding, and retries MUST stop at the policy bound with a new attempt identity each time

### Requirement: Freshness, cancellation and replay

r[kiln.semantic_merge.freshness]
Kiln MUST reject results from cancelled, superseded or completed attempts as late, and results naming another attempt as replays. Once a re-observation shows any changed input, the evaluation MUST become stale, naming the changed component. Every later result MUST be rejected. A gate decision MUST compare the result binding with a fresh re-observation.

#### Scenario: Late, replayed and cancelled results

r[kiln.semantic_merge.freshness.scenario.late]
- GIVEN a cancelled attempt, a superseded first attempt after retry, and a result copied from another attempt
- WHEN their results are observed
- THEN each MUST be rejected, and none MUST change the recorded outcome

#### Scenario: Concurrent source change

r[kiln.semantic_merge.freshness.scenario.concurrent]
- GIVEN a dispatched attempt and a re-observation in which a role's channel state has moved
- WHEN the re-observation and then the original result are processed
- THEN the evaluation MUST become stale for that role, the pending attempt MUST be cancelled, and the result MUST be rejected

### Requirement: Independent gates and non-claims

r[kiln.semantic_merge.independence]
The semantic gate MUST be combined with independently requested build, test, lint and review gates without substituting for them. A clean text merge, equal source trees, passing tests or an accepted evidence envelope MUST NOT satisfy the required semantic result. Each verdict MUST carry a revalidation obligation for the exact candidate and expected predecessor context. A downstream merge or publish operation MUST revalidate that obligation under its own race-safe protocol. This change MUST grant no mutation authority. CI success MUST NOT claim:
- a semantic guarantee outside the checked relation;
- source trust;
- automatic merge authority;
- blanket release eligibility.

#### Scenario: No substitution

r[kiln.semantic_merge.independence.scenario.substitution]
- GIVEN passing build, test, lint and review gates with an unknown semantic result, or a verified semantic result with a failing build gate, or substitute evidence
- WHEN the gates are combined or the evidence is observed
- THEN the combination MUST fail, and substitute evidence MUST be rejected

#### Scenario: Downstream revalidation

r[kiln.semantic_merge.independence.scenario.revalidation]
- GIVEN a satisfied verdict and a downstream observation in which the candidate or an expected predecessor moved
- WHEN the revalidation obligation is checked
- THEN the check MUST fail naming the moved role, and the verdict MUST NOT grant mutation authority
