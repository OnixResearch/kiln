# GitHub CI Adapter Specification

## ADDED Requirements

### Requirement: Explicit GitHub profiles

r[kiln.github.profile] GitHub intake, workflow, job and status behavior MUST use typed versioned profiles with explicit repository identity, capabilities, immutable operation/protocol identities, paths and positive resource bounds. Unsupported or malformed profiles MUST fail before external effects. Core crates MUST remain independent of GitHub implementation types.

#### Scenario: Complete profile is selected

- GIVEN a complete admitted GitHub profile and reviewed workflow route
- WHEN pure profile admission runs
- THEN the shell MAY create the declared local resources and provider ports.

#### Scenario: Capability or cohort drifts

- GIVEN an unsupported capability, unknown schema, invalid bound or changed pinned protocol
- WHEN profile admission runs
- THEN admission MUST fail without a provider call.

### Requirement: Authenticate original event bytes

r[kiln.github.authentication] Intake MUST bound the request and verify the exact original bytes using `X-Hub-Signature-256` HMAC-SHA256 before payload decoding, persistence or provider effects. Delivery/event/signature headers MUST be unique and structurally admitted. Invalid signatures MUST NOT expose secrets or payloads in diagnostics.

#### Scenario: Signed delivery arrives

- GIVEN one valid signature over the exact bounded body
- WHEN authenticated intake runs
- THEN the body MAY enter pure event admission.

#### Scenario: Body or authentication changes

- GIVEN a changed byte, missing or duplicate authentication header, malformed signature or oversized body
- WHEN intake runs
- THEN no run, database reservation or provider effect MAY be created.

### Requirement: Exact push and pull-request admission

r[kiln.github.events] Intake MUST match the configured GitHub repository and admit pushes and pull-request opened, reopened and synchronize events using exact native commit identities. Deleted branches, closed PRs and unsupported events MUST NOT execute jobs. Fork PR execution MUST be an explicit denied-by-default policy. Native identities MUST NOT replace Kiln run/candidate identities.

#### Scenario: Admitted push or PR arrives

- GIVEN an admitted repository, event action and exact nonzero commit
- WHEN pure event admission runs
- THEN the selected revision MUST be that exact commit, not a mutable branch or API-selected tip.

#### Scenario: Repository, revision or fork policy is wrong

- GIVEN another repository, malformed commit or denied fork PR
- WHEN event admission runs
- THEN no workflow dispatch MAY occur.

### Requirement: Durable bounded replay protection

r[kiln.github.replay] Intake MUST durably reserve exact body/delivery and candidate/pipeline identities before acknowledgement. Equal deliveries or equivalent candidate/pipeline requests MUST NOT create another execution. A changed body under one delivery identity MUST fail. Record limits and mismatched stored/profile identities MUST fail closed without erasing replay facts.

#### Scenario: Delivery is repeated or the service restarts

- GIVEN an existing exact delivery or candidate/pipeline record
- WHEN intake is retried before or after restart
- THEN the original run identity MUST be reused without a new workflow dispatch.

#### Scenario: Replay identity conflicts or storage fills

- GIVEN changed bytes under an existing delivery ID or an exhausted record bound
- WHEN reservation runs
- THEN intake MUST reject without eviction or an external effect.

### Requirement: Exact admitted workflow lifecycle

r[kiln.github.workflow] Workflow plans MUST bind the Kiln run, candidate, pipeline, policy, exact GitHub repository/revision, graph and provider profile. Dispatch intent MUST be persisted before the external call. An interrupted or ambiguous dispatch MUST use recovery with the original plan and MUST NOT redispatch. Every observation MUST pass existing core linkage/outcome admission before CI or publication state changes.

#### Scenario: Execution completes

- GIVEN an exact accepted workflow and matching terminal node/receipt observations
- WHEN observation admission runs
- THEN the CI result MAY become the admitted success, failure or cancellation result.

#### Scenario: Dispatch is ambiguous or an observation is stale

- GIVEN a lost dispatch response, restarted reservation, stale sequence or wrong candidate/execution identity
- WHEN recovery or observation runs
- THEN execution MUST NOT be duplicated and terminal success MUST NOT be invented.

### Requirement: Bounded exact-source job execution

