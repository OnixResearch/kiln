## ADDED Requirements

### Requirement: Declared change authorship

r[kiln.review.authorship]
Change creation, branch contribution intake and patch-set upload MUST accept an optional self-declared authorship of `human`, `agent` with a harness and an optional model, or `mixed`. The default MUST be `undeclared`. Kiln MUST NOT infer authorship from any other fact. The declared value MUST be stored on the patch set inside the journaled review command before any provider effect. A change's class MUST be derived only from its declared patch sets. Service-created patch sets MUST carry `undeclared`.

#### Scenario: Declared intake is journaled

r[kiln.review.authorship.scenario.declared]
- GIVEN an authenticated writer contributes or creates a change declaring `agent` with harness `omp` and model `claude`
- WHEN the request is admitted
- THEN the pending journaled command and the resulting patch set MUST carry exactly that authorship
- AND the change class MUST be `agent`

#### Scenario: Old journals replay undeclared

r[kiln.review.authorship.scenario.legacy]
- GIVEN a review state document or pending journal entry written before authorship existed
- WHEN it is loaded and replayed
- THEN every patch set MUST be `undeclared`, and re-serialization MUST NOT add an authorship field
- AND existing request digests MUST be unchanged

#### Scenario: Invalid authorship is rejected

r[kiln.review.authorship.scenario.invalid]
- GIVEN an authorship with an unknown kind, unknown fields, an empty or control-character harness or model, or harness or model text over the named byte bound
- WHEN it is submitted or applied
- THEN the request MUST fail before mutation or provider effect

### Requirement: Console authorship declaration

r[kiln.review.authorship.console]
The console create and contribution forms MUST offer a labelled authorship select that defaults to Undeclared. Selecting Agent MUST reveal a labelled harness input and an optional labelled model input. Change detail MUST show the change's authorship class and each patch set's declared authorship as self-declared.
