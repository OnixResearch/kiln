# Maintained OnixOS CI Pilot Specification

## ADDED Requirements

### Requirement: Explicit immutable Onix discovery source

r[kiln.onixos_pilot.source] An explicitly selected Onix evaluator profile MUST bind one immutable Nix store source root, its flake-byte BLAKE3, exact root selector and platform independently of the fixed evaluator/plugin cohort. Pure request admission MUST match that binding; shell preflight MUST measure the bound flake before roots or execution. Stock selection and serialization MUST remain unchanged.

#### Scenario: A real source-bound consumer

- GIVEN a non-fixture OnixOS consumer frozen in the Nix store and an exact explicit source binding
- WHEN request admission and preflight run
- THEN the request MAY proceed through the existing bounded Onix shell.

#### Scenario: Source or selector drifts

- GIVEN a request whose source, flake bytes, selector or platform differs from its bound profile, or a mutable source path
- WHEN admission or preflight runs
- THEN the request MUST fail before GC-root creation or evaluator execution.

### Requirement: Authenticated operator request precedes effects

r[kiln.onixos_pilot.trigger] The isolated pilot MUST authenticate an exact bounded operator request using a fresh explicitly allowlisted Ed25519 key before any host/provider effect. The signature MUST bind the request domain, source and cohort identities and freshness window. The existing versioned operator ingress MUST carry genuine published Radicle source facts; independently selected OnixOS inputs MUST remain explicit.

#### Scenario: Authenticated bounded initiation

- GIVEN an exact signed current request and the admitted operator key and cohort
- WHEN initiation runs
- THEN the coordinator MAY persist canonical start intent and submit the existing operator ingress.

#### Scenario: Tampered or stale initiation

- GIVEN changed bytes, a wrong key, wrong cohort or an expired request
- WHEN initiation runs
- THEN no new host or provider operation MUST be submitted.

### Requirement: Real maintained-cohort discovery and build

r[kiln.onixos_pilot.runtime] The pilot MUST execute the maintained Aspen host and extension, real Lattice workflow and exact Nix build provider with measured source, executable and profile identities. Its selected artifact MUST be an existing OnixOS application generated from real inventory and Nickel service modules. Discovery-only fixtures, synthetic transport handlers and echo-only derivations MUST NOT substitute for the consumer build.

#### Scenario: Real application succeeds

- GIVEN the exact frozen consumer and locally provisioned admitted input closure
- WHEN authenticated discovery and rooted build complete
- THEN retained evidence MUST identify actual discovery tasks, provider operations, native results and exact rooted application outputs.

#### Scenario: A cohort input is unavailable or incompatible

- GIVEN missing or incompatible source, evaluator, plugin, workflow or local build inputs
- WHEN the pilot reaches that boundary
- THEN it MUST fail closed and retain the actual diagnostic without fallback or a successful-build claim.

### Requirement: Canonical journal backs CI observation

r[kiln.onixos_pilot.journal] The coordinator MUST use existing pure CI commands and transitions, persist start intent before dispatch and admit terminal events only from exact host and actual worker/build observations. A bounded versioned canonical event journal MUST reconstruct the observed state. Neither synthetic State construction nor an Aspen operation ledger alone MAY establish canonical CI outcome.

#### Scenario: Real completion becomes observable

- GIVEN an exact terminal host acknowledgement and matching real worker/build result
- WHEN canonical completion is admitted and journaled
- THEN replay MUST reconstruct the run and attempt outcome exposed by observation.

#### Scenario: Provider truth is absent

- GIVEN only a process exit, an unmatched report, an interrupted intent or an unknown host outcome
- WHEN completion admission runs
- THEN successful canonical CI completion MUST NOT be synthesized.

### Requirement: Duplicate and restart recovery preserve the original operation

r[kiln.onixos_pilot.recovery] The isolated pilot MUST redeliver the same logical request and restart its maintained host/controller while the independently owned real workflow continues. Recovery MUST reconcile the original operation without normal redispatch. Output GC roots MUST remain until durable result/journal handoff and MUST support explicit idempotent release.

#### Scenario: Duplicate and host restart

- GIVEN one persisted operation and an independently running provider
- WHEN the same request is redelivered and the host/controller restarts
- THEN operation identities and invocation observations MUST show reconciliation of the original execution without a second dispatch.

#### Scenario: Root handoff and release

- GIVEN a completed rooted build with durable canonical result
- WHEN retained output roots are checked and explicitly released twice
- THEN the exact outputs MUST have remained rooted through handoff and the second release MUST NOT delete unrelated state.

### Requirement: Fresh UCAN observes the actual pilot over TLS

r[kiln.onixos_pilot.observation] The existing HTTPS observation service MUST serve only the pilot's replayed canonical state and admitted windows on an isolated loopback TLS endpoint. Fresh anchored UCAN issuer and holder credentials MUST authorize exact read targets. The consumer MUST verify view identity and actual canonical outcome; retained evidence MUST omit keys and bearer/invocation credentials.

#### Scenario: Authorized real outcome

- GIVEN fresh admitted issuer/holder credentials and the actual replayed pilot journal
- WHEN a TLS request for its exact run or attempt is admitted
- THEN the returned canonical view and BLAKE3 identity MUST match the replayed outcome.

#### Scenario: Wrong holder, target or replay

- GIVEN a wrong holder, unauthorized target or replayed invocation
- WHEN the request reaches the same live service
- THEN access MUST be denied without changing canonical state.

### Requirement: Genuine failures remain failures or uncertainty

r[kiln.onixos_pilot.failure] The pilot MUST exercise genuine negative evaluation/build/outcome observations and malformed or stale inputs. It MUST retain exact native diagnostics and classify failure, timeout, cancellation and uncertainty through existing admission rather than promote missing or ambiguous observations to success.

#### Scenario: Actual negative result

- GIVEN a real negative consumer/provider execution
- WHEN the exact native result is admitted
- THEN canonical observation MUST preserve its non-success classification and corresponding diagnostic evidence.

#### Scenario: Ambiguous observation

- GIVEN absent or conflicting completion observations
- WHEN reconciliation cannot establish an exact terminal result
- THEN the operation MUST remain uncertain and MUST NOT be silently retried or marked successful.

### Requirement: Exact repeatable evidence preserves authority boundaries

r[kiln.onixos_pilot.evidence] Kiln maintainers MUST retain repeatable commands, exact source/input/executable/profile identities, actual outputs and failures, native gates and cohort-bound receipts. The pilot MUST use private bounded state and fresh credentials, leave production state and remote status writes untouched and retire private processes/credentials. Historical evidence and candidate inputs MUST remain distinguished. Evidence MUST NOT grant publication, deployment, source trust, confinement, global availability, distributed exactly-once or release authority.

#### Scenario: Local qualification is repeatable

- GIVEN the declared maintained cohort, frozen consumer and provisioned inputs
- WHEN an operator repeats the bounded local procedure
- THEN its journal, runtime observations, authenticated views and artifact identities MUST be retained under that exact exercised cohort.

#### Scenario: A stronger claim is proposed

- GIVEN a historical fixture pass, a changed executable or an isolated local result
- WHEN evidence is described as production deployment, arbitrary runtime correctness or qualification of another cohort
- THEN the claim MUST be rejected and the actual scope MUST remain explicit.
