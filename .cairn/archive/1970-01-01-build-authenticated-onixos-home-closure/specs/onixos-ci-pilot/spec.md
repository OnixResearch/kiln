# OnixOS authenticated home closure

## ADDED Requirements

### Requirement: External preparation excludes consumer construction

r[kiln.onixos_pilot.closure.preparation] The pilot MUST identify its real launcher, activation and home-file derivations explicitly, compute external preparation dependencies with a pure deterministic decision and reject a preparation dependency closure containing any selected consumer derivation or output. It MUST record external preparation separately from authenticated construction. It MUST NOT infer this boundary from store-path name prefixes or prebuild generated consumer outputs.

#### Scenario: Only external dependencies are prepared

- GIVEN exact evaluated consumer derivations and an external dependency closure disjoint from their identities
- WHEN the pure preparation decision admits that closure
- THEN only admitted external dependencies MAY be realised before authenticated dispatch.

#### Scenario: An external dependency reaches a consumer

- GIVEN incomplete consumer records or an external dependency closure containing a consumer derivation or output
- WHEN preparation admission runs
- THEN the pilot MUST reject it before realising preparation dependencies or dispatching the workflow.

### Requirement: Authenticated construction covers generated consumer outputs

r[kiln.onixos_pilot.closure.construction] The pilot MUST bind a fresh identity into the real consumer inventory's generated Git configuration and require the selected launcher, activation and home-file outputs to be absent immediately before authenticated dispatch. Success MUST establish their actual construction through the existing maintained runtime and rooted Nix provider, verify the generated content and exact launcher closure and never execute activation. Its negative consumer case MUST fail in the actual home-file builder and preserve the native diagnostic and non-success canonical classification.

#### Scenario: The real generated closure is built

- GIVEN admitted external inputs and all three consumer outputs absent before authenticated dispatch
- WHEN the original real worker completes successfully
- THEN the generated Git configuration MUST contain the bound identity and the retained launcher closure MUST include the exact activation and home-file outputs.

#### Scenario: A consumer home-file target is invalid

- GIVEN a typed consumer configuration targeting a child beneath its generated `.gitconfig` file
- WHEN the actual consumer home-file builder runs after authenticated dispatch
- THEN it MUST fail with its native conflicting-target diagnostic, canonical outcome MUST be `DomainFailure` and no successful generated-consumer construction MAY be claimed.

### Requirement: Full-closure evidence preserves existing runtime authority boundaries

r[kiln.onixos_pilot.closure.evidence] The repeatable rail MUST retain exact source, input, executable, preparation, before/after consumer-output and native result identities together with canonical journal evidence. It MUST preserve pending replay across restart, reconciliation without redispatch, exact successful launcher rooting through durable handoff, explicit idempotent release and fresh UCAN-authenticated outcome views and denials. It MUST retire private credentials/processes and MUST NOT claim activation, autonomous forge delivery, production deployment, general reproducibility, publication or release authority. Historical launcher-only qualification MUST remain separately labelled.

#### Scenario: Full-closure qualification repeats

- GIVEN an immutable reviewed source cohort and fresh private state
- WHEN success, actual home-file failure and evaluation failure are exercised
- THEN evidence MUST retain their actual classifications, original provider invocation, authenticated views, output boundaries and teardown observations.

#### Scenario: A cached or stronger claim is offered

- GIVEN an already realised consumer output, historical launcher-only receipt or a proposed deployment or release claim
- WHEN evidence is admitted or described
- THEN cached consumer construction MUST be rejected by the rail and the stronger claim MUST NOT be inferred.
