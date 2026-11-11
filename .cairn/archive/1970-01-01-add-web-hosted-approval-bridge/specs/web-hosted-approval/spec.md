## MODIFIED Requirements

### Requirement: Exact durable Web dispatch binding
r[kiln.web_hosted.binding]

An explicitly selected hosted Web execution provider MUST receive the run, project, exact source resolution and stored approval gate from Web durable state on dispatch and recovery. Awaiting, superseded, unauthorized or wrong-commit approvals MUST NOT dispatch. Existing Lattice dispatch remains separately admitted.

#### Scenario: An operator approves an exact run

- GIVEN an untrusted Web run blocked on commit A
- WHEN an interactive operator approves A
- THEN the bridge receives the persisted exact run/source/approval binding, while an approval naming another commit is refused

### Requirement: Dedicated-issuer hosted source admission
r[kiln.web_hosted.admission]

The hosted bridge MUST admit only a configured authenticated Web peer with current organization/project authority, exact project/repository/pipeline policy and a valid approved source binding. It MUST independently acquire and verify canonical source bytes from an operator-enrolled exact Git commit; caller workspace paths and unverified provenance MUST NOT substitute for that verification. Ordinary execution.run MUST NOT claim verified Web approval. Existing untrusted host and runner constraints MUST remain mandatory.

#### Scenario: Source or issuer is substituted

- GIVEN an approved exact Web request
- WHEN the issuer, repository, commit, canonical tree or configured execution policy differs
- THEN admission fails before workload execution or financial reservation

### Requirement: Once-only Web attempt and read-only recovery
r[kiln.web_hosted.replay]

The hosted bridge MUST bind each Web run/attempt to one durable complete request identity before effects. Identical replay MUST observe the original attempt; changed bindings MUST conflict. Restart or uncertainty recovery MUST NOT start a new execution or create a workspace.

#### Scenario: Dispatch reply is lost

- GIVEN an exact dispatch already reserved or executed
- WHEN Web restarts or repeats the same binding
- THEN it recovers the original attempt without another execution and rejects changed-source replay

### Requirement: Explicit hosted workflow adapter
r[kiln.web_hosted.adapter]

The supported composition MUST use authenticated hosted transport and preserve truthful per-node state, exact execution identity, retained logs and cancellation semantics. Unsupported controls or graph mappings MUST fail before effects. Core and Web crates MUST remain independent of hosted implementation imports.

#### Scenario: Unsupported operation is selected

- GIVEN a hosted provider mapping
- WHEN a plan requests unsupported control or topology
- THEN admission fails without requesting hosted execution

### Requirement: Scoped real qualification
r[kiln.web_hosted.qualification]

Qualification MUST exercise the actual authenticated Web-to-hosted route and exact Git source, including refused approval/issuer/source substitutions and durable replay/recovery. Local handoff admission or expected host-safety refusal MUST NOT be described as successful untrusted VM execution. Full untrusted execution requires an eligible allocated host and retained teardown evidence; hostile_isolation_claim MUST remain false.

#### Scenario: Local host is ineligible

- GIVEN a valid approved handoff on a host below its safety threshold
- WHEN the real hosted backend evaluates admission
- THEN host preflight refuses execution without bypass, and evidence distinguishes handoff verification from pending VM qualification
