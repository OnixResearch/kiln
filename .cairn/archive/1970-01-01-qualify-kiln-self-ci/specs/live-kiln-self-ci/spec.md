## ADDED Requirements

### Requirement: Dedicated self-CI fork authority

r[kiln.self_ci.authority] The qualification MUST create a dedicated fork RID from the exact Kiln source under fresh canary identities, on the declared remote host/root, with finite source, process and build bounds. It MUST NOT push to or mutate the canonical Kiln repository, its `main`, delegates, threshold, visibility, mirrors, governance, existing Radicle homes or production services. It MUST refuse pre-existing state, an undeclared host, a missing base commit or unavailable mutation authority before source, signing or network effects.

#### Scenario: Dedicated fork starts

- GIVEN a fresh local/remote root, a declared host/tailnet and the exact base commit
- WHEN the canary creates the fork and its identities
- THEN every effect MUST stay inside the fork and the declared roots.

#### Scenario: Canonical or non-fresh state is requested

- GIVEN the canonical RID, pre-existing fork state, an undeclared host or a missing base commit
- WHEN admission runs
- THEN it MUST reject before any source, signing or network effect.

### Requirement: Exact Kiln source and sufficient bounded limits

r[kiln.self_ci.source] The fork MUST be created from the committed Kiln history at a recorded base commit, with no uncommitted working-copy content. The source selection MUST use finite typed bounds sufficient for the measured tree and MUST fail closed when the entry, file or total byte bound is exceeded. The exercised revisions MUST be real Kiln source commits on the fork's `ci-canary`; a synthetic `jobs.app`, `value.txt`, fixture provider or fabricated source MUST NOT substitute.

#### Scenario: Full Kiln tree is acquired

- GIVEN the recorded base commit and raised finite source bounds
- WHEN the CI acquires and materializes the fork source
- THEN the exact revision and real tree MUST reach the worker within the measured bounds.

#### Scenario: Source exceeds a bound or differs from the base

- GIVEN an over-bound tree, a dirty clone or a source revision outside the fork branch
- WHEN the source criterion is evaluated
- THEN it MUST fail closed rather than truncate, synthesize or fall back.

### Requirement: Real Kiln package build and genuine failure

r[kiln.self_ci.build] The discovery MUST select `packages.x86_64-linux` and the `kiln` task through the existing adapters. A successful case MUST produce the real package output with the exact rooted product containing `bin/kiln`, canonical `Success` and matching remote status. A failing case MUST introduce a source-controlled Rust syntax error and produce its genuine Nix compiler diagnostic, canonical `DomainFailure` and matching remote failure status. No hardcoded `jobs.app` output, fixture build or fabricated result MAY be admitted.

#### Scenario: Kiln builds its product

- GIVEN a real source revision on the fork
- WHEN discovery and build run through the unchanged adapters
- THEN the exact `kiln` package output and root MUST be retained and admitted.

#### Scenario: Kiln source genuinely fails

- GIVEN the source-controlled syntax error revision
- WHEN the real Nix build runs
- THEN the compiler diagnostic, canonical failure and remote failure status MUST agree.

### Requirement: Live duplicate, host-loss and unavailable recovery

r[kiln.self_ci.recovery] During real fork delivery the canary MUST exercise duplicate native fetch, local controller/host loss while the worker executes, remote publisher restart/reconnect and unavailable-provider uncertainty. Canonical recovery MUST preserve the original operation and exactly one worker invocation per genuine source revision. The unavailable case MUST retain no final status and no fabricated broker completion.

#### Scenario: A running self-CI event is recovered

- GIVEN a real fork source event and an in-flight worker
- WHEN the controller host is killed, restarted and reconciled and the remote publisher restarts
- THEN the original operation and product result MUST remain with no redispatch.

#### Scenario: Duplicate or unavailable state cannot establish acceptance

- GIVEN a replayed fetch or an unavailable provider
- WHEN recovery and finite observation run
- THEN uncertainty MUST remain and no new operation, success or final status MAY be invented.

### Requirement: Bounded self-CI evidence

r[kiln.self_ci.evidence] Kiln maintainers MUST retain the base commit, clone/fork identity, measured source shape and raised bounds, selector/task, real source revisions, broker ingress, canonical journals, package products and compiler diagnostics, remote status readback, duplicate/restart/host-kill observations, root discharge, rejected attempts, retirement and the exact repeat command. They MUST run required gates and synchronize/archive only after actual prerequisites pass.

#### Scenario: Self-CI repeats

- GIVEN the recorded base commit, cohort, host prerequisites and fresh state
- WHEN all declared cases and gates execute
- THEN the retained evidence MUST reproduce the fork source-to-product-to-status path.

#### Scenario: A prerequisite is absent

- GIVEN a missing base commit, source bound, remote binary or host
- WHEN the qualification is evaluated
- THEN the missing criterion MUST remain explicit and no fixture or historical result MAY replace it.

### Requirement: Bounded self-CI claims

r[kiln.self_ci.claims] The evidence MUST identify the run as a fork-based Kiln-source self-CI qualification over a private tailnet between trusted hosts. It MUST NOT promote it to canonical publication, delegate or governance change, hostile-node isolation, WAN behavior, production deployment, activation or release authority.

#### Scenario: Evidence is interpreted

- GIVEN the retained fork and product observations
- WHEN claims are projected
- THEN they MUST name the fork boundary, trusted hosts and every excluded stronger claim.

#### Scenario: A stronger claim is requested

- GIVEN a request to call the run canonical, production or release-qualified
- WHEN the evidence is evaluated
- THEN the request MUST be rejected and the fork/private-overlay boundary retained.