r[kiln.github.source] The job command MUST verify its BLAKE3-bound profile and fixed Lattice source keys, acquire only the configured repository's exact commit under bounded process/transfer policies, admit its canonical Git snapshot using the existing Git adapter, materialize a fresh owned checkout and execute only reviewed argv with a clear declared environment, bounded output/deadline and process-group teardown. Source credentials MUST NOT enter the CI command environment or saved Git URL.

#### Scenario: Exact source runs a reviewed command

- GIVEN the configured repository, matching job profile identity and an available exact commit
- WHEN the job command runs
- THEN the command MUST observe that commit's admitted checkout and its actual exit outcome MUST be reported.

#### Scenario: Source, profile or process fails

- GIVEN profile drift, another repository, missing commit, unsafe/oversized tree, transfer failure, timeout, cancellation or output overflow
- WHEN source/job execution runs
- THEN no unrelated revision or fallback command MAY run and failure/uncertainty MUST remain explicit.

### Requirement: Exact GitHub commit-status projection

r[kiln.github.status] Status requests MUST bind the exact repository, commit, fixed context, Kiln run identity and admitted CI state using GitHub REST version `2022-11-28`. Redirects and automatic mutation retries MUST be disabled. A durable reservation MUST precede POST. Lost or ambiguous responses MUST be reconciled through bounded read-only exact status lookup, never blindly reposted. Publication failure MUST NOT alter the CI outcome.

#### Scenario: Terminal result is published

- GIVEN an admitted terminal CI result and exact status request
- WHEN publication succeeds with a matching bounded response
- THEN the recorded projection MUST retain the exact result/run/revision binding.

#### Scenario: Publication is ambiguous or stale

- GIVEN a lost POST response, mismatched response, authorization/rate/provider failure or conflicting remote status
- WHEN publication/reconciliation runs
- THEN the projection MUST remain failed or unknown without another POST or a changed CI result.

### Requirement: SecretSpec-resolved redacted credentials

r[kiln.github.secrets] Webhook, GitHub API and optional source secrets MUST be explicit SecretSpec references resolved using a pinned executable through Bounded Exec. Secrets MUST remain out of serialized profiles, URLs, SQLite records, diagnostic text and job environment. Provider failures and invalid/oversized secret output MUST fail closed.

#### Scenario: Declared secret resolves

- GIVEN the pinned resolver and declared secret reference
- WHEN bounded resolution succeeds
- THEN only the owning authentication/provider operation MAY consume its in-memory value.

#### Scenario: Resolver or secret is invalid

- GIVEN executable drift, missing credential, malformed output or a resolver timeout
- WHEN resolution runs
- THEN the effect MUST fail without ambient credential fallback or secret-bearing diagnostics.

### Requirement: Bounded executable shell

r[kiln.github.shell] The executable MUST separate prompt bounded HTTP acknowledgement from workflow execution, bound headers/body/read time/connections, SQLite retention/concurrency, observation polling and external responses, and expose honest provider errors. Unsupported HTTP framing or methods MUST fail before intake. Restart MUST preserve stored intent and recovery classification.

#### Scenario: Slow job executes after acknowledgement

- GIVEN an authenticated durably admitted event
- WHEN a workflow takes longer than the webhook acknowledgement window
- THEN the HTTP path MUST NOT wait for job completion.

#### Scenario: Resource or provider limit is reached

- GIVEN malformed framing, excessive input, an exhausted bound or unavailable provider
- WHEN the shell processes the request/work
- THEN it MUST fail within its declared mechanism limits without fake success or a new fallback execution.

### Requirement: Explicit authority and evidence boundaries

r[kiln.github.nonclaims] Documentation and evidence MUST identify Kiln maintainers as maintenance owners and describe repeatable setup, isolation and credential requirements. Passing local fixtures MUST NOT claim live GitHub mutation, source trust, workflow correctness, host confinement, exactly-once distributed effects, deployment success or release eligibility. GitHub hosting remains a projection unless separately authorized.

#### Scenario: Local smoke passes

- GIVEN local protocol and actual command evidence
- WHEN results are recorded
- THEN evidence MUST name actual exercised components and any absent live provider authority.

#### Scenario: Job isolation or deployment is inferred

- GIVEN bounded processes or a temporary checkout without an admitted sandbox/deployment
- WHEN a result is described
- THEN it MUST NOT claim confinement, safe untrusted execution or deployed GitHub CI.
