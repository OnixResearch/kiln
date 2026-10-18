# GitHub CI Specification Delta

## Added Requirements

### Requirement: Local CI composition E2E rail

r[kiln.github.local_e2e] The maintained local GitHub CI rail MUST compose the real GitHub shell, real SQLite replay, real Lattice Unix transport, and the real exact-source job helper under the `protocol-fixtures` feature. It MUST replace only the workflow executor and status endpoint with test-owned peers, and it MUST make no live GitHub request.

#### Scenario: Exact-source pass and failure

r[kiln.github.local_e2e.outcomes]
- GIVEN a signed webhook for one exact Git commit and a reviewed job profile
- WHEN the local composition dispatches the exact-source job helper
- THEN the command MUST observe that commit, no Git metadata, and no ambient source or status credential
- AND a zero exit MUST become a published success status
- AND a nonzero exit MUST become a published failure status

#### Scenario: Duplicate delivery and restart

r[kiln.github.local_e2e.recovery]
- GIVEN one durably admitted run
- WHEN an equal delivery arrives or the shell restarts with the same SQLite and Lattice socket
- THEN the original run MUST be reused
- AND the local composition MUST NOT execute a second job
- AND restart recovery MUST use `RecoverDispatch` rather than a normal dispatch

#### Scenario: Real binary process

r[kiln.github.local_e2e.process]
- GIVEN the `protocol-fixtures` build selects a loopback status endpoint through `KILN_GITHUB_FIXTURE_STATUS_ENDPOINT`
- WHEN the real `kiln-adapter-github` binary serves an authenticated webhook
- THEN it MUST execute the exact-source job helper over the real Lattice Unix transport
- AND the fixture status peer MUST receive the pending and terminal projections
- AND an invalid signature MUST be rejected before any run, job, or status effect
- AND a nonzero command MUST produce a failure projection
- AND an equal duplicate delivery MUST NOT add a job
- AND restarting the binary against the same SQLite and socket MUST recover the original execution without a second job

#### Scenario: Ambiguous status publication

r[kiln.github.local_e2e.status_recovery]
- GIVEN the fixture status server accepts a status POST but drops the response
- WHEN the real binary processes the pending and terminal projections
- THEN the canonical CI outcome MUST remain unchanged
- AND each projection MUST be reconciled by read-only exact lookup
- AND no status POST may be repeated
- AND conflicting status evidence MUST leave the projection `Unknown`, not `Published`

#### Scenario: Malformed webhook framing

r[kiln.github.local_e2e.framing]
- GIVEN the real binary receives duplicate critical headers, an oversized body, or an unsupported method
- WHEN intake runs
- THEN it MUST reject the request with its declared status
- AND no run, job, or status effect may be created

#### Scenario: Profile drift against durable replay

r[kiln.github.local_e2e.profile_drift]
- GIVEN a terminal run persisted under one admitted service profile
- WHEN the real binary starts with a changed profile against the same SQLite database
- THEN it MUST exit nonzero without rewriting or accepting the existing state
- AND the original run and publications MUST remain intact

#### Scenario: Pull request policy

r[kiln.github.local_e2e.events]
- GIVEN the real binary receives a same-repository pull request, a fork pull request, and a closed pull request
- WHEN admission runs
- THEN the same-repository pull request MUST execute the exact head revision
- AND the fork pull request MUST be denied before any effect
- AND the closed pull request MUST be ignored without any effect

#### Scenario: Partial request and overload

r[kiln.github.local_e2e.shell_bounds]
- GIVEN the real binary is handling partial requests at its connection bound
- WHEN another valid webhook arrives
- THEN it MUST return the declared capacity response without any effect
- AND after the partial requests time out, a valid webhook MUST still complete

#### Scenario: Fixture boundary

r[kiln.github.local_e2e.nonclaims]
- GIVEN a passing local composition
- WHEN evidence is recorded
- THEN the fixture Lattice and status peers MUST be named
- AND the result MUST NOT claim live GitHub mutation, host confinement, distributed exactly-once execution, deployment success, or release eligibility
