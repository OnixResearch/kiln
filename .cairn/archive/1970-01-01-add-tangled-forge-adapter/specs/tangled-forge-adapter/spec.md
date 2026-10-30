## ADDED Requirements

### Requirement: Tangled design note

r[kiln.tangled.note]
`docs/tangled-integration.md` MUST describe these topics, each citing the primary sources it was read from, with dates:
- knot → spindle event delivery;
- spindle authentication;
- status publication;
- the repository DID identity model;
- exact-commit source acquisition;
- the mapping onto Kiln ports.

It MUST list explicitly what was not established.

#### Scenario: Unverified protocol parts are listed

r[kiln.tangled.note.scenario.unverified]
- GIVEN the design note
- WHEN a reader looks for live interoperability, pull request intake and unsupported methods
- THEN each MUST appear in the "Not established" list

### Requirement: Pinned Tangled repository identity

r[kiln.tangled.identity]
The adapter MUST serve only repository DIDs pinned in its admitted profile. Each pin names a knot DID, an HTTPS clone URL, trusted actors and approvers.

Tangled DIDs, object ids and pipeline ids MUST be kept as `ExternalRef` provider facts. Kiln run, candidate and record identities MUST be BLAKE3.

#### Scenario: Unknown repository DID

r[kiln.tangled.identity.scenario.unknown-repo]
- GIVEN a validly signed notifyEvent for a repository DID that is not pinned
- WHEN it is admitted
- THEN it MUST be refused before any fetch, and no pipeline is created

### Requirement: Service-auth and webhook authentication

r[kiln.tangled.auth]
notifyEvent, triggerPipeline, approvePipeline and retryPipeline MUST require an AT Protocol service-auth JWT. The JWT MUST meet all of these conditions:
- its ES256K or ES256 signature verifies over the exact signing input against the issuer DID document's `#atproto` key, with low-S;
- `aud` is the spindle DID;
- `lxm` is the method NSID;
- it has not expired;
- its lifetime is at most the named bound;
- it carries a `jti`.

A notifyEvent push MUST be issued by the pinned knot. Status deliveries MUST carry an `X-Tangled-Signature-256` HMAC-SHA256 signature in the Tangled webhook format, and the provided verifier MUST check it in constant time.

#### Scenario: Bad signature

r[kiln.tangled.auth.scenario.bad-signature]
- GIVEN a token whose signature does not verify, or a signed delivery whose HMAC does not match
- WHEN it is admitted
- THEN admission MUST fail with an authentication error, and no event is decoded into a run

### Requirement: Event intake

r[kiln.tangled.events]
The adapter MUST admit `org.tangled.event.push` and `org.tangled.ci.trigger.manual` through notifyEvent, and `org.tangled.event.pullRequest` through triggerPipeline. It MUST apply these bounds and validation rules:
- bounded JSON;
- DID, ref and 40- or 64-hex object-id validation;
- the push `repo` equals the request `repo`;
- the push `actor` is present.

It MUST ignore, without creating a run:
- refs outside `refs/heads/` and `refs/tags/`;
- deletions;
- `skip-ci` and `ci-skip` pushes.

#### Scenario: Malformed XRPC

r[kiln.tangled.events.scenario.malformed]
- GIVEN a notifyEvent body that is not JSON, lacks `event`, or names a non-hex object id, or a query missing `repo`
- WHEN the XRPC request is handled
- THEN the response MUST be an `InvalidRequest` error, and no pipeline exists

### Requirement: Replay protection

r[kiln.tangled.replay]
A push with the same `(repo, ref, oldSha, newSha)` MUST return the existing pipeline id without a new run, including after a restart. A service-auth `jti` MUST NOT be accepted twice while its token can still be valid.

#### Scenario: Replayed push

r[kiln.tangled.replay.scenario.push]
- GIVEN a push that already produced a pipeline
- WHEN the knot redelivers it with a fresh token
- THEN the response MUST name the same pipeline, and the store MUST hold one pipeline and the same number of runs

### Requirement: Exact source acquisition

r[kiln.tangled.source]
The adapter MUST fetch the exact pushed object id from the pinned clone URL into a private object store of the matching SHA-1 or SHA-256 format. It MUST verify that the fetched commit has that id, and MUST read workflow definitions only at that commit. Every Kiln run MUST carry that object id as its source revision. A transport failure MUST yield a retryable response and no pipeline.

