# Web Hosted Approval Specification

## Purpose

Defines the `web-hosted-approval` capability.

## Requirements

### Requirement: Exact durable Web dispatch binding
r[kiln.web_hosted.binding]

Web MUST dispatch through the maintained native Lattice workflow provider. Its authenticated read-only worker binding projection MUST expose the persisted exact run, project, source, approval gate, workflow plan, native Lattice execution and durable cancellation intent for that attempt. The fixed-profile Lattice task MUST reject missing, ambiguous, stale or mismatched source/run/node bindings and MUST NOT newly execute an attempt whose cancellation is already requested. Web MUST NOT select or run a direct hosted workflow provider.

#### Scenario: Approved source reaches a native Lattice node

- GIVEN an untrusted Web run blocked on commit A
- WHEN an interactive operator approves A
- THEN native Lattice admits and executes the exact reviewed saved workflow, and its hosted task obtains the persisted approval/source binding without accepting another commit or inventing an execution identity

### Requirement: Dedicated-issuer hosted source admission
r[kiln.web_hosted.admission]

Hosted Lattice backend admission MUST require the configured authenticated Lattice job peer with current project authority and exact node/operation/reviewed-pipeline policy. Before a new execution it MUST independently fetch the exact active persisted Web binding from the operator-configured HTTPS authority and compare the complete source, approval, workflow and native execution identity. Worker assertions or caller-chosen authority URLs MUST NOT substitute for that check. Hosted MUST independently acquire and verify exact Git bytes. Existing untrusted runner and host constraints MUST remain mandatory; generic execution.run MUST NOT inherit verified Web approval.

#### Scenario: Relay invents or changes an approval

- GIVEN a scoped relay able to submit a backend task
- WHEN its approval, native execution, node, source or workflow differs from the configured Web authority's persisted binding
- THEN hosted refuses before workload execution or financial reservation

### Requirement: Once-only Web attempt and read-only recovery
r[kiln.web_hosted.replay]

Each Web run and native Lattice node MUST bind to one complete durable hosted request/configuration identity before execution. Identical replay MUST observe the original attempt, and changed source, gate, native execution, node or policy MUST conflict. Read-only recovery MUST NOT dispatch, rematerialize source or reacquire a new approval. A reserved uncertain attempt MUST remain uncertain rather than becoming permission for another execution. A substituted execution row MUST NOT inherit verified provenance.

#### Scenario: Native task or response is interrupted

- GIVEN an exact per-node backend task already reserved or executed
- WHEN Web or Lattice recovers its original execution
- THEN retained backend observations do not create a second execution or financial effect and changed replay is refused

### Requirement: Explicit hosted workflow adapter
r[kiln.web_hosted.adapter]

The supported hosted integration MUST be a fixed-profile execution task inside the genuine native Lattice saved workflow. Lattice MUST own graph scheduling, native workflow/node outcomes, control and lifecycle. Hosted MUST remain the reviewed execution backend, not a second Web workflow provider or a Lattice-shaped forwarding server. The task and hosted MUST observe durable Web cancellation intent for the exact attempt so that a native task destroyed without a signal cannot leave the backend running to success. The task MUST preserve real bounded stdout/stderr and exact cancellation/teardown observations. Unsupported or failed admission MUST never become synthetic successful work. The direct kiln-web-hosted binary, provider selector and bridge configuration MUST be removed without aliases.

#### Scenario: A native workflow invokes hosted execution

- GIVEN an exact route_exact and shell_environment saved workflow mapping
- WHEN the native Lattice runner starts its reviewed hosted job helper
- THEN hosted runs only the policy-bound backend pipeline, real outputs reach Web worker ingress, and cancellation does not claim completion before actual backend teardown

### Requirement: Scoped real qualification
r[kiln.web_hosted.qualification]

Qualification MUST exercise actual HTTPS Web to the genuine native Lattice application to authenticated hosted execution, not a fixture transport handler or the superseded direct route. It MUST include exact Git source, approval/relay/source/node substitutions, replay, actual logs, live cancellation, restart and owned teardown evidence. Prior direct-route receipts MUST remain explicitly superseded history. Correct host-safety refusal MUST NOT be described as successful approved-untrusted VM execution. Eligible-host and distinct-physical-host allocation remain separate prerequisites; hostile_isolation_claim MUST remain false.

#### Scenario: The local host is ineligible

- GIVEN a valid exact approval routed through native Lattice
- WHEN hosted independently verifies Web/source and reaches Linux 6.18.50 untrusted preflight
- THEN it refuses without execution or financial effects, without a security waiver, while controlled trusted execution and cancellation are recorded separately
