# Design: Local CI Composition E2E

## Ownership

Kiln owns CI meaning, replay admission, workflow linkage, and terminal outcome
classification. The test owns only fixture inputs and peer behavior. It adds no
production fallback, provider selection, or runtime authority.

The test lives in `crates/kiln-adapter-github/tests/local_ci_e2e.rs` and is
gated by the existing `protocol-fixtures` feature. The in-process cases use
public composition seams: `admit_config`, `GithubService::new`, `serve_with`,
`open_store`, `LatticeAdapter`, `UnixWorkflowExchange`, and `StatusPort`. The
process cases launch the real `kiln-adapter-github` binary with the same
profile and a fixture-only loopback status endpoint selected by
`KILN_GITHUB_FIXTURE_STATUS_ENDPOINT`.

The endpoint selection is compiled only under `protocol-fixtures`. Production
builds keep the fixed GitHub API endpoint and never read the environment
variable.

## Topology

```text
test driver
  -> raw HTTP POST + HMAC to serve_with (real TCP, real SQLite)
  -> LatticeAdapter<UnixWorkflowExchange> (real Unix frames)
  -> test WorkflowExchangeHandler
       Dispatch: run CARGO_BIN_EXE_kiln-github-job over a bare Git repo
       Observe:  return the terminal contract observation
  -> test StatusPort recorder
```

```text
test driver
  -> spawn kiln-adapter-github --profile service.json
  -> real binary resolves both secrets, serves TCP, opens SQLite,
     connects the real Lattice Unix transport, and runs kiln-github-job
  -> test loopback status server returns GitHub-shaped status responses
```

The process cases additionally assert that the real binary selects the
fixture-only status transport, publishes the pending and terminal projections,
and can be killed and restarted against the same durable state. The Lattice
handler and status server are closed fixture peers. They validate the real
transport contracts but do not execute a workflow runtime or mutate GitHub.

## Fixtures

- A temporary bare Git repository contains one exact selected commit and a
  later unselected tip. The job must acquire the selected commit, not the tip.
- The job profile is byte-encoded once and its BLAKE3 digest is passed to the
  helper. The reviewed command checks the checkout content, absence of `.git`,
  and absence of `GITHUB_TOKEN` and `HOME`.
- The secret reference uses `/bin/sh` with a `get` protocol script in a
  temporary project root. The test pins the actual `/bin/sh` BLAKE3 digest.
- The service profile uses a loopback listener, a temporary SQLite path, an
  allowed current user ID for the Unix peer, and a temporary socket.
- The in-process status peer records exact `StatusPlan` values and returns
  deterministic publication identities. The process status server accepts the
  fixed GitHub REST paths and returns GitHub-shaped status objects with exact
  run descriptions and commit URLs.

## Ordering and Recovery

The first pass sends one accepted push, drains the service until the status is
published, and asserts one job execution and one status publication. A second
identical delivery asserts the same run identity and no new job.

The recovery case stops the service after a durable dispatch or nonterminal
observation, starts a new service against the same SQLite database and Lattice
socket, and asserts the original execution completes without a normal
redispatch. The fixture handler counts `RecoverDispatch` and `Dispatch`
separately. The process recovery case performs the same assertions after
killing and restarting the real binary.

## Failure and Bounds

The process rail also covers fail-closed behavior at the durable and HTTP
boundaries:

- a changed service profile against an existing SQLite database exits nonzero
  and leaves the original run and publications intact;
- a conflicting read-only status projection leaves the terminal publication
  `Unknown`, never `Published`;
- duplicate critical headers, oversized bodies, and unsupported methods create
  no run, job, or status effect;
- partial requests time out, connection saturation returns the declared
  capacity response, and a later valid webhook still completes;
- same-repository PRs execute the exact head revision, fork PRs are denied
  before effects, and closed PRs are ignored.

The existing DNS resolver teardown test now waits up to a two-second bound for
asynchronous process-group reaping instead of requiring an instantaneous exit.
It still fails if the leader or descendant remains running; the final `/proc`
leader assertion is unchanged.

## Failure

Invalid signatures, changed repository identities, denied events, and missing
commits remain covered by existing adapter tests. This rail adds only the
cross-boundary cases that need the real job process and Unix transport:
passing, failing, duplicate, and restart recovery.

## Non-Claims

The fixture proves local composition and replay behavior only. It does not prove
live GitHub status mutation, source trust, workflow correctness, host
confinement, distributed exactly-once execution, deployment success, or release
eligibility.
