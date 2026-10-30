# Hosted Microvm Debug Hold Specification

## Purpose

Defines the `hosted-microvm-debug-hold` capability.

## Requirements

### Requirement: Opt-in bounded debug hold policy

r[kiln.hosted.microvm.debug_hold.policy]
A Firecracker runner MAY declare `debug_hold = { enabled, grace_seconds, max_concurrent }`. The option MUST be absent or disabled by default. `grace_seconds` MUST be within a named maximum, and `max_concurrent` MUST be within a named maximum and MUST NOT exceed the execution `maximum_concurrent`. Out-of-bounds values MUST be rejected by Rust admission and by the Nickel contracts before any effect. When the option is absent or disabled, lifecycle transitions, unit limits, admission reservations, configuration identity and receipts MUST be identical to a runner without the option.

#### Scenario: Disabled hold is identical to today

r[kiln.hosted.microvm.debug_hold.policy.scenario.disabled]
- GIVEN a Firecracker runner without `debug_hold` or with `enabled = false`
- WHEN a job succeeds or fails
- THEN the lifecycle MUST follow exactly the transitions and effects of a runner without the option
- AND an execution profile without the option MUST keep its serialized configuration identity

#### Scenario: Out-of-bounds hold policy

r[kiln.hosted.microvm.debug_hold.policy.scenario.bounds]
- GIVEN a debug hold with zero or excessive grace, zero or excessive concurrency, or concurrency above `maximum_concurrent`
- WHEN the execution configuration or Nickel profile is admitted
- THEN admission MUST fail

### Requirement: Held only after journaling a failed job without grants

r[kiln.hosted.microvm.debug_hold.lifecycle]
The lifecycle MUST enter `Held` only from `Journaled`, only after the guest reported completion of its session, and only when the hold is enabled, the job failed, the job had no secret grant, no workload-token grant, the run was not untrusted, and the runner's current holds are below `max_concurrent`. There MUST be no override. The journaled job result MUST NOT change after the hold starts.

#### Scenario: Held reachability

r[kiln.hosted.microvm.debug_hold.lifecycle.scenario.reachability]
- GIVEN a microVM lifecycle in any state
- WHEN the shell requests a hold
- THEN the hold MUST be rejected unless the state is `Journaled` after a completed guest session and every eligibility fact permits it
- AND a rejected hold MUST leave the state unchanged

### Requirement: Every hold ends in destruction or Abandoned

r[kiln.hosted.microvm.debug_hold.end]
A hold MUST end on grace expiry, explicit release, capacity pressure, attempt cancellation, VMM exit or loss of the guest session. After a hold ends, the lifecycle MUST request destruction and MUST finish in `Destroyed` or, when teardown cannot be proven, `Abandoned`. A held VM MUST keep its attempt's capacity slot.

#### Scenario: Grace expiry

r[kiln.hosted.microvm.debug_hold.end.scenario.expiry]
- GIVEN a held VM
- WHEN its grace elapses without release
- THEN the hold MUST end as expired, the VM MUST be destroyed, and the lifecycle MUST end in `Destroyed` or `Abandoned`
- AND no other event than the hold end MUST be accepted while held

### Requirement: Single-use debug capability for write-access principals

r[kiln.hosted.microvm.debug_hold.capability]
A debug capability MUST be issued only for an active hold, only to a principal with write access to the job's project, and MUST expire no later than the hold. Only a digest of the capability MUST be stored. A capability MUST authorize exactly one debug command, presented by the principal it was issued to, while that principal still has write access and the hold is active. Every other use MUST be refused.

#### Scenario: Single use and expiry

r[kiln.hosted.microvm.debug_hold.capability.scenario.single_use]
- GIVEN an issued capability
- WHEN it is redeemed once and then presented again, or presented after its expiry or after the hold ended
- THEN only the first redemption MUST run a command and every later presentation MUST be refused

#### Scenario: Write access required

r[kiln.hosted.microvm.debug_hold.capability.scenario.write_access]
- GIVEN a principal without write access to the project, or a principal other than the issuing one
- WHEN it requests or presents a capability
- THEN the request MUST be refused and no command MUST run

### Requirement: Debug relay over the existing vsock session

r[kiln.hosted.microvm.debug_hold.relay]
Debug commands MUST reach the guest only through a host-side relay over the vsock session the host already accepted. The relay MUST NOT re-open a host listener, MUST NOT add a network device, and MUST NOT allow a guest-initiated connection. The guest MUST accept debug commands only after its single run failed, and MUST run them as the unprivileged guest user with bounded deadline and output.

#### Scenario: Real held microVM

r[kiln.hosted.microvm.debug_hold.relay.scenario.host]
- GIVEN KVM, the built guest image and a Firecracker runner with debug hold enabled
- WHEN a job fails
- THEN its VM MUST be held, one command MUST run through a capability, a second use MUST be refused, and expiry MUST destroy the VM
- AND the timings MUST be recorded

### Requirement: Recorded sessions and connection instructions

r[kiln.hosted.microvm.debug_hold.sessions]
Each debug session MUST be recorded with the principal, request time, start and end times, and duration. Sessions MUST NOT grant source, CI or review authority. While a hold is active, `execution.get` MUST end with the connection instructions. The final receipt of a held job MUST end its microVM observation with the hold outcome and the instructions that applied.
