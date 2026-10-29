# Web Remote Lattice Workers

## ADDED Requirements

### Requirement: Exact source binding across distinct checkouts

r[kiln.web.remote_workers.source]

The live job worker MUST require a reviewed v2 profile declaring the controller's canonical Git repository identity separately from its worker-local backing checkout. It MUST reject old, missing or malformed profile bindings. It MUST admit producer source context only for the declared repository and exact commit, preserve local acquisition identities, and bind callbacks to that declared repository plus the exact acquired commit and canonical tree before running commands or uploading content.

#### Scenario: Independent clone of the exact source

r[kiln.web.remote_workers.source.scenario.clone]

- GIVEN a declared controller repository and an independent worker clone containing the exact admitted commit
- WHEN the source context and callback match the declared repository, commit, canonical tree, project, attempt and execution
- THEN the worker executes the reviewed command from immutable committed bytes despite distinct local repository identities

#### Scenario: Missing or substituted binding

r[kiln.web.remote_workers.source.scenario.denied]

- GIVEN an old profile, missing identity, wrong source repository, wrong callback repository, revision or canonical tree
- WHEN worker admission runs
- THEN it fails closed before source commands or content publication and never infers identity from a local path

### Requirement: Existing remote workflow path remains exact

r[kiln.web.remote_workers.transport]

The maintained native web-service exercise MUST be able to select the existing enrolled Iroh workflow bridge with an independent worker checkout. It MUST observe real exact-source success and failure, dependency execution, actual logs and artifact bytes, exact recovery without redispatch, cancellation and unavailable-provider behavior. It MUST retire owned resources and keep loopback evidence distinct from WAN, per-node placement, source trust, confinement, deployment and release claims.

#### Scenario: Remote execution and recovery

r[kiln.web.remote_workers.transport.scenario.native]

- GIVEN private ephemeral enrolled peers, an exact saved native Lattice workflow and a distinct worker clone
- WHEN the live service dispatches, observes, restarts and cancels work through Iroh
- THEN logs, artifact bytes and outcomes belong to the exact source and execution, restart does not redispatch accepted work, and unsupported or stale controls remain rejected
