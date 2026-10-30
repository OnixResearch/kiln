# Radicle Job Cobs Specification

## Purpose

Defines the `radicle-job-cobs` capability: bounded publication of
`xyz.radworks.job` objects compatible with Tangled/Radicle CI.

## Requirements

### Requirement: Wire-compatible job publication

r[kiln.radicle.job] Kiln MUST publish `xyz.radworks.job` objects using the
`Request`, `Run` and `Finished` action schema of the pinned `radicle-job`
cohort. A root `Request` MUST bind exactly one lowercase 40-hex Git commit. A
`Run` MUST carry one UUID and a bounded log URL. A `Finished` action MUST carry
one reason that is either `Succeeded` or `Failed`. The reducer MUST reject
unknown actions, unknown fields, duplicate starts for one author and UUID, a
finish without a matching started run, conflicting terminal reasons, and a
second root request naming another commit.

#### Scenario: Fresh job is published
r[kiln.radicle.job.scenario.fresh]

- GIVEN an admitted repository, exact commit, run id, log URL and terminal outcome
- WHEN Kiln publishes the job
- THEN the repository MUST contain one `xyz.radworks.job` object whose root binds the commit and whose Kiln namespace contains `Run` then `Finished` for the deterministic UUID.

#### Scenario: Malformed action is rejected

- GIVEN an unknown action, an unknown field, an invalid commit, a non-UUID run id or a non-URL log
- WHEN the object is admitted
- THEN reduction MUST fail closed and no observation is reported.

### Requirement: Exact job identity and extension

r[kiln.radicle.job.identity] An existing job MUST be identified by reducing the
bounded set of job objects and matching its root commit. A single match MUST be
extended; multiple matches MUST fail as ambiguous unless the plan names the
exact native object id; a named object whose root commit differs MUST fail. A
job for another commit MUST NOT be extended. A fresh job MUST be created only
when no job for the commit exists.

#### Scenario: Another node's job is extended
r[kiln.radicle.job.scenario.extend]

- GIVEN a job for the exact commit created and signed by another node
- WHEN Kiln publishes its run for that commit
- THEN Kiln MUST append to that object in its own namespace and the other node's run and head MUST remain unchanged.

#### Scenario: Another commit's job is refused
r[kiln.radicle.job.scenario.mismatch]

- GIVEN a job whose root binds a different commit
- WHEN Kiln is asked to publish to that object id
- THEN it MUST reject before any write.

### Requirement: Replay does not duplicate a run

r[kiln.radicle.job.replay] The run UUID MUST be deterministic from the
repository, revision and Kiln run id. Publishing the same state again MUST
return the existing observation without adding an entry. Replaying a started
run with the same log MUST append only the terminal action. A different log, a
different terminal reason, or a start after terminal finish MUST fail closed.

#### Scenario: Replay is idempotent
r[kiln.radicle.job.scenario.replay]

- GIVEN a successfully published job run
- WHEN the same publish is repeated
- THEN the object head and run set MUST be unchanged and the observation MUST still name the same object id.

#### Scenario: Replay conflicts
r[kiln.radicle.job.scenario.replay_conflict]

- GIVEN a published run and a replay with another log or opposite terminal reason
- WHEN publication is attempted
- THEN it MUST fail without a write.

### Requirement: Multi-writer head-set append

r[kiln.radicle.job.multi_writer] Kiln MUST read every validated head for the
object across namespaces. A new Kiln entry MUST descend from every observed
valid tip and MUST advance only the Kiln signer's namespace. Kiln MUST NOT
require a single head, MUST NOT delete or rewrite another namespace, and MUST
reject entries whose signatures do not verify through the pinned change graph.

#### Scenario: Divergent heads are joined

- GIVEN two signed heads for one job created independently by two other nodes
- WHEN Kiln publishes its run
- THEN the Kiln entry's parents MUST include both heads and a later read MUST reduce all runs.

### Requirement: Signer and profile authority

r[kiln.radicle.job.authority] The profile MUST pin the job type, cohort
versions, executable identity, repository allowlist, signer public identity and
bounded limits. The resolved signer MUST equal the declared identity before the
first effect. The profile MUST NOT grant network peer, merge, identity,
delegation, cache or canonical-state authority.

#### Scenario: Foreign signer is refused

- GIVEN a resolved signer that differs from the declared profile identity
- WHEN publication runs
- THEN it MUST reject before creating or updating any object.

#### Scenario: Foreign repository is refused

- GIVEN a repository outside the profile allowlist
- WHEN publication is planned
- THEN it MUST reject before signer resolution or storage access.

### Requirement: Bounds and reconciliation

r[kiln.radicle.job.bounds] Payload bytes, log URL bytes, scanned objects and
runs per node MUST be bounded by named limits. Reconcile MUST re-read the
published object without resolving credentials or redispatching; it MUST return
the existing observation only when the Kiln run matches the requested state.

#### Scenario: Oversized publication is refused

- GIVEN a log URL or payload above its named bound
- WHEN publication is admitted
- THEN it MUST fail before any signer or storage effect.

#### Scenario: Reconcile is read-only

- GIVEN a published job
- WHEN reconcile runs with the provider unavailable
- THEN it MUST return the recorded observation without creating, updating or redispatching.

### Requirement: Bounded console projection

r[kiln.radicle.job.console] The COB browser MUST list a Kiln-published
`xyz.radworks.job` object and show its commit-bound history without claiming
build or release authority.

#### Scenario: Browser shows the published job

- GIVEN a fixture repository containing a Kiln-published job
- WHEN the COB browser lists job objects and opens the object
- THEN it MUST show the job type, object id and its actions.