#### Scenario: Exact revision run

r[kiln.tangled.source.scenario.exact]
- GIVEN a fixture knot push for commit C
- WHEN it is processed
- THEN every created Kiln run's `WorkflowSourceContext.revision` MUST be C, and its workflow operation identity MUST bind the reviewed definition read at C

### Requirement: Reviewed workflow mapping

r[kiln.tangled.workflows]
`.tangled/workflows/*.yml` MUST be mapped through an explicit reviewed subset, and every unsupported field MUST be refused with a path-qualified diagnostic. `when` MUST map onto trigger filters: event, branch globs, tag globs, pull-request types and changed-path globs. Unknown changed files MUST run path-filtered workflows.

#### Scenario: Unsupported when keys

r[kiln.tangled.workflows.scenario.unsupported]
- GIVEN a workflow with `when.schedule`, an unknown `when` key, `tag` on `pull_request`, `cache`, or `submodules: true`
- WHEN it is mapped
- THEN the mapping MUST fail naming the field, and that workflow never runs

### Requirement: Spindle-compatible XRPC shell

r[kiln.tangled.xrpc]
The service MUST serve these methods with the lexicon shapes and XRPC error envelopes `{error, message}`:
- `org.tangled.event.notifyEvent`;
- `org.tangled.ci.listWorkflowDefinitions`;
- `org.tangled.ci.getWorkflowDefinition`;
- `org.tangled.ci.retryPipeline`;
- `org.tangled.ci.approvePipeline`;
- `org.tangled.ci.triggerPipeline`;
- `org.tangled.ci.getPipeline`.

The HTTP shell MUST bound headers, bodies and time, and MUST require `Content-Length`.

#### Scenario: Retry duplicates a pipeline

r[kiln.tangled.xrpc.scenario.retry]
- GIVEN a finished pipeline
- WHEN an approver calls retryPipeline with a workflow filter
- THEN a new pipeline id MUST be returned, bound to the same commit and reviewed definitions with only the named workflows; a blocked or unknown pipeline MUST yield `PipelineNotRetryable` or `PipelineNotFound`

### Requirement: Status publication

r[kiln.tangled.status]
The adapter MUST map Kiln outcomes onto Tangled workflow statuses, and MUST NOT report `success` for an unknown outcome. It MUST serve the `#pipeline` view through getPipeline, and MUST deliver each changed view to the profile's status endpoint with an HMAC signature. A delivery whose result is uncertain MUST be recorded as `Unknown` and reconciled, never as published.

#### Scenario: Status is published back

r[kiln.tangled.status.scenario.published]
- GIVEN a pipeline whose run completes successfully
- WHEN the service ticks
- THEN the local status endpoint MUST receive a correctly signed view with `success` for that workflow at the exact commit

### Requirement: Untrusted pull requests follow #12

r[kiln.tangled.untrusted]
A pull request from another repository, or from an author who is not a trusted actor, MUST create a pipeline whose workflows are `blocked` and not dispatched. Only a pinned approver's approvePipeline MAY release it, bound to the pipeline's exact commit. Approved pipelines and their retries MUST dispatch under the profile's untrusted workflow policy.

#### Scenario: Untrusted pull request is held

r[kiln.tangled.untrusted.scenario.held]
- GIVEN a fork pull request trigger
- WHEN it is admitted and the service ticks
- THEN nothing is dispatched until an approver approves it; a non-approver's approval MUST be refused, and a second approval MUST yield `PipelineNotBlocked`

### Requirement: Typed Nickel profile

r[kiln.tangled.profile]
The spindle profile MUST be authored in typed Nickel with named limits. The Rust admission MUST reject the following:
- a schema mismatch;
- an invalid spindle or repository DID;
- non-HTTPS clone or status URLs;
- duplicate repositories;
- zero bounds;
- empty approvers or non-claims.

#### Scenario: Invalid profile

r[kiln.tangled.profile.scenario.invalid]
- GIVEN a profile fixture with a non-HTTPS clone URL or a zero bound
- WHEN it is typechecked and exported, or admitted
- THEN it MUST fail
