# Web Service Deployment Specification

## Purpose

Defines the `web-service-deployment` capability.

## Requirements

### Requirement: Explicit runtime admission and readiness

r[kiln.web.deploy.runtime] The service MUST expose non-mutating `check --profile` admission, MUST report readiness only after its TLS/listener/canonical state are initialized and MUST expose minimal HTTPS health without private state. Credential/key admission MUST preserve no-follow single-link regular-file checks and admit only owner-private 0400/0600 files, or root-owned 0440 files on a read-only mount with an exact ACL granting read solely to root and the current non-root UID. Group, other-user, world and writable-mount grants MUST be rejected. Key reads MUST be bounded. Shutdown MUST retain original durable workflow recovery without blind redispatch.

#### Scenario: Supervised startup and restart

r[kiln.web.deploy.runtime.scenario.ready]
- GIVEN valid owned runtime inputs and persistent state
- WHEN the supervisor starts or restarts the service
- THEN readiness and health MUST correspond to initialized service state and original accepted work MUST remain recoverable.

#### Scenario: Unsafe runtime input

r[kiln.web.deploy.runtime.scenario.denied]
- GIVEN malformed configuration, a symlink, wrong owner, group/world-readable secret, missing credential or invalid TLS input
- WHEN preflight or startup runs
- THEN admission MUST fail without readiness, workflow effects or secret disclosure; preflight MUST NOT mutate state.

### Requirement: Installable least-privilege deployment

r[kiln.web.deploy.install] Kiln MUST ship a dedicated package and typed deployment definition using existing OnixOS/systemd mechanisms, explicit profile and credential references, a dedicated non-root identity, private persistent state, bounded restart/stop behavior and read-only source access. Host activation MUST remain an explicit operator action. Secrets MUST NOT be embedded into store-backed configuration, command lines or workload environments.

#### Scenario: Operator installs reviewed deployment

r[kiln.web.deploy.install.scenario.install]
- GIVEN an immutable package, reviewed profile and provisioned credentials
- WHEN the supported deployment is assembled and started in an isolated environment
- THEN the actual packaged HTTPS service MUST run with its declared identity, credentials, writable state and supervised lifecycle.

#### Scenario: Missing deployment authority

r[kiln.web.deploy.install.scenario.denied]
- GIVEN missing credentials, unsupported configuration or an unreviewed activation target
- WHEN deployment preparation runs
- THEN it MUST fail visibly or remain unactivated, without generated fallback secrets or provider substitution.

### Requirement: Correct native cancellation qualification

r[kiln.web.deploy.cancellation] The owning Lattice producer MUST preserve observed owned cancellation separately from ordinary failure and completed execution, including cancellation/completion races. Kiln MUST consume an exact measured corrected producer and retain its strict `Cancelled` assertion rather than relabeling outcomes or weakening qualification.

#### Scenario: Active work is cancelled

r[kiln.web.deploy.cancellation.scenario.cancel]
- GIVEN a real active native command and an admitted exact control request
- WHEN cancellation stops owned execution
- THEN the producer and Kiln MUST preserve canonical cancellation and pending dependent effects MUST NOT execute.

#### Scenario: Completion wins or cancellation is absent

r[kiln.web.deploy.cancellation.scenario.race]
- GIVEN successful/failed completed work or a late cancel request
- WHEN completion is classified
- THEN cancellation intent alone MUST NOT rewrite observed completion or manufacture successful cancellation.

### Requirement: Repeatable deployment evidence

r[kiln.web.deploy.evidence] Maintainers MUST retain paired runtime-admission tests and actual isolated packaged service-manager/HTTPS/native workflow observations covering startup, health, credentials, stop/restart, recovery, real logs/artifacts and cancellation. Documentation MUST identify consumer, owner, installation, rotation, recovery, exact runtime inputs and non-claims. Checks MUST NOT imply production activation, source trust, lifecycle acceptance, hostile-code confinement or release readiness.

#### Scenario: Qualification is repeated

r[kiln.web.deploy.evidence.scenario.repeat]
- GIVEN declared immutable inputs and disposable owned state
- WHEN deployment qualification runs
- THEN actual outcomes and exact identities MUST be retained and temporary credentials/processes/state MUST be retired.

#### Scenario: A gate fails

r[kiln.web.deploy.evidence.scenario.failure]
- GIVEN a failed native or repository gate
- WHEN readiness is reported
- THEN failure MUST remain explicit without acceptance/archive, hidden retries or a passing qualification claim.
